use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use mc_core::{BlockPos, ChunkPos, Difficulty, DimensionId, GameMode, SharedConstants};
use mc_data::GameData;
use mc_entities::{Entity, Player};
use mc_nbt::{NbtCompound, NbtTag, RegionCompression, RegionFile};
use mc_world::{Chunk, World};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMetadata {
    pub name: String,
    pub seed: i64,
    pub game_mode: GameMode,
    pub difficulty: Difficulty,
    pub day_time: i64,
    pub world_tick: u64,
    pub spawn_point: BlockPos,
}

#[derive(Debug, Clone)]
pub struct WorldSummary {
    pub folder_name: String,
    pub dir: PathBuf,
    pub meta: WorldMetadata,
}

pub fn default_saves_root() -> PathBuf {
    PathBuf::from("saves")
}

pub fn list_worlds(saves_root: &Path) -> Vec<WorldSummary> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(saves_root) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let level_dat = dir.join("level.dat");
        if !level_dat.exists() {
            continue;
        }
        if let Ok((meta, _player)) = load_level_dat(&level_dat) {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            out.push(WorldSummary {
                folder_name,
                dir,
                meta,
            });
        }
    }
    out.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));
    out
}

pub fn sanitize_world_folder(name: &str) -> String {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "New World".to_string()
    } else {
        clean
    }
}

pub fn region_dir(world_dir: &Path, dim: DimensionId) -> PathBuf {
    match dim {
        DimensionId::Overworld => world_dir.join("region"),
        DimensionId::TheNether => world_dir.join("DIM-1").join("region"),
        DimensionId::TheEnd => world_dir.join("DIM1").join("region"),
    }
}

fn dim_slug(dim: DimensionId) -> &'static str {
    match dim {
        DimensionId::Overworld => "overworld",
        DimensionId::TheNether => "the_nether",
        DimensionId::TheEnd => "the_end",
    }
}

pub fn region_file_path(world_dir: &Path, dim: DimensionId, cpos: ChunkPos) -> PathBuf {
    region_dir(world_dir, dim).join(format!("r.{}.{}.mca", cpos.region_x(), cpos.region_z()))
}

pub fn save_level_dat(world_dir: &Path, world: &World, world_name: &str, player: &Player) -> io::Result<()> {
    fs::create_dir_all(world_dir)?;
    let mut data = NbtCompound::new();
    data.insert("DataVersion".into(), NbtTag::Int(SharedConstants::WORLD_VERSION));
    data.insert("version".into(), NbtTag::Int(19133));
    data.insert("LevelName".into(), NbtTag::String(world_name.to_string()));
    data.insert("RandomSeed".into(), NbtTag::Long(world.seed));
    data.insert("GameType".into(), NbtTag::Int(player.game_mode as i32));
    data.insert("Difficulty".into(), NbtTag::Byte(world.difficulty as i8));
    data.insert("DayTime".into(), NbtTag::Long(world.day_time));
    data.insert("Time".into(), NbtTag::Long(world.world_tick as i64));
    data.insert("SpawnX".into(), NbtTag::Int(world.spawn_point.x));
    data.insert("SpawnY".into(), NbtTag::Int(world.spawn_point.y));
    data.insert("SpawnZ".into(), NbtTag::Int(world.spawn_point.z));
    data.insert("raining".into(), NbtTag::Byte(i8::from(world.is_raining)));
    data.insert("thundering".into(), NbtTag::Byte(i8::from(world.is_thundering)));

    let player_json = serde_json::to_string(player).unwrap_or_default();
    data.insert("PlayerState".into(), NbtTag::String(player_json));

    let mut root = NbtCompound::new();
    root.insert("Data".into(), NbtTag::Compound(data));
    let tag = NbtTag::Compound(root);

    let file = File::create(world_dir.join("level.dat"))?;
    let mut enc = GzEncoder::new(file, Compression::default());
    tag.write_named(&mut enc, "")?;
    enc.finish()?;
    Ok(())
}

