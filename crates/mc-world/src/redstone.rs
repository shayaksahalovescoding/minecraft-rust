use crate::level::World;
use mc_core::{BlockPos, Direction, SharedConstants};
use mc_data::{BlockRegistry, BlockState};
use std::collections::HashSet;

/// Compute strong + weak redstone signal emitted by `pos` toward `dir` (0..15).
pub fn get_signal_to(world: &World, pos: BlockPos, dir: Direction, blocks: &BlockRegistry) -> u8 {
    let st = world.get_block(pos);
    let id = st.id();

    if id == blocks.redstone_block {
        return 15;
    }
    if id == blocks.redstone_torch {
        return if st.is_active() { 15 } else { 0 };
    }
    if id == blocks.lever || id == blocks.oak_button || id == blocks.stone_pressure_plate {
        return if st.is_active() { 15 } else { 0 };
    }
    if id == blocks.redstone_wire {
        return st.level();
    }
    if id == blocks.repeater {
        if st.is_active() && st.facing() == dir.opposite() {
            return 15;
        }
        return 0;
    }
    if id == blocks.comparator {
        if st.facing() == dir.opposite() {
            return st.level();
        }
        return 0;
    }
    if id == blocks.observer {
        if st.is_active() && st.facing() == dir.opposite() {
            return 15;
        }
        return 0;
    }

    // Solid opaque blocks conduct strong power
    if blocks.get(id).is_opaque {
        let mut best = 0u8;
        for d in Direction::ALL {
            let nb = pos.relative(d);
            let nst = world.get_block(nb);
            if nst.id() == blocks.repeater && nst.is_active() && nst.facing() == d {
                return 15;
            }
            if nst.id() == blocks.redstone_torch && d == Direction::Down && nst.is_active() {
                return 15;
            }
            if nst.id() == blocks.lever && nst.is_active() {
                best = best.max(15);
            }
        }
        return best;
    }

    0
}

pub fn has_neighbor_signal(world: &World, pos: BlockPos, blocks: &BlockRegistry) -> u8 {
    let mut best = 0u8;
    for d in Direction::ALL {
        let nb = pos.relative(d);
        best = best.max(get_signal_to(world, nb, d, blocks));
    }
    best
}

/// Propagate redstone updates around `origin`.
pub fn update_redstone_around(world: &mut World, origin: BlockPos, blocks: &BlockRegistry) {
    let mut visited = HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(origin);
    for d in Direction::ALL {
        queue.push_back(origin.relative(d));
    }

    while let Some(pos) = queue.pop_front() {
        if !visited.insert(pos) || visited.len() > 2048 {
            continue;
        }
        let st = world.get_block(pos);
        let id = st.id();

        if id == blocks.redstone_wire {
            let mut max_in = 0u8;
            for d in Direction::ALL {
                let nb = pos.relative(d);
                let nst = world.get_block(nb);
                if nst.id() == blocks.redstone_wire {
                    max_in = max_in.max(nst.level().saturating_sub(1));
                } else {
                    max_in = max_in.max(get_signal_to(world, nb, d, blocks));
                }
                // Step up/down wire connections
                if d != Direction::Up && d != Direction::Down {
                    let up = nb.relative(Direction::Up);
                    let ust = world.get_block(up);
                    if ust.id() == blocks.redstone_wire
                        && !blocks.get(world.get_block(pos.relative(Direction::Up)).id()).is_opaque
                    {
                        max_in = max_in.max(ust.level().saturating_sub(1));
                    }
                    let dn = nb.relative(Direction::Down);
                    let dst = world.get_block(dn);
                    if dst.id() == blocks.redstone_wire && !blocks.get(nst.id()).is_opaque {
                        max_in = max_in.max(dst.level().saturating_sub(1));
                    }
                }
            }
            if max_in != st.level() {
                world.set_block_internal(pos, st.with_level(max_in), blocks);
                for d in Direction::ALL {
                    let nb = pos.relative(d);
                    queue.push_back(nb);
                    queue.push_back(nb.relative(Direction::Up));
                    queue.push_back(nb.relative(Direction::Down));
                }
            }
        } else if id == blocks.redstone_lamp {
            let powered = has_neighbor_signal(world, pos, blocks) > 0;
            if powered != st.is_active() {
                world.set_block_internal(pos, st.with_active(powered), blocks);
            }
        } else if id == blocks.redstone_torch {
            let below = world.get_block(pos.relative(Direction::Down));
            let powered_base = if blocks.get(below.id()).is_opaque {
                has_neighbor_signal(world, pos.relative(Direction::Down), blocks) > 0
            } else {
                false
            };
            let should_lit = !powered_base;
            if should_lit != st.is_active() {
                world.schedule_tick(pos, blocks.redstone_torch, 2);
            }
        } else if id == blocks.repeater {
            let back_pos = pos.relative(st.facing().opposite());
            let back_sig = get_signal_to(world, back_pos, st.facing().opposite(), blocks);
            let should_power = back_sig > 0;
            if should_power != st.is_active() {
                let delay_ticks = ((st.level() & 0x3) + 1) as u64 * 2;
                world.schedule_tick(pos, blocks.repeater, delay_ticks);
            }
        } else if id == blocks.comparator {
            world.schedule_tick(pos, blocks.comparator, 2);
        } else if id == blocks.piston || id == blocks.sticky_piston {
            let powered = has_neighbor_signal(world, pos, blocks) > 0;
            if powered != st.is_active() {
                world.schedule_tick(pos, id, 1);
            }
        } else if id == blocks.tnt {
            if has_neighbor_signal(world, pos, blocks) > 0 {
                world.ignite_tnt(pos, blocks);
            }
        } else if id == blocks.dispenser || id == blocks.dropper {
            let powered = has_neighbor_signal(world, pos, blocks) > 0;
            if powered && !st.is_active() {
                world.set_block_internal(pos, st.with_active(true), blocks);
                world.trigger_dispenser(pos, blocks);
            } else if !powered && st.is_active() {
                world.set_block_internal(pos, st.with_active(false), blocks);
            }
        }
    }
}

