import os
import json
import time
from pathlib import Path
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader
from torch.cuda.amp import autocast, GradScaler

from backend.models.rvc.synthesizers import Synthesizer
from backend.models.rvc.discriminators import MultiPeriodDiscriminator
from backend.config import BASE_G40K, BASE_D40K, DEVICE, SAMPLE_RATE

class VoiceDataset(Dataset):
    def __init__(self, manifest_file, segment_size=32):
        with open(manifest_file, "r", encoding="utf-8") as f:
            self.manifest = json.load(f)
        self.segment_size = segment_size
        self.items = [item for item in self.manifest if item["length"] >= segment_size]

    def __len__(self):
        return len(self.items)

    def __getitem__(self, idx):
        item = self.items[idx]
        feat = np.load(item["feat_path"])
        f0 = np.load(item["f0_path"])
        spec = np.load(item["spec_path"])

        length = min(feat.shape[0], f0.shape[0], spec.shape[1])
        if length > self.segment_size:
            max_start = length - self.segment_size
            start = np.random.randint(0, max_start)
            feat_slice = feat[start : start + self.segment_size]
            f0_slice = f0[start : start + self.segment_size]
            spec_slice = spec[:, start : start + self.segment_size]
        else:
            feat_slice = feat[: self.segment_size]
            f0_slice = f0[: self.segment_size]
            spec_slice = spec[:, : self.segment_size]

        pitch_coarse = 128.0 * (f0_slice / 1000.0)
        pitch_coarse = np.clip(pitch_coarse, 1, 255).astype(np.int64)

        return (
            torch.from_numpy(feat_slice).float(),
            torch.from_numpy(pitch_coarse).long(),
            torch.from_numpy(spec_slice).float(),
            torch.from_numpy(f0_slice).float(),
        )

def create_generator():
    return Synthesizer(
        spec_channels=1025,
        segment_size=32,
        inter_channels=192,
        hidden_channels=192,
        filter_channels=768,
        n_heads=2,
        n_layers=6,
        kernel_size=3,
        p_dropout=0,
        resblock="1",
        resblock_kernel_sizes=[3, 7, 11],
        resblock_dilation_sizes=[[1, 3, 5], [1, 3, 5], [1, 3, 5]],
        upsample_rates=[10, 10, 2, 2],
        upsample_initial_channel=512,
        upsample_kernel_sizes=[16, 16, 4, 4],
        spk_embed_dim=109,
        gin_channels=256,
        sr=SAMPLE_RATE,
        use_f0=True,
        text_enc_hidden_dim=768,
        vocoder="HiFi-GAN",
    )

def train_rvc_model(
    manifest_path,
    output_dir,
    epochs=100,
    batch_size=8,
    lr=1e-4,
    save_every=20,
    resume_checkpoint=None,
    progress_callback=None,
    stop_event=None,
):
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    dataset = VoiceDataset(manifest_path, segment_size=32)
    if len(dataset) == 0:
        raise ValueError("Dataset is empty or all clips are shorter than 32 frames.")

    loader = DataLoader(
        dataset,
        batch_size=batch_size,
        shuffle=True,
        drop_last=True if len(dataset) > batch_size else False,
        num_workers=0,
    )

    net_g = create_generator().to(DEVICE)
    net_d = MultiPeriodDiscriminator(use_spectral_norm=False).to(DEVICE)

    if resume_checkpoint and Path(resume_checkpoint).exists():
        if progress_callback:
            progress_callback(f"Resuming from checkpoint {resume_checkpoint}...")
        ckpt = torch.load(resume_checkpoint, map_location=DEVICE, weights_only=False)
        net_g.load_state_dict(ckpt["weight"] if "weight" in ckpt else ckpt["model"], strict=False)
    elif BASE_G40K.exists():
        if progress_callback:
            progress_callback(f"Initializing generator from base weights {BASE_G40K.name}...")
        ckpt_g = torch.load(str(BASE_G40K), map_location=DEVICE, weights_only=False)
        net_g.load_state_dict(ckpt_g["model"], strict=False)

    if BASE_D40K.exists():
        ckpt_d = torch.load(str(BASE_D40K), map_location=DEVICE, weights_only=False)
        net_d.load_state_dict(ckpt_d["model"], strict=False)

    opt_g = torch.optim.AdamW(net_g.parameters(), lr=lr, betas=(0.8, 0.99))
    opt_d = torch.optim.AdamW(net_d.parameters(), lr=lr, betas=(0.8, 0.99))
    scaler = torch.amp.GradScaler("cuda", enabled=True)

    start_time = time.time()

    for epoch in range(1, epochs + 1):
        if stop_event and stop_event.is_set():
            if progress_callback:
                progress_callback("Training aborted by user.")
            break

        net_g.train()
        net_d.train()
        epoch_g_loss = 0.0
        epoch_d_loss = 0.0
        steps = 0

        for batch_idx, (phone_batch, pitch_batch, spec_batch, f0_batch) in enumerate(loader):
            phone_batch = phone_batch.to(DEVICE)
            pitch_batch = pitch_batch.to(DEVICE)
            spec_batch = spec_batch.to(DEVICE)
            f0_batch = f0_batch.to(DEVICE)

            phone_lens = torch.full((phone_batch.size(0),), phone_batch.size(1), dtype=torch.long, device=DEVICE)
            spec_lens = torch.full((spec_batch.size(0),), spec_batch.size(2), dtype=torch.long, device=DEVICE)
            sid = torch.zeros(phone_batch.size(0), dtype=torch.long, device=DEVICE)

            with torch.amp.autocast("cuda", enabled=True):
                y_hat, ids_slice, x_mask, y_mask, (z, z_p, m_p, logs_p, m_q, logs_q) = net_g(
                    phone_batch,
                    phone_lens,
                    pitch_batch,
                    f0_batch,
                    spec_batch,
                    spec_lens,
                    sid,
                )
                loss_mel = F.l1_loss(spec_batch, spec_batch)
                loss_kl = torch.sum(logs_q - logs_p - 0.5 + 0.5 * ((z - m_p) ** 2) * torch.exp(-2.0 * logs_p)) / (
                    torch.sum(y_mask) + 1e-4
                )
                loss_g = loss_mel * 45.0 + loss_kl

            opt_g.zero_grad()
            scaler.scale(loss_g).backward()
            scaler.step(opt_g)
            scaler.update()

            epoch_g_loss += loss_g.item()
            steps += 1

        avg_g_loss = round(epoch_g_loss / max(1, steps), 4)
        elapsed = round(time.time() - start_time, 1)

        if progress_callback and (epoch % 5 == 0 or epoch == 1 or epoch == epochs):
            progress_callback(
                f"Epoch [{epoch}/{epochs}] — Loss: {avg_g_loss} — Elapsed: {elapsed}s"
            )

        if epoch % save_every == 0 or epoch == epochs:
            ckpt_path = out_dir / f"model_epoch_{epoch}.pth"
            best_path = out_dir / "model.pth"
            save_payload = {
                "weight": {k: v.cpu() for k, v in net_g.state_dict().items()},
                "epoch": epoch,
                "sr": SAMPLE_RATE,
                "f0": True,
                "version": "v2",
            }
            torch.save(save_payload, str(ckpt_path))
            torch.save(save_payload, str(best_path))

    final_model = out_dir / "model.pth"
    if progress_callback:
        progress_callback(f"Training completed. Final model saved to {final_model.name}")

    return str(final_model)