pub fn load_level_dat(path: &Path) -> io::Result<(WorldMetadata, Option<Player>)> {
    let file = File::open(path)?;
    let mut dec = GzDecoder::new(file);
    let (_, root) = NbtTag::read_named(&mut dec)?;
    let Some(root_c) = root.as_compound() else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid level.dat root"));
    };
    let data = root_c
        .get("Data")
        .and_then(|t| t.as_compound())
        .unwrap_or(root_c);

    let name = data
        .get("LevelName")
        .and_then(|t| t.as_str())
        .unwrap_or("World")
        .to_string();
    let seed = data.get("RandomSeed").and_then(|t| t.as_i64()).unwrap_or(1);
    let gm_id = data.get("GameType").and_then(|t| t.as_i64()).unwrap_or(0);
    let game_mode = match gm_id {
        1 => GameMode::Creative,
        2 => GameMode::Adventure,
        3 => GameMode::Spectator,
        _ => GameMode::Survival,
    };
    let diff_id = data.get("Difficulty").and_then(|t| t.as_i64()).unwrap_or(2);
    let difficulty = match diff_id {
        0 => Difficulty::Peaceful,
        1 => Difficulty::Easy,
        3 => Difficulty::Hard,
        _ => Difficulty::Normal,
    };
    let day_time = data.get("DayTime").and_then(|t| t.as_i64()).unwrap_or(1000);
    let world_tick = data.get("Time").and_then(|t| t.as_i64()).unwrap_or(0).max(0) as u64;
    let sx = data.get("SpawnX").and_then(|t| t.as_i64()).unwrap_or(0) as i32;
    let sy = data.get("SpawnY").and_then(|t| t.as_i64()).unwrap_or(80) as i32;
    let sz = data.get("SpawnZ").and_then(|t| t.as_i64()).unwrap_or(0) as i32;

    let player = data
        .get("PlayerState")
        .and_then(|t| t.as_str())
        .and_then(|s| serde_json::from_str::<Player>(s).ok());

    Ok((
        WorldMetadata {
            name,
            seed,
            game_mode,
            difficulty,
            day_time,
            world_tick,
            spawn_point: BlockPos::new(sx, sy, sz),
        },
        player,
    ))
}

pub fn save_dirty_chunks(world_dir: &Path, world: &mut World, data: &GameData) -> io::Result<usize> {
    let mut by_region: HashMap<(i32, i32), Vec<ChunkPos>> = HashMap::new();
    for (pos, chunk) in &world.chunks {
        if chunk.dirty_save {
            by_region
                .entry((pos.region_x(), pos.region_z()))
                .or_default()
                .push(*pos);
        }
    }

    let mut saved = 0usize;
    for ((rx, rz), positions) in by_region {
        let path = region_dir(world_dir, world.dimension).join(format!("r.{rx}.{rz}.mca"));
        let mut rf = RegionFile::open(&path)?;
        for cpos in positions {
            if let Some(chunk) = world.chunks.get_mut(&cpos) {
                let nbt = chunk.to_nbt(&data.blocks);
                rf.write_chunk_nbt(cpos, &nbt, RegionCompression::Zlib)?;
                chunk.dirty_save = false;
                saved += 1;
            }
        }
    }
    Ok(saved)
}

pub fn load_chunk_from_disk(
    world_dir: &Path,
    dim: DimensionId,
    cpos: ChunkPos,
    data: &GameData,
) -> Option<Chunk> {
    let path = region_file_path(world_dir, dim, cpos);
    if !path.exists() {
        return None;
    }
    let mut rf = RegionFile::open(&path).ok()?;
    let nbt = rf.read_chunk_nbt(cpos).ok().flatten()?;
    Chunk::from_nbt(&nbt, dim, &data.blocks)
}

pub fn save_entities(world_dir: &Path, dim: DimensionId, entities: &[Entity]) -> io::Result<()> {
    let dir = world_dir.join("entities");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", dim_slug(dim)));
    let json = serde_json::to_string(entities).unwrap_or_else(|_| "[]".into());
    fs::write(path, json)
}

pub fn load_entities(world_dir: &Path, dim: DimensionId) -> Vec<Entity> {
    let path = world_dir.join("entities").join(format!("{}.json", dim_slug(dim)));
    let Ok(s) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&s).unwrap_or_default()
}