pub fn tick_redstone_component(
    world: &mut World,
    pos: BlockPos,
    bid: mc_data::BlockId,
    blocks: &BlockRegistry,
) {
    let st = world.get_block(pos);
    if st.id() != bid {
        return;
    }
    if bid == blocks.redstone_torch {
        let below = pos.relative(Direction::Down);
        let powered = blocks.get(world.get_block(below).id()).is_opaque
            && has_neighbor_signal(world, below, blocks) > 0;
        let should_lit = !powered;
        if should_lit != st.is_active() {
            world.set_block_internal(pos, st.with_active(should_lit), blocks);
            update_redstone_around(world, pos, blocks);
        }
    } else if bid == blocks.repeater {
        let back = pos.relative(st.facing().opposite());
        let sig = get_signal_to(world, back, st.facing().opposite(), blocks);
        let should = sig > 0;
        if should != st.is_active() {
            world.set_block_internal(pos, st.with_active(should), blocks);
            update_redstone_around(world, pos.relative(st.facing()), blocks);
        }
    } else if bid == blocks.comparator {
        let back_pos = pos.relative(st.facing().opposite());
        let mut rear = get_signal_to(world, back_pos, st.facing().opposite(), blocks);
        if let Some(be) = world.get_block_entity(back_pos) {
            rear = rear.max(be.container_fullness_signal());
        }
        let left = get_signal_to(
            world,
            pos.relative(st.facing().counter_clock_wise()),
            st.facing().counter_clock_wise(),
            blocks,
        );
        let right = get_signal_to(
            world,
            pos.relative(st.facing().clock_wise()),
            st.facing().clock_wise(),
            blocks,
        );
        let side = left.max(right);
        let out = if st.is_active() {
            rear.saturating_sub(side)
        } else if rear >= side {
            rear
        } else {
            0
        };
        if out != st.level() {
            world.set_block_internal(pos, st.with_level(out), blocks);
            update_redstone_around(world, pos.relative(st.facing()), blocks);
        }
    } else if bid == blocks.observer {
        if st.is_active() {
            world.set_block_internal(pos, st.with_active(false), blocks);
            update_redstone_around(world, pos.relative(st.facing()), blocks);
        }
    } else if bid == blocks.piston || bid == blocks.sticky_piston {
        let powered = has_neighbor_signal(world, pos, blocks) > 0;
        let dir = st.facing();
        if powered && !st.is_active() {
            if try_push_piston(world, pos, dir, blocks) {
                world.set_block_internal(pos, st.with_active(true), blocks);
            }
        } else if !powered && st.is_active() {
            world.set_block_internal(pos, st.with_active(false), blocks);
            let head_pos = pos.relative(dir);
            if world.get_block(head_pos).id() == blocks.piston_head {
                world.set_block_internal(head_pos, BlockState::AIR, blocks);
            }
            if bid == blocks.sticky_piston {
                let pull_pos = pos.relative_n(dir, 2);
                let pull_st = world.get_block(pull_pos);
                if !pull_st.is_air() && blocks.get(pull_st.id()).hardness >= 0.0 {
                    world.set_block_internal(pull_pos, BlockState::AIR, blocks);
                    world.set_block_internal(head_pos, pull_st, blocks);
                }
            }
        }
    }
}

/// Resolve and execute a piston push up to `SharedConstants::PISTON_MAX_PUSH_DEPTH` (12 blocks).
fn try_push_piston(
    world: &mut World,
    piston_pos: BlockPos,
    dir: Direction,
    blocks: &BlockRegistry,
) -> bool {
    let mut chain = Vec::new();
    let mut cur = piston_pos.relative(dir);
    loop {
        let st = world.get_block(cur);
        if st.is_air() || blocks.get(st.id()).is_replaceable {
            break;
        }
        let def = blocks.get(st.id());
        if def.hardness < 0.0 || st.id() == blocks.obsidian || world.get_block_entity(cur).is_some() {
            return false;
        }
        chain.push((cur, st));
        if chain.len() > SharedConstants::PISTON_MAX_PUSH_DEPTH {
            return false;
        }
        cur = cur.relative(dir);
    }

    for (p, st) in chain.into_iter().rev() {
        world.set_block_internal(p.relative(dir), st, blocks);
        world.set_block_internal(p, BlockState::AIR, blocks);
    }
    world.set_block_internal(
        piston_pos.relative(dir),
        BlockState::new(blocks.piston_head).with_facing(dir),
        blocks,
    );
    true
}
