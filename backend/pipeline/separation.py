import os
import sys
import subprocess
import shutil
from pathlib import Path
from backend.config import DEVICE

def separate_stems(
    audio_path,
    output_dir,
    device=DEVICE,
    model_name="htdemucs",
    progress_callback=None,
):
    audio_file = Path(audio_path).resolve()
    out_dir = Path(output_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    if not audio_file.exists():
        raise FileNotFoundError(f"Input audio file not found: {audio_file}")

    if progress_callback:
        progress_callback(f"Starting Demucs ({model_name}) separation on {audio_file.name} using {device}...")

    cmd = [
        sys.executable,
        "-m",
        "demucs.separate",
        "-n",
        model_name,
        "--two-stems",
        "vocals",
        "-d",
        device,
        "-o",
        str(out_dir),
        str(audio_file),
    ]

    process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        universal_newlines=True,
    )

    for line in process.stdout:
        line_s = line.strip()
        if line_s and progress_callback:
            if "%" in line_s or "Separating" in line_s:
                progress_callback(f"Demucs: {line_s}")

    process.wait()
    if process.returncode != 0:
        raise RuntimeError(f"Demucs process failed with return code {process.returncode}")

    stem_dir = out_dir / model_name / audio_file.stem
    if not stem_dir.exists():
        candidates = list(out_dir.glob(f"**/{audio_file.stem}"))
        if candidates:
            stem_dir = candidates[0]
        else:
            raise FileNotFoundError(f"Could not locate Demucs output directory for {audio_file.stem}")

    vocals_src = stem_dir / "vocals.wav"
    instrumental_src = stem_dir / "no_vocals.wav"

    final_vocals = out_dir / "vocals.wav"
    final_instrumental = out_dir / "instrumental.wav"

    if vocals_src.exists():
        shutil.copy2(str(vocals_src), str(final_vocals))
    if instrumental_src.exists():
        shutil.copy2(str(instrumental_src), str(final_instrumental))

    if progress_callback:
        progress_callback("Stem separation complete: vocals.wav and instrumental.wav extracted.")

    return {
        "vocals_path": str(final_vocals),
        "instrumental_path": str(final_instrumental),
    }
