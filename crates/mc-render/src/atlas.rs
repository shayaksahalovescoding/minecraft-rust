use image::imageops::FilterType;
use mc_core::Direction;
use mc_data::{BlockDef, BlockRegistry, GameData, ItemRegistry};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy)]
pub struct BlockFaceUvs {
    pub top: [f32; 4],
    pub bottom: [f32; 4],
    pub side: [f32; 4],
    pub front: [f32; 4],
    pub overlay: Option<[f32; 4]>,
}

impl BlockFaceUvs {
    pub fn for_face(&self, dir: Direction, facing: Direction) -> [f32; 4] {
        match dir {
            Direction::Up => self.top,
            Direction::Down => self.bottom,
            _ if dir == facing => self.front,
            _ => self.side,
        }
    }
}

pub struct TextureAtlas {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub uv_by_name: HashMap<String, [f32; 4]>,
    pub block_uvs: Vec<BlockFaceUvs>,
    pub item_uvs: Vec<[f32; 4]>,
    pub destroy_uvs: [[f32; 4]; 10],
    pub white_uv: [f32; 4],
    pub loaded_from_jar: bool,
}

impl TextureAtlas {
    pub fn build(data: &GameData) -> Self {
        let mut needed: HashSet<String> = HashSet::new();
        for b in data.blocks.all() {
            for t in [
                &b.textures.top,
                &b.textures.bottom,
                &b.textures.side,
                &b.textures.front,
                &b.textures.overlay,
            ]
            .into_iter()
            .flatten()
            {
                needed.insert(t.clone());
            }
        }
        for item in data.items.all() {
            if let Some(t) = &item.texture {
                needed.insert(t.clone());
            }
        }
        for stage in 0..10 {
            needed.insert(format!("block/destroy_stage_{stage}"));
        }
        needed.insert("block/water_still".to_string());
        needed.insert("block/lava_still".to_string());
        needed.insert("block/nether_portal".to_string());
        needed.insert("block/fire_0".to_string());

        let (jar_tiles, loaded_from_jar) = load_jar_textures(&needed);

        let mut tile_list: Vec<(String, [u8; 16 * 16 * 4])> = Vec::with_capacity(needed.len() + 2);
        tile_list.push(("__white".to_string(), [255u8; 16 * 16 * 4]));

        let mut sorted_names: Vec<String> = needed.into_iter().collect();
        sorted_names.sort();

        for name in sorted_names {
            let pixels = if let Some(px) = jar_tiles.get(&name) {
                *px
            } else {
                generate_procedural_tile(&name, data)
            };
            tile_list.push((name, pixels));
        }

        let tile_count = tile_list.len() as u32;
        let cols: u32 = 64;
        let rows: u32 = tile_count.div_ceil(cols).max(1);
        let cell: u32 = 18; // 16x16 with 1px extruded border on each side
        let width = cols * cell;
        let height = rows * cell;
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        let mut uv_by_name = HashMap::with_capacity(tile_list.len());

        for (idx, (name, pixels)) in tile_list.iter().enumerate() {
            let col = (idx as u32) % cols;
            let row = (idx as u32) / cols;
            let ox = col * cell + 1;
            let oy = row * cell + 1;

            // Blit 16x16 inner region
            for y in 0..16u32 {
                for x in 0..16u32 {
                    let src = ((y * 16 + x) * 4) as usize;
                    put_px(&mut rgba, width, ox + x, oy + y, [
                        pixels[src],
                        pixels[src + 1],
                        pixels[src + 2],
                        pixels[src + 3],
                    ]);
                }
            }
            // Extrude 1px borders to prevent any sampling artifacts
            for x in 0..16u32 {
                let top_px = get_px(&rgba, width, ox + x, oy);
                put_px(&mut rgba, width, ox + x, oy - 1, top_px);
                let bot_px = get_px(&rgba, width, ox + x, oy + 15);
                put_px(&mut rgba, width, ox + x, oy + 16, bot_px);
            }
            for y in 0..18u32 {
                let py = oy - 1 + y;
                let left_px = get_px(&rgba, width, ox, py);
                put_px(&mut rgba, width, ox - 1, py, left_px);
                let right_px = get_px(&rgba, width, ox + 15, py);
                put_px(&mut rgba, width, ox + 16, py, right_px);
            }

            let u0 = ox as f32 / width as f32;
            let v0 = oy as f32 / height as f32;
            let u1 = (ox + 16) as f32 / width as f32;
            let v1 = (oy + 16) as f32 / height as f32;
            uv_by_name.insert(name.clone(), [u0, v0, u1, v1]);
        }

        let white_uv = *uv_by_name.get("__white").unwrap();
        let block_uvs = build_block_uvs(&data.blocks, &uv_by_name, white_uv);
        let item_uvs = build_item_uvs(&data.items, &block_uvs, &uv_by_name, white_uv);
        let destroy_uvs = std::array::from_fn(|i| {
            *uv_by_name
                .get(&format!("block/destroy_stage_{i}"))
                .unwrap_or(&white_uv)
        });

        Self {
            width,
            height,
            rgba,
            uv_by_name,
            block_uvs,
            item_uvs,
            destroy_uvs,
            white_uv,
            loaded_from_jar,
        }
    }

