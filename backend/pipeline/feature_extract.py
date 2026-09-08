import os
import json
from pathlib import Path
import numpy as np
import torch
import librosa
from backend.models.hubert.extractor import ContentVecExtractor
from backend.models.rmvpe.model import RMVPE0Predictor
from backend.config import RMVPE_PATH, DEVICE, SAMPLE_RATE

def compute_spec(audio, n_fft=2048, hop_length=512, win_length=2048):
    tensor = torch.from_numpy(audio).float()
    window = torch.hann_window(win_length)
    stft = torch.stft(
        tensor,
        n_fft=n_fft,
        hop_length=hop_length,
        win_length=win_length,
        window=window,
        center=False,
        return_complex=True,
    )
    spec = torch.sqrt(stft.real.pow(2) + stft.imag.pow(2) + 1e-6)
    return spec.numpy()

def extract_dataset_features(
    dataset_dir,
    output_dir,
    device=DEVICE,
    progress_callback=None,
):
    dataset_path = Path(dataset_dir)
    cleaned_dir = dataset_path / "cleaned"
    if not cleaned_dir.exists():
        cleaned_dir = dataset_path

    wav_files = sorted(list(cleaned_dir.glob("*.wav")))
    if not wav_files:
        raise ValueError(f"No WAV files found in {cleaned_dir}")

    out_path = Path(output_dir)
    feats_dir = out_path / "feats"
    f0_dir = out_path / "f0"
    spec_dir = out_path / "spec"

    for d in [feats_dir, f0_dir, spec_dir]:
        d.mkdir(parents=True, exist_ok=True)

    if progress_callback:
        progress_callback("Loading ContentVec feature encoder...")
    hubert = ContentVecExtractor(device=device)

    if progress_callback:
        progress_callback("Loading RMVPE pitch estimator...")
    rmvpe = RMVPE0Predictor(str(RMVPE_PATH), device=device)

    manifest = []
    total = len(wav_files)

    for idx, wav_path in enumerate(wav_files):
        stem = wav_path.stem
        if progress_callback:
            progress_callback(f"Extracting features [{idx + 1}/{total}]: {wav_path.name}")

        audio_40k, _ = librosa.load(str(wav_path), sr=SAMPLE_RATE, mono=True)
        audio_16k = librosa.resample(audio_40k, orig_sr=SAMPLE_RATE, target_sr=16000)

        feats = hubert.extract_features(audio_16k, sr=16000)
        f0 = rmvpe.infer_from_audio(audio_16k, thred=0.03)
        spec = compute_spec(audio_40k)

        min_len = min(feats.shape[0], f0.shape[0], spec.shape[1])
        feats = feats[:min_len]
        f0 = f0[:min_len]
        spec = spec[:, :min_len]

        feat_file = feats_dir / f"{stem}.npy"
        f0_file = f0_dir / f"{stem}.npy"
        spec_file = spec_dir / f"{stem}.npy"

        np.save(str(feat_file), feats.astype(np.float32))
        np.save(str(f0_file), f0.astype(np.float32))
        np.save(str(spec_file), spec.astype(np.float32))

        manifest.append(
            {
                "name": stem,
                "length": int(min_len),
                "feat_path": str(feat_file),
                "f0_path": str(f0_file),
                "spec_path": str(spec_file),
            }
        )

    manifest_file = out_path / "manifest.json"
    with open(manifest_file, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    if progress_callback:
        progress_callback(f"Feature extraction complete for {total} files.")

    return manifest
