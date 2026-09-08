import os
from pathlib import Path
import torch

BASE_DIR = Path(__file__).resolve().parent.parent
PRETRAINED_DIR = BASE_DIR / "pretrained"
DATASETS_DIR = BASE_DIR / "datasets"
MODELS_DIR = BASE_DIR / "models_trained"
STORAGE_DIR = BASE_DIR / "storage"
UPLOADS_DIR = STORAGE_DIR / "uploads"
STEMS_DIR = STORAGE_DIR / "stems"
CONVERTED_DIR = STORAGE_DIR / "converted"
REMIXED_DIR = STORAGE_DIR / "remixed"

for d in [
    PRETRAINED_DIR,
    DATASETS_DIR,
    MODELS_DIR,
    STORAGE_DIR,
    UPLOADS_DIR,
    STEMS_DIR,
    CONVERTED_DIR,
    REMIXED_DIR,
]:
    d.mkdir(parents=True, exist_ok=True)

DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
DEVICE_NAME = torch.cuda.get_device_name(0) if torch.cuda.is_available() else "CPU"
VRAM_GB = (
    round(torch.cuda.get_device_properties(0).total_memory / (1024**3), 1)
    if torch.cuda.is_available()
    else 0.0
)

SAMPLE_RATE = 40000
HOP_SIZE = 512
DEFAULT_BATCH_SIZE = 8 if VRAM_GB >= 8 else 4
DEFAULT_EPOCHS = 100

HUBERT_PATH = PRETRAINED_DIR / "hubert_base.pt"
RMVPE_PATH = PRETRAINED_DIR / "rmvpe.pt"
BASE_G40K = PRETRAINED_DIR / "f0G40k.pth"
BASE_D40K = PRETRAINED_DIR / "f0D40k.pth"
