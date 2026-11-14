use crate::block_entity::{BlockEntityData, CookingKindDef};
use crate::chunk::Chunk;
use crate::fluids::tick_fluid;
use crate::lighting::{compute_chunk_lighting, update_light_at};
use crate::redstone::{tick_redstone_component, update_redstone_around};
use glam::DVec3;
use mc_core::{
    BlockPos, ChunkPos, Difficulty, DimensionId, Direction, GameRules, SharedConstants, AABB,
};
use mc_data::{BlockId, BlockRegistry, BlockState, GameData, ItemStack};
use rand::Rng;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScheduledTick {
    pub execute_tick: u64,
    pub pos: BlockPos,
    pub block: BlockId,
}

impl Ord for ScheduledTick {
    fn cmp(&self, other: &Self) -> Ordering {
        other.execute_tick.cmp(&self.execute_tick)
    }
}

impl PartialOrd for ScheduledTick {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct RaycastHit {
    pub pos: BlockPos,
    pub face: Direction,
    pub hit_point: DVec3,
    pub distance: f64,
    pub state: BlockState,
}

#[derive(Debug, Clone)]
pub struct SoundTrigger {
    pub event: String,
    pub pos: DVec3,
    pub volume: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone)]
pub struct PendingExplosion {
    pub center: DVec3,
    pub radius: f32,
    pub causes_fire: bool,
}

#[derive(Debug, Clone)]
pub struct FallingBlockSpawn {
    pub pos: BlockPos,
    pub state: BlockState,
}

pub struct World {
    pub dimension: DimensionId,
    pub seed: i64,
    pub world_tick: u64,
    pub day_time: i64,
    pub is_raining: bool,
    pub is_thundering: bool,
    pub weather_timer: i32,
    pub difficulty: Difficulty,
    pub game_rules: GameRules,
    pub spawn_point: BlockPos,
    pub chunks: HashMap<ChunkPos, Chunk>,
    tick_queue: BinaryHeap<ScheduledTick>,
    pub pending_drops: Vec<(DVec3, ItemStack)>,
    pub pending_sounds: Vec<SoundTrigger>,
    pub pending_tnt: Vec<BlockPos>,
    pub pending_falling: Vec<FallingBlockSpawn>,
    pub pending_explosions: Vec<PendingExplosion>,
}

impl World {
    pub fn new(dimension: DimensionId, seed: i64) -> Self {
        Self {
            dimension,
            seed,
            world_tick: 0,
            day_time: 1000, // Morning sun
            is_raining: false,
            is_thundering: false,
            weather_timer: 12000,
            difficulty: Difficulty::Normal,
            game_rules: GameRules::default(),
            spawn_point: BlockPos::new(0, 72, 0),
            chunks: HashMap::new(),
            tick_queue: BinaryHeap::new(),
            pending_drops: Vec::new(),
            pending_sounds: Vec::new(),
            pending_tnt: Vec::new(),
            pending_falling: Vec::new(),
            pending_explosions: Vec::new(),
        }
    }

    #[inline]
    pub fn get_block(&self, pos: BlockPos) -> BlockState {
        if pos.y < self.dimension.min_y() || pos.y > self.dimension.max_y() {
            return BlockState::AIR;
        }
        self.chunks
            .get(&pos.chunk_pos())
            .map(|c| c.get_block(pos.local_x(), pos.y, pos.local_z()))
            .unwrap_or(BlockState::AIR)
    }

    #[inline]
    pub fn get_sky_light(&self, pos: BlockPos) -> u8 {
        if pos.y > self.dimension.max_y() {
            return if self.dimension.has_skylight() { 15 } else { 0 };
        }
        if pos.y < self.dimension.min_y() {
            return 0;
        }
        self.chunks
            .get(&pos.chunk_pos())
            .map(|c| c.get_sky_light(pos.local_x(), pos.y, pos.local_z()))
            .unwrap_or(if self.dimension.has_skylight() { 15 } else { 0 })
    }

