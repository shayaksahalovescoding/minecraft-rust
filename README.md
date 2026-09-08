# VoiceClone — Personal AI Singing Voice Cloning Pipeline

VoiceClone is a local, GPU-accelerated singing voice cloning studio. It trains a custom voice conversion model on samples of your own voice, takes any uploaded song or YouTube link, re-sings the vocals with your trained voice while preserving melody, rhythm, phrasing, and dynamics, and automatically remixes the output with the original instrumental track.

The entire pipeline runs 100% on-device on your local machine. No voice recordings or processed songs ever leave your system.

---

## Key Features

- **Private & Local-First**: Operates strictly offline. Zero cloud API calls, zero telemetry, and zero third-party data transmission.
- **High-Fidelity Neural Conversion**: Powered by RVC v2 (Retrieval-based Voice Conversion) with HiFi-GAN NSF vocoding at 40kHz.
- **Robust Pitch & Timbre Extraction**: Employs RMVPE (Robust Model for Vocal Pitch Estimation) and 768-dimensional ContentVec HuBERT acoustic embeddings.
- **FAISS Timbre Retrieval**: Combines neural generator weights with FAISS L2/IVF nearest-neighbor vector indexing to reproduce your personal vocal tone.
- **Demucs Stem Separation**: Isolates pristine vocal and backing instrumental stems from any song using Meta's `htdemucs` architecture.
- **Automated Stem Remixing**: Aligns converted vocals to the instrumental backing using envelope cross-correlation latency compensation, sub-80Hz high-pass filtering, and RMS energy matching.
- **Glassmorphic Web Studio**: Intuitive browser interface with real-time SSE log streaming, responsive sliders, quick-transposition presets, and audio audition players.
- **Lossless & Master Export**: Instant one-click exports to lossless 40kHz WAV and 320kbps high-bitrate MP3.

---

## Architecture Overview

```
                      [ Raw Voice Samples ]
                                |
                   [ Stage 1: Quality Gate ]
           (Silence Trim, <50Hz Cut, SNR/Clip Audit)
                                |
               [ Stage 2: Feature & Index Training ]
        (ContentVec 768-dim Embeddings + RMVPE Pitch Grid)
                                |
               +----------------+----------------+
               |                                 |
     [ RVC v2 Generator ]               [ FAISS Vector Index ]
      (models_trained/)                   (models_trained/)
               |                                 |
               +----------------+----------------+
                                |
[ Song Audio / YouTube ]        |
           |                    |
  [ Stage 3: Demucs ]           |
  (htdemucs Stem Split)         |
     /           \              |
[Inst Stem]   [Vocal Stem]      |
     |             \            |
     |       [ Stage 4: Voice Conversion ]
     |       (Pitch Shift, Timbre Retrieval, Protect)
     |                     |
     |             [ Cloned Vocal ]
     \                     /
      \                   /
    [ Stage 5: Remix & Latency Alignment ]
   (Cross-Correlation, 80Hz HPF, RMS Level Match)
                     |
        [ Master Output: WAV / MP3 ]
```

---

## Prerequisites & Requirements

### Hardware Requirements

| Component | Minimum Specification | Recommended Specification |
| :--- | :--- | :--- |
| **GPU** | NVIDIA GPU with 6 GB VRAM | NVIDIA GeForce RTX 3060 / 4070 / 5070 (8–12+ GB VRAM) |
| **CUDA** | CUDA 12.1+ | CUDA 12.4 or 12.8 |
| **System RAM** | 16 GB | 32 GB |
| **Storage** | 10 GB free space | SSD with 25+ GB free space |

### Software Requirements

- **Operating System**: Windows 10/11 (64-bit), Linux (Ubuntu 20.04+), or macOS
- **Python**: Version 3.10, 3.11, or 3.12 (Python 3.12 tested and verified)
- **FFmpeg**: Required for audio format transcoding, `yt-dlp` stream extraction, and Demucs processing

#### Installing FFmpeg

**Windows (PowerShell as Administrator or User):**
```powershell
winget install Gyan.FFmpeg
```
Alternatively via Scoop:
```powershell
scoop install ffmpeg
```
Alternatively via Chocolatey:
```powershell
choco install ffmpeg
```

**Ubuntu / Debian:**
```bash
sudo apt update && sudo apt install -y ffmpeg
```

**macOS (Homebrew):**
```bash
brew install ffmpeg
```

Verify your installation:
```bash
ffmpeg -version
```

---

## Installation Guide

### 1. Clone or Open the Repository

Open your terminal or PowerShell and navigate to the project root:
```bash
cd VoiceClone
```

### 2. Set Up a Virtual Environment

**Windows (PowerShell):**
```powershell
python -m venv venv
.\venv\Scripts\activate
```

