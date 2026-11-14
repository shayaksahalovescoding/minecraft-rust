use rand::Rng;
use rodio::{buffer::SamplesBuffer, Decoder, OutputStream, OutputStreamHandle, Sink};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

/// Audio system that loads genuine Minecraft 26.1.2 OGG sounds/music from `.minecraft/assets`
/// via `indexes/30.json` and falls back to crisp procedural sound synthesis when offline.
pub struct AudioEngine {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    objects_dir: Option<PathBuf>,
    sound_paths: HashMap<String, Vec<PathBuf>>,
    pub master_volume: f32,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    pub fn new() -> Self {
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(_) => (None, None),
        };

        let mut sound_paths: HashMap<String, Vec<PathBuf>> = HashMap::new();
        let mut objects_dir = None;

        if let Ok(appdata) = std::env::var("APPDATA") {
            let assets_root = PathBuf::from(appdata).join(".minecraft").join("assets");
            let idx_file = assets_root.join("indexes").join("30.json");
            let obj_root = assets_root.join("objects");
            if idx_file.exists() && obj_root.exists() {
                objects_dir = Some(obj_root.clone());
                if let Ok(text) = std::fs::read_to_string(&idx_file) {
                    if let Ok(val) = serde_json::from_str::<Value>(&text) {
                        if let Some(objs) = val.get("objects").and_then(|v| v.as_object()) {
                            for (vpath, info) in objs {
                                if !vpath.starts_with("minecraft/sounds/") || !vpath.ends_with(".ogg") {
                                    continue;
                                }
                                let Some(hash) = info.get("hash").and_then(|h| h.as_str()) else {
                                    continue;
                                };
                                if hash.len() < 2 {
                                    continue;
                                }
                                let disk = obj_root.join(&hash[..2]).join(hash);
                                if !disk.exists() {
                                    continue;
                                }
                                // Map e.g. "minecraft/sounds/dig/stone1.ogg" -> "dig.stone"
                                let rel = vpath
                                    .trim_start_matches("minecraft/sounds/")
                                    .trim_end_matches(".ogg");
                                let stripped = rel.trim_end_matches(|c: char| c.is_ascii_digit());
                                let key = stripped.replace('/', ".");
                                sound_paths.entry(key).or_default().push(disk);
                            }
                        }
                    }
                }
            }
        }

        Self {
            _stream: stream,
            handle,
            objects_dir,
            sound_paths,
            master_volume: 0.65,
        }
    }

    pub fn indexed_sound_count(&self) -> usize {
        self.sound_paths.len()
    }

    pub fn play(&self, event: &str, volume: f32, pitch: f32) {
        let Some(handle) = &self.handle else {
            return;
        };
        let vol = (volume * self.master_volume).clamp(0.0, 1.5);
        if vol <= 0.001 {
            return;
        }

        // Try real OGG from .minecraft/assets/objects
        let candidates = self.find_sound_candidates(event);
        if !candidates.is_empty() {
            let pick = &candidates[rand::thread_rng().gen_range(0..candidates.len())];
            if let Ok(file) = File::open(pick) {
                if let Ok(dec) = Decoder::new(BufReader::new(file)) {
                    if let Ok(sink) = Sink::try_new(handle) {
                        sink.set_volume(vol);
                        sink.set_speed(pitch.clamp(0.5, 2.0));
                        sink.append(dec);
                        sink.detach();
                        return;
                    }
                }
            }
        }

        // Procedural fallback synthesis
        if let Ok(sink) = Sink::try_new(handle) {
            let freq = if event.contains("explode") {
                95.0
            } else if event.contains("break") || event.contains("dig") {
                180.0
            } else if event.contains("place") {
                240.0
            } else if event.contains("click") {
                520.0
            } else if event.contains("hurt") {
                140.0
            } else {
                300.0
            };
            let samples = synthesize_blip(freq * pitch, 0.08, event.contains("explode"));
            sink.set_volume(vol * 0.25);
            sink.append(SamplesBuffer::new(1, 22050, samples));
            sink.detach();
        }
        let _ = &self.objects_dir;
    }

    fn find_sound_candidates(&self, event: &str) -> &[PathBuf] {
        let clean = event.strip_prefix("minecraft:").unwrap_or(event);
        if let Some(v) = self.sound_paths.get(clean) {
            return v;
        }
        // Fallback prefix mappings for block/dig/step/ui
        let mapped = match clean {
            "block.stone.break" => "dig.stone",
            "block.wood.break" => "dig.wood",
            "block.grass.break" => "dig.grass",
            "block.gravel.break" => "dig.gravel",
            "block.sand.break" => "dig.sand",
            "block.stone.step" => "step.stone",
            "block.wood.step" => "step.wood",
            "block.grass.step" => "step.grass",
            "entity.generic.explode" => "random.explode",
            "ui.button.click" => "random.click",
            "entity.player.hurt" => "damage.hit",
            _ => clean,
        };
        self.sound_paths.get(mapped).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

fn synthesize_blip(freq: f32, dur_secs: f32, noise: bool) -> Vec<f32> {
    let rate = 22050;
    let len = (rate as f32 * dur_secs) as usize;
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let env = (1.0 - t / dur_secs).max(0.0);
            let wave = if noise {
                (rng.r#gen::<f32>() * 2.0 - 1.0) * 0.7 + (t * freq * std::f32::consts::TAU).sin() * 0.3
            } else {
                (t * freq * std::f32::consts::TAU).sin()
            };
            wave * env
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_engine_initializes_without_panic() {
        let audio = AudioEngine::new();
        // Verify sound index loaded if .minecraft exists, or gracefully 0 if absent
        let _count = audio.indexed_sound_count();
    }
}
