import torch
import numpy as np
import librosa
from pathlib import Path
from backend.models.rmvpe.model import RMVPE

class RMVPEExtractor:
    def __init__(self, model_path, device="cuda"):
        self.device = device
        self.model = RMVPE(model_path=model_path, device=device)
        self.mel_basis = librosa.filters.mel(
            sr=16000,
            n_fft=1024,
            n_mels=128,
            fmin=30,
            fmax=8000,
            htk=True,
            norm=None,
        )
        self.mel_basis = torch.from_numpy(self.mel_basis).float().to(device)

    def extract_f0(self, audio, sr=16000, hop_length=160, threshold=0.03):
        if sr != 16000:
            audio = librosa.resample(audio, orig_sr=sr, target_sr=16000)
        audio_tensor = torch.from_numpy(audio).float().to(self.device)
        pad_len = 1024 // 2
        audio_tensor = torch.nn.functional.pad(
            audio_tensor.unsqueeze(0), (pad_len, pad_len), mode="reflect"
        ).squeeze(0)

        window = torch.hann_window(1024).to(self.device)
        stft = torch.stft(
            audio_tensor,
            n_fft=1024,
            hop_length=hop_length,
            win_length=1024,
            window=window,
            center=False,
            return_complex=True,
        )
        magnitudes = torch.abs(stft)
        mel = torch.matmul(self.mel_basis, magnitudes)
        log_mel = torch.log(torch.clamp(mel, min=1e-5))

        cents = self.model.mel2cents(log_mel.unsqueeze(0))
        f0 = self.model.cents2f0(cents, threshold=threshold)
        return f0
