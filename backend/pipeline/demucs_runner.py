import sys
import torch
import torchaudio
import soundfile as sf
import demucs.audio
import demucs.separate

def patch_torchaudio():
    def _save(uri, src, sample_rate, channels_first=True, **kwargs):
        if isinstance(src, torch.Tensor):
            src = src.detach().cpu().numpy()
        if channels_first and src.ndim > 1:
            src = src.T
        sf.write(str(uri), src, sample_rate)
    torchaudio.save = _save
    demucs.audio.ta.save = _save

if __name__ == "__main__":
    patch_torchaudio()
    demucs.separate.main(sys.argv[1:])
