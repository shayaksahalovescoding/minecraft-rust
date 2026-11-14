use crate::save;
use crate::ui::{self, HudContext, Screen, TitleSubpage, UiAction};
use crossbeam_channel::{unbounded, Receiver, Sender};
use glam::DVec3;
use mc_audio::AudioEngine;
use mc_core::{BlockPos, ChunkPos, DimensionId, Direction, GameMode, SharedConstants};
use mc_data::{BlockState, GameData, ItemStack, LootContext};
use mc_entities::{execute_command, EntityKind, EntityManager, ParticleKind, Player, PlayerInput};
use mc_gen::WorldGenerator;
use mc_proto::{ping_server, LanServer};
use mc_render::{mesh_chunk, ChunkMeshResult, ChunkNeighborhood, FrameRenderInput, Renderer, TextureAtlas, UiBatch};
use mc_world::{BlockEntityData, Chunk, World};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

#[derive(Default)]
struct KeyState {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    space: bool,
    shift: bool,
    ctrl: bool,
    left_mouse: bool,
    last_space_down: Option<Instant>,
}

pub struct ActiveSession {
    pub world_name: String,
    pub world_dir: PathBuf,
    pub worlds: HashMap<DimensionId, World>,
    pub current_dim: DimensionId,
    pub player: Player,
    pub entities_by_dim: HashMap<DimensionId, EntityManager>,
    pub generator: Arc<WorldGenerator>,
    pub pending_chunks: HashSet<(DimensionId, ChunkPos)>,
    pub chunk_tx: Sender<(DimensionId, Chunk)>,
    pub chunk_rx: Receiver<(DimensionId, Chunk)>,
    pub lan_server: Option<LanServer>,
    pub autosave_ticks: u32,
}

impl ActiveSession {
    pub fn world(&self) -> &World {
        self.worlds.get(&self.current_dim).expect("active dim world")
    }

    pub fn entities(&self) -> &EntityManager {
        self.entities_by_dim.get(&self.current_dim).expect("active dim entities")
    }
}

pub struct McApp {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    audio: AudioEngine,
    data: Arc<GameData>,
    ui_batch: UiBatch,
    screen: Screen,
    session: Option<ActiveSession>,

    keys: KeyState,
    mouse_x: f32,
    mouse_y: f32,
    cursor_locked: bool,

    render_distance: i32,
    fov_deg: f32,
    show_f3: bool,
    third_person: bool,

    break_target: Option<BlockPos>,
    break_ticks: u32,
    swing_timer: f32,

    chat_log: Vec<(String, f32)>,
    toast: Option<(String, f32)>,
    prev_adv_count: usize,

    last_frame: Instant,
    accumulator: f64,
    start_time: Instant,
    fps_counter: u32,
    fps_value: u32,
    fps_timer: f32,
}

impl McApp {
    pub fn new() -> Self {
        let data = GameData::load();
        let audio = AudioEngine::new();
        let worlds = save::list_worlds(&save::default_saves_root());
        Self {
            window: None,
            renderer: None,
            audio,
            data,
            ui_batch: UiBatch::default(),
            screen: Screen::Title {
                worlds,
                selected: 0,
                subpage: TitleSubpage::Main,
                new_name: "New World".to_string(),
                new_seed: "2612026".to_string(),
                new_mode: GameMode::Survival,
                mp_addr: "127.0.0.1:25565".to_string(),
                mp_status: "Ready".to_string(),
            },
            session: None,
            keys: KeyState::default(),
            mouse_x: 0.0,
            mouse_y: 0.0,
            cursor_locked: false,
            render_distance: 6,
            fov_deg: 75.0,
            show_f3: false,
            third_person: false,
            break_target: None,
            break_ticks: 0,
            swing_timer: 0.0,
            chat_log: vec![("Minecraft 26.1.2 Clean-Room Rust Edition loaded! Press T for Chat/Commands, E for Inventory, F3 Debug, F5 Camera.".into(), 10.0)],
            toast: None,
            prev_adv_count: 0,
            last_frame: Instant::now(),
            accumulator: 0.0,
            start_time: Instant::now(),
            fps_counter: 0,
            fps_value: 60,
            fps_timer: 0.0,
        }
    }

    fn set_cursor_grab(&mut self, grab: bool) {
        self.cursor_locked = grab;
        if let Some(win) = &self.window {
            let _ = if grab {
                win.set_cursor_grab(CursorGrabMode::Confined)
                    .or_else(|_| win.set_cursor_grab(CursorGrabMode::Locked))
            } else {
                win.set_cursor_grab(CursorGrabMode::None)
            };
            win.set_cursor_visible(!grab);
        }
    }

    fn start_new_world(&mut self, name: String, seed: i64, mode: GameMode) {
        let folder = save::sanitize_world_folder(&name);
        let world_dir = save::default_saves_root().join(&folder);
        let world_gen = Arc::new(WorldGenerator::new(seed));

        let mut world = World::new(DimensionId::Overworld, seed);
        // Synchronously generate 3x3 spawn chunks so the player starts on solid ground immediately!
        for dz in -1..=1 {
            for dx in -1..=1 {
                let cpos = ChunkPos::new(dx, dz);
                let chunk = world_gen.generate_chunk(DimensionId::Overworld, cpos, &self.data);
                world.insert_chunk(chunk, &self.data.blocks);
            }
        }
        let spawn_y = find_safe_spawn_y(&world, 8, 8);
        let spawn = BlockPos::new(8, spawn_y, 8);
        world.spawn_point = spawn;

        let mut player = Player::new("Steve", spawn, mode);
        if mode == GameMode::Survival {
            // Starter pack for smooth onboarding (still full survival!)
            if let Some(id) = self.data.items.by_name("wooden_pickaxe") {
                player.give_item(ItemStack::new(id, 1), &self.data);
            }
            if let Some(id) = self.data.items.by_name("torch") {
                player.give_item(ItemStack::new(id, 8), &self.data);
            }
            if let Some(id) = self.data.items.by_name("bread") {
                player.give_item(ItemStack::new(id, 6), &self.data);
            }
        }
        player.advancements.insert("minecraft:story/root".into());
        self.prev_adv_count = player.advancements.len();

        let mut entities = EntityManager::new();
        entities.spawn(EntityKind::Pig, DVec3::new(13.5, spawn_y as f64 + 1.0, 10.5), &self.data);
        entities.spawn(EntityKind::Cow, DVec3::new(5.5, spawn_y as f64 + 1.0, 14.5), &self.data);
        entities.spawn(EntityKind::Villager, DVec3::new(10.5, spawn_y as f64 + 1.0, 12.5), &self.data);

        let mut worlds = HashMap::new();
        worlds.insert(DimensionId::Overworld, world);
        worlds.insert(DimensionId::TheNether, World::new(DimensionId::TheNether, seed));
        worlds.insert(DimensionId::TheEnd, World::new(DimensionId::TheEnd, seed));

        let mut entities_by_dim = HashMap::new();
        entities_by_dim.insert(DimensionId::Overworld, entities);
        entities_by_dim.insert(DimensionId::TheNether, EntityManager::new());
        entities_by_dim.insert(DimensionId::TheEnd, EntityManager::new());

        let (chunk_tx, chunk_rx) = unbounded();
        if let Some(r) = &mut self.renderer {
            r.clear_chunks();
        }
        self.session = Some(ActiveSession {
            world_name: name,
            world_dir,
            worlds,
            current_dim: DimensionId::Overworld,
            player,
            entities_by_dim,
            generator: world_gen,
            pending_chunks: HashSet::new(),
            chunk_tx,
            chunk_rx,
            lan_server: None,
            autosave_ticks: 0,
        });
        self.screen = Screen::InGame;
        self.set_cursor_grab(true);
    }