    #[inline]
    pub fn set_sky_light(&mut self, pos: BlockPos, val: u8) {
        if let Some(c) = self.chunks.get_mut(&pos.chunk_pos()) {
            c.set_sky_light(pos.local_x(), pos.y, pos.local_z(), val);
            c.dirty_mesh = true;
        }
    }

    #[inline]
    pub fn get_block_light(&self, pos: BlockPos) -> u8 {
        self.chunks
            .get(&pos.chunk_pos())
            .map(|c| c.get_block_light(pos.local_x(), pos.y, pos.local_z()))
            .unwrap_or(0)
    }

    #[inline]
    pub fn set_block_light(&mut self, pos: BlockPos, val: u8) {
        if let Some(c) = self.chunks.get_mut(&pos.chunk_pos()) {
            c.set_block_light(pos.local_x(), pos.y, pos.local_z(), val);
            c.dirty_mesh = true;
        }
    }

    pub fn insert_chunk(&mut self, mut chunk: Chunk, blocks: &BlockRegistry) {
        let pos = chunk.pos;
        chunk.recompute_heightmap();
        let needs_light = !chunk.light_populated;
        self.chunks.insert(pos, chunk);
        if needs_light {
            compute_chunk_lighting(self, pos, blocks);
        }
        // Mark immediate neighbor chunks dirty so boundary faces update cleanly
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(nc) = self.chunks.get_mut(&ChunkPos::new(pos.x + dx, pos.z + dz)) {
                nc.dirty_mesh = true;
            }
        }
    }

    pub fn set_block_internal(
        &mut self,
        pos: BlockPos,
        state: BlockState,
        blocks: &BlockRegistry,
    ) -> BlockState {
        if pos.y < self.dimension.min_y() || pos.y > self.dimension.max_y() {
            return BlockState::AIR;
        }
        let cp = pos.chunk_pos();
        let Some(chunk) = self.chunks.get_mut(&cp) else {
            return BlockState::AIR;
        };
        let prev = chunk.set_block(pos.local_x(), pos.y, pos.local_z(), state);
        if prev != state {
            if state.is_air() || prev.id() != state.id() {
                chunk.block_entities.remove(&pos);
            }
            // Mark adjacent chunks dirty when modifying chunk edge
            let lx = pos.local_x();
            let lz = pos.local_z();
            if lx == 0 {
                if let Some(n) = self.chunks.get_mut(&ChunkPos::new(cp.x - 1, cp.z)) {
                    n.dirty_mesh = true;
                }
            } else if lx == 15 {
                if let Some(n) = self.chunks.get_mut(&ChunkPos::new(cp.x + 1, cp.z)) {
                    n.dirty_mesh = true;
                }
            }
            if lz == 0 {
                if let Some(n) = self.chunks.get_mut(&ChunkPos::new(cp.x, cp.z - 1)) {
                    n.dirty_mesh = true;
                }
            } else if lz == 15 {
                if let Some(n) = self.chunks.get_mut(&ChunkPos::new(cp.x, cp.z + 1)) {
                    n.dirty_mesh = true;
                }
            }
            update_light_at(self, pos, blocks);
        }
        prev
    }

    /// Place or break a block with full neighbor updates, fluid scheduling, gravity blocks, and block entities.
    pub fn set_block(&mut self, pos: BlockPos, state: BlockState, blocks: &BlockRegistry) -> BlockState {
        let prev = self.set_block_internal(pos, state, blocks);
        let id = state.id();

        if id == blocks.chest {
            self.set_block_entity(pos, BlockEntityData::new_chest());
        } else if id == blocks.barrel {
            self.set_block_entity(pos, BlockEntityData::new_barrel());
        } else if id == blocks.furnace {
            self.set_block_entity(pos, BlockEntityData::new_furnace(CookingKindDef::Smelting));
        } else if id == blocks.blast_furnace {
            self.set_block_entity(pos, BlockEntityData::new_furnace(CookingKindDef::Blasting));
        } else if id == blocks.smoker {
            self.set_block_entity(pos, BlockEntityData::new_furnace(CookingKindDef::Smoking));
        } else if id == blocks.brewing_stand {
            self.set_block_entity(pos, BlockEntityData::new_brewing_stand());
        } else if id == blocks.hopper {
            self.set_block_entity(pos, BlockEntityData::new_hopper());
        } else if id == blocks.dispenser || id == blocks.dropper {
            self.set_block_entity(pos, BlockEntityData::new_dispenser());
        } else if id == blocks.enchanting_table {
            self.set_block_entity(pos, BlockEntityData::EnchantingTable);
        } else if id == blocks.anvil {
            self.set_block_entity(pos, BlockEntityData::Anvil);
        }

        // Schedule fluid & gravity & redstone updates on self and neighbors
        if blocks.get(id).is_fluid {
            let delay = if id == blocks.water {
                5
            } else if self.dimension.fast_lava() {
                10
            } else {
                30
            };
            self.schedule_tick(pos, id, delay);
        }
        self.check_gravity_block(pos, blocks);
        for d in Direction::ALL {
            let nb = pos.relative(d);
            let nst = self.get_block(nb);
            if blocks.get(nst.id()).is_fluid {
                let delay = if nst.id() == blocks.water { 5 } else { 30 };
                self.schedule_tick(nb, nst.id(), delay);
            }
            if nst.id() == blocks.observer && nst.facing() == d.opposite() {
                self.set_block_internal(nb, nst.with_active(true), blocks);
                self.schedule_tick(nb, blocks.observer, 2);
            }
            self.check_gravity_block(nb, blocks);
        }
        update_redstone_around(self, pos, blocks);
        prev
    }

    fn check_gravity_block(&mut self, pos: BlockPos, blocks: &BlockRegistry) {
        let st = self.get_block(pos);
        let id = st.id();
        if id == blocks.sand || id == blocks.red_sand || id == blocks.gravel || id == blocks.anvil {
            let below = self.get_block(pos.relative(Direction::Down));
            if below.is_air() || blocks.get(below.id()).is_fluid || blocks.get(below.id()).is_replaceable {
                self.pending_falling.push(FallingBlockSpawn { pos, state: st });
            }
        }
    }

    pub fn schedule_tick(&mut self, pos: BlockPos, block: BlockId, delay: u64) {
        self.tick_queue.push(ScheduledTick {
            execute_tick: self.world_tick + delay.max(1),
            pos,
            block,
        });
    }

    pub fn get_block_entity(&self, pos: BlockPos) -> Option<&BlockEntityData> {
        self.chunks
            .get(&pos.chunk_pos())
            .and_then(|c| c.block_entities.get(&pos))
    }

    pub fn get_block_entity_mut(&mut self, pos: BlockPos) -> Option<&mut BlockEntityData> {
        let cp = pos.chunk_pos();
        if let Some(c) = self.chunks.get_mut(&cp) {
            c.dirty_save = true;
            return c.block_entities.get_mut(&pos);
        }
        None
    }

    pub fn set_block_entity(&mut self, pos: BlockPos, data: BlockEntityData) {
        if let Some(c) = self.chunks.get_mut(&pos.chunk_pos()) {
            c.block_entities.insert(pos, data);
            c.dirty_save = true;
        }
    }

    pub fn ignite_tnt(&mut self, pos: BlockPos, blocks: &BlockRegistry) {
        if self.get_block(pos).id() == blocks.tnt {
            self.set_block_internal(pos, BlockState::AIR, blocks);
            self.pending_tnt.push(pos);
            self.pending_sounds.push(SoundTrigger {
                event: "entity.tnt.primed".to_string(),
                pos: pos.center(),
                volume: 1.0,
                pitch: 1.0,
            });
        }
    }

    pub fn trigger_dispenser(&mut self, pos: BlockPos, blocks: &BlockRegistry) {
        let st = self.get_block(pos);
        let dir = st.facing();
        let drop_pos = pos.relative(dir).center();
        if let Some(BlockEntityData::Dispenser { items }) = self.get_block_entity_mut(pos) {
            for slot in items.iter_mut() {
                if !slot.is_empty() {
                    let mut one = slot.clone();
                    one.count = 1;
                    slot.count -= 1;
                    if slot.count == 0 {
                        *slot = ItemStack::EMPTY;
                    }
                    self.pending_drops.push((drop_pos, one));
                    self.pending_sounds.push(SoundTrigger {
                        event: "block.dispenser.dispense".to_string(),
                        pos: pos.center(),
                        volume: 1.0,
                        pitch: 1.0,
                    });
                    return;
                }
            }
        }
        let _ = blocks;
    }

    /// Execute an explosion at `center` with `power`, breaking destructible blocks and dropping loot.
    pub fn explode(&mut self, center: DVec3, power: f32, causes_fire: bool, data: &GameData) {
        let mut rng = rand::thread_rng();
        let rays = 16;
        let mut to_destroy = Vec::new();
        for gx in 0..rays {
            for gy in 0..rays {
                for gz in 0..rays {
                    if gx != 0 && gx != rays - 1 && gy != 0 && gy != rays - 1 && gz != 0 && gz != rays - 1 {
                        continue;
                    }
                    let dir = DVec3::new(
                        gx as f64 / (rays - 1) as f64 * 2.0 - 1.0,
                        gy as f64 / (rays - 1) as f64 * 2.0 - 1.0,
                        gz as f64 / (rays - 1) as f64 * 2.0 - 1.0,
                    )
                    .normalize();
                    let mut intensity = power * (0.7 + rng.r#gen::<f32>() * 0.6);
                    let mut p = center;
                    while intensity > 0.0 {
                        let bp = BlockPos::from_vec3(p);
                        let st = self.get_block(bp);
                        if !st.is_air() {
                            let res = data.blocks.get(st.id()).blast_resistance;
                            intensity -= (res + 0.3) * 0.3;
                            if intensity > 0.0 {
                                to_destroy.push(bp);
                            }
                        }
                        p += dir * 0.3;
                        intensity -= 0.225;
                    }
                }
            }
        }

        to_destroy.sort();
        to_destroy.dedup();

        let mut ctx = mc_data::LootContext::default();
        ctx.explosion_survived = true;
        for bp in to_destroy {
            let st = self.get_block(bp);
            if st.is_air() {
                continue;
            }
            if st.id() == data.blocks.tnt {
                self.ignite_tnt(bp, &data.blocks);
                continue;
            }
            if self.game_rules.do_tile_drops && rng.r#gen::<f32>() < (1.0 / power).min(1.0) {
                let bname = &data.blocks.get(st.id()).name;
                for drop in data.loot.roll_block(bname, &ctx, &mut rng) {
                    self.pending_drops.push((bp.center(), drop));
                }
            }
            self.set_block_internal(bp, BlockState::AIR, &data.blocks);
            if causes_fire && rng.r#gen::<f32>() < 0.25 {
                let below = self.get_block(bp.relative(Direction::Down));
                if data.blocks.get(below.id()).is_solid {
                    self.set_block_internal(bp, BlockState::new(data.blocks.fire), &data.blocks);
                }
            }
        }
        self.pending_sounds.push(SoundTrigger {
            event: "entity.generic.explode".to_string(),
            pos: center,
            volume: 4.0,
            pitch: 0.8 + rng.r#gen::<f32>() * 0.2,
        });
    }

    /// Exact 3D DDA voxel raycast.
    pub fn raycast_block(
        &self,
        origin: DVec3,
        dir: DVec3,
        max_dist: f64,
        blocks: &BlockRegistry,
    ) -> Option<RaycastHit> {
        let d = dir.normalize_or_zero();
        if d.length_squared() < 1e-12 {
            return None;
        }

        let mut x = origin.x.floor() as i32;
        let mut y = origin.y.floor() as i32;
        let mut z = origin.z.floor() as i32;

        let step_x = if d.x > 0.0 { 1 } else { -1 };
        let step_y = if d.y > 0.0 { 1 } else { -1 };
        let step_z = if d.z > 0.0 { 1 } else { -1 };

        let t_delta_x = if d.x.abs() > 1e-12 { (1.0 / d.x).abs() } else { f64::INFINITY };
        let t_delta_y = if d.y.abs() > 1e-12 { (1.0 / d.y).abs() } else { f64::INFINITY };
        let t_delta_z = if d.z.abs() > 1e-12 { (1.0 / d.z).abs() } else { f64::INFINITY };

        let mut t_max_x = if d.x > 0.0 {
            ((x as f64 + 1.0) - origin.x) * t_delta_x
        } else {
            (origin.x - x as f64) * t_delta_x
        };
        let mut t_max_y = if d.y > 0.0 {
            ((y as f64 + 1.0) - origin.y) * t_delta_y
        } else {
            (origin.y - y as f64) * t_delta_y
        };
        let mut t_max_z = if d.z > 0.0 {
            ((z as f64 + 1.0) - origin.z) * t_delta_z
        } else {
            (origin.z - z as f64) * t_delta_z
        };

        let mut t = 0.0;
        let mut face = Direction::Up;

        while t <= max_dist {
            let bp = BlockPos::new(x, y, z);
            let st = self.get_block(bp);
            let def = blocks.get(st.id());
            if !st.is_air() && !def.is_fluid {
                return Some(RaycastHit {
                    pos: bp,
                    face,
                    hit_point: origin + d * t,
                    distance: t,
                    state: st,
                });
            }

            if t_max_x < t_max_y {
                if t_max_x < t_max_z {
                    t = t_max_x;
                    t_max_x += t_delta_x;
                    x += step_x;
                    face = if step_x > 0 { Direction::West } else { Direction::East };
                } else {
                    t = t_max_z;
                    t_max_z += t_delta_z;
                    z += step_z;
                    face = if step_z > 0 { Direction::North } else { Direction::South };
                }
            } else if t_max_y < t_max_z {
                t = t_max_y;
                t_max_y += t_delta_y;
                y += step_y;
                face = if step_y > 0 { Direction::Down } else { Direction::Up };
            } else {
                t = t_max_z;
                t_max_z += t_delta_z;
                z += step_z;
                face = if step_z > 0 { Direction::North } else { Direction::South };
            }
        }
        None
    }

    /// Collect all solid block AABBs intersecting `box_area`.
    pub fn collect_block_colliders(&self, box_area: AABB, blocks: &BlockRegistry) -> Vec<AABB> {
        let min_x = box_area.min.x.floor() as i32;
        let max_x = box_area.max.x.floor() as i32;
        let min_y = box_area.min.y.floor() as i32;
        let max_y = box_area.max.y.floor() as i32;
        let min_z = box_area.min.z.floor() as i32;
        let max_z = box_area.max.z.floor() as i32;

        let mut out = Vec::new();
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let st = self.get_block(BlockPos::new(x, y, z));
                    let def = blocks.get(st.id());
                    if def.is_solid {
                        let max_h = match def.shape {
                            mc_data::BlockShape::Slab if !st.is_top_half() => 0.5,
                            mc_data::BlockShape::Carpet => 0.0625,
                            mc_data::BlockShape::SnowLayer => ((st.level().max(1) as f64) * 0.125).min(1.0),
                            mc_data::BlockShape::Farmland => 0.9375,
                            _ => 1.0,
                        };
                        let min_h = match def.shape {
                            mc_data::BlockShape::Slab if st.is_top_half() => 0.5,
                            _ => 0.0,
                        };
                        out.push(AABB::new(
                            x as f64,
                            y as f64 + min_h,
                            z as f64,
                            (x + 1) as f64,
                            y as f64 + max_h,
                            (z + 1) as f64,
                        ));
                    }
                }
            }
        }
        out
    }

    /// Swept axis-separated AABB collision with automatic step-up (0.6 blocks).
    pub fn move_aabb(
        &self,
        aabb: AABB,
        delta: DVec3,
        step_height: f64,
        blocks: &BlockRegistry,
    ) -> (DVec3, bool, bool) {
        let expanded = aabb.expand_towards(delta.x, delta.y, delta.z).inflate(0.1, step_height, 0.1);
        let colliders = self.collect_block_colliders(expanded, blocks);

        let (actual_dx, actual_dy, actual_dz) = resolve_axes(aabb, delta, &colliders);

        let collided_horiz = (delta.x - actual_dx).abs() > 1e-6 || (delta.z - actual_dz).abs() > 1e-6;
        let on_ground = delta.y < 0.0 && (delta.y - actual_dy).abs() > 1e-6;

        // Step-up if blocked horizontally while grounded
        if collided_horiz && step_height > 0.0 && (on_ground || delta.y.abs() < 1e-6) {
            let mut stepped = aabb;
            let mut up_dy = step_height;
            for &c in &colliders {
                up_dy = stepped.clip_y_collide(c, up_dy);
            }
            stepped = stepped.move_by(0.0, up_dy, 0.0);
            let (sx, _, sz) = resolve_axes(stepped, DVec3::new(delta.x, 0.0, delta.z), &colliders);
            stepped = stepped.move_by(sx, 0.0, sz);
            let mut down_dy = -up_dy;
            for &c in &colliders {
                down_dy = stepped.clip_y_collide(c, down_dy);
            }
            if sx * sx + sz * sz > actual_dx * actual_dx + actual_dz * actual_dz {
                return (DVec3::new(sx, up_dy + down_dy, sz), true, true);
            }
        }

        (DVec3::new(actual_dx, actual_dy, actual_dz), on_ground, collided_horiz)
    }

    pub fn is_in_fluid(&self, aabb: AABB, fluid_id: BlockId) -> bool {
        let min_x = aabb.min.x.floor() as i32;
        let max_x = aabb.max.x.floor() as i32;
        let min_y = aabb.min.y.floor() as i32;
        let max_y = aabb.max.y.floor() as i32;
        let min_z = aabb.min.z.floor() as i32;
        let max_z = aabb.max.z.floor() as i32;
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let st = self.get_block(BlockPos::new(x, y, z));
                    if st.id() == fluid_id || (fluid_id.0 != 0 && st.is_waterlogged()) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Run one 50ms game tick: time, weather, scheduled ticks, block entities, random ticks.
    pub fn tick(&mut self, data: &GameData, loaded_center: ChunkPos) {
        self.world_tick += 1;
        if self.game_rules.do_daylight_cycle {
            self.day_time = (self.day_time + 1) % SharedConstants::DAY_TICKS;
        }
        if self.game_rules.do_weather_cycle {
            self.weather_timer -= 1;
            if self.weather_timer <= 0 {
                let mut rng = rand::thread_rng();
                if self.is_raining {
                    self.is_raining = false;
                    self.is_thundering = false;
                    self.weather_timer = rng.gen_range(12000..=36000);
                } else {
                    self.is_raining = true;
                    self.is_thundering = rng.r#gen::<f32>() < 0.35;
                    self.weather_timer = rng.gen_range(6000..=18000);
                }
            }
        }

        // Scheduled ticks (fluids, redstone repeaters, torches, pistons, observers)
        let mut due = Vec::new();
        while let Some(top) = self.tick_queue.peek() {
            if top.execute_tick <= self.world_tick {
                due.push(self.tick_queue.pop().unwrap());
            } else {
                break;
            }
        }
        for t in due {
            if data.blocks.get(t.block).is_fluid {
                tick_fluid(self, t.pos, t.block, &data.blocks);
            } else {
                tick_redstone_component(self, t.pos, t.block, &data.blocks);
            }
        }

        // Tick block entities (Furnaces, Brewing Stands, Hoppers)
        self.tick_block_entities(data);

        // Random ticks around loaded_center
        if self.game_rules.random_tick_speed > 0 {
            self.run_random_ticks(data, loaded_center);
        }
    }

    fn tick_block_entities(&mut self, data: &GameData) {
        let positions: Vec<(ChunkPos, BlockPos)> = self
            .chunks
            .iter()
            .flat_map(|(&cp, ch)| ch.block_entities.keys().map(move |&bp| (cp, bp)))
            .collect();

        for (cp, bp) in positions {
            let Some(chunk) = self.chunks.get_mut(&cp) else {
                continue;
            };
            let Some(be) = chunk.block_entities.get_mut(&bp) else {
                continue;
            };
            let mut lit_changed: Option<bool> = None;
            match be {
                BlockEntityData::Furnace {
                    kind,
                    input,
                    fuel,
                    output,
                    burn_time,
                    burn_duration,
                    cook_time,
                    cook_total,
                } => {
                    let was_lit = *burn_time > 0;
                    if *burn_time > 0 {
                        *burn_time -= 1;
                    }
                    let recipe = if !input.is_empty() {
                        data.recipes.match_cooking((*kind).into(), input.item)
                    } else {
                        None
                    };
                    let can_output = recipe.as_ref().is_some_and(|(res_id, _, _)| {
                        output.is_empty()
                            || (output.item == *res_id
                                && output.count < data.items.get(*res_id).max_stack)
                    });

                    if *burn_time == 0 && can_output && !fuel.is_empty() {
                        let ticks = data.items.get(fuel.item).fuel_ticks;
                        if ticks > 0 {
                            *burn_time = ticks;
                            *burn_duration = ticks;
                            fuel.count -= 1;
                            if fuel.count == 0 {
                                *fuel = ItemStack::EMPTY;
                            }
                        }
                    }

                    if *burn_time > 0 && can_output {
                        if let Some((res_id, _, _)) = recipe {
                            *cook_time += 1;
                            if *cook_time >= *cook_total {
                                *cook_time = 0;
                                input.count -= 1;
                                if input.count == 0 {
                                    *input = ItemStack::EMPTY;
                                }
                                if output.is_empty() {
                                    *output = ItemStack::new(res_id, 1);
                                } else {
                                    output.count += 1;
                                }
                            }
                        }
                    } else {
                        *cook_time = 0;
                    }

                    let now_lit = *burn_time > 0;
                    if was_lit != now_lit {
                        lit_changed = Some(now_lit);
                    }
                }
                BlockEntityData::BrewingStand {
                    bottles,
                    ingredient,
                    fuel,
                    brew_time,
                    fuel_charge,
                } => {
                    if *fuel_charge == 0 && !fuel.is_empty() && data.items.get(fuel.item).name == "blaze_powder" {
                        *fuel_charge = 20;
                        fuel.count -= 1;
                        if fuel.count == 0 {
                            *fuel = ItemStack::EMPTY;
                        }
                    }
                    if *fuel_charge > 0
                        && !ingredient.is_empty()
                        && bottles.iter().any(|b| !b.is_empty())
                    {
                        if *brew_time == 0 {
                            *brew_time = 400;
                        } else {
                            *brew_time -= 1;
                            if *brew_time == 0 {
                                *fuel_charge -= 1;
                                ingredient.count -= 1;
                                if ingredient.count == 0 {
                                    *ingredient = ItemStack::EMPTY;
                                }
                            }
                        }
                    } else {
                        *brew_time = 0;
                    }
                }
                _ => {}
            }

            if let Some(lit) = lit_changed {
                let st = self.get_block(bp);
                self.set_block_internal(bp, st.with_active(lit), &data.blocks);
            }
        }
    }

    fn run_random_ticks(&mut self, data: &GameData, center: ChunkPos) {
        let mut rng = rand::thread_rng();
        let speed = self.game_rules.random_tick_speed.min(12);
        let min_y = self.dimension.min_y();
        let max_y = self.dimension.max_y();

        for dz in -3..=3 {
            for dx in -3..=3 {
                let cp = ChunkPos::new(center.x + dx, center.z + dz);
                if !self.chunks.contains_key(&cp) {
                    continue;
                }
                for _ in 0..speed {
                    let lx = rng.gen_range(0..16);
                    let lz = rng.gen_range(0..16);
                    let y = rng.gen_range(min_y..=max_y);
                    let bp = BlockPos::new(cp.min_block_x() + lx, y, cp.min_block_z() + lz);
                    let st = self.get_block(bp);
                    if st.is_air() {
                        continue;
                    }
                    let id = st.id();
                    if id == data.blocks.wheat || id == data.blocks.carrots || id == data.blocks.potatoes {
                        let age = st.level();
                        if age < 7 {
                            self.set_block_internal(bp, st.with_level(age + 1), &data.blocks);
                        }
                    } else if id == data.blocks.grass_block {
                        let above = self.get_block(bp.relative(Direction::Up));
                        if data.blocks.get(above.id()).light_opacity >= 15 {
                            self.set_block_internal(bp, BlockState::new(data.blocks.dirt), &data.blocks);
                        }
                    } else if id == data.blocks.fire && self.game_rules.do_fire_tick {
                        if rng.r#gen::<f32>() < 0.25 {
                            self.set_block_internal(bp, BlockState::AIR, &data.blocks);
                        }
                    }
                }
            }
        }
    }
}