**Linux / macOS:**
```bash
python3 -m venv venv
source venv/bin/activate
```

### 3. Install PyTorch with CUDA Support

Install the appropriate PyTorch build matching your NVIDIA CUDA driver.

**For CUDA 12.8 / 12.4 (Modern RTX 30/40/50 Series):**
```bash
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu128
```

**For CUDA 12.1:**
```bash
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
```

**For CPU-Only (Testing / Fallback):**
```bash
pip install torch torchvision torchaudio
```

### 4. Install Project Dependencies

Install all remaining audio processing, machine learning, and web framework dependencies:
```bash
pip install -r requirements.txt
```

### 5. Download Pretrained Foundation Models

Download the required base models (`rmvpe.pt`, `f0G40k.pth`, and `f0D40k.pth`):
```bash
python scripts/download_models.py
```

This populates the `pretrained/` directory:
- `pretrained/rmvpe.pt`: Pitch extraction neural model
- `pretrained/f0G40k.pth`: Pretrained RVC v2 40kHz HiFi-GAN generator
- `pretrained/f0D40k.pth`: Pretrained RVC v2 40kHz multi-period discriminator

---

## Quickstart

Start the application launcher:
```bash
python run.py
```

Once the server initializes, open your web browser and navigate to:
```
http://127.0.0.1:8000
```

---

## Step-by-Step User Manual

The studio interface is divided into 4 sequential workflow tabs.

### Tab 1: Voice Studio (Dataset Preparation)

1. **Record Voice Samples**:
   - Provide 15 to 30 minutes of clean, isolated recordings of your own voice.
   - Sing across your comfortable pitch range with varied dynamics (soft vocals, head voice, chest resonance, falsetto).
   - Accepted file types: `.wav`, `.mp3`, `.flac`, `.m4a`, `.ogg`.
2. **Upload Recordings**:
   - Enter a dataset identifier (for example, `my_voice`).
   - Drag and drop your audio files into the drop zone and click **Upload Voice Files**.
3. **Run Preprocessing & Quality Verification**:
   - Set slice duration boundaries (default: 1.0s minimum, 4.0s maximum).
   - Click **Run Preprocessing & Quality Verification**.
   - The quality gate automatically trims silence, cuts sub-50Hz microphone rumble, resamples audio to 40kHz, and audits audio for clipping and noise rejection.

### Tab 2: Model Training (Fine-Tuning & FAISS Indexing)

1. **Select Prepared Dataset**:
   - Choose your preprocessed dataset from the dropdown.
2. **Configure Training Parameters**:
   - **Target Model Name**: Unique name for your model checkpoint (e.g., `my_singing_voice`).
   - **Total Epochs**: Set to 100 (standard for 15–30 minute voice datasets).
   - **Batch Size**: Set to 8 (for 12GB VRAM) or 4 (for 6–8GB VRAM).
   - **Learning Rate**: Default is `0.0001`.
3. **Train Model**:
   - Click **Start GPU Training**.
   - The engine extracts 768-dimensional ContentVec representations and RMVPE pitch grids, trains a FAISS vector index, and fine-tunes the generator.
   - Checkpoints and index files are saved in `models_trained/<model_name>/`.

### Tab 3: Song Stems (Ingestion & Separation)

1. **Ingest Song**:
   - **Direct File Upload**: Upload any full song audio file (`.wav`, `.mp3`, `.flac`, `.m4a`).
   - **URL / YouTube**: Paste any song URL (YouTube, SoundCloud, or direct audio link) and click **Download Song via yt-dlp**.
2. **Split Stems with Demucs**:
   - Click **Split into Vocals & Instrumental**.
   - Meta's `htdemucs` isolates the original vocal track and instrumental backing track.
   - Audition both stems directly in the built-in media players.

### Tab 4: Convert & Remix

1. **Select Voice Model**:
   - Choose your trained voice model from the dropdown.
2. **Tune Conversion Controls**:
   - **Pitch Shift (Semitones)**: Adjust from -24 to +24 semitones.
     - `0`: Same vocal range.
     - `-12`: Transpose female original to male singing voice.
     - `+12`: Transpose male original to female singing voice.
   - **FAISS Index Ratio**: Slider from `0.0` to `1.0` (recommended: `0.75`). Controls the blend ratio between retrieved timbre vectors and raw acoustic embeddings.
   - **Voiceless Consonant & Breath Protect**: Slider from `0.0` to `0.5` (recommended: `0.33`). Protects unvoiced consonants, sibilants (`s`, `t`, `k`), and breathing sounds from vocoder distortion.
   - **Vocal Balance Gain**: Adjust vocal volume (-12 dB to +12 dB) to sit the converted vocals cleanly into the track.
