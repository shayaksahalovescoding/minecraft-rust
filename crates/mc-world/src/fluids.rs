use crate::level::World;
use mc_core::{BlockPos, Direction};
use mc_data::{BlockId, BlockRegistry, BlockState};

/// Water & Lava fluid simulation matching reverse-engineered `WaterFluid` and `LavaFluid` rules:
/// - Water: slope_find_distance = 4, drop_off = 1 (max level 7 -> 0 is source), tick_delay = 5
/// - Lava: Overworld slope=2, drop_off=2, delay=30; Nether (fast_lava) slope=4, drop_off=1, delay=10
pub fn tick_fluid(world: &mut World, pos: BlockPos, fluid_id: BlockId, blocks: &BlockRegistry) {
    let is_water = fluid_id == blocks.water;
    let is_lava = fluid_id == blocks.lava;
    if !is_water && !is_lava {
        return;
    }

    let cur = world.get_block(pos);
    if cur.id() != fluid_id {
        return;
    }

    let level = cur.level(); // 0 = source, 1..7 = flowing
    let drop_off = if is_water || world.dimension.fast_lava() {
        1u8
    } else {
        2u8
    };
    let delay = if is_water {
        5u64
    } else if world.dimension.fast_lava() {
        10
    } else {
        30
    };

    // Non-source blocks check if they are still supported by a source or higher fluid
    if level > 0 {
        let above = world.get_block(pos.relative(Direction::Up));
        let mut best_neighbor = 8u8;
        let mut adj_sources = 0u8;
        for dir in Direction::HORIZONTAL {
            let n = world.get_block(pos.relative(dir));
            if n.id() == fluid_id {
                let nl = n.level();
                if nl == 0 {
                    adj_sources += 1;
                }
                if nl < best_neighbor {
                    best_neighbor = nl;
                }
            }
        }

        // Infinite water source creation: 2+ adjacent water sources over solid or water
        let below = world.get_block(pos.relative(Direction::Down));
        let below_solid_or_source =
            blocks.get(below.id()).is_solid || (below.id() == blocks.water && below.level() == 0);
        let new_level = if is_water && adj_sources >= 2 && below_solid_or_source {
            0
        } else if above.id() == fluid_id {
            1 // falling fluid stream
        } else if best_neighbor < 7 {
            best_neighbor.saturating_add(drop_off).min(8)
        } else {
            8 // dry up
        };

        if new_level >= 8 {
            world.set_block_internal(pos, BlockState::AIR, blocks);
            for d in Direction::ALL {
                let nb = pos.relative(d);
                let nst = world.get_block(nb);
                if nst.id() == fluid_id {
                    world.schedule_tick(nb, fluid_id, delay);
                }
            }
            return;
        } else if new_level != level {
            world.set_block_internal(pos, BlockState::new(fluid_id).with_level(new_level), blocks);
            world.schedule_tick(pos, fluid_id, delay);
        }
    }

    // Spread downward first; only spread horizontally if blocked below or source
    let below_pos = pos.relative(Direction::Down);
    if below_pos.y >= world.dimension.min_y() {
        let below = world.get_block(below_pos);
        if can_fluid_replace(below, fluid_id, blocks) {
            if is_lava && below.id() == blocks.water {
                world.set_block_internal(below_pos, BlockState::new(blocks.stone), blocks);
                return;
            }
            world.set_block_internal(below_pos, BlockState::new(fluid_id).with_level(1), blocks);
            world.schedule_tick(below_pos, fluid_id, delay);
            if level > 0 {
                return;
            }
        } else if below.id() == fluid_id && below.level() != 0 {
            return;
        }
    }

    // Spread horizontally
    let cur_lvl = world.get_block(pos).level();
    let next_lvl = if cur_lvl == 0 {
        drop_off
    } else {
        cur_lvl.saturating_add(drop_off)
    };
    if next_lvl <= 7 {
        for dir in Direction::HORIZONTAL {
            let nb_pos = pos.relative(dir);
            let nb = world.get_block(nb_pos);
            if is_water && nb.id() == blocks.lava {
                let result = if nb.level() == 0 {
                    blocks.obsidian
                } else {
                    blocks.cobblestone
                };
                world.set_block_internal(nb_pos, BlockState::new(result), blocks);
                continue;
            }
            if is_lava && nb.id() == blocks.water {
                world.set_block_internal(nb_pos, BlockState::new(blocks.cobblestone), blocks);
                continue;
            }
            if can_fluid_replace(nb, fluid_id, blocks)
                || (nb.id() == fluid_id && nb.level() > next_lvl)
            {
                world.set_block_internal(
                    nb_pos,
                    BlockState::new(fluid_id).with_level(next_lvl),
                    blocks,
                );
                world.schedule_tick(nb_pos, fluid_id, delay);
            }
        }
    }
}

fn can_fluid_replace(st: BlockState, _fluid: BlockId, blocks: &BlockRegistry) -> bool {
    if st.is_air() {
        return true;
    }
    let def = blocks.get(st.id());
    !def.is_solid && !def.is_fluid && def.is_replaceable
}
