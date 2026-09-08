import os
import sys
import shutil
from pathlib import Path

root_dir = Path(__file__).resolve().parent.parent
if str(root_dir) not in sys.path:
    sys.path.insert(0, str(root_dir))

import numpy as np
import soundfile as sf
import torch

from backend.config import DATASETS_DIR, MODELS_DIR, STORAGE_DIR, SAMPLE_RATE, DEVICE
from backend.pipeline.dataset_prep import preprocess_dataset
from backend.pipeline.feature_extract import extract_dataset_features
from backend.pipeline.train_index import build_faiss_index
from backend.pipeline.train_model import train_rvc_model
from backend.pipeline.conversion import VoiceConverter
from backend.pipeline.remix import remix_stems

def generate_synthetic_dataset(output_dir, num_clips=6):
    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)
    generated = []
    frequencies = [220.0, 330.0, 440.0, 520.0, 660.0, 440.0]

    for i in range(num_clips):
        duration = 2.5
        t = np.linspace(0, duration, int(SAMPLE_RATE * duration), endpoint=False)
        f = frequencies[i % len(frequencies)]
        wave = 0.5 * np.sin(2 * np.pi * f * t) + 0.25 * np.sin(2 * np.pi * 2 * f * t)
        env = np.sin(np.pi * t / duration) ** 0.5
        wave = (wave * env).astype(np.float32)
        clip_path = out / f"voice_sample_{i+1}.wav"
        sf.write(str(clip_path), wave, SAMPLE_RATE)
        generated.append(clip_path)

    return generated

def run_e2e_verification():
    print(">>> [1/6] Generating synthetic voice dataset...")
    test_ds_name = "test_e2e_voice"
    raw_dir = DATASETS_DIR / test_ds_name / "raw"
    raw_files = generate_synthetic_dataset(raw_dir, num_clips=6)

    print(">>> [2/6] Running Stage 1: Audio Preprocessing & Quality Verification...")
    stats = preprocess_dataset(
        raw_files,
        DATASETS_DIR / test_ds_name,
        target_sr=SAMPLE_RATE,
        min_duration=1.0,
        max_duration=4.0,
    )
    print("Preprocessing stats:", stats)
    assert stats["accepted_slices"] > 0, "No slices accepted during preprocessing!"

    print(">>> [3/6] Running Stage 2: Feature Extraction (ContentVec + RMVPE)...")
    feats_dir = DATASETS_DIR / test_ds_name / "features"
    manifest = extract_dataset_features(
        DATASETS_DIR / test_ds_name,
        feats_dir,
        device=DEVICE,
    )
    print(f"Features extracted for {len(manifest)} clips.")
    assert len(manifest) > 0, "Manifest is empty after feature extraction!"

    print(">>> [4/6] Building FAISS Retrieval Index...")
    model_name = "test_e2e_model"
    model_dir = MODELS_DIR / model_name
    model_dir.mkdir(parents=True, exist_ok=True)
    index_file = model_dir / f"{model_name}.index"
    build_faiss_index(feats_dir / "feats", index_file)
    assert index_file.exists(), "FAISS index file was not generated!"

    print(">>> [5/6] Running Model Training (2 epochs verification)...")
    manifest_file = feats_dir / "manifest.json"
    trained_model_path = train_rvc_model(
        manifest_file,
        model_dir,
        epochs=2,
        batch_size=2,
        lr=1e-4,
        save_every=2,
    )
    assert Path(trained_model_path).exists(), "Model checkpoint was not created!"

    print(">>> [6/6] Running Stage 4 & 5: Voice Conversion & Remixing...")
    converter = VoiceConverter(trained_model_path, index_file, device=DEVICE)
    test_vocal = raw_files[0]
    out_converted = STORAGE_DIR / "test_converted.wav"
    converter.convert_vocal(
        test_vocal,
        out_converted,
        pitch_shift=0,
        index_rate=0.75,
        protect=0.33,
    )
    assert out_converted.exists(), "Converted vocal audio file was not produced!"

    t = np.linspace(0, 2.5, int(SAMPLE_RATE * 2.5), endpoint=False)
    inst = 0.4 * np.sin(2 * np.pi * 110 * t)
    test_inst = STORAGE_DIR / "test_instrumental.wav"
    sf.write(str(test_inst), inst, SAMPLE_RATE)

    remix_out_dir = STORAGE_DIR / "test_remix_out"
    remix_res = remix_stems(
        str(test_vocal),
        str(out_converted),
        str(test_inst),
        remix_out_dir,
    )
    print("Remix outputs:", remix_res)
    assert Path(remix_res["final_mix_wav"]).exists(), "Final mix WAV was not produced!"

    print("\n" + "=" * 60)
    print("  ALL 6 PIPELINE STAGES PASSED END-TO-END ON GPU!")
    print("=" * 60)

if __name__ == "__main__":
    run_e2e_verification()
