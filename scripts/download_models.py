import os
import sys
from pathlib import Path
from huggingface_hub import hf_hub_download
import shutil

ROOT_DIR = Path(__file__).resolve().parent.parent
PRETRAINED_DIR = ROOT_DIR / "pretrained"
PRETRAINED_DIR.mkdir(parents=True, exist_ok=True)

MODELS = [
    {
        "repo_id": "lj1995/VoiceConversionWebUI",
        "filename": "hubert_base.pt",
        "dest": PRETRAINED_DIR / "hubert_base.pt",
    },
    {
        "repo_id": "lj1995/VoiceConversionWebUI",
        "filename": "rmvpe.pt",
        "dest": PRETRAINED_DIR / "rmvpe.pt",
    },
    {
        "repo_id": "lj1995/VoiceConversionWebUI",
        "filename": "pretrained_v2/f0G40k.pth",
        "dest": PRETRAINED_DIR / "f0G40k.pth",
    },
    {
        "repo_id": "lj1995/VoiceConversionWebUI",
        "filename": "pretrained_v2/f0D40k.pth",
        "dest": PRETRAINED_DIR / "f0D40k.pth",
    },
]

def download_all(progress_callback=None):
    results = {}
    for item in MODELS:
        dest = item["dest"]
        if dest.exists() and dest.stat().st_size > 1024:
            if progress_callback:
                progress_callback(f"Already present: {dest.name}")
            results[dest.name] = str(dest)
            continue
        if progress_callback:
            progress_callback(f"Downloading {dest.name} from {item['repo_id']}...")
        try:
            cached_path = hf_hub_download(
                repo_id=item["repo_id"],
                filename=item["filename"],
                local_dir_use_symlinks=False,
            )
            shutil.copy2(cached_path, dest)
            results[dest.name] = str(dest)
            if progress_callback:
                progress_callback(f"Successfully downloaded {dest.name}")
        except Exception as e:
            if progress_callback:
                progress_callback(f"Failed to download {dest.name}: {e}")
            results[dest.name] = None
    return results

if __name__ == "__main__":
    def log(msg):
        print(msg, flush=True)
    res = download_all(log)
    print("Download completed:", res)
