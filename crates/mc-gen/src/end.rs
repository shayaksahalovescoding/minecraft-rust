use crate::noise::OctaveNoise;
use mc_core::{ChunkPos, DimensionId};
use mc_data::{BlockState, GameData};
use mc_world::Chunk;

pub struct EndGenerator {
    pub seed: i64,
    island_noise: OctaveNoise,
}

impl EndGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            island_noise: OctaveNoise::new(seed.wrapping_add(202), 4),
        }
    }

    pub fn generate_chunk(&self, cpos: ChunkPos, data: &GameData) -> Chunk {
        let blocks = &data.blocks;
        let mut chunk = Chunk::new(cpos, DimensionId::TheEnd);
        let bx = cpos.min_block_x();
        let bz = cpos.min_block_z();

        for lz in 0..16usize {
            for lx in 0..16usize {
                let wx = bx + lx as i32;
                let wz = bz + lz as i32;
                let dist = ((wx * wx + wz * wz) as f64).sqrt();

                let biome = if dist < 120.0 { "the_end" } else { "end_highlands" };
                chunk.set_biome_at(lx, lz, biome.to_string());

                // Main End Island or Outer End Islands
                let h = if dist < 96.0 {
                    let falloff = (dist / 96.0).powi(2);
                    let n = self.island_noise.sample2d(wx as f64, wz as f64, 0.02) * 8.0;
                    Some((62.0 - falloff * 20.0 + n).round() as i32)
                } else if dist > 180.0 {
                    let n = self.island_noise.sample2d(wx as f64, wz as f64, 0.014);
                    if n > 0.14 {
                        Some((56.0 + (n - 0.14) * 35.0).round() as i32)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(top_y) = h {
                    let bottom_y = (top_y - 16 - ((top_y - 40).max(0) / 2)).max(10);
                    for y in bottom_y..=top_y {
                        chunk.set_block(lx, y, lz, BlockState::new(blocks.end_stone));
                    }
                }
            }
        }

        // 10 Obsidian Pillars around main End Island (radius 42)
        for i in 0..10 {
            let angle = (i as f64) * std::f64::consts::TAU / 10.0;
            let px = (angle.cos() * 42.0).round() as i32;
            let pz = (angle.sin() * 42.0).round() as i32;
            let pillar_h = 76 + (i as i32 % 4) * 8;
            for dz in -2..=2 {
                for dx in -2..=2 {
                    if dx * dx + dz * dz > 5 {
                        continue;
                    }
                    let wx = px + dx;
                    let wz = pz + dz;
                    if wx >= bx && wx < bx + 16 && wz >= bz && wz < bz + 16 {
                        let lx = (wx - bx) as usize;
                        let lz = (wz - bz) as usize;
                        for y in 45..=pillar_h {
                            chunk.set_block(lx, y, lz, BlockState::new(blocks.obsidian));
                        }
                        if dx == 0 && dz == 0 {
                            chunk.set_block(lx, pillar_h + 1, lz, BlockState::new(blocks.bedrock));
                        }
                    }
                }
            }
        }

        // Exit Portal frame at (0, 64, 0)
        if cpos.x == 0 && cpos.z == 0 {
            for dz in -2..=2i32 {
                for dx in -2..=2i32 {
                    if dx.abs() == 2 && dz.abs() == 2 {
                        continue;
                    }
                    let lx = (8 + dx) as usize;
                    let lz = (8 + dz) as usize;
                    chunk.set_block(lx, 63, lz, BlockState::new(blocks.bedrock));
                }
            }
            for y in 64..=67 {
                chunk.set_block(8, y, 8, BlockState::new(blocks.bedrock));
            }
            chunk.set_block(8, 66, 7, BlockState::new(blocks.torch));
            chunk.set_block(8, 66, 9, BlockState::new(blocks.torch));
        }

        // Obsidian spawn platform at (100, 48, 0)
        for wx in 98..=102 {
            for wz in -2..=2 {
                if wx >= bx && wx < bx + 16 && wz >= bz && wz < bz + 16 {
                    let lx = (wx - bx) as usize;
                    let lz = (wz - bz) as usize;
                    chunk.set_block(lx, 48, lz, BlockState::new(blocks.obsidian));
                    for y in 49..=51 {
                        chunk.set_block(lx, y, lz, BlockState::AIR);
                    }
                }
            }
        }

        chunk.recompute_heightmap();
        chunk
    }
}
