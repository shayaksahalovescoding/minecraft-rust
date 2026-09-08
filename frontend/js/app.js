document.addEventListener('DOMContentLoaded', () => {
  let activeSongId = null;
  let datasetFiles = [];
  let songFile = null;

  const tabBtns = document.querySelectorAll('.tab-btn');
  const tabPanes = document.querySelectorAll('.tab-pane');

  tabBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      tabBtns.forEach(b => b.classList.remove('active'));
      tabPanes.forEach(p => p.classList.remove('active'));
      btn.classList.add('active');
      const tabId = btn.getAttribute('data-tab');
      const pane = document.getElementById(tabId);
      if (pane) pane.classList.add('active');
    });
  });

  const subTabBtns = document.querySelectorAll('.sub-tab-btn');
  const subPanes = document.querySelectorAll('.sub-pane');

  subTabBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      subTabBtns.forEach(b => b.classList.remove('active'));
      subPanes.forEach(p => p.classList.remove('active'));
      btn.classList.add('active');
      const subId = btn.getAttribute('data-sub');
      const pane = document.getElementById(subId);
      if (pane) pane.classList.add('active');
    });
  });

  const consoleLogs = document.getElementById('console-logs');
  const btnClearLogs = document.getElementById('btn-clear-logs');

  function appendLog(message, category = 'info') {
    const timeStr = new Date().toLocaleTimeString();
    const line = document.createElement('div');
    line.className = `log-line log-${category}`;
    line.innerHTML = `<span class="log-time">[${timeStr}]</span> <span>${message}</span>`;
    consoleLogs.appendChild(line);
    consoleLogs.scrollTop = consoleLogs.scrollHeight;
  }

  if (btnClearLogs) {
    btnClearLogs.addEventListener('click', () => {
      consoleLogs.innerHTML = '';
    });
  }

  function initSSE() {
    const evtSource = new EventSource('/api/stream/logs');
    evtSource.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        appendLog(data.message, data.category || 'info');
      } catch (err) {
        appendLog(e.data);
      }
    };
    evtSource.onerror = () => {
      setTimeout(initSSE, 5000);
    };
  }
  initSSE();

  async function refreshSystemInfo() {
    try {
      const res = await fetch('/api/system');
      const data = await res.json();
      
      const gpuBadge = document.getElementById('gpu-badge');
      if (gpuBadge && data.device_name) {
        gpuBadge.textContent = `${data.device_name} • ${data.vram_gb}GB VRAM`;
      }

      const trainDatasetSelect = document.getElementById('train-dataset-select');
      if (trainDatasetSelect) {
        trainDatasetSelect.innerHTML = '';
        data.datasets.forEach(ds => {
          const opt = document.createElement('option');
          opt.value = ds.name;
          opt.textContent = `${ds.name} (${ds.clips} slices)`;
          trainDatasetSelect.appendChild(opt);
        });
      }

      const convertModelSelect = document.getElementById('convert-model-select');
      const modelsList = document.getElementById('models-list');
      if (convertModelSelect) convertModelSelect.innerHTML = '';
      if (modelsList) modelsList.innerHTML = '';

      if (data.models.length === 0) {
        if (modelsList) modelsList.innerHTML = '<div class="hint">No models trained yet. Train one in Tab 2.</div>';
      } else {
        data.models.forEach(m => {
          if (convertModelSelect) {
            const opt = document.createElement('option');
            opt.value = m.name;
            opt.textContent = `${m.name} ${m.has_index ? '(+FAISS Index)' : ''}`;
            convertModelSelect.appendChild(opt);
          }
          if (modelsList) {
            const item = document.createElement('div');
            item.className = 'model-card-item';
            item.innerHTML = `
              <span class="model-card-name">${m.name}</span>
              <span class="badge ${m.has_index ? 'badge-accent' : ''}">${m.has_index ? 'FAISS Index Ready' : 'Base Weights'}</span>
            `;
            modelsList.appendChild(item);
          }
        });
      }
    } catch (err) {
      appendLog(`Failed to fetch system info: ${err.message}`, 'error');
    }
  }
  refreshSystemInfo();

  const datasetDropZone = document.getElementById('dataset-drop-zone');
  const datasetFileInput = document.getElementById('dataset-file-input');
  const datasetFileList = document.getElementById('dataset-file-list');
  const btnUploadDataset = document.getElementById('btn-upload-dataset');
  const btnPreprocessDataset = document.getElementById('btn-preprocess-dataset');

  if (datasetDropZone) {
    datasetDropZone.addEventListener('click', () => datasetFileInput.click());
    datasetDropZone.addEventListener('dragover', (e) => {
      e.preventDefault();
      datasetDropZone.style.borderColor = 'var(--accent-primary)';
    });
    datasetDropZone.addEventListener('dragleave', () => {
      datasetDropZone.style.borderColor = '';
    });
    datasetDropZone.addEventListener('drop', (e) => {
      e.preventDefault();
      datasetDropZone.style.borderColor = '';
      handleDatasetFiles(e.dataTransfer.files);
    });
  }

  if (datasetFileInput) {
    datasetFileInput.addEventListener('change', (e) => {
      handleDatasetFiles(e.target.files);
    });
  }

  function handleDatasetFiles(files) {
    datasetFiles = Array.from(files);
    datasetFileList.innerHTML = '';
    datasetFiles.forEach(f => {
      const chip = document.createElement('span');
      chip.className = 'chip';
      chip.textContent = `${f.name} (${Math.round(f.size / 1024)} KB)`;
      datasetFileList.appendChild(chip);
    });
  }

  if (btnUploadDataset) {
    btnUploadDataset.addEventListener('click', async () => {
      if (datasetFiles.length === 0) {
        alert('Please select audio files first.');
        return;
      }
      const datasetName = document.getElementById('dataset-name-input').value.trim() || 'my_voice';
      const formData = new FormData();
      formData.append('dataset_name', datasetName);
      datasetFiles.forEach(f => formData.append('files', f));

      btnUploadDataset.disabled = true;
      btnUploadDataset.textContent = 'Uploading...';
      try {
        const res = await fetch('/api/dataset/upload', {
          method: 'POST',
          body: formData,
        });
        const data = await res.json();
        appendLog(`Uploaded ${data.files_count} files to dataset '${datasetName}'.`, 'success');
        refreshSystemInfo();
      } catch (err) {
        appendLog(`Upload failed: ${err.message}`, 'error');
      } finally {
        btnUploadDataset.disabled = false;
        btnUploadDataset.textContent = 'Upload Voice Files';
      }
    });
  }

  if (btnPreprocessDataset) {
    btnPreprocessDataset.addEventListener('click', async () => {
      const datasetName = document.getElementById('dataset-name-input').value.trim() || 'my_voice';
      const minDur = document.getElementById('min-duration-input').value;
      const maxDur = document.getElementById('max-duration-input').value;

      const formData = new FormData();
      formData.append('dataset_name', datasetName);
      formData.append('min_duration', minDur);
      formData.append('max_duration', maxDur);

      btnPreprocessDataset.disabled = true;
      btnPreprocessDataset.textContent = 'Processing Audio...';
      try {
        const res = await fetch('/api/dataset/preprocess', {
          method: 'POST',
          body: formData,
        });
        const data = await res.json();
        if (data.status === 'ok') {
          const stats = data.stats;
          document.getElementById('stat-clean-duration').textContent = `${Math.round(stats.clean_duration_sec / 60)}m`;
          document.getElementById('stat-clean-clips').textContent = stats.accepted_slices;
          document.getElementById('stat-rejected-clipped').textContent = stats.rejected_clipped;
          document.getElementById('stat-rejected-noisy').textContent = stats.rejected_noisy;
          document.getElementById('dataset-stats-card').classList.remove('hidden');
          appendLog(`Preprocessing complete! ${stats.accepted_slices} slices created.`, 'success');
          refreshSystemInfo();
        }
      } catch (err) {
        appendLog(`Preprocessing error: ${err.message}`, 'error');
      } finally {
        btnPreprocessDataset.disabled = false;
        btnPreprocessDataset.textContent = 'Run Preprocessing & Quality Verification';
      }
    });
  }

  const btnStartTrain = document.getElementById('btn-start-train');
  const btnStopTrain = document.getElementById('btn-stop-train');

  if (btnStartTrain) {
    btnStartTrain.addEventListener('click', async () => {
      const dsSelect = document.getElementById('train-dataset-select');
      const datasetName = dsSelect ? dsSelect.value : 'my_voice';
      const modelName = document.getElementById('train-model-name').value.trim() || 'my_singing_voice';
      const epochs = document.getElementById('train-epochs').value;
      const batchSize = document.getElementById('train-batch-size').value;
      const lr = document.getElementById('train-lr').value;

      if (!datasetName) {
        alert('Please select or prepare a dataset first.');
        return;
      }

      const formData = new FormData();
      formData.append('dataset_name', datasetName);
      formData.append('model_name', modelName);
      formData.append('epochs', epochs);
      formData.append('batch_size', batchSize);
      formData.append('lr', lr);

      btnStartTrain.disabled = true;
      btnStopTrain.disabled = false;

      try {
        const res = await fetch('/api/train/start', {
          method: 'POST',
          body: formData,
        });
        const data = await res.json();
        if (res.ok) {
          appendLog(`Training started for model '${modelName}'.`, 'success');
        } else {
          appendLog(`Failed to start training: ${data.detail}`, 'error');
          btnStartTrain.disabled = false;
          btnStopTrain.disabled = true;
        }
      } catch (err) {
        appendLog(`Training error: ${err.message}`, 'error');
        btnStartTrain.disabled = false;
        btnStopTrain.disabled = true;
      }
    });
  }

  if (btnStopTrain) {
    btnStopTrain.addEventListener('click', async () => {
      try {
        await fetch('/api/train/stop', { method: 'POST' });
        appendLog('Stop command sent to training worker.', 'warning');
        btnStartTrain.disabled = false;
        btnStopTrain.disabled = true;
      } catch (err) {
        appendLog(`Failed to stop training: ${err.message}`, 'error');
      }
    });
  }

  const songDropZone = document.getElementById('song-drop-zone');
  const songFileInput = document.getElementById('song-file-input');
  const songSelectedName = document.getElementById('song-selected-name');
  const btnUploadSong = document.getElementById('btn-upload-song');
  const btnDownloadUrl = document.getElementById('btn-download-url');
  const activeSongInput = document.getElementById('active-song-id');
  const btnRunSeparation = document.getElementById('btn-run-separation');

  if (songDropZone) {
    songDropZone.addEventListener('click', () => songFileInput.click());
    songFileInput.addEventListener('change', (e) => {
      if (e.target.files.length > 0) {
        songFile = e.target.files[0];
        songSelectedName.textContent = `Selected: ${songFile.name} (${Math.round(songFile.size / 1024)} KB)`;
      }
    });
  }

  if (btnUploadSong) {
    btnUploadSong.addEventListener('click', async () => {
      if (!songFile) {
        alert('Please choose an audio file first.');
        return;
      }
      const formData = new FormData();
      formData.append('file', songFile);
      btnUploadSong.disabled = true;
      btnUploadSong.textContent = 'Uploading Song...';

      try {
        const res = await fetch('/api/song/upload', {
          method: 'POST',
          body: formData,
        });
        const data = await res.json();
        activeSongId = data.song_id;
        activeSongInput.value = activeSongId;
        appendLog(`Song ingested successfully: ${data.filename} [ID: ${activeSongId}]`, 'success');
      } catch (err) {
        appendLog(`Upload failed: ${err.message}`, 'error');
      } finally {
        btnUploadSong.disabled = false;
        btnUploadSong.textContent = 'Ingest Song File';
      }
    });
  }

  if (btnDownloadUrl) {
    btnDownloadUrl.addEventListener('click', async () => {
      const url = document.getElementById('song-url-input').value.trim();
      if (!url) {
        alert('Please enter a YouTube or media URL.');
        return;
      }
      btnDownloadUrl.disabled = true;
      btnDownloadUrl.textContent = 'Downloading...';

      try {
        const res = await fetch('/api/song/url', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ url }),
        });
        const data = await res.json();
        if (res.ok) {
          activeSongId = data.song_id;
          activeSongInput.value = activeSongId;
          appendLog(`Downloaded URL song with ID: ${activeSongId}`, 'success');
        } else {
          appendLog(`Download failed: ${data.detail}`, 'error');
        }
      } catch (err) {
        appendLog(`Download error: ${err.message}`, 'error');
      } finally {
        btnDownloadUrl.disabled = false;
        btnDownloadUrl.textContent = 'Download Song via yt-dlp';
      }
    });
  }

  if (btnRunSeparation) {
    btnRunSeparation.addEventListener('click', async () => {
      if (!activeSongId) {
        alert('Please ingest a song first in step 3.');
        return;
      }

      btnRunSeparation.disabled = true;
      btnRunSeparation.textContent = 'Separating Stems (Demucs)...';

      const formData = new FormData();
      formData.append('song_id', activeSongId);

      try {
        const res = await fetch('/api/song/separate', {
          method: 'POST',
          body: formData,
        });
        const data = await res.json();
        if (res.ok) {
          appendLog('Demucs vocal & instrumental stem separation completed!', 'success');
          const vAudio = document.getElementById('audio-vocal-stem');
          const iAudio = document.getElementById('audio-inst-stem');
          vAudio.src = `/api/audio/storage/stems/${activeSongId}/vocals.wav?t=${Date.now()}`;
          iAudio.src = `/api/audio/storage/stems/${activeSongId}/instrumental.wav?t=${Date.now()}`;
          document.getElementById('stems-preview').classList.remove('hidden');
        } else {
          appendLog(`Separation failed: ${data.detail}`, 'error');
        }
      } catch (err) {
        appendLog(`Separation error: ${err.message}`, 'error');
      } finally {
        btnRunSeparation.disabled = false;
        btnRunSeparation.textContent = 'Split into Vocals & Instrumental';
      }
    });
  }

  const pitchSlider = document.getElementById('pitch-shift-range');
  const pitchVal = document.getElementById('pitch-val');
  const indexSlider = document.getElementById('index-rate-range');
  const indexVal = document.getElementById('index-val');
  const protectSlider = document.getElementById('protect-range');
  const protectVal = document.getElementById('protect-val');
  const gainSlider = document.getElementById('vocal-gain-range');
  const gainVal = document.getElementById('gain-val');

  if (pitchSlider) {
    pitchSlider.addEventListener('input', (e) => {
      pitchVal.textContent = (e.target.value > 0 ? `+${e.target.value}` : e.target.value);
    });
  }

  document.querySelectorAll('.btn-chip').forEach(btn => {
    btn.addEventListener('click', () => {
      const p = btn.getAttribute('data-pitch');
      pitchSlider.value = p;
      pitchVal.textContent = (p > 0 ? `+${p}` : p);
    });
  });

  if (indexSlider) {
    indexSlider.addEventListener('input', (e) => {
      indexVal.textContent = e.target.value;
    });
  }

  if (protectSlider) {
    protectSlider.addEventListener('input', (e) => {
      protectVal.textContent = e.target.value;
    });
  }

  if (gainSlider) {
    gainSlider.addEventListener('input', (e) => {
      gainVal.textContent = `${e.target.value} dB`;
    });
  }

  const btnConvertRemix = document.getElementById('btn-convert-remix');
  if (btnConvertRemix) {
    btnConvertRemix.addEventListener('click', async () => {
      if (!activeSongId) {
        alert('Please ingest a song and separate stems first in Tab 3.');
        return;
      }
      const modelSelect = document.getElementById('convert-model-select');
      const modelName = modelSelect ? modelSelect.value : '';
      if (!modelName) {
        alert('Please train or select a singing voice model first in Tab 2.');
        return;
      }

      btnConvertRemix.disabled = true;
      btnConvertRemix.textContent = 'Synthesizing Vocals & Realigning...';

      const convertForm = new FormData();
      convertForm.append('song_id', activeSongId);
      convertForm.append('model_name', modelName);
      convertForm.append('pitch_shift', pitchSlider.value);
      convertForm.append('index_rate', indexSlider.value);
      convertForm.append('protect', protectSlider.value);

      try {
        appendLog(`Triggering RVC v2 inference on vocals for song ${activeSongId}...`);
        const convRes = await fetch('/api/convert', {
          method: 'POST',
          body: convertForm,
        });
        const convData = await convRes.json();
        if (!convRes.ok) {
          throw new Error(convData.detail || 'Conversion failed');
        }

        appendLog('Vocal conversion finished. Now realigning latency drift & mastering mix...');
        const remixForm = new FormData();
        remixForm.append('song_id', activeSongId);
        remixForm.append('vocal_gain_db', gainSlider.value);
        remixForm.append('inst_gain_db', '0.0');

        const remixRes = await fetch('/api/remix', {
          method: 'POST',
          body: remixForm,
        });
        const remixData = await remixRes.json();
        if (!remixRes.ok) {
          throw new Error(remixData.detail || 'Remix failed');
        }

        appendLog(`Remix complete! Latency correction applied: ${remixData.outputs.latency_correction_ms}ms`, 'success');

        const audioSolo = document.getElementById('audio-converted-solo');
        const audioFinal = document.getElementById('audio-final-mix');
        const btnWav = document.getElementById('btn-download-wav');
        const btnMp3 = document.getElementById('btn-download-mp3');

        const convUrl = `/api/audio/storage/converted/${activeSongId}/converted_vocal.wav?t=${Date.now()}`;
        const mixWavUrl = `/api/audio/storage/remixed/${activeSongId}/final_mix.wav?t=${Date.now()}`;
        const mixMp3Url = `/api/audio/storage/remixed/${activeSongId}/final_mix.mp3?t=${Date.now()}`;

        audioSolo.src = convUrl;
        audioFinal.src = mixWavUrl;

        btnWav.href = mixWavUrl;
        btnWav.download = `voiceclone_${modelName}_${activeSongId}.wav`;

        btnMp3.href = mixMp3Url;
        btnMp3.download = `voiceclone_${modelName}_${activeSongId}.mp3`;

      } catch (err) {
        appendLog(`Error during conversion & remix: ${err.message}`, 'error');
      } finally {
        btnConvertRemix.disabled = false;
        btnConvertRemix.textContent = 'Convert Vocal & Master Remix';
      }
    });
  }
});
