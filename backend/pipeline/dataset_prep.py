import os
import shutil
from pathlib import Path
import numpy as np
import soundfile as sf
import librosa
from scipy.signal import butter, sosfilt

def butter_highpass(cutoff, fs, order=5):
    nyq = 0.5 * fs
    normal_cutoff = cutoff / nyq
    return butter(order, normal_cutoff, btype='high', analog=False, output='sos')

def highpass_filter(data, cutoff=50, fs=40000):
    sos = butter_highpass(cutoff, fs)
    return sosfilt(sos, data)

def is_clipped(audio, threshold=0.99, min_consecutive=3):
    clipped_mask = np.abs(audio) >= threshold
    count = 0
    for val in clipped_mask:
        if val:
            count += 1
            if count >= min_consecutive:
                return True
        else:
            count = 0
    return False

def calculate_snr(audio):
    energy = np.mean(audio ** 2)
    if energy < 1e-7:
        return 0.0
    sorted_frames = np.sort(np.abs(audio))
    noise_floor = np.mean(sorted_frames[: max(1, int(len(sorted_frames) * 0.1))] ** 2)
    if noise_floor < 1e-9:
        return 40.0
    return 10 * np.log10(energy / noise_floor)

def normalize_loudness(audio, target_peak=0.95):
    peak = np.max(np.abs(audio))
    if peak > 1e-5:
        return audio * (target_peak / peak)
    return audio

def preprocess_dataset(
    input_files,
    output_dir,
    target_sr=40000,
    min_duration=1.0,
    max_duration=4.5,
    progress_callback=None,
):
    output_path = Path(output_dir)
    cleaned_dir = output_path / "cleaned"
    cleaned_dir.mkdir(parents=True, exist_ok=True)

    stats = {
        "total_files": len(input_files),
        "total_clips_generated": 0,
        "accepted_clips": 0,
        "rejected_too_short": 0,
        "rejected_clipped": 0,
        "rejected_noisy": 0,
        "total_clean_duration_sec": 0.0,
        "clips": [],
    }

    clip_index = 0

    for file_idx, fpath in enumerate(input_files):
        fpath = Path(fpath)
        if not fpath.exists():
            continue

        if progress_callback:
            progress_callback(
                f"Processing file {file_idx + 1}/{len(input_files)}: {fpath.name}"
            )

        try:
            audio, sr = librosa.load(str(fpath), sr=target_sr, mono=True)
        except Exception as e:
            if progress_callback:
                progress_callback(f"Error loading {fpath.name}: {e}")
            continue

        if len(audio) == 0:
            continue

        audio = highpass_filter(audio, cutoff=50, fs=target_sr)

        intervals = librosa.effects.split(audio, top_db=30, frame_length=2048, hop_length=512)

        for start_idx, end_idx in intervals:
            segment = audio[start_idx:end_idx]
            seg_duration = len(segment) / target_sr

            if seg_duration < min_duration:
                stats["rejected_too_short"] += 1
                stats["total_clips_generated"] += 1
                continue

            sub_clips = []
            max_samples = int(max_duration * target_sr)
            if len(segment) > max_samples:
                for chunk_start in range(0, len(segment), max_samples):
                    chunk = segment[chunk_start : chunk_start + max_samples]
                    if len(chunk) / target_sr >= min_duration:
                        sub_clips.append(chunk)
            else:
                sub_clips.append(segment)

            for chunk in sub_clips:
                stats["total_clips_generated"] += 1
                duration = len(chunk) / target_sr

                if is_clipped(chunk):
                    stats["rejected_clipped"] += 1
                    continue

                snr = calculate_snr(chunk)
                if snr < 10.0:
                    stats["rejected_noisy"] += 1
                    continue

                chunk = normalize_loudness(chunk)

                clip_name = f"clip_{clip_index:05d}.wav"
                clip_file = cleaned_dir / clip_name
                sf.write(str(clip_file), chunk, target_sr, subtype="PCM_16")

                stats["accepted_clips"] += 1
                stats["total_clean_duration_sec"] += duration
                stats["clips"].append(
                    {
                        "filename": clip_name,
                        "duration": round(duration, 2),
                        "snr": round(float(snr), 1),
                    }
                )
                clip_index += 1

    stats["total_clean_duration_min"] = round(stats["total_clean_duration_sec"] / 60.0, 2)
    stats["total_clean_duration_sec"] = round(stats["total_clean_duration_sec"], 2)
    stats["accepted_slices"] = stats["accepted_clips"]
    stats["clean_duration_sec"] = stats["total_clean_duration_sec"]

    if progress_callback:
        progress_callback(
            f"Dataset prep complete: {stats['accepted_clips']} clean clips ({stats['total_clean_duration_min']} min). "
            f"Rejected: {stats['rejected_clipped']} clipped, {stats['rejected_noisy']} noisy, {stats['rejected_too_short']} short."
        )

    return stats
