import os
import shutil
import subprocess
from pathlib import Path
import yt_dlp
import soundfile as sf
import librosa

def process_uploaded_file(input_file_path, output_wav_path, target_sr=44100):
    output_path = Path(output_wav_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        cmd = [
            "ffmpeg",
            "-y",
            "-v",
            "error",
            "-i",
            str(input_file_path),
            "-ac",
            "2",
            "-ar",
            str(target_sr),
            "-f",
            "wav",
            str(output_path),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode == 0 and output_path.exists() and output_path.stat().st_size > 1000:
            return str(output_path)
    except Exception:
        pass

    try:
        import soundfile as sf
        audio, sr = sf.read(str(input_file_path))
        if sr != target_sr:
            import librosa
            if audio.ndim == 2:
                audio = librosa.resample(audio.T, orig_sr=sr, target_sr=target_sr).T
            else:
                audio = librosa.resample(audio, orig_sr=sr, target_sr=target_sr)
        sf.write(str(output_path), audio, target_sr, subtype="PCM_16")
        return str(output_path)
    except Exception:
        pass

    import numpy as np
    import soundfile as sf
    import librosa
    audio, sr = librosa.load(str(input_file_path), sr=target_sr, mono=False)
    if audio.ndim == 1:
        audio = audio[np.newaxis, :]
    sf.write(str(output_path), audio.T, target_sr, subtype="PCM_16")
    return str(output_path)

def download_youtube_audio(url, output_wav_path, progress_callback=None):
    output_path = Path(output_wav_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    base_out = str(output_path.with_suffix(""))

    ydl_opts = {
        "format": "bestaudio/best",
        "outtmpl": f"{base_out}.%(ext)s",
        "postprocessors": [
            {
                "key": "FFmpegExtractAudio",
                "preferredcodec": "wav",
                "preferredquality": "192",
            }
        ],
        "quiet": True,
        "no_warnings": True,
    }

    if progress_callback:
        progress_callback(f"Fetching audio via yt-dlp: {url}")

    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        ydl.download([url])

    result_wav = output_path.with_suffix(".wav")
    if not result_wav.exists():
        found = list(output_path.parent.glob(f"{output_path.stem}*.wav"))
        if found:
            shutil.move(str(found[0]), str(output_path))
            return str(output_path)
        raise FileNotFoundError("yt-dlp failed to produce output WAV.")
    return str(result_wav)
