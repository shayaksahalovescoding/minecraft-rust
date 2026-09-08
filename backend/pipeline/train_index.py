import os
from pathlib import Path
import numpy as np
import faiss

def build_faiss_index(features_dir, output_index_path, progress_callback=None):
    feats_path = Path(features_dir)
    npy_files = sorted(list(feats_path.glob("*.npy")))
    if not npy_files:
        raise ValueError(f"No feature .npy files found in {features_dir}")

    if progress_callback:
        progress_callback(f"Loading features from {len(npy_files)} files...")

    all_feats = []
    for f in npy_files:
        data = np.load(str(f))
        if data.ndim == 2 and data.shape[1] == 768:
            all_feats.append(data)

    if not all_feats:
        raise ValueError("No valid 768-dim feature arrays found.")

    big_array = np.concatenate(all_feats, axis=0).astype(np.float32)
    n_samples, dim = big_array.shape

    if progress_callback:
        progress_callback(f"Building FAISS index with {n_samples} vectors of dim {dim}...")

    output_path = Path(output_index_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if n_samples >= 10000:
        nlist = int(np.sqrt(n_samples))
        quantizer = faiss.IndexFlatL2(dim)
        index = faiss.IndexIVFFlat(quantizer, dim, nlist, faiss.METRIC_L2)
        index.train(big_array)
        index.add(big_array)
    else:
        index = faiss.IndexFlatL2(dim)
        index.add(big_array)

    faiss.write_index(index, str(output_path))

    if progress_callback:
        progress_callback(f"FAISS index successfully built and saved to {output_path.name} ({round(output_path.stat().st_size / (1024*1024), 2)} MB).")

    return str(output_path)
