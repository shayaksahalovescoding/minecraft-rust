import os
import sys
import json
import uuid
import asyncio
import threading
from pathlib import Path
from typing import Optional, List
from fastapi import FastAPI, UploadFile, File, Form, BackgroundTasks, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse, StreamingResponse, JSONResponse
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from backend.config import (
    BASE_DIR,
    DATASETS_DIR,
    MODELS_DIR,
    UPLOADS_DIR,
    STEMS_DIR,
    CONVERTED_DIR,
    REMIXED_DIR,
    DEVICE,
    DEVICE_NAME,
    VRAM_GB,
    SAMPLE_RATE,
    DEFAULT_BATCH_SIZE,
    DEFAULT_EPOCHS,
)
from backend.pipeline.dataset_prep import preprocess_dataset
from backend.pipeline.feature_extract import extract_dataset_features
from backend.pipeline.train_index import build_faiss_index
from backend.pipeline.train_model import train_rvc_model
from backend.pipeline.downloader import process_uploaded_file, download_youtube_audio
from backend.pipeline.separation import separate_stems
from backend.pipeline.conversion import VoiceConverter
from backend.pipeline.remix import remix_stems

app = FastAPI(title="VoiceClone API", version="1.0.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

log_subscribers: List[asyncio.Queue] = []
training_state = {
    "is_training": False,
    "current_epoch": 0,
    "total_epochs": 0,
    "current_loss": 0.0,
    "elapsed_sec": 0,
    "model_name": None,
    "logs": [],
}
training_stop_event = threading.Event()

def broadcast_log(message: str, category: str = "info"):
    entry = {"message": message, "category": category}
    training_state["logs"].append(entry)
    if len(training_state["logs"]) > 200:
        training_state["logs"].pop(0)

    loop = None
    try:
        loop = asyncio.get_running_loop()
    except RuntimeError:
        pass

    for q in list(log_subscribers):
        try:
            if loop and loop.is_running():
                loop.call_soon_threadsafe(q.put_nowait, entry)
            else:
                q.put_nowait(entry)
        except Exception:
            pass

@app.get("/api/system")
def get_system_info():
    models = []
    for p in MODELS_DIR.glob("*"):
        if p.is_dir():
            pth = list(p.glob("*.pth"))
            idx = list(p.glob("*.index"))
            if pth:
                models.append({
                    "name": p.name,
                    "pth": str(pth[0].name),
                    "has_index": len(idx) > 0,
                })
    
    datasets = []
    for d in DATASETS_DIR.glob("*"):
        if d.is_dir():
            cleaned = d / "cleaned"
            clip_count = len(list(cleaned.glob("*.wav"))) if cleaned.exists() else 0
            datasets.append({
                "name": d.name,
                "clips": clip_count,
            })

    return {
        "device": DEVICE,
        "device_name": DEVICE_NAME,
        "vram_gb": VRAM_GB,
        "models": models,
        "datasets": datasets,
        "sample_rate": SAMPLE_RATE,
        "default_batch_size": DEFAULT_BATCH_SIZE,
        "default_epochs": DEFAULT_EPOCHS,
    }

@app.get("/api/stream/logs")
async def stream_logs():
    queue = asyncio.Queue()
    log_subscribers.append(queue)

    async def event_generator():
        try:
            for past_log in training_state["logs"][-20:]:
                yield f"data: {json.dumps(past_log)}\n\n"
            while True:
                data = await queue.get()
                yield f"data: {json.dumps(data)}\n\n"
        except asyncio.CancelledError:
            pass
        finally:
            if queue in log_subscribers:
                log_subscribers.remove(queue)

    return StreamingResponse(event_generator(), media_type="text/event-stream")

@app.post("/api/dataset/upload")
async def upload_dataset_files(
    dataset_name: str = Form(...),
    files: List[UploadFile] = File(...),
):
    target_dir = DATASETS_DIR / dataset_name / "raw"
    target_dir.mkdir(parents=True, exist_ok=True)
    saved_paths = []

    for f in files:
        dest = target_dir / f.filename
        content = await f.read()
        dest.write_bytes(content)
        saved_paths.append(str(dest))

    broadcast_log(f"Uploaded {len(saved_paths)} audio file(s) to dataset '{dataset_name}'.")
    return {"status": "ok", "dataset_name": dataset_name, "files_count": len(saved_paths)}

@app.post("/api/dataset/preprocess")
def run_dataset_preprocess(
    dataset_name: str = Form(...),
    min_duration: float = Form(1.0),
    max_duration: float = Form(4.5),
):
    raw_dir = DATASETS_DIR / dataset_name / "raw"
    if not raw_dir.exists():
        raise HTTPException(status_code=404, detail="Dataset raw files not found.")

    raw_files = list(raw_dir.glob("*.*"))
    if not raw_files:
        raise HTTPException(status_code=400, detail="No raw audio files in dataset.")

    broadcast_log(f"Beginning preprocessing for '{dataset_name}' ({len(raw_files)} source files)...")

    def progress(msg):
        broadcast_log(msg)

    stats = preprocess_dataset(
        raw_files,
        DATASETS_DIR / dataset_name,
        target_sr=SAMPLE_RATE,
        min_duration=min_duration,
        max_duration=max_duration,
        progress_callback=progress,
    )
    return {"status": "ok", "stats": stats}

@app.post("/api/train/start")
def start_training(
    dataset_name: str = Form(...),
    model_name: str = Form(...),
    epochs: int = Form(DEFAULT_EPOCHS),
    batch_size: int = Form(DEFAULT_BATCH_SIZE),
    lr: float = Form(1e-4),
    background_tasks: BackgroundTasks = None,
):
    if training_state["is_training"]:
        raise HTTPException(status_code=400, detail="Training is already in progress.")

    dataset_path = DATASETS_DIR / dataset_name
    cleaned_dir = dataset_path / "cleaned"
    if not cleaned_dir.exists() or len(list(cleaned_dir.glob("*.wav"))) == 0:
        raise HTTPException(status_code=400, detail="Cleaned clips not found. Preprocess dataset first.")

    training_stop_event.clear()
    training_state["is_training"] = True
    training_state["model_name"] = model_name
    training_state["total_epochs"] = epochs
    training_state["current_epoch"] = 0

    def run_full_training():
        try:
            broadcast_log(f"Starting feature extraction for '{dataset_name}'...")
            features_dir = dataset_path / "features"
            manifest = extract_dataset_features(
                dataset_path,
                features_dir,
                progress_callback=lambda m: broadcast_log(m),
            )

            broadcast_log(f"Building FAISS retrieval index for '{model_name}'...")
            out_model_dir = MODELS_DIR / model_name
            out_model_dir.mkdir(parents=True, exist_ok=True)
            index_path = out_model_dir / f"{model_name}.index"
            build_faiss_index(
                features_dir / "feats",
                index_path,
                progress_callback=lambda m: broadcast_log(m),
            )

            broadcast_log(f"Beginning RVC v2 fine-tuning: {epochs} epochs, batch size {batch_size}...")
            manifest_file = features_dir / "manifest.json"
            train_rvc_model(
                manifest_file,
                out_model_dir,
                epochs=epochs,
                batch_size=batch_size,
                lr=lr,
                save_every=max(10, epochs // 5),
                progress_callback=lambda m: broadcast_log(m),
                stop_event=training_stop_event,
            )
            broadcast_log(f"Model '{model_name}' successfully trained and ready for singing inference!", "success")
        except Exception as e:
            broadcast_log(f"Training error: {e}", "error")
        finally:
            training_state["is_training"] = False

    thread = threading.Thread(target=run_full_training, daemon=True)
    thread.start()

    return {"status": "started", "model_name": model_name, "epochs": epochs}

@app.post("/api/train/stop")
def stop_training_job():
    if not training_state["is_training"]:
        return {"status": "not_training"}
    training_stop_event.set()
    broadcast_log("Stopping training requested by user...", "warning")
    return {"status": "stopping"}

@app.post("/api/song/upload")
async def upload_song_file(file: UploadFile = File(...)):
    song_id = str(uuid.uuid4())[:8]
    ext = Path(file.filename).suffix or ".wav"
    temp_path = UPLOADS_DIR / f"{song_id}_temp{ext}"
    final_path = UPLOADS_DIR / f"{song_id}.wav"

    content = await file.read()
    temp_path.write_bytes(content)

    process_uploaded_file(temp_path, final_path)
    if temp_path.exists() and temp_path != final_path:
        temp_path.unlink()

    broadcast_log(f"Uploaded song '{file.filename}' (ID: {song_id}).")
    return {"status": "ok", "song_id": song_id, "filename": file.filename}

class UrlRequest(BaseModel):
    url: str

@app.post("/api/song/url")
def ingest_song_url(req: UrlRequest):
    song_id = str(uuid.uuid4())[:8]
    output_wav = UPLOADS_DIR / f"{song_id}.wav"
    try:
        download_youtube_audio(req.url, output_wav, progress_callback=lambda m: broadcast_log(m))
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
    broadcast_log(f"Successfully downloaded song from URL (ID: {song_id}).")
    return {"status": "ok", "song_id": song_id}

@app.post("/api/song/separate")
def separate_song(song_id: str = Form(...)):
    song_wav = UPLOADS_DIR / f"{song_id}.wav"
    if not song_wav.exists():
        raise HTTPException(status_code=404, detail="Uploaded song not found.")

    out_stems = STEMS_DIR / song_id
    try:
        stems = separate_stems(song_wav, out_stems, progress_callback=lambda m: broadcast_log(m))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

    return {"status": "ok", "song_id": song_id, "stems": stems}

@app.post("/api/convert")
def convert_song_vocal(
    song_id: str = Form(...),
    model_name: str = Form(...),
    pitch_shift: int = Form(0),
    index_rate: float = Form(0.75),
    protect: float = Form(0.33),
):
    stems_dir = STEMS_DIR / song_id
    vocal_wav = stems_dir / "vocals.wav"
    if not vocal_wav.exists():
        raise HTTPException(status_code=404, detail="Separated vocal stem not found. Run separation first.")

    model_dir = MODELS_DIR / model_name
    model_pth = model_dir / "model.pth"
    if not model_pth.exists():
        pths = list(model_dir.glob("*.pth"))
        if pths:
            model_pth = pths[0]
        else:
            raise HTTPException(status_code=404, detail=f"No trained .pth model found in {model_name}")

    index_file = model_dir / f"{model_name}.index"
    if not index_file.exists():
        idxs = list(model_dir.glob("*.index"))
        index_file = idxs[0] if idxs else None

    out_converted = CONVERTED_DIR / song_id / "converted_vocal.wav"
    
    broadcast_log(f"Converting vocals using '{model_name}' (pitch shift: {pitch_shift} st, index: {index_rate}, protect: {protect})...")
    converter = VoiceConverter(str(model_pth), str(index_file) if index_file else None)
    result_path = converter.convert_vocal(
        str(vocal_wav),
        str(out_converted),
        pitch_shift=pitch_shift,
        index_rate=index_rate,
        protect=protect,
        progress_callback=lambda m: broadcast_log(m),
    )

    return {"status": "ok", "song_id": song_id, "converted_vocal": result_path}

@app.post("/api/remix")
def remix_song_tracks(
    song_id: str = Form(...),
    vocal_gain_db: float = Form(0.0),
    inst_gain_db: float = Form(0.0),
):
    vocal_orig = STEMS_DIR / song_id / "vocals.wav"
    instrumental = STEMS_DIR / song_id / "instrumental.wav"
    vocal_conv = CONVERTED_DIR / song_id / "converted_vocal.wav"

    if not vocal_conv.exists() or not instrumental.exists() or not vocal_orig.exists():
        raise HTTPException(status_code=400, detail="Missing required stem files for remixing.")

    out_remix_dir = REMIXED_DIR / song_id
    broadcast_log("Starting remix alignment, loudness matching, and stereo mastering...")
    res = remix_stems(
        str(vocal_orig),
        str(vocal_conv),
        str(instrumental),
        out_remix_dir,
        vocal_gain_db=vocal_gain_db,
        inst_gain_db=inst_gain_db,
        progress_callback=lambda m: broadcast_log(m),
    )
    broadcast_log("Remix completed! Final mix ready for audition and download.", "success")
    return {"status": "ok", "song_id": song_id, "outputs": res}

@app.get("/api/audio/{path:path}")
def get_audio_file(path: str):
    full_path = (BASE_DIR / path).resolve()
    if not full_path.exists() or not str(full_path).startswith(str(BASE_DIR)):
        raise HTTPException(status_code=404, detail="Audio file not found.")
    media = "audio/mpeg" if full_path.suffix == ".mp3" else "audio/wav"
    return FileResponse(str(full_path), media_type=media)

frontend_dir = BASE_DIR / "frontend"
if frontend_dir.exists():
    app.mount("/", StaticFiles(directory=str(frontend_dir), html=True), name="frontend")
