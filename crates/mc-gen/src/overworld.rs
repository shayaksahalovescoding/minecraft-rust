use crate::noise::{hash_3d, OctaveNoise};
use mc_core::{BlockPos, ChunkPos, DimensionId, JavaRandom, SharedConstants};
use mc_data::{BlockId, BlockRegistry, BlockState, GameData, ItemStack};
use mc_world::{BlockEntityData, Chunk};

pub struct OverworldGenerator {
    pub seed: i64,
    continentalness: OctaveNoise,
    erosion: OctaveNoise,
    weirdness: OctaveNoise,
    temperature: OctaveNoise,
    humidity: OctaveNoise,
    terrain_3d: OctaveNoise,
    cave_cheese: OctaveNoise,
    cave_spaghetti_a: OctaveNoise,
    cave_spaghetti_b: OctaveNoise,
    detail: OctaveNoise,
}

impl OverworldGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            continentalness: OctaveNoise::new(seed.wrapping_add(1), 5),
            erosion: OctaveNoise::new(seed.wrapping_add(2), 4),
            weirdness: OctaveNoise::new(seed.wrapping_add(3), 4),
            temperature: OctaveNoise::new(seed.wrapping_add(4), 4),
            humidity: OctaveNoise::new(seed.wrapping_add(5), 4),
            terrain_3d: OctaveNoise::new(seed.wrapping_add(6), 4),
            cave_cheese: OctaveNoise::new(seed.wrapping_add(7), 3),
            cave_spaghetti_a: OctaveNoise::new(seed.wrapping_add(8), 3),
            cave_spaghetti_b: OctaveNoise::new(seed.wrapping_add(9), 3),
            detail: OctaveNoise::new(seed.wrapping_add(10), 3),
        }
    }

    pub fn biome_at(&self, wx: i32, wz: i32) -> &'static str {
        let x = wx as f64;
        let z = wz as f64;
        let c = self.continentalness.sample2d(x, z, 0.0012);
        let e = self.erosion.sample2d(x, z, 0.0018);
        let w = self.weirdness.sample2d(x, z, 0.003);
        let t = self.temperature.sample2d(x, z, 0.0009);
        let h = self.humidity.sample2d(x, z, 0.0009);

        if w.abs() < 0.032 && c > -0.18 {
            return if t < -0.25 { "frozen_river" } else { "river" };
        }
        if c < -0.25 {
            return if t > 0.2 { "warm_ocean" } else { "deep_ocean" };
        }
        if c < -0.08 {
            return if t < -0.25 { "frozen_ocean" } else { "ocean" };
        }
        if c < -0.02 {
            return if t < -0.2 { "snowy_beach" } else { "beach" };
        }

        if e < -0.22 && c > 0.15 {
            if t < -0.15 {
                return "jagged_peaks";
            }
            if w > 0.1 {
                return "cherry_grove";
            }
            return if t > 0.1 { "stony_peaks" } else { "meadow" };
        }

        if t < -0.28 {
            return if h > 0.1 { "snowy_taiga" } else { "snowy_plains" };
        }
        if t < -0.05 {
            return if h > 0.05 { "taiga" } else { "birch_forest" };
        }
        if t > 0.28 {
            return if h < -0.1 {
                "desert"
            } else if h < 0.1 {
                if e < 0.0 { "badlands" } else { "savanna" }
            } else {
                "jungle"
            };
        }
        if h > 0.22 && e > 0.15 {
            return if w > 0.0 { "mangrove_swamp" } else { "swamp" };
        }
        if h > 0.15 {
            return if w < -0.1 { "dark_forest" } else { "forest" };
        }
        "plains"
    }

    pub fn base_surface_height(&self, wx: i32, wz: i32) -> i32 {
        let x = wx as f64;
        let z = wz as f64;
        let c = self.continentalness.sample2d(x, z, 0.0012);
        let e = self.erosion.sample2d(x, z, 0.0018);
        let w = self.weirdness.sample2d(x, z, 0.003);
        let ridge = 1.0 - (3.0 * w.abs() - 1.0).abs().min(1.0);
        let river_dip = if w.abs() < 0.038 && c > -0.18 {
            -9.0 * (1.0 - w.abs() / 0.038)
        } else {
            0.0
        };

        let continent_h = if c < -0.08 {
            42.0 + c * 48.0
        } else {
            64.0 + c * 32.0 + (-e).max(0.0) * 38.0 + ridge * 24.0 * (-e + 0.3).max(0.0)
        };
        let detail_h = self.detail.sample2d(x, z, 0.018) * 6.0;
        (continent_h + detail_h + river_dip).round() as i32
    }

    pub fn generate_chunk(&self, cpos: ChunkPos, data: &GameData) -> Chunk {
        let blocks = &data.blocks;
        let mut chunk = Chunk::new(cpos, DimensionId::Overworld);
        let min_y = SharedConstants::OVERWORLD_MIN_Y;
        let sea = SharedConstants::SEA_LEVEL;
        let bx = cpos.min_block_x();
        let bz = cpos.min_block_z();

        let mut surface_ys = [64i32; 256];
        let mut col_biomes = ["plains"; 256];

        // Pass 1: Terrain density + caves + aquifers + bedrock + deepslate
        for lz in 0..16usize {
            for lx in 0..16usize {
                let wx = bx + lx as i32;
                let wz = bz + lz as i32;
                let biome = self.biome_at(wx, wz);
                col_biomes[lz * 16 + lx] = biome;
                chunk.set_biome_at(lx, lz, biome.to_string());

                let base_h = self.base_surface_height(wx, wz);
                let mut top_solid = min_y;

                for y in min_y..=SharedConstants::OVERWORLD_MAX_Y {
                    if y <= min_y + 4 {
                        let prob = (min_y + 5 - y) as u64;
                        if y == min_y || (hash_3d(wx, y, wz, self.seed) % 5) < prob {
                            chunk.set_block(lx, y, lz, BlockState::new(blocks.bedrock));
                            continue;
                        }
                    }

                    // 3D terrain overhang density near surface
                    let density = if y <= base_h - 14 {
                        1.0
                    } else if y > base_h + 24 {
                        -1.0
                    } else {
                        let base_d = (base_h - y) as f64 * 0.09;
                        let n3 = self.terrain_3d.sample3d(wx as f64, y as f64, wz as f64, 0.022, 1.4);
                        base_d + n3 * 0.85
                    };

                    if density > 0.0 {
                        // 3D Cheese & Spaghetti Cave Carvers
                        let mut carved = false;
                        if y < base_h - 2 && y > min_y + 3 {
                            let cheese = self.cave_cheese.sample3d(
                                wx as f64,
                                y as f64,
                                wz as f64,
                                0.032,
                                1.6,
                            );
                            let sa = self.cave_spaghetti_a.sample3d(
                                wx as f64,
                                y as f64,
                                wz as f64,
                                0.025,
                                1.2,
                            );
                            let sb = self.cave_spaghetti_b.sample3d(
                                wx as f64,
                                y as f64,
                                wz as f64,
                                0.025,
                                1.2,
                            );
                            if cheese > 0.42 || (sa.abs() < 0.045 && sb.abs() < 0.045) {
                                carved = true;
                            }
                        }

                        if carved {
                            if y <= -54 {
                                chunk.set_block(lx, y, lz, BlockState::new(blocks.lava));
                            }
                        } else {
                            let stone_type = if y < 0
                                || (y <= 8 && (hash_3d(wx, y, wz, self.seed + 77) % 9) > y as u64)
                            {
                                blocks.deepslate
                            } else {
                                blocks.stone
                            };
                            chunk.set_block(lx, y, lz, BlockState::new(stone_type));
                            top_solid = y;
                        }
                    } else if y <= sea {
                        let water_or_ice = if y == sea
                            && matches!(biome, "snowy_plains" | "snowy_taiga" | "frozen_river" | "frozen_ocean" | "ice_spikes")
                        {
                            blocks.ice
                        } else {
                            blocks.water
                        };
                        chunk.set_block(lx, y, lz, BlockState::new(water_or_ice));
                    }
                }
                surface_ys[lz * 16 + lx] = top_solid;
            }
        }

        // Pass 2: Surface Rules (grass, dirt, sand, terracotta, snow)
        for lz in 0..16usize {
            for lx in 0..16usize {
                let top = surface_ys[lz * 16 + lx];
                if top <= min_y + 5 {
                    continue;
                }
                let biome = col_biomes[lz * 16 + lx];
                let underwater = top < sea;

                for depth in 0..4 {
                    let y = top - depth;
                    if y < min_y + 5 || chunk.get_block(lx, y, lz).is_air() {
                        continue;
                    }
                    let blk = match biome {
                        "desert" | "beach" => {
                            if depth < 3 {
                                blocks.sand
                            } else {
                                blocks.sandstone
                            }
                        }
                        "badlands" => {
                            if depth == 0 {
                                blocks.red_sand
                            } else {
                                blocks.terracotta
                            }
                        }
                        "stony_peaks" | "jagged_peaks" => {
                            if biome == "jagged_peaks" && top > 110 && depth == 0 {
                                blocks.snow_block
                            } else {
                                blocks.stone
                            }
                        }
                        "swamp" | "mangrove_swamp" => {
                            if underwater {
                                blocks.mud
                            } else if depth == 0 {
                                blocks.grass_block
                            } else {
                                blocks.dirt
                            }
                        }
                        _ => {
                            if underwater {
                                if depth < 2 { blocks.sand } else { blocks.gravel }
                            } else if depth == 0 {
                                blocks.grass_block
                            } else {
                                blocks.dirt
                            }
                        }
                    };
                    chunk.set_block(lx, y, lz, BlockState::new(blk));
                }

                if !underwater
                    && matches!(biome, "snowy_plains" | "snowy_taiga" | "snowy_beach" | "ice_spikes")
                    && top + 1 <= SharedConstants::OVERWORLD_MAX_Y
                {
                    chunk.set_block(lx, top + 1, lz, BlockState::new(blocks.snow).with_level(1));
                }
            }
        }

        // Pass 3: Ores (Coal, Copper, Iron, Gold, Redstone, Lapis, Diamond, Emerald)
        let mut rng = JavaRandom::new(
            self.seed
                ^ ((cpos.x as i64).wrapping_mul(341873128712))
                ^ ((cpos.z as i64).wrapping_mul(132897987541)),
        );
        self.place_ore_veins(&mut chunk, &mut rng, blocks, blocks.coal_ore, blocks.deepslate_coal_ore, 18, 12, 0, 192);
        self.place_ore_veins(&mut chunk, &mut rng, blocks, blocks.copper_ore, blocks.deepslate_copper_ore, 14, 9, -16, 112);
        self.place_ore_veins(&mut chunk, &mut rng, blocks, blocks.iron_ore, blocks.deepslate_iron_ore, 16, 8, -64, 160);
        self.place_ore_veins(&mut chunk, &mut rng, blocks, blocks.gold_ore, blocks.deepslate_gold_ore, 5, 7, -64, 32);
        self.place_ore_veins(&mut chunk, &mut rng, blocks, blocks.redstone_ore, blocks.deepslate_redstone_ore, 8, 7, -64, 16);
        self.place_ore_veins(&mut chunk, &mut rng, blocks, blocks.lapis_ore, blocks.deepslate_lapis_ore, 3, 6, -64, 64);
        self.place_ore_veins(&mut chunk, &mut rng, blocks, blocks.diamond_ore, blocks.deepslate_diamond_ore, 6, 6, -64, 16);

        // Pass 4: Trees & Surface Flora
        let center_biome = col_biomes[8 * 16 + 8];
        let tree_attempts = match center_biome {
            "forest" | "birch_forest" | "dark_forest" | "jungle" => 7,
            "taiga" | "snowy_taiga" | "cherry_grove" | "swamp" | "mangrove_swamp" => 4,
            "savanna" | "meadow" => 1,
            "plains" => if rng.next_int_bounded(5) == 0 { 1 } else { 0 },
            _ => 0,
        };
        for _ in 0..tree_attempts {
            let lx = 2 + rng.next_int_bounded(12) as usize;
            let lz = 2 + rng.next_int_bounded(12) as usize;
            let y = surface_ys[lz * 16 + lx];
            if y >= sea && chunk.get_block(lx, y, lz).id() == blocks.grass_block {
                self.place_tree(&mut chunk, lx, y + 1, lz, col_biomes[lz * 16 + lx], &mut rng, blocks);
            }
        }

        for lz in 0..16usize {
            for lx in 0..16usize {
                let y = surface_ys[lz * 16 + lx];
                if y < sea || y >= SharedConstants::OVERWORLD_MAX_Y - 3 {
                    continue;
                }
                let surf = chunk.get_block(lx, y, lz).id();
                if !chunk.get_block(lx, y + 1, lz).is_air() {
                    continue;
                }
                let b = col_biomes[lz * 16 + lx];
                if surf == blocks.grass_block {
                    let r = rng.next_int_bounded(100);
                    if r < 14 {
                        chunk.set_block(lx, y + 1, lz, BlockState::new(blocks.short_grass));
                    } else if r == 14 {
                        let flower = if rng.next_bool() { blocks.dandelion } else { blocks.poppy };
                        chunk.set_block(lx, y + 1, lz, BlockState::new(flower));
                    }
                } else if surf == blocks.sand && b == "desert" && rng.next_int_bounded(140) == 0 {
                    let h = 1 + rng.next_int_bounded(3);
                    for dy in 1..=h {
                        chunk.set_block(lx, y + dy, lz, BlockState::new(blocks.cactus));
                    }
                }
            }
        }

        // Pass 5: Deterministic Structures (Villages, Ruined Portals, Dungeons, Stronghold with End Portal)
        self.try_place_structures(cpos, &mut chunk, &surface_ys, &mut rng, data);

        chunk.recompute_heightmap();
        chunk
    }

    fn place_ore_veins(
        &self,
        chunk: &mut Chunk,
        rng: &mut JavaRandom,
        blocks: &BlockRegistry,
        stone_ore: BlockId,
        deep_ore: BlockId,
        attempts: usize,
        size: usize,
        min_y: i32,
        max_y: i32,
    ) {
        if max_y <= min_y {
            return;
        }
        for _ in 0..attempts {
            let mut x = rng.next_int_bounded(16);
            let mut y = min_y + rng.next_int_bounded(max_y - min_y);
            let mut z = rng.next_int_bounded(16);
            for _ in 0..size {
                if (0..16).contains(&x) && (0..16).contains(&z) {
                    let cur = chunk.get_block(x as usize, y, z as usize).id();
                    if cur == blocks.stone {
                        chunk.set_block(x as usize, y, z as usize, BlockState::new(stone_ore));
                    } else if cur == blocks.deepslate {
                        chunk.set_block(x as usize, y, z as usize, BlockState::new(deep_ore));
                    }
                }
                x += rng.next_int_bounded(3) - 1;
                y += rng.next_int_bounded(3) - 1;
                z += rng.next_int_bounded(3) - 1;
            }
        }
    }

    fn place_tree(
        &self,
        chunk: &mut Chunk,
        lx: usize,
        base_y: i32,
        lz: usize,
        biome: &str,
        rng: &mut JavaRandom,
        blocks: &BlockRegistry,
    ) {
        let (log, leaves, height) = match biome {
            "birch_forest" => (blocks.birch_log, blocks.birch_leaves, 5 + rng.next_int_bounded(3)),
            "taiga" | "snowy_taiga" => (blocks.spruce_log, blocks.spruce_leaves, 6 + rng.next_int_bounded(3)),
            "jungle" => (blocks.jungle_log, blocks.jungle_leaves, 7 + rng.next_int_bounded(4)),
            "savanna" => (blocks.acacia_log, blocks.acacia_leaves, 5 + rng.next_int_bounded(2)),
            "dark_forest" => (blocks.dark_oak_log, blocks.dark_oak_leaves, 5 + rng.next_int_bounded(2)),
            "cherry_grove" => (blocks.cherry_log, blocks.cherry_leaves, 5 + rng.next_int_bounded(2)),
            "mangrove_swamp" => (blocks.mangrove_log, blocks.mangrove_leaves, 5 + rng.next_int_bounded(2)),
            _ => (blocks.oak_log, blocks.oak_leaves, 4 + rng.next_int_bounded(3)),
        };

        for dy in 0..height {
            chunk.set_block(lx, base_y + dy, lz, BlockState::new(log));
        }

        let top = base_y + height;
        for dy in -2..=1 {
            let rad: i32 = if dy >= 0 { 1 } else { 2 };
            for dz in -rad..=rad {
                for dx in -rad..=rad {
                    let nx = lx as i32 + dx;
                    let nz = lz as i32 + dz;
                    if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
                        continue;
                    }
                    if dx.abs() == rad && dz.abs() == rad && (dy == 1 || rng.next_bool()) {
                        continue;
                    }
                    if chunk.get_block(nx as usize, top + dy, nz as usize).is_air() {
                        chunk.set_block(nx as usize, top + dy, nz as usize, BlockState::new(leaves));
                    }
                }
            }
        }
    }

    fn try_place_structures(
        &self,
        cpos: ChunkPos,
        chunk: &mut Chunk,
        surface_ys: &[i32; 256],
        rng: &mut JavaRandom,
        data: &GameData,
    ) {
        let blocks = &data.blocks;
        let sea = SharedConstants::SEA_LEVEL;
        let center_y = surface_ys[8 * 16 + 8];

        // Stronghold End Portal Room at deterministic chunk (12, 12) and (-16, 10)
        if (cpos.x == 12 && cpos.z == 12) || (cpos.x == -16 && cpos.z == 10) {
            let py = 24;
            for x in 2..14 {
                for z in 2..14 {
                    for y in py - 1..=py + 5 {
                        let wall = x == 2 || x == 13 || z == 2 || z == 13 || y == py - 1 || y == py + 5;
                        let st = if wall {
                            BlockState::new(blocks.stone_bricks)
                        } else {
                            BlockState::AIR
                        };
                        chunk.set_block(x, y, z, st);
                    }
                }
            }
            // End Portal 3x3 ring of End Portal Frames (12 frames)
            for i in 0..3 {
                let frame = BlockState::new(blocks.end_portal_frame);
                chunk.set_block(6 + i, py, 5, frame);
                chunk.set_block(6 + i, py, 9, frame);
                chunk.set_block(5, py, 6 + i, frame);
                chunk.set_block(9, py, 6 + i, frame);
            }
            chunk.set_block(7, py + 1, 7, BlockState::new(blocks.spawner));
            chunk.block_entities.insert(
                BlockPos::new(cpos.min_block_x() + 7, py + 1, cpos.min_block_z() + 7),
                BlockEntityData::Spawner {
                    entity_type: "silverfish".to_string(),
                    delay: 100,
                },
            );
            return;
        }

        // Village house on grid (every 10 chunks)
        if cpos.x.rem_euclid(10) == 2 && cpos.z.rem_euclid(10) == 2 && center_y >= sea {
            let fy = center_y;
            for x in 4..12 {
                for z in 4..12 {
                    chunk.set_block(x, fy, z, BlockState::new(blocks.cobblestone));
                    for y in fy + 1..=fy + 4 {
                        let wall = x == 4 || x == 11 || z == 4 || z == 11;
                        let roof = y == fy + 4;
                        if roof {
                            chunk.set_block(x, y, z, BlockState::new(blocks.oak_planks));
                        } else if wall {
                            let door = z == 4 && x == 7 && y <= fy + 2;
                            let win = (x == 4 || x == 11) && z == 7 && y == fy + 2;
                            let st = if door {
                                BlockState::AIR
                            } else if win {
                                BlockState::new(blocks.glass)
                            } else {
                                BlockState::new(blocks.oak_planks)
                            };
                            chunk.set_block(x, y, z, st);
                        } else {
                            chunk.set_block(x, y, z, BlockState::AIR);
                        }
                    }
                }
            }
            chunk.set_block(5, fy + 1, 10, BlockState::new(blocks.crafting_table));
            chunk.set_block(6, fy + 1, 10, BlockState::new(blocks.furnace));
            chunk.block_entities.insert(
                BlockPos::new(cpos.min_block_x() + 6, fy + 1, cpos.min_block_z() + 10),
                BlockEntityData::new_furnace(mc_world::block_entity::CookingKindDef::Smelting),
            );
            chunk.set_block(10, fy + 1, 10, BlockState::new(blocks.chest));
            let mut chest = BlockEntityData::new_chest();
            if let BlockEntityData::Chest { ref mut items } = chest {
                if let Some(bread) = data.items.by_name("bread") {
                    items[0] = ItemStack::new(bread, 5);
                }
                if let Some(emerald) = data.items.by_name("emerald") {
                    items[1] = ItemStack::new(emerald, 3);
                }
                if let Some(iron) = data.items.by_name("iron_ingot") {
                    items[2] = ItemStack::new(iron, 4);
                }
            }
            chunk.block_entities.insert(
                BlockPos::new(cpos.min_block_x() + 10, fy + 1, cpos.min_block_z() + 10),
                chest,
            );
            chunk.set_block(8, fy + 3, 5, BlockState::new(blocks.torch));
        }

        // Ruined Portal occasionally
        if cpos.x.rem_euclid(14) == 7 && cpos.z.rem_euclid(14) == 5 && center_y >= sea {
            let fy = center_y + 1;
            for x in 6..10 {
                chunk.set_block(x, fy, 8, BlockState::new(blocks.obsidian));
                chunk.set_block(x, fy + 4, 8, BlockState::new(blocks.crying_obsidian));
            }
            for y in fy..=fy + 4 {
                chunk.set_block(6, y, 8, BlockState::new(blocks.obsidian));
                if y < fy + 4 {
                    chunk.set_block(9, y, 8, BlockState::new(blocks.obsidian));
                }
            }
            chunk.set_block(7, fy, 7, BlockState::new(blocks.chest));
            let mut chest = BlockEntityData::new_chest();
            if let BlockEntityData::Chest { ref mut items } = chest {
                if let Some(obs) = data.items.by_name("obsidian") {
                    items[0] = ItemStack::new(obs, 3);
                }
                if let Some(flint) = data.items.by_name("flint_and_steel") {
                    items[1] = ItemStack::new(flint, 1);
                }
                if let Some(gapple) = data.items.by_name("golden_apple") {
                    items[2] = ItemStack::new(gapple, 1);
                }
            }
            chunk.block_entities.insert(
                BlockPos::new(cpos.min_block_x() + 7, fy, cpos.min_block_z() + 7),
                chest,
            );
        }

        // Underground Monster Dungeon
        if rng.next_int_bounded(40) == 0 {
            let dy = 10 + rng.next_int_bounded(30);
            for x in 5..11 {
                for z in 5..11 {
                    for y in dy..=dy + 4 {
                        let wall = x == 5 || x == 10 || z == 5 || z == 10 || y == dy;
                        if wall {
                            let b = if rng.next_bool() { blocks.mossy_cobblestone } else { blocks.cobblestone };
                            chunk.set_block(x, y, z, BlockState::new(b));
                        } else {
                            chunk.set_block(x, y, z, BlockState::AIR);
                        }
                    }
                }
            }
            chunk.set_block(8, dy + 1, 8, BlockState::new(blocks.spawner));
            chunk.block_entities.insert(
                BlockPos::new(cpos.min_block_x() + 8, dy + 1, cpos.min_block_z() + 8),
                BlockEntityData::Spawner {
                    entity_type: if rng.next_bool() { "zombie".into() } else { "skeleton".into() },
                    delay: 100,
                },
            );
            chunk.set_block(6, dy + 1, 6, BlockState::new(blocks.chest));
            chunk.block_entities.insert(
                BlockPos::new(cpos.min_block_x() + 6, dy + 1, cpos.min_block_z() + 6),
                BlockEntityData::new_chest(),
            );
        }
    }
}
