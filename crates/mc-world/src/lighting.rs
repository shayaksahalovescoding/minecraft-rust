use crate::level::World;
use mc_core::{BlockPos, ChunkPos, Direction};
use mc_data::BlockRegistry;
use std::collections::VecDeque;

/// Compute initial sky and block lighting for a chunk using Minecraft's 0..15 BFS attenuation rules.
pub fn compute_chunk_lighting(world: &mut World, cpos: ChunkPos, blocks: &BlockRegistry) {
    let min_y = world.dimension.min_y();
    let max_y = world.dimension.max_y();
    let has_sky = world.dimension.has_skylight();

    let mut sky_q: VecDeque<(BlockPos, u8)> = VecDeque::new();
    let mut block_q: VecDeque<(BlockPos, u8)> = VecDeque::new();

    if let Some(chunk) = world.chunks.get_mut(&cpos) {
        let base_x = cpos.min_block_x();
        let base_z = cpos.min_block_z();

        for lz in 0..16usize {
            for lx in 0..16usize {
                let mut sky = if has_sky { 15u8 } else { 0u8 };
                for y in (min_y..=max_y).rev() {
                    let st = chunk.get_block(lx, y, lz);
                    let def = blocks.get(st.id());
                    if sky > 0 {
                        let op = def.light_opacity;
                        if op >= sky {
                            sky = 0;
                        } else if op > 0 {
                            sky = sky.saturating_sub(op);
                        }
                    }
                    chunk.set_sky_light(lx, y, lz, sky);
                    if sky > 1 && def.light_opacity == 0 {
                        sky_q.push_back((BlockPos::new(base_x + lx as i32, y, base_z + lz as i32), sky));
                    }

                    let mut emit = def.light_emission;
                    if (st.id() == blocks.furnace
                        || st.id() == blocks.blast_furnace
                        || st.id() == blocks.smoker
                        || st.id() == blocks.redstone_lamp)
                        && st.is_active()
                    {
                        emit = if st.id() == blocks.redstone_lamp { 15 } else { 13 };
                    }
                    chunk.set_block_light(lx, y, lz, emit);
                    if emit > 1 {
                        block_q.push_back((BlockPos::new(base_x + lx as i32, y, base_z + lz as i32), emit));
                    }
                }
            }
        }
        chunk.light_populated = true;
    }

    // Also seed from neighboring chunks' boundary emitters so cross-chunk light blends seamlessly
    let min_x = cpos.min_block_x();
    let min_z = cpos.min_block_z();
    for y in min_y..=max_y {
        for i in 0..16 {
            let border = [
                BlockPos::new(min_x - 1, y, min_z + i),
                BlockPos::new(min_x + 16, y, min_z + i),
                BlockPos::new(min_x + i, y, min_z - 1),
                BlockPos::new(min_x + i, y, min_z + 16),
            ];
            for bp in border {
                let bl = world.get_block_light(bp);
                if bl > 1 {
                    block_q.push_back((bp, bl));
                }
                if has_sky {
                    let sl = world.get_sky_light(bp);
                    if sl > 1 {
                        sky_q.push_back((bp, sl));
                    }
                }
            }
        }
    }

    flood_block_light(world, block_q, blocks);
    if has_sky {
        flood_sky_light(world, sky_q, blocks);
    }
}