3. **Convert & Master Remix**:
   - Click **Convert Vocal & Master Remix**.
   - The engine runs inference, aligns latency via cross-correlation, applies an 80Hz high-pass filter, balances RMS energy levels, and renders the master mix.
4. **Audition & Export**:
   - Preview the **Cloned Vocal (Solo)** or the **Final Master Mix**.
   - Click **Download WAV (Lossless)** for production mixing, or **Download MP3 (320kbps)** for sharing.

---

## Directory Structure

```
VoiceClone/
├── backend/
│   ├── app.py                     - FastAPI REST API & SSE event streaming
│   ├── config.py                  - Global configurations, paths & device detection
│   ├── models/
│   │   ├── hubert/                - ContentVec 768-dim acoustic feature extractor
│   │   ├── rmvpe/                 - RMVPE DeepUnet pitch estimation model
│   │   └── rvc/                   - RVC v2 SynthesizerTrn & MultiPeriodDiscriminator
│   └── pipeline/
│       ├── dataset_prep.py        - Denoising, rumble filter, silence trimming, SNR audit
│       ├── feature_extract.py     - ContentVec & RMVPE batch extraction
│       ├── train_index.py         - FAISS vector index builder (IndexFlatL2 / IndexIVFFlat)
│       ├── train_model.py         - PyTorch mixed-precision generator fine-tuning loop
│       ├── downloader.py          - Direct upload handler and yt-dlp audio downloader
│       ├── separation.py          - Demucs htdemucs stem separation pipeline
│       ├── conversion.py          - RVC v2 inference, pitch shifting & FAISS blending
│       └── remix.py               - Latency drift alignment, HPF & master mixer
├── frontend/
│   ├── index.html                 - 4-tab glassmorphic web dashboard
│   ├── css/
│   │   └── style.css              - Custom responsive dark-theme stylesheet
│   └── js/
│       └── app.js                 - UI controllers, audio players, SSE log stream
├── datasets/                      - Raw and preprocessed voice sample datasets
├── models_trained/                - Custom trained weights (model.pth) & FAISS indices (model.index)
├── pretrained/                    - Foundation model checkpoints (rmvpe.pt, f0G40k.pth, f0D40k.pth)
├── scripts/
│   ├── download_models.py         - Script to download foundation checkpoints
│   └── test_pipeline_e2e.py       - End-to-end pipeline verification test
├── storage/
│   ├── uploads/                   - Ingested song audio files
│   ├── stems/                     - Demucs separated vocal and instrumental stems
│   ├── converted/                 - Solo converted vocal files
│   └── remixed/                   - Final mastered WAV and MP3 mixes
├── requirements.txt               - Pinned Python package dependencies
├── run.py                         - Application launcher & environment validator
└── README.md                      - Project documentation
```

---

## Automated Pipeline Verification

To verify that your GPU, CUDA toolkit, PyTorch installation, and all pipeline stages are functioning properly:

```bash
python scripts/test_pipeline_e2e.py
```

This script automatically executes:
1. Synthetic voice dataset synthesis
2. Audio preprocessing, trimming, and quality audit
3. ContentVec and RMVPE feature extraction
4. FAISS index construction
5. Multi-epoch PyTorch generator fine-tuning with mixed precision
6. Voice conversion inference with pitch shift and index retrieval
7. Latency alignment and master remix rendering

---

## Troubleshooting & FAQ

### CUDA Out of Memory (OOM) During Training
If training fails with a PyTorch CUDA memory error:
- Reduce the batch size in Tab 2 from `8` to `4` or `2`.
- Ensure no other GPU-intensive applications (such as 3D games or video editors) are active.

### FFmpeg Not Found Error
If you encounter an error stating `ffmpeg is not recognized` or `FileNotFoundError: ffmpeg`:
- Verify FFmpeg is installed and added to your system `PATH`.
- Close and reopen your terminal after installing FFmpeg.
- Test by running `ffmpeg -version` in a fresh terminal window.

### Best Practices for Voice Recording
- Record using a condenser microphone in a quiet room with minimal room reverb.
- Keep a consistent distance (6–8 inches) from the microphone using a pop filter.
- Sing vocal scales, melodic phrases, sustained notes, and dynamic variations across low, mid, and high registers.
- Avoid recordings with background music or instrumental bleed.

### Cross-Gender Vocal Conversion
- When converting a female vocal track to a male voice, set **Pitch Shift** to `-12` semitones.
- When converting a male vocal track to a female voice, set **Pitch Shift** to `+12` semitones.
- For subtle timbre adjustment within the same vocal range, leave the slider at `0`.

---

## License

This project is intended for personal, creative, and educational use. Always ensure you have appropriate rights and permissions when using vocal recordings and song compositions.