    pub fn block_uv(&self, id: mc_data::BlockId) -> BlockFaceUvs {
        self.block_uvs
            .get(id.0 as usize)
            .copied()
            .unwrap_or(BlockFaceUvs {
                top: self.white_uv,
                bottom: self.white_uv,
                side: self.white_uv,
                front: self.white_uv,
                overlay: None,
            })
    }

    pub fn item_uv(&self, id: mc_data::ItemId) -> [f32; 4] {
        self.item_uvs
            .get(id.0 as usize)
            .copied()
            .unwrap_or(self.white_uv)
    }
}

fn put_px(buf: &mut [u8], width: u32, x: u32, y: u32, px: [u8; 4]) {
    let i = ((y * width + x) * 4) as usize;
    buf[i..i + 4].copy_from_slice(&px);
}

fn get_px(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

fn build_block_uvs(
    blocks: &BlockRegistry,
    uv_by_name: &HashMap<String, [f32; 4]>,
    fallback: [f32; 4],
) -> Vec<BlockFaceUvs> {
    blocks
        .all()
        .iter()
        .map(|b| {
            let resolve = |opt: &Option<String>| -> Option<[f32; 4]> {
                opt.as_ref().and_then(|s| uv_by_name.get(s).copied())
            };
            let side = resolve(&b.textures.side)
                .or_else(|| resolve(&b.textures.top))
                .or_else(|| uv_by_name.get(&format!("block/{}", b.name)).copied())
                .unwrap_or(fallback);
            let top = resolve(&b.textures.top).unwrap_or(side);
            let bottom = resolve(&b.textures.bottom).unwrap_or(top);
            let front = resolve(&b.textures.front).unwrap_or(side);
            let overlay = resolve(&b.textures.overlay);
            BlockFaceUvs {
                top,
                bottom,
                side,
                front,
                overlay,
            }
        })
        .collect()
}

fn build_item_uvs(
    items: &ItemRegistry,
    block_uvs: &[BlockFaceUvs],
    uv_by_name: &HashMap<String, [f32; 4]>,
    fallback: [f32; 4],
) -> Vec<[f32; 4]> {
    items
        .all()
        .iter()
        .map(|item| {
            if let Some(t) = &item.texture {
                if let Some(uv) = uv_by_name.get(t) {
                    return *uv;
                }
            }
            if let Some(uv) = uv_by_name.get(&format!("item/{}", item.name)) {
                return *uv;
            }
            if let Some(bid) = item.places_block {
                if let Some(buv) = block_uvs.get(bid.0 as usize) {
                    return buv.front;
                }
            }
            fallback
        })
        .collect()
}

fn find_minecraft_jar() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let mc = PathBuf::from(appdata).join(".minecraft").join("versions");
        candidates.push(mc.join("Fabric 26.1.2").join("Fabric 26.1.2.jar"));
        candidates.push(mc.join("26.1.2").join("26.1.2.jar"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mc = PathBuf::from(home).join(".minecraft").join("versions");
        candidates.push(mc.join("Fabric 26.1.2").join("Fabric 26.1.2.jar"));
        candidates.push(mc.join("26.1.2").join("26.1.2.jar"));
    }
    candidates.into_iter().find(|p| p.exists())
}

fn load_jar_textures(needed: &HashSet<String>) -> (HashMap<String, [u8; 16 * 16 * 4]>, bool) {
    let mut out = HashMap::new();
    let Some(jar_path) = find_minecraft_jar() else {
        return (out, false);
    };
    let Ok(file) = File::open(&jar_path) else {
        return (out, false);
    };
    let Ok(mut archive) = ZipArchive::new(BufReader::new(file)) else {
        return (out, false);
    };

    let prefix = "assets/minecraft/textures/";
    let mut bytes = Vec::with_capacity(4096);
    for idx in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(idx) else {
            continue;
        };
        let name = entry.name().to_string();
        if !name.starts_with(prefix) || !name.ends_with(".png") {
            continue;
        }
        let key = &name[prefix.len()..name.len() - 4];
        if !needed.contains(key) {
            continue;
        }
        bytes.clear();
        if entry.read_to_end(&mut bytes).is_err() {
            continue;
        }
        if let Some(tile) = decode_16x16_tile(&bytes) {
            out.insert(key.to_string(), tile);
        }
    }

    let ok = !out.is_empty();
    (out, ok)
}

