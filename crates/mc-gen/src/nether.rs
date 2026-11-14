use crate::noise::{hash_3d, OctaveNoise};
use mc_core::{ChunkPos, DimensionId, JavaRandom};
use mc_data::{BlockState, GameData};
use mc_world::Chunk;

pub struct NetherGenerator {
    pub seed: i64,
    terrain: OctaveNoise,
    biome_noise: OctaveNoise,
    detail: OctaveNoise,
}

impl NetherGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            terrain: OctaveNoise::new(seed.wrapping_add(101), 4),
            biome_noise: OctaveNoise::new(seed.wrapping_add(102), 3),
            detail: OctaveNoise::new(seed.wrapping_add(103), 3),
        }
    }

    pub fn biome_at(&self, wx: i32, wz: i32) -> &'static str {
        let t = self.biome_noise.sample2d(wx as f64, wz as f64, 0.0022);
        let h = self.detail.sample2d(wx as f64, wz as f64, 0.0025);
        if t < -0.22 {
            "soul_sand_valley"
        } else if t < -0.05 {
            "crimson_forest"
        } else if t > 0.24 {
            "basalt_deltas"
        } else if h > 0.18 {
            "warped_forest"
        } else {
            "nether_wastes"
        }
    }

    pub fn generate_chunk(&self, cpos: ChunkPos, data: &GameData) -> Chunk {
        let blocks = &data.blocks;
        let mut chunk = Chunk::new(cpos, DimensionId::TheNether);
        let bx = cpos.min_block_x();
        let bz = cpos.min_block_z();
        let lava_sea = 31;

        for lz in 0..16usize {
            for lx in 0..16usize {
                let wx = bx + lx as i32;
                let wz = bz + lz as i32;
                let biome = self.biome_at(wx, wz);
                chunk.set_biome_at(lx, lz, biome.to_string());

                for y in 0..=127 {
                    if y <= 4 && (y == 0 || (hash_3d(wx, y, wz, self.seed) % 5) < (5 - y) as u64) {
                        chunk.set_block(lx, y, lz, BlockState::new(blocks.bedrock));
                        continue;
                    }
                    if y >= 123 && (y == 127 || (hash_3d(wx, y, wz, self.seed) % 5) < (y - 122) as u64) {
                        chunk.set_block(lx, y, lz, BlockState::new(blocks.bedrock));
                        continue;
                    }

                    let edge_falloff = if y < 14 {
                        (14 - y) as f64 * 0.06
                    } else if y > 112 {
                        (y - 112) as f64 * 0.06
                    } else {
                        0.0
                    };
                    let d = self.terrain.sample3d(wx as f64, y as f64, wz as f64, 0.025, 1.5) + edge_falloff;

                    if d > -0.06 {
                        let blk = match biome {
                            "soul_sand_valley" => {
                                if y < 40 && d < 0.08 { blocks.soul_sand } else { blocks.netherrack }
                            }
                            "basalt_deltas" => {
                                if d < 0.15 { blocks.basalt } else { blocks.blackstone }
                            }
                            _ => blocks.netherrack,
                        };
                        chunk.set_block(lx, y, lz, BlockState::new(blk));
                    } else if y <= lava_sea {
                        chunk.set_block(lx, y, lz, BlockState::new(blocks.lava));
                    }
                }

                // Biome top surface + glowstone
                for y in 32..118 {
                    let cur = chunk.get_block(lx, y, lz).id();
                    let above = chunk.get_block(lx, y + 1, lz).id();
                    if cur == blocks.netherrack && above == blocks.air {
                        match biome {
                            "crimson_forest" => {
                                chunk.set_block(lx, y, lz, BlockState::new(blocks.crimson_nylium));
                            }
                            "warped_forest" => {
                                chunk.set_block(lx, y, lz, BlockState::new(blocks.warped_nylium));
                            }
                            "soul_sand_valley" => {
                                chunk.set_block(lx, y, lz, BlockState::new(blocks.soul_soil));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Nether Quartz, Nether Gold, Ancient Debris, Glowstone
        let mut rng = JavaRandom::new(self.seed ^ (cpos.x as i64 * 734287) ^ (cpos.z as i64 * 912931));
        for _ in 0..16 {
            let x = rng.next_int_bounded(16) as usize;
            let y = 10 + rng.next_int_bounded(105);
            let z = rng.next_int_bounded(16) as usize;
            if chunk.get_block(x, y, z).id() == blocks.netherrack {
                chunk.set_block(x, y, z, BlockState::new(blocks.nether_quartz_ore));
            }
        }
        for _ in 0..8 {
            let x = rng.next_int_bounded(16) as usize;
            let y = 10 + rng.next_int_bounded(105);
            let z = rng.next_int_bounded(16) as usize;
            if chunk.get_block(x, y, z).id() == blocks.netherrack {
                chunk.set_block(x, y, z, BlockState::new(blocks.nether_gold_ore));
            }
        }
        for _ in 0..3 {
            let x = rng.next_int_bounded(16) as usize;
            let y = 8 + rng.next_int_bounded(16);
            let z = rng.next_int_bounded(16) as usize;
            if chunk.get_block(x, y, z).id() == blocks.netherrack {
                chunk.set_block(x, y, z, BlockState::new(blocks.ancient_debris));
            }
        }
        for _ in 0..6 {
            let x = rng.next_int_bounded(16) as usize;
            let y = 50 + rng.next_int_bounded(65);
            let z = rng.next_int_bounded(16) as usize;
            if chunk.get_block(x, y, z).is_air() && !chunk.get_block(x, y + 1, z).is_air() {
                chunk.set_block(x, y, z, BlockState::new(blocks.glowstone));
            }
        }

        chunk.recompute_heightmap();
        chunk
    }
}