pub fn update_light_at(world: &mut World, pos: BlockPos, blocks: &BlockRegistry) {
    // Local relight around pos (radius 15) for fast incremental updates when breaking/placing blocks
    let min_y = world.dimension.min_y();
    let max_y = world.dimension.max_y();
    let has_sky = world.dimension.has_skylight();

    // Update sky light column at (pos.x, pos.z)
    if has_sky {
        let top_sky = world.get_sky_light(BlockPos::new(pos.x, (pos.y + 1).min(max_y), pos.z));
        let mut sky = if pos.y >= max_y { 15 } else { top_sky };
        let mut sky_q = VecDeque::new();
        for y in (min_y..=pos.y.min(max_y)).rev() {
            let p = BlockPos::new(pos.x, y, pos.z);
            let op = blocks.get(world.get_block(p).id()).light_opacity;
            sky = sky.saturating_sub(op);
            world.set_sky_light(p, sky);
            if sky > 1 {
                sky_q.push_back((p, sky));
            }
        }
        // Seed surrounding clear neighbors
        for dir in Direction::ALL {
            let nb = pos.relative(dir);
            let s = world.get_sky_light(nb);
            if s > 1 {
                sky_q.push_back((nb, s));
            }
        }
        flood_sky_light(world, sky_q, blocks);
    }

    // Clear block light in a small 14-block neighborhood and re-flood from sources
    let mut sources = VecDeque::new();
    let rad = 8;
    for dy in -rad..=rad {
        let y = pos.y + dy;
        if y < min_y || y > max_y {
            continue;
        }
        for dz in -rad..=rad {
            for dx in -rad..=rad {
                let p = BlockPos::new(pos.x + dx, y, pos.z + dz);
                if dx.abs() + dy.abs() + dz.abs() <= rad {
                    let st = world.get_block(p);
                    let mut emit = blocks.get(st.id()).light_emission;
                    if (st.id() == blocks.furnace
                        || st.id() == blocks.blast_furnace
                        || st.id() == blocks.smoker
                        || st.id() == blocks.redstone_lamp)
                        && st.is_active()
                    {
                        emit = if st.id() == blocks.redstone_lamp { 15 } else { 13 };
                    }
                    world.set_block_light(p, emit);
                    if emit > 1 {
                        sources.push_back((p, emit));
                    }
                } else {
                    let bl = world.get_block_light(p);
                    if bl > 1 {
                        sources.push_back((p, bl));
                    }
                }
            }
        }
    }
    flood_block_light(world, sources, blocks);
}

fn flood_block_light(world: &mut World, mut q: VecDeque<(BlockPos, u8)>, blocks: &BlockRegistry) {
    let min_y = world.dimension.min_y();
    let max_y = world.dimension.max_y();
    while let Some((p, lvl)) = q.pop_front() {
        if lvl <= 1 {
            continue;
        }
        for dir in Direction::ALL {
            let nb = p.relative(dir);
            if nb.y < min_y || nb.y > max_y {
                continue;
            }
            if !world.chunks.contains_key(&nb.chunk_pos()) {
                continue;
            }
            let op = blocks.get(world.get_block(nb).id()).light_opacity.max(1);
            let next_lvl = lvl.saturating_sub(op);
            if next_lvl > world.get_block_light(nb) {
                world.set_block_light(nb, next_lvl);
                if next_lvl > 1 {
                    q.push_back((nb, next_lvl));
                }
            }
        }
    }
}

fn flood_sky_light(world: &mut World, mut q: VecDeque<(BlockPos, u8)>, blocks: &BlockRegistry) {
    let min_y = world.dimension.min_y();
    let max_y = world.dimension.max_y();
    while let Some((p, lvl)) = q.pop_front() {
        if lvl <= 1 {
            continue;
        }
        for dir in Direction::ALL {
            let nb = p.relative(dir);
            if nb.y < min_y || nb.y > max_y {
                continue;
            }
            if !world.chunks.contains_key(&nb.chunk_pos()) {
                continue;
            }
            let op = blocks.get(world.get_block(nb).id()).light_opacity;
            let cost = if dir == Direction::Down && lvl == 15 && op == 0 {
                0
            } else {
                op.max(1)
            };
            let next_lvl = lvl.saturating_sub(cost);
            if next_lvl > world.get_sky_light(nb) {
                world.set_sky_light(nb, next_lvl);
                if next_lvl > 1 {
                    q.push_back((nb, next_lvl));
                }
            }
        }
    }
}