    fn load_saved_world(&mut self, summary: save::WorldSummary) {
        let Ok((meta, maybe_player)) = save::load_level_dat(&summary.dir.join("level.dat")) else {
            return;
        };
        let world_gen = Arc::new(WorldGenerator::new(meta.seed));
        let player = maybe_player.unwrap_or_else(|| Player::new("Steve", meta.spawn_point, meta.game_mode));
        let dim = player.dimension;

        let mut worlds = HashMap::new();
        for d in [DimensionId::Overworld, DimensionId::TheNether, DimensionId::TheEnd] {
            let mut w = World::new(d, meta.seed);
            w.day_time = meta.day_time;
            w.world_tick = meta.world_tick;
            w.difficulty = meta.difficulty;
            w.spawn_point = meta.spawn_point;
            worlds.insert(d, w);
        }
        // Preload immediate area around player
        let pchunk = BlockPos::from_vec3(player.pos).chunk_pos();
        if let Some(w) = worlds.get_mut(&dim) {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let cp = ChunkPos::new(pchunk.x + dx, pchunk.z + dz);
                    let chunk = save::load_chunk_from_disk(&summary.dir, dim, cp, &self.data)
                        .unwrap_or_else(|| world_gen.generate_chunk(dim, cp, &self.data));
                    w.insert_chunk(chunk, &self.data.blocks);
                }
            }
        }

        let mut entities_by_dim = HashMap::new();
        for d in [DimensionId::Overworld, DimensionId::TheNether, DimensionId::TheEnd] {
            let mut em = EntityManager::new();
            em.entities = save::load_entities(&summary.dir, d);
            entities_by_dim.insert(d, em);
        }

        self.prev_adv_count = player.advancements.len();
        let (chunk_tx, chunk_rx) = unbounded();
        if let Some(r) = &mut self.renderer {
            r.clear_chunks();
        }
        self.session = Some(ActiveSession {
            world_name: meta.name,
            world_dir: summary.dir,
            worlds,
            current_dim: dim,
            player,
            entities_by_dim,
            generator: world_gen,
            pending_chunks: HashSet::new(),
            chunk_tx,
            chunk_rx,
            lan_server: None,
            autosave_ticks: 0,
        });
        self.screen = Screen::InGame;
        self.set_cursor_grab(true);
    }

    fn save_current_world(&mut self) {
        let Some(sess) = &mut self.session else {
            return;
        };
        let dim = sess.current_dim;
        if let Some(w) = sess.worlds.get_mut(&dim) {
            let _ = save::save_dirty_chunks(&sess.world_dir, w, &self.data);
            let _ = save::save_level_dat(&sess.world_dir, w, &sess.world_name, &sess.player);
        }
        if let Some(em) = sess.entities_by_dim.get(&dim) {
            let _ = save::save_entities(&sess.world_dir, dim, &em.entities);
        }
    }

    fn save_and_quit(&mut self) {
        self.save_current_world();
        self.session = None;
        if let Some(r) = &mut self.renderer {
            r.clear_chunks();
        }
        self.set_cursor_grab(false);
        let worlds = save::list_worlds(&save::default_saves_root());
        self.screen = Screen::Title {
            worlds,
            selected: 0,
            subpage: TitleSubpage::Main,
            new_name: "New World".into(),
            new_seed: "2612026".into(),
            new_mode: GameMode::Survival,
            mp_addr: "127.0.0.1:25565".into(),
            mp_status: "Ready".into(),
        };
    }

    fn step_simulation_tick(&mut self) {
        let Some(sess) = &mut self.session else {
            return;
        };

        // 1. Drain completed background chunk generation/loads
        while let Ok((dim, chunk)) = sess.chunk_rx.try_recv() {
            let cpos = chunk.pos;
            sess.pending_chunks.remove(&(dim, cpos));
            if let Some(w) = sess.worlds.get_mut(&dim) {
                w.insert_chunk(chunk, &self.data.blocks);
                // Mark neighboring chunks dirty for border culling/AO
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if let Some(nc) = w.chunks.get_mut(&ChunkPos::new(cpos.x + dx, cpos.z + dz)) {
                        nc.dirty_mesh = true;
                    }
                }
            }
        }

        // 2. Request missing chunks in render distance
        let dim = sess.current_dim;
        let pchunk = BlockPos::from_vec3(sess.player.pos).chunk_pos();
        let rd = self.render_distance;
        for dz in -rd..=rd {
            for dx in -rd..=rd {
                let cp = ChunkPos::new(pchunk.x + dx, pchunk.z + dz);
                let exists = sess.worlds.get(&dim).is_some_and(|w| w.chunks.contains_key(&cp));
                if !exists && !sess.pending_chunks.contains(&(dim, cp)) && sess.pending_chunks.len() < 16 {
                    sess.pending_chunks.insert((dim, cp));
                    let tx = sess.chunk_tx.clone();
                    let world_gen = Arc::clone(&sess.generator);
                    let data = Arc::clone(&self.data);
                    let world_dir = sess.world_dir.clone();
                    rayon::spawn(move || {
                        let chunk = save::load_chunk_from_disk(&world_dir, dim, cp, &data)
                            .unwrap_or_else(|| world_gen.generate_chunk(dim, cp, &data));
                        let _ = tx.send((dim, chunk));
                    });
                }
            }
        }

        // 3. Build player input & tick player + world + entities
        let input = if matches!(self.screen, Screen::InGame) && !sess.player.dead {
            PlayerInput {
                forward: (self.keys.w as i32 - self.keys.s as i32) as f32,
                strafe: (self.keys.d as i32 - self.keys.a as i32) as f32,
                jump: self.keys.space,
                sneak: self.keys.shift,
                sprint: self.keys.ctrl,
                yaw: sess.player.yaw,
                pitch: sess.player.pitch,
            }
        } else {
            PlayerInput {
                yaw: sess.player.yaw,
                pitch: sess.player.pitch,
                ..Default::default()
            }
        };

        let mut world = sess.worlds.remove(&dim).unwrap();
        let mut entities = sess.entities_by_dim.remove(&dim).unwrap();

        sess.player.tick(&input, &world, &self.data);
        world.tick(&self.data, pchunk);
        entities.tick(&mut world, &mut sess.player, &self.data);

        // Drain pending block item drops / primed TNT / falling blocks into entities
        for (pos, stack) in world.pending_drops.drain(..) {
            entities.spawn_item_drop(pos, stack, &self.data);
        }
        for bp in world.pending_tnt.drain(..) {
            entities.spawn(EntityKind::PrimedTnt, bp.center(), &self.data);
        }
        for fb in world.pending_falling.drain(..) {
            let id = entities.spawn(EntityKind::FallingBlock, fb.pos.center(), &self.data);
            if let Some(e) = entities.entities.iter_mut().find(|e| e.id == id) {
                e.falling_state = Some(fb.state);
            }
        }
        let pending_exps: Vec<_> = world.pending_explosions.drain(..).collect();
        for exp in pending_exps {
            world.explode(exp.center, exp.radius, exp.causes_fire, &self.data);
        }

        // Play triggered world sounds
        for snd in world.pending_sounds.drain(..) {
            self.audio.play(&snd.event, snd.volume, snd.pitch);
        }

        // Continuous mining while holding Left Mouse in InGame
        let mut mined_now = false;
        if self.keys.left_mouse && matches!(self.screen, Screen::InGame) && !sess.player.dead {
            let eye = sess.player.eye_pos();
            let dir = sess.player.look_dir();
            let reach = if sess.player.game_mode.insta_build() { 5.0 } else { SharedConstants::BLOCK_INTERACTION_RANGE };
            if let Some(hit) = world.raycast_block(eye, dir, reach, &self.data.blocks) {
                if self.break_target == Some(hit.pos) {
                    self.break_ticks += 1;
                } else {
                    self.break_target = Some(hit.pos);
                    self.break_ticks = 1;
                }
                if let Some(req) = sess.player.block_break_ticks(hit.state, &self.data) {
                    if self.break_ticks >= req.max(1) {
                        mined_now = true;
                        let broken_def = self.data.blocks.get(hit.state.id());
                        if sess.player.game_mode.is_survival_or_adventure() && world.game_rules.do_tile_drops {
                            let held = sess.player.main_hand();
                            let ctx = LootContext {
                                fortune: held.enchantment_level("fortune"),
                                silk_touch: held.enchantment_level("silk_touch") > 0,
                                explosion_survived: false,
                            };
                            let mut rng = rand::thread_rng();
                            for drop in self.data.loot.roll_block(&broken_def.name, &ctx, &mut rng) {
                                entities.spawn_item_drop(hit.pos.center(), drop, &self.data);
                            }
                            if broken_def.name.contains("stone") {
                                sess.player.advancements.insert("minecraft:story/mine_stone".into());
                            } else if broken_def.name.contains("diamond") {
                                sess.player.advancements.insert("minecraft:story/mine_diamond".into());
                            }
                        }
                        world.set_block(hit.pos, BlockState::AIR, &self.data.blocks);
                        for _ in 0..8 {
                            entities.spawn_particle(
                                ParticleKind::BlockBreak,
                                hit.pos.center(),
                                DVec3::new(0.0, 0.12, 0.0),
                                [
                                    broken_def.fallback_rgba[0] as f32 / 255.0,
                                    broken_def.fallback_rgba[1] as f32 / 255.0,
                                    broken_def.fallback_rgba[2] as f32 / 255.0,
                                    1.0,
                                ],
                            );
                        }
                        self.audio.play("block.stone.break", 0.9, 1.0);
                        self.break_target = None;
                        self.break_ticks = 0;
                        self.swing_timer = 0.25;
                    }
                }
            } else {
                self.break_target = None;
                self.break_ticks = 0;
            }
        } else {
            self.break_target = None;
            self.break_ticks = 0;
        }
        let _ = mined_now;

        // Ambient mob spawning at night / caves in Overworld
        if world.world_tick % 100 == 0 && world.game_rules.do_mob_spawning && entities.entities.len() < 32 {
            let mut rng = rand::thread_rng();
            let ang = rng.r#gen::<f64>() * std::f64::consts::TAU;
            let dist = rng.gen_range(14.0..28.0);
            let sx = (sess.player.pos.x + ang.cos() * dist).floor() as i32;
            let sz = (sess.player.pos.z + ang.sin() * dist).floor() as i32;
            let sy = find_safe_spawn_y(&world, sx, sz);
            let night = (world.day_time % SharedConstants::DAY_TICKS) >= 13000;
            let kind = if world.dimension == DimensionId::TheNether {
                EntityKind::Piglin
            } else if world.dimension == DimensionId::TheEnd {
                EntityKind::Enderman
            } else if night {
                match rng.gen_range(0..4) {
                    0 => EntityKind::Zombie,
                    1 => EntityKind::Skeleton,
                    2 => EntityKind::Creeper,
                    _ => EntityKind::Spider,
                }
            } else {
                match rng.gen_range(0..4) {
                    0 => EntityKind::Pig,
                    1 => EntityKind::Cow,
                    2 => EntityKind::Sheep,
                    _ => EntityKind::Chicken,
                }
            };
            entities.spawn(kind, DVec3::new(sx as f64 + 0.5, sy as f64 + 0.5, sz as f64 + 0.5), &self.data);
        }

        // Portal check for dimension transitions
        let standing_block = world.get_block(BlockPos::from_vec3(sess.player.pos)).id();
        let mut portal_target_dim = None;
        if sess.player.portal_cooldown == 0 {
            if standing_block == self.data.blocks.nether_portal {
                portal_target_dim = Some(if dim == DimensionId::TheNether {
                    DimensionId::Overworld
                } else {
                    DimensionId::TheNether
                });
            } else if standing_block == self.data.blocks.end_portal {
                portal_target_dim = Some(if dim == DimensionId::TheEnd {
                    DimensionId::Overworld
                } else {
                    DimensionId::TheEnd
                });
            }
        }

        // Mesh up to 4 dirty chunks in parallel right now via rayon
        if let Some(r) = &mut self.renderer {
            let dirty_hoods: Vec<ChunkNeighborhood> = world
                .chunks
                .iter()
                .filter(|(_, c)| c.dirty_mesh)
                .take(4)
                .filter_map(|(&cp, _)| ChunkNeighborhood::capture(&world, cp))
                .collect();
            if !dirty_hoods.is_empty() {
                for h in &dirty_hoods {
                    if let Some(c) = world.chunks.get_mut(&h.chunk_pos) {
                        c.dirty_mesh = false;
                    }
                }
                let atlas = &r.atlas;
                let blocks = &self.data.blocks;
                use rayon::prelude::*;
                let results: Vec<ChunkMeshResult> = dirty_hoods
                    .par_iter()
                    .map(|h| mesh_chunk(h, blocks, atlas))
                    .collect();
                for m in results {
                    r.upload_chunk_mesh(m);
                }
            }
            r.retain_chunks(pchunk, rd + 2);
        }

        if sess.player.advancements.len() > self.prev_adv_count {
            self.prev_adv_count = sess.player.advancements.len();
            self.toast = Some(("New Advancement Unlocked!".into(), 4.5));
        }

        let died_now = sess.player.dead && !matches!(self.screen, Screen::Dead);
        if died_now {
            self.screen = Screen::Dead;
        }

        sess.worlds.insert(dim, world);
        sess.entities_by_dim.insert(dim, entities);

        if died_now {
            self.set_cursor_grab(false);
        }

        if let Some(new_dim) = portal_target_dim {
            self.transfer_dimension(new_dim);
        }

        if let Some(s) = &mut self.session {
            s.autosave_ticks += 1;
            if s.autosave_ticks >= 600 {
                s.autosave_ticks = 0;
                self.save_current_world();
            }
        }
    }

    fn transfer_dimension(&mut self, target_dim: DimensionId) {
        let Some(sess) = &mut self.session else { return; };
        let old_dim = sess.current_dim;
        if old_dim == target_dim {
            return;
        }
        sess.player.portal_cooldown = 80;
        let mut nx = sess.player.pos.x;
        let mut nz = sess.player.pos.z;
        if old_dim == DimensionId::Overworld && target_dim == DimensionId::TheNether {
            nx /= 8.0;
            nz /= 8.0;
        } else if old_dim == DimensionId::TheNether && target_dim == DimensionId::Overworld {
            nx *= 8.0;
            nz *= 8.0;
        } else if target_dim == DimensionId::TheEnd {
            nx = 0.5;
            nz = 0.5;
        }

        let target_cp = BlockPos::new(nx.floor() as i32, 64, nz.floor() as i32).chunk_pos();
        if let Some(tw) = sess.worlds.get_mut(&target_dim) {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let cp = ChunkPos::new(target_cp.x + dx, target_cp.z + dz);
                    if !tw.chunks.contains_key(&cp) {
                        let ch = save::load_chunk_from_disk(&sess.world_dir, target_dim, cp, &self.data)
                            .unwrap_or_else(|| sess.generator.generate_chunk(target_dim, cp, &self.data));
                        tw.insert_chunk(ch, &self.data.blocks);
                    }
                }
            }
            let sy = if target_dim == DimensionId::TheEnd {
                66
            } else {
                find_safe_spawn_y(tw, nx.floor() as i32, nz.floor() as i32).clamp(16, 110)
            };
            sess.player.pos = DVec3::new(nx, sy as f64, nz);
            sess.player.vel = DVec3::ZERO;
            if target_dim == DimensionId::TheNether {
                sess.player.advancements.insert("minecraft:story/enter_the_nether".into());
            } else if target_dim == DimensionId::TheEnd {
                sess.player.advancements.insert("minecraft:story/enter_the_end".into());
                if let Some(em) = sess.entities_by_dim.get_mut(&DimensionId::TheEnd) {
                    if !em.dragon_defeated && !em.entities.iter().any(|e| e.kind == EntityKind::EnderDragon) {
                        em.spawn(EntityKind::EnderDragon, DVec3::new(0.0, 78.0, 0.0), &self.data);
                    }
                }
            }
        }
        sess.current_dim = target_dim;
        sess.player.dimension = target_dim;
        if let Some(r) = &mut self.renderer {
            r.clear_chunks();
        }
        self.chat_log.push((format!("Entered {}", target_dim.id_str()), 8.0));
        self.audio.play("block.portal.travel", 0.9, 1.0);
    }

    fn handle_left_click_ingame(&mut self) {
        let Some(sess) = &mut self.session else { return; };
        if sess.player.dead {
            return;
        }
        self.swing_timer = 0.25;
        let eye = sess.player.eye_pos();
        let dir = sess.player.look_dir();
        let dim = sess.current_dim;

        let mut world = sess.worlds.remove(&dim).unwrap();
        let mut entities = sess.entities_by_dim.remove(&dim).unwrap();

        let block_hit = world.raycast_block(eye, dir, SharedConstants::BLOCK_INTERACTION_RANGE, &self.data.blocks);
        let entity_hit = entities.raycast_entity(eye, dir, SharedConstants::ENTITY_INTERACTION_RANGE);

        if let Some((eid, edist)) = entity_hit {
            if block_hit.as_ref().is_none_or(|b| edist < b.distance) {
                let held = sess.player.main_hand();
                let mut dmg = self.data.items.get(held.item).attack_damage;
                let sharp = held.enchantment_level("sharpness");
                if sharp > 0 {
                    dmg += 0.5 + sharp as f32 * 0.5;
                }
                let is_crit = sess.player.vel.y < -0.05 && !sess.player.on_ground;
                if is_crit {
                    dmg *= 1.5;
                }
                entities.damage_entity(eid, dmg, dir, &mut world, &self.data);
                entities.spawn_particle(
                    if is_crit { ParticleKind::Crit } else { ParticleKind::Heart },
                    eye + dir * edist,
                    DVec3::new(0.0, 0.08, 0.0),
                    [1.0, 0.2, 0.2, 1.0],
                );
                self.audio.play("entity.player.attack.strong", 1.0, 1.0);
            }
        }
        sess.worlds.insert(dim, world);
        sess.entities_by_dim.insert(dim, entities);
    }

    fn handle_right_click_ingame(&mut self) {
        let Some(sess) = &mut self.session else { return; };
        if sess.player.dead {
            return;
        }
        let dim = sess.current_dim;
        let eye = sess.player.eye_pos();
        let dir = sess.player.look_dir();

        // Check entity interaction (Villager trade)
        if let Some(em) = sess.entities_by_dim.get(&dim) {
            if let Some((eid, _)) = em.raycast_entity(eye, dir, SharedConstants::ENTITY_INTERACTION_RANGE) {
                if let Some(e) = em.entities.iter().find(|e| e.id == eid) {
                    if e.kind == EntityKind::Villager {
                        self.screen = Screen::VillagerTrade {
                            entity_id: eid,
                            cursor: ItemStack::EMPTY,
                        };
                        self.set_cursor_grab(false);
                        return;
                    }
                }
            }
        }

        // Block interaction or block placement
        let mut world = sess.worlds.remove(&dim).unwrap();
        let mut opened_screen: Option<Screen> = None;

        if let Some(hit) = world.raycast_block(eye, dir, SharedConstants::BLOCK_INTERACTION_RANGE, &self.data.blocks) {
            let b_id = hit.state.id();
            if b_id == self.data.blocks.crafting_table {
                opened_screen = Some(Screen::CraftingTable {
                    grid: std::array::from_fn(|_| ItemStack::EMPTY),
                    cursor: ItemStack::EMPTY,
                });
            } else if b_id == self.data.blocks.furnace
                || b_id == self.data.blocks.blast_furnace
                || b_id == self.data.blocks.smoker
            {
                if world.get_block_entity(hit.pos).is_none() {
                    world.set_block_entity(hit.pos, BlockEntityData::new_furnace(mc_world::block_entity::CookingKindDef::Smelting));
                }
                opened_screen = Some(Screen::Furnace {
                    pos: hit.pos,
                    cursor: ItemStack::EMPTY,
                });
            } else if b_id == self.data.blocks.chest || b_id == self.data.blocks.barrel {
                if world.get_block_entity(hit.pos).is_none() {
                    world.set_block_entity(hit.pos, BlockEntityData::new_chest());
                }
                opened_screen = Some(Screen::Chest {
                    pos: hit.pos,
                    cursor: ItemStack::EMPTY,
                });
            } else if b_id == self.data.blocks.enchanting_table {
                opened_screen = Some(Screen::Enchanting {
                    _pos: hit.pos,
                    item: ItemStack::EMPTY,
                    lapis: ItemStack::EMPTY,
                    cursor: ItemStack::EMPTY,
                });
            } else if b_id == self.data.blocks.anvil {
                opened_screen = Some(Screen::Anvil {
                    _pos: hit.pos,
                    left: ItemStack::EMPTY,
                    right: ItemStack::EMPTY,
                    cursor: ItemStack::EMPTY,
                });
            } else if b_id == self.data.blocks.brewing_stand {
                if world.get_block_entity(hit.pos).is_none() {
                    world.set_block_entity(hit.pos, BlockEntityData::new_brewing_stand());
                }
                opened_screen = Some(Screen::BrewingStand {
                    pos: hit.pos,
                    cursor: ItemStack::EMPTY,
                });
            } else if b_id == self.data.blocks.lever || b_id == self.data.blocks.oak_button {
                let toggled = hit.state.with_active(!hit.state.is_active());
                world.set_block(hit.pos, toggled, &self.data.blocks);
                if b_id == self.data.blocks.oak_button {
                    world.schedule_tick(hit.pos, b_id, 20);
                }
                self.audio.play("block.lever.click", 0.8, 1.0);
            } else if b_id == self.data.blocks.repeater {
                let lvl = (hit.state.level() + 1) % 4;
                world.set_block(hit.pos, hit.state.with_level(lvl), &self.data.blocks);
            } else if self.data.blocks.get(b_id).shape == mc_data::BlockShape::Door
                || self.data.blocks.get(b_id).shape == mc_data::BlockShape::Trapdoor
            {
                world.set_block(hit.pos, hit.state.with_active(!hit.state.is_active()), &self.data.blocks);
                self.audio.play("block.wooden_door.open", 0.8, 1.0);
            } else if self.data.blocks.get(b_id).shape == mc_data::BlockShape::Bed {
                if world.dimension.bed_explodes() {
                    world.set_block(hit.pos, BlockState::AIR, &self.data.blocks);
                    world.explode(hit.pos.center(), 5.0, true, &self.data);
                } else {
                    sess.player.spawn_pos = hit.pos;
                    sess.player.spawn_dim = dim;
                    world.day_time = 1000;
                    self.chat_log.push(("Respawn point set & slept until morning.".into(), 8.0));
                }
            } else {
                // Place block / use held item
                let place_pos = hit.pos.relative(hit.face);
                let held = sess.player.main_hand().clone();
                if !held.is_empty() {
                    let idef = self.data.items.get(held.item);
                    if idef.name == "flint_and_steel" {
                        if b_id == self.data.blocks.tnt {
                            world.ignite_tnt(hit.pos, &self.data.blocks);
                        } else if b_id == self.data.blocks.obsidian {
                            // Light Nether portal frame around place_pos
                            for dy in 0..3 {
                                for dx in 0..2 {
                                    world.set_block(
                                        BlockPos::new(place_pos.x + dx, place_pos.y + dy, place_pos.z),
                                        BlockState::new(self.data.blocks.nether_portal),
                                        &self.data.blocks,
                                    );
                                }
                            }
                            self.audio.play("block.portal.trigger", 1.0, 1.0);
                        } else {
                            world.set_block(place_pos, BlockState::new(self.data.blocks.fire), &self.data.blocks);
                        }
                    } else if idef.name == "water_bucket" {
                        world.set_block(place_pos, BlockState::new(self.data.blocks.water), &self.data.blocks);
                    } else if idef.name == "lava_bucket" {
                        world.set_block(place_pos, BlockState::new(self.data.blocks.lava), &self.data.blocks);
                    } else if let Some(place_bid) = idef.places_block {
                        let block_aabb = mc_core::AABB::unit_block(place_pos.x, place_pos.y, place_pos.z);
                        if !block_aabb.intersects(sess.player.aabb()) || !self.data.blocks.get(place_bid).is_solid {
                            let facing = yaw_to_facing(sess.player.yaw);
                            let st = BlockState::new(place_bid).with_facing(facing);
                            world.set_block(place_pos, st, &self.data.blocks);
                            self.audio.play("block.stone.place", 0.9, 1.0);
                            if sess.player.game_mode.is_survival_or_adventure() {
                                let slot = sess.player.main_hand_mut();
                                slot.count -= 1;
                                if slot.count == 0 {
                                    *slot = ItemStack::EMPTY;
                                }
                            }
                        }
                    } else if !sess.player.eat_food(&self.data) && !sess.player.try_equip_armor_from_hand(&self.data) {
                        if matches!(idef.name.as_str(), "bow" | "crossbow" | "ender_pearl" | "snowball") {
                            let kind = match idef.name.as_str() {
                                "ender_pearl" => EntityKind::EnderPearl,
                                "snowball" => EntityKind::Snowball,
                                _ => EntityKind::Arrow,
                            };
                            if let Some(em) = sess.entities_by_dim.get_mut(&dim) {
                                let id = em.spawn(kind, eye + dir * 0.6, &self.data);
                                if let Some(e) = em.entities.iter_mut().find(|e| e.id == id) {
                                    e.vel = dir * 1.4;
                                    e.owner_projectile_from_player = true;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Try eating/equipping/throwing when looking at sky
            if !sess.player.eat_food(&self.data) {
                sess.player.try_equip_armor_from_hand(&self.data);
            }
        }

        sess.worlds.insert(dim, world);
        if let Some(next_screen) = opened_screen {
            self.screen = next_screen;
            self.set_cursor_grab(false);
        }
    }

    fn apply_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::None => {}
            UiAction::Resume => {
                self.screen = Screen::InGame;
                self.set_cursor_grab(true);
            }
            UiAction::StartWorld { name, seed, mode } => {
                self.start_new_world(name, seed, mode);
            }
            UiAction::LoadWorld { summary } => {
                self.load_saved_world(summary);
            }
            UiAction::SaveAndQuit => {
                self.save_and_quit();
            }
            UiAction::Respawn => {
                if let Some(sess) = &mut self.session {
                    sess.player.dead = false;
                    sess.player.health = sess.player.max_health;
                    sess.player.food_level = 20;
                    sess.player.fire_ticks = 0;
                    sess.player.fall_distance = 0.0;
                    sess.player.pos = DVec3::new(
                        sess.player.spawn_pos.x as f64 + 0.5,
                        sess.player.spawn_pos.y as f64 + 1.0,
                        sess.player.spawn_pos.z as f64 + 0.5,
                    );
                    sess.player.vel = DVec3::ZERO;
                }
                self.screen = Screen::InGame;
                self.set_cursor_grab(true);
            }
            UiAction::SendChat(raw) => {
                let trimmed = raw.trim().to_string();
                if !trimmed.is_empty() {
                    if trimmed.starts_with('/') {
                        if let Some(sess) = &mut self.session {
                            let dim = sess.current_dim;
                            let mut w = sess.worlds.remove(&dim).unwrap();
                            let mut em = sess.entities_by_dim.remove(&dim).unwrap();
                            let reply = execute_command(&trimmed, &mut sess.player, &mut w, &mut em, &self.data);
                            sess.worlds.insert(dim, w);
                            sess.entities_by_dim.insert(dim, em);
                            self.chat_log.push((reply, 10.0));
                        }
                    } else {
                        self.chat_log.push((format!("<Steve> {trimmed}"), 10.0));
                    }
                }
                self.screen = Screen::InGame;
                self.set_cursor_grab(true);
            }
            UiAction::PingServer(addr) => {
                let msg = match ping_server(&addr) {
                    Ok(st) => format!("Online: {} ({}/{})", st.description.text, st.players.online, st.players.max),
                    Err(e) => format!("Ping failed: {e}"),
                };
                if let Screen::Title { mp_status, .. } = &mut self.screen {
                    *mp_status = msg;
                }
            }
            UiAction::ConnectServer(addr) => {
                let msg = match ping_server(&addr) {
                    Ok(_) => "Handshaked! Entering synced world...".to_string(),
                    Err(e) => format!("Connect note ({e}) -> offline local world"),
                };
                self.chat_log.push((msg, 8.0));
                self.start_new_world("Multiplayer Session".into(), 2612026, GameMode::Survival);
            }
            UiAction::OpenLan => {
                if let Some(sess) = &mut self.session {
                    if sess.lan_server.is_none() {
                        let seed = sess.world().seed;
                        if let Ok(srv) = LanServer::start(0, seed) {
                            let port = srv.port;
                            sess.lan_server = Some(srv);
                            self.chat_log.push((format!("Local game hosted on port {port}"), 10.0));
                        }
                    }
                }
            }
            UiAction::ToggleRenderDist => {
                self.render_distance = match self.render_distance {
                    4 => 6,
                    6 => 8,
                    8 => 10,
                    _ => 4,
                };
            }
            UiAction::ToggleFov => {
                self.fov_deg = if self.fov_deg > 95.0 { 65.0 } else { self.fov_deg + 10.0 };
            }
            UiAction::ToggleSound => {
                self.audio.master_volume = if self.audio.master_volume > 0.0 { 0.0 } else { 0.6 };
            }
        }
    }

    fn return_container_items(&mut self) {
        let Some(sess) = &mut self.session else { return; };
        match &mut self.screen {
            Screen::Inventory { craft, cursor } => {
                for s in craft.iter_mut() {
                    if !s.is_empty() {
                        sess.player.give_item(std::mem::take(s), &self.data);
                    }
                }
                if !cursor.is_empty() {
                    sess.player.give_item(std::mem::take(cursor), &self.data);
                }
            }
            Screen::CraftingTable { grid, cursor } => {
                for s in grid.iter_mut() {
                    if !s.is_empty() {
                        sess.player.give_item(std::mem::take(s), &self.data);
                    }
                }
                if !cursor.is_empty() {
                    sess.player.give_item(std::mem::take(cursor), &self.data);
                }
            }
            Screen::Enchanting { item, lapis, cursor, .. } => {
                for s in [item, lapis, cursor] {
                    if !s.is_empty() {
                        sess.player.give_item(std::mem::take(s), &self.data);
                    }
                }
            }
            Screen::Anvil { left, right, cursor, .. } => {
                for s in [left, right, cursor] {
                    if !s.is_empty() {
                        sess.player.give_item(std::mem::take(s), &self.data);
                    }
                }
            }
            Screen::Creative { cursor, .. }
            | Screen::Furnace { cursor, .. }
            | Screen::Chest { cursor, .. }
            | Screen::BrewingStand { cursor, .. }
            | Screen::VillagerTrade { cursor, .. } => {
                if !cursor.is_empty() {
                    sess.player.give_item(std::mem::take(cursor), &self.data);
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for McApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Minecraft 26.1.2 — Rust Clean-Room Edition")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let renderer = Renderer::new(Arc::clone(&window), &self.data);
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.cursor_locked && matches!(self.screen, Screen::InGame) {
                if let Some(sess) = &mut self.session {
                    let sens = 0.0022f32;
                    sess.player.yaw += dx as f32 * sens;
                    sess.player.pitch = (sess.player.pitch + dy as f32 * sens).clamp(-1.55, 1.55);
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.save_current_world();
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(new_size);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32;
                self.mouse_y = position.y as f32;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 30.0,
                };
                if matches!(self.screen, Screen::InGame) {
                    if let Some(sess) = &mut self.session {
                        let cur = sess.player.selected_slot as i32;
                        let step = if y > 0.0 { -1 } else if y < 0.0 { 1 } else { 0 };
                        sess.player.selected_slot = (cur + step).rem_euclid(9) as usize;
                    }
                } else if let Screen::Creative { scroll, .. } | Screen::Advancements { scroll } = &mut self.screen {
                    if y > 0.0 {
                        *scroll = scroll.saturating_sub(2);
                    } else if y < 0.0 {
                        *scroll += 2;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                if button == MouseButton::Left {
                    self.keys.left_mouse = pressed;
                }
                if pressed {
                    if matches!(self.screen, Screen::InGame) {
                        if !self.cursor_locked {
                            self.set_cursor_grab(true);
                        } else if button == MouseButton::Left {
                            self.handle_left_click_ingame();
                        } else if button == MouseButton::Right {
                            self.handle_right_click_ingame();
                        }
                    } else if matches!(button, MouseButton::Left | MouseButton::Right) {
                        let (w, h) = self
                            .renderer
                            .as_ref()
                            .map(|r| (r.config.width as f32, r.config.height as f32))
                            .unwrap_or((1280.0, 720.0));
                        let rc = button == MouseButton::Right;
                        let action = if let Some(sess) = &mut self.session {
                            let dim = sess.current_dim;
                            let mut w_opt = Some(sess.worlds.remove(&dim).unwrap());
                            let mut em_opt = Some(sess.entities_by_dim.remove(&dim).unwrap());
                            let mut p_opt = Some(std::mem::replace(&mut sess.player, Player::new("", BlockPos::ZERO, GameMode::Survival)));
                            let act = ui::handle_ui_click(
                                &mut self.screen,
                                &mut w_opt,
                                &mut p_opt,
                                &mut em_opt,
                                &self.data,
                                w,
                                h,
                                self.mouse_x,
                                self.mouse_y,
                                rc,
                            );
                            sess.player = p_opt.unwrap();
                            sess.worlds.insert(dim, w_opt.unwrap());
                            sess.entities_by_dim.insert(dim, em_opt.unwrap());
                            act
                        } else {
                            let mut none_w = None;
                            let mut none_p = None;
                            let mut none_e = None;
                            ui::handle_ui_click(
                                &mut self.screen,
                                &mut none_w,
                                &mut none_p,
                                &mut none_e,
                                &self.data,
                                w,
                                h,
                                self.mouse_x,
                                self.mouse_y,
                                rc,
                            )
                        };
                        self.apply_ui_action(action);
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyW => self.keys.w = pressed,
                        KeyCode::KeyA => self.keys.a = pressed,
                        KeyCode::KeyS => self.keys.s = pressed,
                        KeyCode::KeyD => self.keys.d = pressed,
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => self.keys.shift = pressed,
                        KeyCode::ControlLeft | KeyCode::ControlRight => self.keys.ctrl = pressed,
                        KeyCode::Space => {
                            if pressed && !self.keys.space && matches!(self.screen, Screen::InGame) {
                                let now = Instant::now();
                                if let Some(prev) = self.keys.last_space_down {
                                    if now.duration_since(prev) < Duration::from_millis(300) {
                                        if let Some(sess) = &mut self.session {
                                            if sess.player.game_mode.can_fly() {
                                                sess.player.is_flying = !sess.player.is_flying;
                                            }
                                        }
                                    }
                                }
                                self.keys.last_space_down = Some(now);
                            }
                            self.keys.space = pressed;
                        }
                        _ => {}
                    }
                }

                if pressed {
                    self.handle_key_pressed(event.physical_key, event.logical_key);
                }
            }
            WindowEvent::RedrawRequested => {
                self.update_and_render();
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }
}

impl McApp {
    fn handle_key_pressed(&mut self, phys: PhysicalKey, logical: Key) {
        // Text editing screens first
        match &mut self.screen {
            Screen::Chat { input } => {
                match phys {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        self.screen = Screen::InGame;
                        self.set_cursor_grab(true);
                    }
                    PhysicalKey::Code(KeyCode::Enter) => {
                        let cmd = input.clone();
                        self.apply_ui_action(UiAction::SendChat(cmd));
                    }
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        input.pop();
                    }
                    _ => {
                        if let Key::Character(txt) = &logical {
                            input.push_str(txt.as_str());
                        }
                    }
                }
                return;
            }
            Screen::Title {
                subpage: TitleSubpage::CreateWorld,
                new_name,
                ..
            } => {
                match phys {
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        new_name.pop();
                    }
                    PhysicalKey::Code(KeyCode::Escape) => {
                        if let Screen::Title { subpage, .. } = &mut self.screen {
                            *subpage = TitleSubpage::Singleplayer;
                        }
                    }
                    _ => {
                        if let Key::Character(txt) = &logical {
                            if new_name.len() < 28 {
                                new_name.push_str(txt.as_str());
                            }
                        }
                    }
                }
                return;
            }
            Screen::Title {
                subpage: TitleSubpage::Multiplayer,
                mp_addr,
                ..
            } => {
                match phys {
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        mp_addr.pop();
                    }
                    PhysicalKey::Code(KeyCode::Escape) => {
                        if let Screen::Title { subpage, .. } = &mut self.screen {
                            *subpage = TitleSubpage::Main;
                        }
                    }
                    _ => {
                        if let Key::Character(txt) = &logical {
                            mp_addr.push_str(txt.as_str());
                        }
                    }
                }
                return;
            }
            Screen::Creative { search, scroll, .. } => {
                match phys {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        self.return_container_items();
                        self.screen = Screen::InGame;
                        self.set_cursor_grab(true);
                    }
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        search.pop();
                        *scroll = 0;
                    }
                    _ => {
                        if let Key::Character(txt) = &logical {
                            search.push_str(txt.as_str());
                            *scroll = 0;
                        }
                    }
                }
                return;
            }
            _ => {}
        }

        if let PhysicalKey::Code(code) = phys {
            match code {
                KeyCode::Escape => match self.screen {
                    Screen::InGame => {
                        self.screen = Screen::Pause;
                        self.set_cursor_grab(false);
                    }
                    Screen::Pause => {
                        self.screen = Screen::InGame;
                        self.set_cursor_grab(true);
                    }
                    Screen::Title { ref mut subpage, .. } => {
                        *subpage = TitleSubpage::Main;
                    }
                    _ => {
                        self.return_container_items();
                        self.screen = Screen::InGame;
                        self.set_cursor_grab(true);
                    }
                },
                KeyCode::KeyE => {
                    if matches!(self.screen, Screen::InGame) {
                        if let Some(sess) = &self.session {
                            self.screen = if sess.player.game_mode == GameMode::Creative {
                                Screen::Creative {
                                    search: String::new(),
                                    scroll: 0,
                                    cursor: ItemStack::EMPTY,
                                }
                            } else {
                                Screen::Inventory {
                                    craft: std::array::from_fn(|_| ItemStack::EMPTY),
                                    cursor: ItemStack::EMPTY,
                                }
                            };
                            self.set_cursor_grab(false);
                        }
                    } else if matches!(self.screen, Screen::Inventory { .. } | Screen::Creative { .. }) {
                        self.return_container_items();
                        self.screen = Screen::InGame;
                        self.set_cursor_grab(true);
                    }
                }
                KeyCode::KeyT if matches!(self.screen, Screen::InGame) => {
                    self.screen = Screen::Chat { input: String::new() };
                    self.set_cursor_grab(false);
                }
                KeyCode::Slash if matches!(self.screen, Screen::InGame) => {
                    self.screen = Screen::Chat { input: "/".into() };
                    self.set_cursor_grab(false);
                }
                KeyCode::KeyL if matches!(self.screen, Screen::InGame) => {
                    self.screen = Screen::Advancements { scroll: 0 };
                    self.set_cursor_grab(false);
                }
                KeyCode::F3 => self.show_f3 = !self.show_f3,
                KeyCode::F5 => self.third_person = !self.third_person,
                KeyCode::KeyQ if matches!(self.screen, Screen::InGame) => {
                    if let Some(sess) = &mut self.session {
                        let dim = sess.current_dim;
                        let drop_pos = sess.player.eye_pos() + sess.player.look_dir() * 0.8;
                        let slot = sess.player.main_hand_mut();
                        if !slot.is_empty() {
                            let mut one = slot.clone();
                            one.count = 1;
                            slot.count -= 1;
                            if slot.count == 0 {
                                *slot = ItemStack::EMPTY;
                            }
                            if let Some(em) = sess.entities_by_dim.get_mut(&dim) {
                                em.spawn_item_drop(drop_pos, one, &self.data);
                            }
                        }
                    }
                }
                KeyCode::Digit1 => self.select_hotbar(0),
                KeyCode::Digit2 => self.select_hotbar(1),
                KeyCode::Digit3 => self.select_hotbar(2),
                KeyCode::Digit4 => self.select_hotbar(3),
                KeyCode::Digit5 => self.select_hotbar(4),
                KeyCode::Digit6 => self.select_hotbar(5),
                KeyCode::Digit7 => self.select_hotbar(6),
                KeyCode::Digit8 => self.select_hotbar(7),
                KeyCode::Digit9 => self.select_hotbar(8),
                _ => {
                    let _ = NamedKey::Escape;
                }
            }
        }
    }

    fn select_hotbar(&mut self, idx: usize) {
        if let Some(sess) = &mut self.session {
            sess.player.selected_slot = idx.min(8);
        }
    }

    fn update_and_render(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        self.accumulator += dt;
        self.swing_timer = (self.swing_timer - dt as f32).max(0.0);

        for (_, timer) in &mut self.chat_log {
            *timer = (*timer - dt as f32).max(0.0);
        }
        if let Some((_, timer)) = &mut self.toast {
            *timer = (*timer - dt as f32).max(0.0);
        }

        self.fps_counter += 1;
        self.fps_timer += dt as f32;
        if self.fps_timer >= 0.5 {
            self.fps_value = ((self.fps_counter as f32) / self.fps_timer).round() as u32;
            self.fps_counter = 0;
            self.fps_timer = 0.0;
        }

        let tick_sec = 1.0 / SharedConstants::TICKS_PER_SECOND as f64;
        let mut ticks = 0;
        while self.accumulator >= tick_sec && ticks < 4 {
            self.accumulator -= tick_sec;
            if self.session.is_some() && !matches!(self.screen, Screen::Pause) {
                self.step_simulation_tick();
            }
            ticks += 1;
        }

        let Some(mut renderer) = self.renderer.take() else {
            return;
        };

        let w = renderer.config.width as f32;
        let h = renderer.config.height as f32;
        let loaded_chunks = self.session.as_ref().map(|s| s.world().chunks.len()).unwrap_or(0);
        let lan_port = self
            .session
            .as_ref()
            .and_then(|s| s.lan_server.as_ref().map(|l| l.port));

        let target_hit = self.session.as_ref().and_then(|s| {
            s.world().raycast_block(
                s.player.eye_pos(),
                s.player.look_dir(),
                SharedConstants::BLOCK_INTERACTION_RANGE,
                &self.data.blocks,
            )
        });
        let break_progress = match (&self.session, &target_hit, self.break_target) {
            (Some(s), Some(hit), Some(bt)) if hit.pos == bt => s
                .player
                .block_break_ticks(hit.state, &self.data)
                .map(|req| self.break_ticks as f32 / req.max(1) as f32)
                .unwrap_or(0.0),
            _ => 0.0,
        };

        let ctx = HudContext {
            world: self.session.as_ref().map(|s| s.world()),
            player: self.session.as_ref().map(|s| &s.player),
            entities: self.session.as_ref().map(|s| s.entities()),
            data: &self.data,
            fps: self.fps_value,
            loaded_chunks,
            show_f3: self.show_f3,
            render_distance: self.render_distance,
            fov_deg: self.fov_deg,
            sound_enabled: self.audio.master_volume > 0.0,
            chat_log: &self.chat_log,
            toast: self.toast.as_ref(),
            lan_port,
        };

        ui::build_ui(
            &self.screen,
            &ctx,
            &renderer,
            w,
            h,
            self.mouse_x,
            self.mouse_y,
            &mut self.ui_batch,
        );

        let frame_in = FrameRenderInput {
            world: ctx.world,
            player: ctx.player,
            entities: ctx.entities,
            data: &self.data,
            target_block: target_hit.map(|h| h.pos),
            break_progress,
            third_person: self.third_person,
            fov_deg: self.fov_deg,
            render_distance: self.render_distance,
            time_secs: now.duration_since(self.start_time).as_secs_f32(),
            swing_progress: if self.swing_timer > 0.0 {
                1.0 - (self.swing_timer / 0.25)
            } else {
                0.0
            },
        };

        match renderer.render(frame_in, &self.ui_batch) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost) => renderer.resize(renderer.size),
            Err(_) => {}
        }

        self.renderer = Some(renderer);
        let _ = TextureAtlas::block_uv;
    }
}

fn find_safe_spawn_y(world: &World, x: i32, z: i32) -> i32 {
    for y in (0..220).rev() {
        let b = world.get_block(BlockPos::new(x, y, z));
        if !b.is_air() {
            return y + 2;
        }
    }
    70
}

fn yaw_to_facing(yaw: f32) -> Direction {
    let deg = yaw.to_degrees().rem_euclid(360.0);
    if deg < 45.0 || deg >= 315.0 {
        Direction::South
    } else if deg < 135.0 {
        Direction::West
    } else if deg < 225.0 {
        Direction::North
    } else {
        Direction::East
    }
}
