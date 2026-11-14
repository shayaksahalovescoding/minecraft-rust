use crate::block_entity::BlockEntityData;
use mc_core::{BlockPos, ChunkPos, DimensionId, SharedConstants};
use mc_data::{BlockRegistry, BlockState};
use mc_nbt::{NbtCompound, NbtTag};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ChunkSection {
    pub blocks: Box<[BlockState; 4096]>,
    pub sky_light: Box<[u8; 2048]>,
    pub block_light: Box<[u8; 2048]>,
    pub non_air_count: u16,
    pub dirty_mesh: bool,
}

impl Default for ChunkSection {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkSection {
    pub fn new() -> Self {
        Self {
            blocks: Box::new([BlockState::AIR; 4096]),
            sky_light: Box::new([0xFF; 2048]),
            block_light: Box::new([0x00; 2048]),
            non_air_count: 0,
            dirty_mesh: true,
        }
    }

    #[inline]
    pub const fn idx(x: usize, y: usize, z: usize) -> usize {
        (y << 8) | (z << 4) | x
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockState {
        self.blocks[Self::idx(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, state: BlockState) -> BlockState {
        let i = Self::idx(x, y, z);
        let prev = self.blocks[i];
        if prev.is_air() && !state.is_air() {
            self.non_air_count += 1;
        } else if !prev.is_air() && state.is_air() {
            self.non_air_count = self.non_air_count.saturating_sub(1);
        }
        self.blocks[i] = state;
        self.dirty_mesh = true;
        prev
    }

    #[inline]
    pub fn get_sky_light(&self, x: usize, y: usize, z: usize) -> u8 {
        let i = Self::idx(x, y, z);
        let b = self.sky_light[i >> 1];
        if (i & 1) == 0 {
            b & 0x0F
        } else {
            (b >> 4) & 0x0F
        }
    }

    #[inline]
    pub fn set_sky_light(&mut self, x: usize, y: usize, z: usize, val: u8) {
        let i = Self::idx(x, y, z);
        let slot = &mut self.sky_light[i >> 1];
        if (i & 1) == 0 {
            *slot = (*slot & 0xF0) | (val & 0x0F);
        } else {
            *slot = (*slot & 0x0F) | ((val & 0x0F) << 4);
        }
    }

    #[inline]
    pub fn get_block_light(&self, x: usize, y: usize, z: usize) -> u8 {
        let i = Self::idx(x, y, z);
        let b = self.block_light[i >> 1];
        if (i & 1) == 0 {
            b & 0x0F
        } else {
            (b >> 4) & 0x0F
        }
    }

    #[inline]
    pub fn set_block_light(&mut self, x: usize, y: usize, z: usize, val: u8) {
        let i = Self::idx(x, y, z);
        let slot = &mut self.block_light[i >> 1];
        if (i & 1) == 0 {
            *slot = (*slot & 0xF0) | (val & 0x0F);
        } else {
            *slot = (*slot & 0x0F) | ((val & 0x0F) << 4);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub pos: ChunkPos,
    pub dimension: DimensionId,
    pub min_y: i32,
    pub sections: Vec<ChunkSection>,
    pub biomes: Vec<String>,
    pub heightmap: [i16; 256],
    pub block_entities: HashMap<BlockPos, BlockEntityData>,
    pub dirty_mesh: bool,
    pub dirty_save: bool,
    pub light_populated: bool,
}

impl Chunk {
    pub fn new(pos: ChunkPos, dimension: DimensionId) -> Self {
        let min_y = dimension.min_y();
        let num_sections = (dimension.height() / 16) as usize;
        let default_sky = if dimension.has_skylight() { 0xFF } else { 0x00 };
        let mut sections = Vec::with_capacity(num_sections);
        for _ in 0..num_sections {
            let mut s = ChunkSection::new();
            s.sky_light.fill(default_sky);
            sections.push(s);
        }
        Self {
            pos,
            dimension,
            min_y,
            sections,
            biomes: vec!["plains".to_string(); 256],
            heightmap: [min_y as i16; 256],
            block_entities: HashMap::new(),
            dirty_mesh: true,
            dirty_save: true,
            light_populated: false,
        }
    }

    #[inline]
    pub fn section_index(&self, y: i32) -> Option<usize> {
        let rel = y - self.min_y;
        if rel < 0 {
            return None;
        }
        let idx = (rel / 16) as usize;
        (idx < self.sections.len()).then_some(idx)
    }

    #[inline]
    pub fn get_block(&self, lx: usize, y: i32, lz: usize) -> BlockState {
        let Some(si) = self.section_index(y) else {
            return BlockState::AIR;
        };
        let ly = y.rem_euclid(16) as usize;
        self.sections[si].get(lx & 15, ly, lz & 15)
    }

    #[inline]
    pub fn set_block(&mut self, lx: usize, y: i32, lz: usize, state: BlockState) -> BlockState {
        let Some(si) = self.section_index(y) else {
            return BlockState::AIR;
        };
        let ly = y.rem_euclid(16) as usize;
        let prev = self.sections[si].set(lx & 15, ly, lz & 15, state);
        self.dirty_mesh = true;
        self.dirty_save = true;
        let col = (lz & 15) * 16 + (lx & 15);
        if !state.is_air() && y as i16 > self.heightmap[col] {
            self.heightmap[col] = y as i16;
        } else if state.is_air() && y as i16 == self.heightmap[col] {
            let mut ny = y - 1;
            while ny >= self.min_y && self.get_block(lx, ny, lz).is_air() {
                ny -= 1;
            }
            self.heightmap[col] = ny as i16;
        }
        prev
    }

    #[inline]
    pub fn get_sky_light(&self, lx: usize, y: i32, lz: usize) -> u8 {
        if let Some(si) = self.section_index(y) {
            self.sections[si].get_sky_light(lx & 15, y.rem_euclid(16) as usize, lz & 15)
        } else if y > self.min_y {
            15
        } else {
            0
        }
    }

    #[inline]
    pub fn set_sky_light(&mut self, lx: usize, y: i32, lz: usize, val: u8) {
        if let Some(si) = self.section_index(y) {
            self.sections[si].set_sky_light(lx & 15, y.rem_euclid(16) as usize, lz & 15, val);
        }
    }

    #[inline]
    pub fn get_block_light(&self, lx: usize, y: i32, lz: usize) -> u8 {
        if let Some(si) = self.section_index(y) {
            self.sections[si].get_block_light(lx & 15, y.rem_euclid(16) as usize, lz & 15)
        } else {
            0
        }
    }

    #[inline]
    pub fn set_block_light(&mut self, lx: usize, y: i32, lz: usize, val: u8) {
        if let Some(si) = self.section_index(y) {
            self.sections[si].set_block_light(lx & 15, y.rem_euclid(16) as usize, lz & 15, val);
        }
    }

    #[inline]
    pub fn biome_at(&self, lx: usize, lz: usize) -> &str {
        &self.biomes[(lz & 15) * 16 + (lx & 15)]
    }

    #[inline]
    pub fn set_biome_at(&mut self, lx: usize, lz: usize, biome: String) {
        self.biomes[(lz & 15) * 16 + (lx & 15)] = biome;
    }

    pub fn recompute_heightmap(&mut self) {
        let max_y = self.min_y + (self.sections.len() as i32) * 16 - 1;
        for z in 0..16 {
            for x in 0..16 {
                let mut h = self.min_y;
                for y in (self.min_y..=max_y).rev() {
                    if !self.get_block(x, y, z).is_air() {
                        h = y;
                        break;
                    }
                }
                self.heightmap[z * 16 + x] = h as i16;
            }
        }
    }

    /// Serialize chunk to Anvil-compatible NBT (`DataVersion = 4790`).
    pub fn to_nbt(&self, blocks: &BlockRegistry) -> NbtTag {
        let mut root = NbtCompound::new();
        root.insert("DataVersion".into(), NbtTag::Int(SharedConstants::WORLD_VERSION));
        root.insert("xPos".into(), NbtTag::Int(self.pos.x));
        root.insert("zPos".into(), NbtTag::Int(self.pos.z));
        root.insert("yPos".into(), NbtTag::Int(self.min_y / 16));
        root.insert("Status".into(), NbtTag::String("minecraft:full".into()));

        let mut sec_tags = Vec::new();
        for (idx, sec) in self.sections.iter().enumerate() {
            if sec.non_air_count == 0 {
                continue;
            }
            let sy = (self.min_y / 16) + idx as i32;
            let mut palette_map: HashMap<u32, i32> = HashMap::new();
            let mut palette_tags = Vec::new();
            let mut indices = Vec::with_capacity(4096);

            for &st in sec.blocks.iter() {
                let p_idx = if let Some(&pi) = palette_map.get(&st.0) {
                    pi
                } else {
                    let pi = palette_tags.len() as i32;
                    palette_map.insert(st.0, pi);
                    let mut entry = NbtCompound::new();
                    entry.insert(
                        "Name".into(),
                        NbtTag::String(format!("minecraft:{}", blocks.get(st.id()).name)),
                    );
                    if st.meta() != 0 {
                        entry.insert("Meta".into(), NbtTag::Int(st.meta() as i32));
                    }
                    palette_tags.push(NbtTag::Compound(entry));
                    pi
                };
                indices.push(p_idx);
            }

            let mut sec_c = NbtCompound::new();
            sec_c.insert("Y".into(), NbtTag::Byte(sy as i8));
            let mut block_states = NbtCompound::new();
            block_states.insert("palette".into(), NbtTag::List(palette_tags));
            block_states.insert("data".into(), NbtTag::IntArray(indices));
            sec_c.insert("block_states".into(), NbtTag::Compound(block_states));
            sec_c.insert(
                "SkyLight".into(),
                NbtTag::ByteArray(sec.sky_light.iter().map(|&b| b as i8).collect()),
            );
            sec_c.insert(
                "BlockLight".into(),
                NbtTag::ByteArray(sec.block_light.iter().map(|&b| b as i8).collect()),
            );
            sec_tags.push(NbtTag::Compound(sec_c));
        }

        root.insert("sections".into(), NbtTag::List(sec_tags));
        let biome_tags = self.biomes.iter().map(|b| NbtTag::String(b.clone())).collect();
        root.insert("biomes_2d".into(), NbtTag::List(biome_tags));
        NbtTag::Compound(root)
    }

    pub fn from_nbt(tag: &NbtTag, dimension: DimensionId, blocks: &BlockRegistry) -> Option<Self> {
        let root = tag.as_compound()?;
        let x = root.get("xPos")?.as_i32()?;
        let z = root.get("zPos")?.as_i32()?;
        let mut chunk = Self::new(ChunkPos::new(x, z), dimension);

        if let Some(NbtTag::List(secs)) = root.get("sections") {
            for s in secs {
                let sc = s.as_compound()?;
                let sy = sc.get("Y")?.as_i32()?;
                let rel = sy - (chunk.min_y / 16);
                if rel < 0 || (rel as usize) >= chunk.sections.len() {
                    continue;
                }
                let sec = &mut chunk.sections[rel as usize];
                if let Some(bs) = sc.get("block_states").and_then(|v| v.as_compound()) {
                    let mut palette = Vec::new();
                    if let Some(NbtTag::List(pal)) = bs.get("palette") {
                        for p in pal {
                            if let Some(pc) = p.as_compound() {
                                let name = pc.get("Name").and_then(|v| v.as_str()).unwrap_or("air");
                                let meta = pc.get("Meta").and_then(|v| v.as_i32()).unwrap_or(0) as u16;
                                let bid = blocks.by_name(name).unwrap_or(blocks.air);
                                palette.push(BlockState::with_meta(bid, meta));
                            }
                        }
                    }
                    if let Some(NbtTag::IntArray(data)) = bs.get("data") {
                        sec.non_air_count = 0;
                        for (i, &pi) in data.iter().take(4096).enumerate() {
                            let st = *palette.get(pi.max(0) as usize).unwrap_or(&BlockState::AIR);
                            sec.blocks[i] = st;
                            if !st.is_air() {
                                sec.non_air_count += 1;
                            }
                        }
                    }
                }
                if let Some(NbtTag::ByteArray(sl)) = sc.get("SkyLight") {
                    for (i, &b) in sl.iter().take(2048).enumerate() {
                        sec.sky_light[i] = b as u8;
                    }
                }
                if let Some(NbtTag::ByteArray(bl)) = sc.get("BlockLight") {
                    for (i, &b) in bl.iter().take(2048).enumerate() {
                        sec.block_light[i] = b as u8;
                    }
                }
            }
        }
        if let Some(NbtTag::List(b)) = root.get("biomes_2d") {
            for (i, v) in b.iter().take(256).enumerate() {
                if let Some(s) = v.as_str() {
                    chunk.biomes[i] = s.to_string();
                }
            }
        }
        chunk.recompute_heightmap();
        chunk.dirty_save = false;
        chunk.light_populated = true;
        Some(chunk)
    }
}
