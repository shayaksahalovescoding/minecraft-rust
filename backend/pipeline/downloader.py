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
