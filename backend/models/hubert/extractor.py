import torch
import torch.nn.functional as F
import numpy as np
import librosa
from transformers import HubertModel

class ContentVecExtractor:
    def __init__(self, model_name="lengyue233/content-vec-best", device="cuda"):
        self.device = device
        self.model = HubertModel.from_pretrained(model_name).to(device)
        self.model.eval()

    def extract_features(self, audio, sr=16000, target_layer=9):
        if sr != 16000:
            audio = librosa.resample(audio, orig_sr=sr, target_sr=16000)
        
        tensor = torch.from_numpy(audio).float().unsqueeze(0).to(self.device)
        
        with torch.no_grad():
            outputs = self.model(tensor, output_hidden_states=True)
            feats = outputs.hidden_states[target_layer]
            feats = feats.squeeze(0).cpu().numpy()
        
        return feats
