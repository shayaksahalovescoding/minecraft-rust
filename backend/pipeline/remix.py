import os
import subprocess
from pathlib import Path
import numpy as np
import librosa
import soundfile as sf
from scipy.signal import correlate, butter, sosfilt

def butter_highpass(cutoff, fs, order=4):
    nyq = 0.5 * fs
    normal_cutoff = cutoff / nyq
    return butter(order, normal_cutoff, btype="high", analog=False, output="sos")

def apply_highpass(audio, cutoff=80, fs=44100):
    sos = butter_highpass(cutoff, fs)
    if audio.ndim == 1:
        return sosfilt(sos, audio)
    else:
        return np.array([sosfilt(sos, audio[ch]) for ch in range(audio.shape[0])])

def align_vocal_latency(orig_vocal, conv_vocal, sr=44100, max_lag_ms=100):
    max_lag_samples = int((max_lag_ms / 1000.0) * sr)
    
    hop = 128
    env_orig = librosa.feature.rms(y=orig_vocal, hop_length=hop)[0]
    env_conv = librosa.feature.rms(y=conv_vocal, hop_length=hop)[0]

    min_frames = min(len(env_orig), len(env_conv))
    env_orig = env_orig[:min_frames]
    env_conv = env_conv[:min_frames]

    env_orig = env_orig - np.mean(env_orig)
    env_conv = env_conv - np.mean(env_conv)

    corr = correlate(env_orig, env_conv, mode="full")
    lags = np.arange(-min_frames + 1, min_frames)
    
    max_lag_frames = max_lag_samples // hop
    valid_mask = np.abs(lags) <= max_lag_frames
    
    corr_valid = corr[valid_mask]
    lags_valid = lags[valid_mask]

    best_lag_frames = lags_valid[np.argmax(corr_valid)]
    best_lag_samples = int(best_lag_frames * hop)

    if best_lag_samples > 0:
        aligned = np.pad(conv_vocal, (best_lag_samples, 0))[: len(orig_vocal)]
    elif best_lag_samples < 0:
        aligned = np.pad(conv_vocal[-best_lag_samples:], (0, -best_lag_samples))[: len(orig_vocal)]
    else:
        aligned = conv_vocal[: len(orig_vocal)]

    if len(aligned) < len(orig_vocal):
        aligned = np.pad(aligned, (0, len(orig_vocal) - len(aligned)))

    return aligned, best_lag_samples

def remix_stems(
    original_vocal_path,
    converted_vocal_path,
    instrumental_path,
    output_dir,
    vocal_gain_db=0.0,
    inst_gain_db=0.0,
    target_sr=44100,
    progress_callback=None,
):
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    if progress_callback:
        progress_callback("Loading audio stems for remixing...")

    orig_vocal, sr_v = librosa.load(str(original_vocal_path), sr=target_sr, mono=True)
    conv_vocal, _ = librosa.load(str(converted_vocal_path), sr=target_sr, mono=True)
    inst, sr_i = librosa.load(str(instrumental_path), sr=target_sr, mono=False)

    if progress_callback:
        progress_callback("Realigning vocal latency drift via envelope cross-correlation...")

    aligned_vocal, lag_samples = align_vocal_latency(orig_vocal, conv_vocal, sr=target_sr)
    lag_ms = round((lag_samples / target_sr) * 1000.0, 2)

    if progress_callback:
        progress_callback(f"Latency alignment applied: {lag_ms}ms ({lag_samples} samples).")

    aligned_vocal = apply_highpass(aligned_vocal, cutoff=80, fs=target_sr)

    rms_orig = np.sqrt(np.mean(orig_vocal ** 2) + 1e-9)
    rms_conv = np.sqrt(np.mean(aligned_vocal ** 2) + 1e-9)
    gain_match = rms_orig / rms_conv
    aligned_vocal = aligned_vocal * gain_match

    user_vocal_mult = 10.0 ** (vocal_gain_db / 20.0)
    user_inst_mult = 10.0 ** (inst_gain_db / 20.0)

    aligned_vocal = aligned_vocal * user_vocal_mult
    inst = inst * user_inst_mult

    max_len = max(
        aligned_vocal.shape[-1],
        inst.shape[-1] if inst.ndim > 1 else len(inst),
    )

    if aligned_vocal.ndim == 1:
        aligned_vocal_padded = np.pad(aligned_vocal, (0, max_len - len(aligned_vocal)))
        vocal_stereo = np.stack([aligned_vocal_padded, aligned_vocal_padded])
    else:
        vocal_stereo = aligned_vocal

    if inst.ndim == 1:
        inst_padded = np.pad(inst, (0, max_len - len(inst)))
        inst_stereo = np.stack([inst_padded, inst_padded])
    else:
        inst_stereo = np.pad(inst, ((0, 0), (0, max_len - inst.shape[1])))

    mix = inst_stereo + vocal_stereo

    peak = np.max(np.abs(mix))
    if peak > 0.95:
        mix = mix * (0.95 / peak)

    wav_mix_path = out_dir / "final_mix.wav"
    mp3_mix_path = out_dir / "final_mix.mp3"
    aligned_vocal_path = out_dir / "aligned_vocal.wav"

    sf.write(str(aligned_vocal_path), vocal_stereo.T, target_sr, subtype="PCM_16")
    sf.write(str(wav_mix_path), mix.T, target_sr, subtype="PCM_16")

    try:
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-i",
                str(wav_mix_path),
                "-codec:a",
                "libmp3lame",
                "-b:a",
                "320k",
                str(mp3_mix_path),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )
    except Exception:
        pass

    if progress_callback:
        progress_callback("Remix complete: final_mix.wav and final_mix.mp3 generated.")

    return {
        "final_mix_wav": str(wav_mix_path),
        "final_mix_mp3": str(mp3_mix_path) if mp3_mix_path.exists() else str(wav_mix_path),
        "aligned_vocal_wav": str(aligned_vocal_path),
        "latency_correction_ms": lag_ms,
    }