fn decode_16x16_tile(png_bytes: &[u8]) -> Option<[u8; 16 * 16 * 4]> {
    let img = image::load_from_memory(png_bytes).ok()?;
    let w = img.width().max(1);
    let h = img.height().max(1);
    // Animated vanilla textures are vertical strips of w x w frames; take frame 0.
    let cropped = if h > w && h % w == 0 {
        img.crop_imm(0, 0, w, w)
    } else {
        img
    };
    let rgba = if cropped.width() == 16 && cropped.height() == 16 {
        cropped.to_rgba8()
    } else {
        cropped.resize_exact(16, 16, FilterType::Nearest).to_rgba8()
    };
    let mut out = [0u8; 16 * 16 * 4];
    out.copy_from_slice(rgba.as_raw());
    Some(out)
}

fn generate_procedural_tile(tex_name: &str, data: &GameData) -> [u8; 16 * 16 * 4] {
    let mut tile = [0u8; 16 * 16 * 4];

    // Special cases for destroy cracks
    if let Some(stage_str) = tex_name.strip_prefix("block/destroy_stage_") {
        let stage = stage_str.parse::<u32>().unwrap_or(0) + 1;
        for y in 0..16u32 {
            for x in 0..16u32 {
                let h = hash2(x, y, stage * 97);
                let crack = (x == y || x + y == 15 || (h & 7) == 0) && (h % 12) <= stage;
                let a = if crack { 170 } else { 0 };
                let idx = ((y * 16 + x) * 4) as usize;
                tile[idx..idx + 4].copy_from_slice(&[20, 20, 20, a]);
            }
        }
        return tile;
    }

    let short = tex_name
        .strip_prefix("block/")
        .or_else(|| tex_name.strip_prefix("item/"))
        .unwrap_or(tex_name);

    let base_rgba = find_block_for_tex(short, &data.blocks)
        .map(|b| b.fallback_rgba)
        .unwrap_or_else(|| hash_color(short));

    for y in 0..16u32 {
        for x in 0..16u32 {
            let h = hash2(x, y, hash_str(short));
            let jitter = ((h % 25) as i16) - 12;
            let mut r = (base_rgba[0] as i16 + jitter).clamp(0, 255) as u8;
            let mut g = (base_rgba[1] as i16 + jitter).clamp(0, 255) as u8;
            let mut b = (base_rgba[2] as i16 + jitter).clamp(0, 255) as u8;
            let mut a = base_rgba[3];

            if short.contains("bricks") || short.contains("cobble") {
                if y % 4 == 0 || ((x + (y / 4) * 4) % 8 == 0) {
                    r = r.saturating_sub(32);
                    g = g.saturating_sub(32);
                    b = b.saturating_sub(32);
                }
            } else if short.contains("planks") {
                if y % 4 == 0 {
                    r = r.saturating_sub(26);
                    g = g.saturating_sub(22);
                    b = b.saturating_sub(18);
                }
            } else if short.contains("log") && !short.ends_with("top") {
                if x % 3 == 0 {
                    r = r.saturating_sub(18);
                    g = g.saturating_sub(18);
                    b = b.saturating_sub(18);
                }
            } else if short.contains("leaves") {
                if (h % 5) == 0 {
                    a = 0;
                }
            } else if short.contains("ore") && (h % 13) == 0 {
                r = r.saturating_add(45);
                g = g.saturating_add(45);
                b = b.saturating_add(45);
            } else if tex_name.starts_with("item/") {
                let dx = (x as i32) - 8;
                let dy = (y as i32) - 8;
                if dx * dx + dy * dy > 42 {
                    a = 0;
                }
            }

            let idx = ((y * 16 + x) * 4) as usize;
            tile[idx..idx + 4].copy_from_slice(&[r, g, b, a]);
        }
    }
    tile
}

fn find_block_for_tex<'a>(short: &str, blocks: &'a BlockRegistry) -> Option<&'a BlockDef> {
    if let Some(id) = blocks.by_name(short) {
        return Some(blocks.get(id));
    }
    let stripped = short
        .trim_end_matches("_top")
        .trim_end_matches("_side")
        .trim_end_matches("_bottom")
        .trim_end_matches("_front");
    blocks.by_name(stripped).map(|id| blocks.get(id))
}

fn hash_str(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

fn hash_color(name: &str) -> [u8; 4] {
    let h = hash_str(name);
    [
        72 + ((h & 0x7F) as u8),
        72 + (((h >> 8) & 0x7F) as u8),
        72 + (((h >> 16) & 0x7F) as u8),
        255,
    ]
}

fn hash2(x: u32, y: u32, seed: u32) -> u32 {
    let mut v = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ seed;
    v = (v ^ (v >> 13)).wrapping_mul(1274126177);
    v ^ (v >> 16)
}
