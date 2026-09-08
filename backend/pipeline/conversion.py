import os
from pathlib import Path
import numpy as np
import torch
import librosa
import soundfile as sf
import faiss

from backend.pipeline.train_model import create_generator
from backend.models.hubert.extractor import ContentVecExtractor
from backend.models.rmvpe.model import RMVPE0Predictor
from backend.config import RMVPE_PATH, DEVICE, SAMPLE_RATE

def f0_to_coarse(f0):
    f0_mel = 1127 * np.log(1 + f0 / 700.0)
    f0_mel_min = 1127 * np.log(1 + 50.0 / 700.0)
    f0_mel_max = 1127 * np.log(1 + 1100.0 / 700.0)
    coarse = (f0_mel - f0_mel_min) * (254.0 / (f0_mel_max - f0_mel_min)) + 1.0
    coarse[f0 <= 50.0] = 1
    coarse[f0 >= 1100.0] = 255
    return np.clip(np.round(coarse), 1, 255).astype(np.int64)

class VoiceConverter:
    def __init__(self, model_path, index_path=None, device=DEVICE):
        self.device = device
        self.model_path = Path(model_path)
        self.index_path = Path(index_path) if index_path and Path(index_path).exists() else None

        self.generator = create_generator().to(device)
        ckpt = torch.load(str(self.model_path), map_location=device, weights_only=False)
        state_dict = ckpt["weight"] if "weight" in ckpt else ckpt.get("model", ckpt)
        self.generator.load_state_dict(state_dict, strict=False)
        self.generator.eval()

        self.hubert = ContentVecExtractor(device=device)
        self.rmvpe = RMVPE0Predictor(str(RMVPE_PATH), device=device)

        self.faiss_index = None
        if self.index_path and self.index_path.exists():
            try:
                self.faiss_index = faiss.read_index(str(self.index_path))
                if hasattr(self.faiss_index, "make_direct_map"):
                    self.faiss_index.make_direct_map()
            except Exception:
                self.faiss_index = None

    def convert_vocal(
        self,
        vocal_audio_path,
        output_wav_path,
        pitch_shift=0,
        index_rate=0.75,
        protect=0.33,
        progress_callback=None,
    ):
        vocal_path = Path(vocal_audio_path)
        out_path = Path(output_wav_path)
        out_path.parent.mkdir(parents=True, exist_ok=True)

        if progress_callback:
            progress_callback(f"Loading vocal track: {vocal_path.name}...")

        audio_orig, sr = librosa.load(str(vocal_path), sr=SAMPLE_RATE, mono=True)
        orig_len = len(audio_orig)

        audio_16k = librosa.resample(audio_orig, orig_sr=SAMPLE_RATE, target_sr=16000)

        if progress_callback:
            progress_callback("Extracting f0 pitch contour with RMVPE...")
        f0 = self.rmvpe.infer_from_audio(audio_16k, thred=0.03)

        if pitch_shift != 0:
            scale = 2.0 ** (pitch_shift / 12.0)
            f0 = f0 * scale

        if progress_callback:
            progress_callback("Extracting phonetic features with ContentVec...")
        feats = self.hubert.extract_features(audio_16k, sr=16000)

        min_len = min(feats.shape[0], f0.shape[0])
        feats = feats[:min_len].copy()
        f0 = f0[:min_len].copy()

        if self.faiss_index is not None and index_rate > 0.0:
            if progress_callback:
                progress_callback(f"Retrieving nearest neighbor timbre (index ratio: {index_rate})...")
            feats_f32 = feats.astype(np.float32)
            k = min(8, self.faiss_index.ntotal)
            if k > 0:
                dists, indices = self.faiss_index.search(feats_f32, k)
                retrieved_feats = np.zeros_like(feats_f32)
                for i in range(min_len):
                    weights = np.exp(-dists[i] / 10.0)
                    weights /= np.sum(weights) + 1e-8
                    vec_list = []
                    for idx in indices[i]:
                        try:
                            vec_list.append(self.faiss_index.reconstruct(int(idx)))
                        except Exception:
                            if hasattr(self.faiss_index, "make_direct_map"):
                                self.faiss_index.make_direct_map()
                                vec_list.append(self.faiss_index.reconstruct(int(idx)))
                            else:
                                vec_list.append(feats_f32[i])
                    vecs = np.array(vec_list)
                    retrieved_feats[i] = np.sum(vecs * weights[:, np.newaxis], axis=0)
                
                feats = (1.0 - index_rate) * feats + index_rate * retrieved_feats

        if protect > 0.0:
            unvoiced_mask = f0 == 0.0
            unvoiced_mask = unvoiced_mask.astype(np.float32)[:, np.newaxis]
            raw_feats = self.hubert.extract_features(audio_16k, sr=16000)[:min_len]
            feats = (1.0 - unvoiced_mask * protect) * feats + (unvoiced_mask * protect) * raw_feats

        pitch_coarse = f0_to_coarse(f0)

        if progress_callback:
            progress_callback("Synthesizing cloned vocals via RVC neural generator...")

        chunk_size = 400
        converted_chunks = []
        
        with torch.no_grad():
            for i in range(0, min_len, chunk_size):
                sub_feats = feats[i : i + chunk_size]
                sub_f0 = f0[i : i + chunk_size]
                sub_pitch = pitch_coarse[i : i + chunk_size]
                sub_len = len(sub_feats)

                t_phone = torch.from_numpy(sub_feats).float().unsqueeze(0).to(self.device)
                t_phone_lens = torch.tensor([sub_len], dtype=torch.long, device=self.device)
                t_pitch = torch.from_numpy(sub_pitch).long().unsqueeze(0).to(self.device)
                t_nsff0 = torch.from_numpy(sub_f0).float().unsqueeze(0).to(self.device)
                sid = torch.zeros(1, dtype=torch.long, device=self.device)

                audio_out = self.generator.infer(
                    t_phone,
                    t_phone_lens,
                    pitch=t_pitch,
                    nsff0=t_nsff0,
                    sid=sid,
                )
                o = audio_out[0] if isinstance(audio_out, tuple) else audio_out
                chunk_np = o.squeeze().cpu().numpy()
                converted_chunks.append(chunk_np)

        converted_audio = np.concatenate(converted_chunks, axis=0) if converted_chunks else np.zeros(orig_len)

        if len(converted_audio) < orig_len:
            converted_audio = np.pad(converted_audio, (0, orig_len - len(converted_audio)))
        elif len(converted_audio) > orig_len:
            converted_audio = converted_audio[:orig_len]

        sf.write(str(out_path), converted_audio, SAMPLE_RATE, subtype="PCM_16")

        if progress_callback:
            progress_callback(f"Conversion complete: saved to {out_path.name}")

        return str(out_path)