fn resolve_axes(aabb: AABB, delta: DVec3, colliders: &[AABB]) -> (f64, f64, f64) {
    let mut dy = delta.y;
    for &c in colliders {
        dy = aabb.clip_y_collide(c, dy);
    }
    let aabb_y = aabb.move_by(0.0, dy, 0.0);

    let mut dx = delta.x;
    for &c in colliders {
        dx = aabb_y.clip_x_collide(c, dx);
    }
    let aabb_yx = aabb_y.move_by(dx, 0.0, 0.0);

    let mut dz = delta.z;
    for &c in colliders {
        dz = aabb_yx.clip_z_collide(c, dz);
    }
    (dx, dy, dz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::DimensionId;

    #[test]
    fn world_raycast_and_water_flow_and_redstone() {
        let data = GameData::load();
        let mut world = World::new(DimensionId::Overworld, 12345);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0), DimensionId::Overworld);

        // Flat stone floor at y=63
        for z in 0..16 {
            for x in 0..16 {
                chunk.set_block(x, 63, z, BlockState::new(data.blocks.stone));
            }
        }
        world.insert_chunk(chunk, &data.blocks);

        // Raycast downward hits (5, 63, 5)
        let hit = world
            .raycast_block(DVec3::new(5.5, 68.0, 5.5), DVec3::NEG_Y, 10.0, &data.blocks)
            .expect("should hit floor");
        assert_eq!(hit.pos, BlockPos::new(5, 63, 5));
        assert_eq!(hit.face, Direction::Up);

        // Redstone lever powers redstone lamp through redstone wire
        world.set_block(
            BlockPos::new(2, 64, 2),
            BlockState::new(data.blocks.lever).with_active(true),
            &data.blocks,
        );
        world.set_block(
            BlockPos::new(3, 64, 2),
            BlockState::new(data.blocks.redstone_wire),
            &data.blocks,
        );
        world.set_block(
            BlockPos::new(4, 64, 2),
            BlockState::new(data.blocks.redstone_lamp),
            &data.blocks,
        );
        assert!(world.get_block(BlockPos::new(4, 64, 2)).is_active());
        assert_eq!(world.get_block(BlockPos::new(3, 64, 2)).level(), 15);

        // Water flows outward after 5 ticks
        world.set_block(
            BlockPos::new(10, 64, 10),
            BlockState::new(data.blocks.water).with_level(0),
            &data.blocks,
        );
        for _ in 0..6 {
            world.tick(&data, ChunkPos::new(0, 0));
        }
        assert_eq!(world.get_block(BlockPos::new(11, 64, 10)).id(), data.blocks.water);
    }
}
