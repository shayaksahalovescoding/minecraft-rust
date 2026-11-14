use crate::player::Player;
use glam::DVec3;
use mc_core::{BlockPos, ChunkPos, Difficulty, DimensionId, SharedConstants, AABB};
use mc_data::{BlockState, GameData, ItemStack, LootContext};
use mc_world::World;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    // Passive
    Pig,
    Cow,
    Sheep,
    Chicken,
    Wolf,
    Cat,
    Horse,
    Villager,
    IronGolem,
    SnowGolem,
    Bee,
    Squid,
    Cod,
    Axolotl,
    // Hostile
    Zombie,
    Husk,
    Drowned,
    Skeleton,
    Stray,
    Creeper,
    Spider,
    CaveSpider,
    Enderman,
    Witch,
    Slime,
    MagmaCube,
    Phantom,
    Pillager,
    Vindicator,
    Silverfish,
    // Nether & End
    Ghast,
    Blaze,
    Piglin,
    ZombifiedPiglin,
    Hoglin,
    WitherSkeleton,
    Shulker,
    Endermite,
    // Bosses
    EnderDragon,
    Wither,
    // Non-living / Projectiles / Vehicles
    ItemDrop,
    ExperienceOrb,
    Arrow,
    Fireball,
    EnderPearl,
    Snowball,
    PrimedTnt,
    FallingBlock,
    Boat,
    Minecart,
    EndCrystal,
}

impl EntityKind {
    pub fn from_name(name: &str) -> Option<Self> {
        let short = name.strip_prefix("minecraft:").unwrap_or(name);
        Some(match short {
            "pig" => Self::Pig,
            "cow" => Self::Cow,
            "sheep" => Self::Sheep,
            "chicken" => Self::Chicken,
            "wolf" => Self::Wolf,
            "cat" => Self::Cat,
            "horse" => Self::Horse,
            "villager" => Self::Villager,
            "iron_golem" => Self::IronGolem,
            "snow_golem" => Self::SnowGolem,
            "bee" => Self::Bee,
            "squid" => Self::Squid,
            "cod" => Self::Cod,
            "axolotl" => Self::Axolotl,
            "zombie" => Self::Zombie,
            "husk" => Self::Husk,
            "drowned" => Self::Drowned,
            "skeleton" => Self::Skeleton,
            "stray" => Self::Stray,
            "creeper" => Self::Creeper,
            "spider" => Self::Spider,
            "cave_spider" => Self::CaveSpider,
            "enderman" => Self::Enderman,
            "witch" => Self::Witch,
            "slime" => Self::Slime,
            "magma_cube" => Self::MagmaCube,
            "phantom" => Self::Phantom,
            "pillager" => Self::Pillager,
            "vindicator" => Self::Vindicator,
            "silverfish" => Self::Silverfish,
            "ghast" => Self::Ghast,
            "blaze" => Self::Blaze,
            "piglin" => Self::Piglin,
            "zombified_piglin" => Self::ZombifiedPiglin,
            "hoglin" => Self::Hoglin,
            "wither_skeleton" => Self::WitherSkeleton,
            "shulker" => Self::Shulker,
            "endermite" => Self::Endermite,
            "ender_dragon" => Self::EnderDragon,
            "wither" => Self::Wither,
            "item" => Self::ItemDrop,
            "experience_orb" => Self::ExperienceOrb,
            "arrow" => Self::Arrow,
            "fireball" => Self::Fireball,
            "ender_pearl" => Self::EnderPearl,
            "snowball" => Self::Snowball,
            "tnt" => Self::PrimedTnt,
            "falling_block" => Self::FallingBlock,
            "boat" | "oak_boat" => Self::Boat,
            "minecart" => Self::Minecart,
            "end_crystal" => Self::EndCrystal,
            _ => return None,
        })
    }

    pub fn registry_name(self) -> &'static str {
        match self {
            Self::Pig => "pig",
            Self::Cow => "cow",
            Self::Sheep => "sheep",
            Self::Chicken => "chicken",
            Self::Wolf => "wolf",
            Self::Cat => "cat",
            Self::Horse => "horse",
            Self::Villager => "villager",
            Self::IronGolem => "iron_golem",
            Self::SnowGolem => "snow_golem",
            Self::Bee => "bee",
            Self::Squid => "squid",
            Self::Cod => "cod",
            Self::Axolotl => "axolotl",
            Self::Zombie => "zombie",
            Self::Husk => "husk",
            Self::Drowned => "drowned",
            Self::Skeleton => "skeleton",
            Self::Stray => "stray",
            Self::Creeper => "creeper",
            Self::Spider => "spider",
            Self::CaveSpider => "cave_spider",
            Self::Enderman => "enderman",
            Self::Witch => "witch",
            Self::Slime => "slime",
            Self::MagmaCube => "magma_cube",
            Self::Phantom => "phantom",
            Self::Pillager => "pillager",
            Self::Vindicator => "vindicator",
            Self::Silverfish => "silverfish",
            Self::Ghast => "ghast",
            Self::Blaze => "blaze",
            Self::Piglin => "piglin",
            Self::ZombifiedPiglin => "zombified_piglin",
            Self::Hoglin => "hoglin",
            Self::WitherSkeleton => "wither_skeleton",
            Self::Shulker => "shulker",
            Self::Endermite => "endermite",
            Self::EnderDragon => "ender_dragon",
            Self::Wither => "wither",
            Self::ItemDrop => "item",
            Self::ExperienceOrb => "experience_orb",
            Self::Arrow => "arrow",
            Self::Fireball => "fireball",
            Self::EnderPearl => "ender_pearl",
            Self::Snowball => "snowball",
            Self::PrimedTnt => "tnt",
            Self::FallingBlock => "falling_block",
            Self::Boat => "oak_boat",
            Self::Minecart => "minecart",
            Self::EndCrystal => "end_crystal",
        }
    }

    pub fn dimensions(self) -> (f64, f64) {
        match self {
            Self::Pig => (0.9, 0.9),
            Self::Cow => (0.9, 1.4),
            Self::Sheep => (0.9, 1.3),
            Self::Chicken => (0.4, 0.7),
            Self::Wolf => (0.6, 0.85),
            Self::Cat => (0.6, 0.7),
            Self::Horse => (1.39, 1.6),
            Self::Villager | Self::Pillager | Self::Vindicator | Self::Witch => (0.6, 1.95),
            Self::IronGolem => (1.4, 2.7),
            Self::SnowGolem => (0.7, 1.9),
            Self::Zombie | Self::Husk | Self::Drowned | Self::Skeleton | Self::Stray | Self::Piglin | Self::ZombifiedPiglin => (0.6, 1.95),
            Self::WitherSkeleton => (0.7, 2.4),
            Self::Creeper => (0.6, 1.7),
            Self::Spider => (1.4, 0.9),
            Self::CaveSpider => (0.7, 0.5),
            Self::Enderman => (0.6, 2.9),
            Self::Slime | Self::MagmaCube => (1.0, 1.0),
            Self::Ghast => (4.0, 4.0),
            Self::Blaze => (0.6, 1.8),
            Self::Hoglin => (1.39, 1.4),
            Self::EnderDragon => (6.0, 4.0),
            Self::Wither => (0.9, 3.5),
            Self::ItemDrop | Self::ExperienceOrb | Self::Arrow | Self::Snowball | Self::EnderPearl => (0.25, 0.25),
            Self::Fireball => (1.0, 1.0),
            Self::PrimedTnt | Self::FallingBlock => (0.98, 0.98),
            Self::Boat => (1.375, 0.5625),
            Self::Minecart => (0.98, 0.7),
            Self::EndCrystal => (2.0, 2.0),
            _ => (0.6, 1.0),
        }
    }

    pub fn max_health(self) -> f32 {
        match self {
            Self::Chicken => 4.0,
            Self::Pig | Self::Sheep | Self::Cow | Self::SnowGolem => 10.0,
            Self::Spider | Self::Slime => 16.0,
            Self::Zombie | Self::Husk | Self::Drowned | Self::Skeleton | Self::Stray | Self::Creeper | Self::Villager | Self::Blaze => 20.0,
            Self::Witch => 26.0,
            Self::Enderman | Self::Hoglin => 40.0,
            Self::IronGolem => 100.0,
            Self::EnderDragon => 200.0,
            Self::Wither => 300.0,
            _ => 20.0,
        }
    }

    pub fn is_hostile(self) -> bool {
        matches!(
            self,
            Self::Zombie
                | Self::Husk
                | Self::Drowned
                | Self::Skeleton
                | Self::Stray
                | Self::Creeper
                | Self::Spider
                | Self::CaveSpider
                | Self::Enderman
                | Self::Witch
                | Self::Slime
                | Self::MagmaCube
                | Self::Phantom
                | Self::Pillager
                | Self::Vindicator
                | Self::Silverfish
                | Self::Ghast
                | Self::Blaze
                | Self::Piglin
                | Self::Hoglin
                | Self::WitherSkeleton
                | Self::Shulker
                | Self::Endermite
                | Self::EnderDragon
                | Self::Wither
        )
    }

    pub fn color_rgba(self) -> [f32; 4] {
        match self {
            Self::Pig => [0.95, 0.65, 0.68, 1.0],
            Self::Cow => [0.40, 0.28, 0.20, 1.0],
            Self::Sheep => [0.95, 0.95, 0.95, 1.0],
            Self::Chicken => [0.98, 0.95, 0.88, 1.0],
            Self::Wolf => [0.75, 0.75, 0.78, 1.0],
            Self::Villager => [0.55, 0.36, 0.25, 1.0],
            Self::IronGolem => [0.82, 0.80, 0.78, 1.0],
            Self::Zombie | Self::Drowned | Self::Husk => [0.28, 0.60, 0.34, 1.0],
            Self::Skeleton | Self::Stray => [0.85, 0.85, 0.82, 1.0],
            Self::Creeper => [0.22, 0.75, 0.24, 1.0],
            Self::Spider | Self::CaveSpider => [0.22, 0.18, 0.20, 1.0],
            Self::Enderman => [0.10, 0.08, 0.14, 1.0],
            Self::Ghast => [0.95, 0.95, 0.97, 1.0],
            Self::Blaze => [0.95, 0.68, 0.15, 1.0],
            Self::EnderDragon => [0.14, 0.10, 0.18, 1.0],
            Self::Wither => [0.15, 0.15, 0.17, 1.0],
            Self::PrimedTnt => [0.90, 0.25, 0.20, 1.0],
            Self::EndCrystal => [0.90, 0.30, 0.95, 1.0],
            Self::ExperienceOrb => [0.45, 0.95, 0.25, 1.0],
            _ => [0.80, 0.65, 0.35, 1.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillagerOffer {
    pub wants: ItemStack,
    pub gives: ItemStack,
    pub uses: u32,
    pub max_uses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: u64,
    pub kind: EntityKind,
    pub pos: DVec3,
    pub vel: DVec3,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub age_ticks: u32,
    pub on_ground: bool,
    pub hurt_ticks: u8,
    pub fuse_ticks: i32,
    pub attack_timer: u32,
    pub wander_dir: DVec3,
    pub wander_timer: u32,
    pub item_stack: Option<ItemStack>,
    pub falling_state: Option<BlockState>,
    pub xp_value: u32,
    pub tamed: bool,
    pub owner_projectile_from_player: bool,
    pub villager_offers: Vec<VillagerOffer>,
    pub removed: bool,
}

impl Entity {
    pub fn aabb(&self) -> AABB {
        let (w, h) = self.kind.dimensions();
        AABB::from_feet(self.pos, w, h)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ParticleKind {
    BlockBreak,
    Flame,
    Smoke,
    Heart,
    Crit,
    Portal,
    Explosion,
    Splash,
}

#[derive(Debug, Clone)]
pub struct Particle {
    pub kind: ParticleKind,
    pub pos: DVec3,
    pub vel: DVec3,
    pub color: [f32; 4],
    pub age: u16,
    pub max_age: u16,
    pub size: f32,
}

pub struct EntityManager {
    next_id: u64,
    pub entities: Vec<Entity>,
    pub particles: Vec<Particle>,
    pub dragon_defeated: bool,
}

impl Default for EntityManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            entities: Vec::new(),
            particles: Vec::new(),
            dragon_defeated: false,
        }
    }

    pub fn spawn(&mut self, kind: EntityKind, pos: DVec3, data: &GameData) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let max_health = kind.max_health();
        let mut offers = Vec::new();
        if kind == EntityKind::Villager {
            for t in data.world.villager_trades.iter().take(6) {
                if let (Some(w), Some(g)) = (
                    data.items.by_name(&t.wants_item),
                    data.items.by_name(&t.gives_item),
                ) {
                    offers.push(VillagerOffer {
                        wants: ItemStack::new(w, t.wants_count),
                        gives: ItemStack::new(g, t.gives_count),
                        uses: 0,
                        max_uses: t.max_uses,
                    });
                }
            }
        }
        self.entities.push(Entity {
            id,
            kind,
            pos,
            vel: DVec3::ZERO,
            yaw: 0.0,
            health: max_health,
            max_health,
            age_ticks: 0,
            on_ground: false,
            hurt_ticks: 0,
            fuse_ticks: if kind == EntityKind::PrimedTnt { 80 } else { 0 },
            attack_timer: 0,
            wander_dir: DVec3::ZERO,
            wander_timer: 0,
            item_stack: None,
            falling_state: None,
            xp_value: if kind == EntityKind::ExperienceOrb { 5 } else { 0 },
            tamed: false,
            owner_projectile_from_player: false,
            villager_offers: offers,
            removed: false,
        });
        id
    }

    pub fn spawn_item_drop(&mut self, pos: DVec3, stack: ItemStack, data: &GameData) {
        if stack.is_empty() {
            return;
        }
        let mut rng = rand::thread_rng();
        let id = self.spawn(EntityKind::ItemDrop, pos, data);
        if let Some(e) = self.entities.iter_mut().find(|e| e.id == id) {
            e.item_stack = Some(stack);
            e.vel = DVec3::new(
                (rng.r#gen::<f64>() - 0.5) * 0.15,
                0.2,
                (rng.r#gen::<f64>() - 0.5) * 0.15,
            );
        }
    }

    pub fn spawn_particle(&mut self, kind: ParticleKind, pos: DVec3, vel: DVec3, color: [f32; 4]) {
        if self.particles.len() > 2048 {
            return;
        }
        self.particles.push(Particle {
            kind,
            pos,
            vel,
            color,
            age: 0,
            max_age: 24,
            size: 0.12,
        });
    }

    pub fn raycast_entity(&self, origin: DVec3, dir: DVec3, max_dist: f64) -> Option<(u64, f64)> {
        let mut best: Option<(u64, f64)> = None;
        for e in &self.entities {
            if e.removed || matches!(e.kind, EntityKind::ItemDrop | EntityKind::ExperienceOrb) {
                continue;
            }
            if let Some(t) = e.aabb().inflate(0.1, 0.1, 0.1).ray_intersect(origin, dir, max_dist) {
                if best.is_none_or(|(_, bt)| t < bt) {
                    best = Some((e.id, t));
                }
            }
        }
        best
    }

    pub fn damage_entity(
        &mut self,
        id: u64,
        amount: f32,
        knockback_dir: DVec3,
        world: &mut World,
        data: &GameData,
    ) -> bool {
        let mut died_info: Option<(EntityKind, DVec3)> = None;
        if let Some(e) = self.entities.iter_mut().find(|e| e.id == id && !e.removed) {
            e.health = (e.health - amount).max(0.0);
            e.hurt_ticks = 10;
            e.vel += knockback_dir.normalize_or_zero() * SharedConstants::DEFAULT_KNOCKBACK as f64
                + DVec3::new(0.0, 0.32, 0.0);
            if e.health <= 0.0 {
                e.removed = true;
                died_info = Some((e.kind, e.pos));
            }
        }
        if let Some((kind, pos)) = died_info {
            let mut rng = rand::thread_rng();
            if world.game_rules.do_mob_loot {
                for drop in data.loot.roll_entity(kind.registry_name(), &LootContext::default(), &mut rng) {
                    self.spawn_item_drop(pos + DVec3::new(0.0, 0.4, 0.0), drop, data);
                }
            }
            if kind == EntityKind::Wither {
                if let Some(star) = data.items.by_name("nether_star") {
                    self.spawn_item_drop(pos, ItemStack::new(star, 1), data);
                }
            } else if kind == EntityKind::EnderDragon {
                self.dragon_defeated = true;
                if let Some(egg) = data.items.by_name("dragon_egg") {
                    self.spawn_item_drop(DVec3::new(0.5, 68.0, 0.5), ItemStack::new(egg, 1), data);
                }
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        world.set_block(
                            BlockPos::new(dx, 64, dz),
                            BlockState::new(data.blocks.end_portal),
                            &data.blocks,
                        );
                    }
                }
            }
            let xp_id = self.spawn(EntityKind::ExperienceOrb, pos, data);
            if let Some(xp) = self.entities.iter_mut().find(|e| e.id == xp_id) {
                xp.xp_value = if matches!(kind, EntityKind::EnderDragon | EntityKind::Wither) {
                    500
                } else if kind.is_hostile() {
                    5
                } else {
                    2
                };
            }
            return true;
        }
        false
    }

    pub fn tick(&mut self, world: &mut World, player: &mut Player, data: &GameData) {
        let mut rng = rand::thread_rng();

        // Drain world-generated item drops, TNT, and falling blocks
        for (pos, stack) in world.pending_drops.drain(..) {
            self.spawn_item_drop(pos, stack, data);
        }
        for tnt_pos in world.pending_tnt.drain(..) {
            self.spawn(EntityKind::PrimedTnt, tnt_pos.center(), data);
        }
        let pending_falling: Vec<_> = world.pending_falling.drain(..).collect();
        for fb in pending_falling {
            let cur = world.get_block(fb.pos);
            if !cur.is_air() {
                world.set_block_internal(fb.pos, BlockState::AIR, &data.blocks);
                let id = self.spawn(
                    EntityKind::FallingBlock,
                    DVec3::new(fb.pos.x as f64 + 0.5, fb.pos.y as f64, fb.pos.z as f64 + 0.5),
                    data,
                );
                if let Some(e) = self.entities.iter_mut().find(|e| e.id == id) {
                    e.falling_state = Some(fb.state);
                }
            }
        }

        // Natural mob spawning around player
        if world.game_rules.do_mob_spawning && world.world_tick % 40 == 0 && self.entities.len() < 48 {
            let angle = rng.r#gen::<f64>() * std::f64::consts::TAU;
            let dist = 18.0 + rng.r#gen::<f64>() * 20.0;
            let sx = (player.pos.x + angle.cos() * dist).round() as i32;
            let sz = (player.pos.z + angle.sin() * dist).round() as i32;
            let cp = ChunkPos::new(sx.div_euclid(16), sz.div_euclid(16));
            if let Some(chunk) = world.chunks.get(&cp) {
                let top_y = chunk.heightmap[sz.rem_euclid(16) as usize * 16 + sx.rem_euclid(16) as usize] as i32;
                let spawn_pos = DVec3::new(sx as f64 + 0.5, (top_y + 1) as f64, sz as f64 + 0.5);
                let night = world.day_time > 13000 && world.day_time < 23000;
                let kind = match world.dimension {
                    DimensionId::TheNether => {
                        let opts = [EntityKind::Piglin, EntityKind::ZombifiedPiglin, EntityKind::Blaze, EntityKind::Ghast, EntityKind::WitherSkeleton];
                        opts[rng.gen_range(0..opts.len())]
                    }
                    DimensionId::TheEnd => EntityKind::Enderman,
                    DimensionId::Overworld => {
                        if night && world.difficulty != Difficulty::Peaceful {
                            let opts = [EntityKind::Zombie, EntityKind::Skeleton, EntityKind::Creeper, EntityKind::Spider, EntityKind::Enderman];
                            opts[rng.gen_range(0..opts.len())]
                        } else {
                            let opts = [EntityKind::Pig, EntityKind::Cow, EntityKind::Sheep, EntityKind::Chicken];
                            opts[rng.gen_range(0..opts.len())]
                        }
                    }
                };
                if !kind.is_hostile() || world.difficulty != Difficulty::Peaceful {
                    self.spawn(kind, spawn_pos, data);
                }
            }
        }

        let mut explosions = Vec::new();
        let mut player_damage = 0.0f32;
        let mut spawns_to_add: Vec<(EntityKind, DVec3, DVec3, bool)> = Vec::new();

        for i in 0..self.entities.len() {
            if self.entities[i].removed {
                continue;
            }
            self.entities[i].age_ticks += 1;
            if self.entities[i].hurt_ticks > 0 {
                self.entities[i].hurt_ticks -= 1;
            }
            if self.entities[i].attack_timer > 0 {
                self.entities[i].attack_timer -= 1;
            }

            let kind = self.entities[i].kind;
            let pos = self.entities[i].pos;

            if kind == EntityKind::ItemDrop {
                if self.entities[i].age_ticks > 6000 {
                    self.entities[i].removed = true;
                    continue;
                }
                if self.entities[i].age_ticks > 10 && pos.distance(player.pos) < 1.6 && !player.dead {
                    if let Some(st) = self.entities[i].item_stack.take() {
                        if player.give_item(st.clone(), data) {
                            self.entities[i].removed = true;
                            continue;
                        } else {
                            self.entities[i].item_stack = Some(st);
                        }
                    }
                }
            } else if kind == EntityKind::ExperienceOrb {
                let d = pos.distance(player.pos);
                if d < 6.0 && !player.dead {
                    let pull = (player.pos + DVec3::new(0.0, 0.5, 0.0) - pos).normalize_or_zero() * 0.25;
                    self.entities[i].vel += pull;
                }
                if d < 1.4 && !player.dead {
                    player.add_xp(self.entities[i].xp_value);
                    self.entities[i].removed = true;
                    continue;
                }
            } else if kind == EntityKind::PrimedTnt {
                self.entities[i].fuse_ticks -= 1;
                if self.entities[i].fuse_ticks <= 0 {
                    self.entities[i].removed = true;
                    explosions.push((pos, 4.0f32, false));
                    continue;
                }
            } else if kind == EntityKind::Arrow || kind == EntityKind::Fireball || kind == EntityKind::Snowball || kind == EntityKind::EnderPearl {
                self.entities[i].vel.y -= if kind == EntityKind::Fireball { 0.0 } else { 0.05 };
                let next_pos = pos + self.entities[i].vel;
                let bp = BlockPos::from_vec3(next_pos);
                let hit_block = data.blocks.get(world.get_block(bp).id()).is_solid;
                let hit_player = !self.entities[i].owner_projectile_from_player
                    && next_pos.distance(player.pos + DVec3::new(0.0, 0.9, 0.0)) < 0.8;
                if hit_block || hit_player || self.entities[i].age_ticks > 200 {
                    self.entities[i].removed = true;
                    if kind == EntityKind::Fireball {
                        explosions.push((pos, 1.5, true));
                    } else if kind == EntityKind::EnderPearl && self.entities[i].owner_projectile_from_player {
                        player.pos = pos;
                        player.fall_distance = 0.0;
                        player.damage(5.0, true, data);
                    } else if hit_player {
                        player_damage += 4.0;
                    }
                    continue;
                }
                self.entities[i].pos = next_pos;
                continue;
            } else if kind == EntityKind::EnderDragon {
                // Orbit around (0, 82, 0) & fire dragon fireballs
                let t = self.entities[i].age_ticks as f64 * 0.03;
                let target = DVec3::new(t.cos() * 34.0, 82.0 + (t * 2.0).sin() * 6.0, t.sin() * 34.0);
                let dir = (target - pos).normalize_or_zero();
                let v = dir * 0.45;
                self.entities[i].vel = v;
                self.entities[i].pos += v;
                self.entities[i].yaw = dir.x.atan2(dir.z) as f32;
                continue;
            } else if kind == EntityKind::Wither || kind == EntityKind::Ghast || kind == EntityKind::Blaze {
                let d_player = pos.distance(player.pos);
                if d_player < 36.0 && !player.dead && player.game_mode.is_survival_or_adventure() {
                    let hover = DVec3::new(player.pos.x, player.pos.y + 6.0, player.pos.z);
                    let dir = (hover - pos).normalize_or_zero();
                    let nv = self.entities[i].vel * 0.85 + dir * 0.05;
                    self.entities[i].vel = nv;
                    if self.entities[i].attack_timer == 0 {
                        self.entities[i].attack_timer = 50;
                        let aim = (player.eye_pos() - pos).normalize_or_zero() * 0.7;
                        spawns_to_add.push((EntityKind::Fireball, pos + aim * 1.5, aim, false));
                    }
                }
                let nv = self.entities[i].vel;
                self.entities[i].pos += nv;
                continue;
            } else {
                // Ground Mob AI (hostile chase + Creeper fuse + Skeleton bow + passive wander)
                let dist = pos.distance(player.pos);
                let target_player = kind.is_hostile()
                    && world.difficulty != Difficulty::Peaceful
                    && player.game_mode.is_survival_or_adventure()
                    && !player.dead
                    && dist < 24.0;

                if target_player {
                    let to_p = DVec3::new(player.pos.x - pos.x, 0.0, player.pos.z - pos.z).normalize_or_zero();
                    self.entities[i].yaw = (-to_p.x).atan2(to_p.z) as f32;

                    if kind == EntityKind::Creeper {
                        if dist < 3.0 {
                            self.entities[i].fuse_ticks += 1;
                            if self.entities[i].fuse_ticks >= 30 {
                                self.entities[i].removed = true;
                                explosions.push((pos, 3.0, false));
                                continue;
                            }
                        } else {
                            self.entities[i].fuse_ticks = (self.entities[i].fuse_ticks - 1).max(0);
                            self.entities[i].vel.x = to_p.x * 0.11;
                            self.entities[i].vel.z = to_p.z * 0.11;
                        }
                    } else if kind == EntityKind::Skeleton || kind == EntityKind::Stray || kind == EntityKind::Pillager {
                        if dist > 10.0 {
                            self.entities[i].vel.x = to_p.x * 0.11;
                            self.entities[i].vel.z = to_p.z * 0.11;
                        } else if self.entities[i].attack_timer == 0 {
                            self.entities[i].attack_timer = 40;
                            let aim = (player.eye_pos() - (pos + DVec3::new(0.0, 1.5, 0.0))).normalize_or_zero() * 0.9;
                            spawns_to_add.push((EntityKind::Arrow, pos + DVec3::new(0.0, 1.5, 0.0), aim, false));
                        }
                    } else {
                        self.entities[i].vel.x = to_p.x * 0.13;
                        self.entities[i].vel.z = to_p.z * 0.13;
                        if dist < 1.6 && self.entities[i].attack_timer == 0 {
                            self.entities[i].attack_timer = 20;
                            player_damage += match world.difficulty {
                                Difficulty::Easy => 2.0,
                                Difficulty::Normal => 3.0,
                                Difficulty::Hard => 4.5,
                                Difficulty::Peaceful => 0.0,
                            };
                        }
                    }
                } else {
                    if self.entities[i].wander_timer == 0 {
                        self.entities[i].wander_timer = rng.gen_range(40..120);
                        if rng.r#gen::<f32>() < 0.6 {
                            let ang = rng.r#gen::<f64>() * std::f64::consts::TAU;
                            self.entities[i].wander_dir = DVec3::new(ang.cos(), 0.0, ang.sin());
                            self.entities[i].yaw = (-ang.cos()).atan2(ang.sin()) as f32;
                        } else {
                            self.entities[i].wander_dir = DVec3::ZERO;
                        }
                    } else {
                        self.entities[i].wander_timer -= 1;
                    }
                    self.entities[i].vel.x = self.entities[i].wander_dir.x * 0.07;
                    self.entities[i].vel.z = self.entities[i].wander_dir.z * 0.07;
                }
            }

            // Gravity & block collision
            self.entities[i].vel.y =
                (self.entities[i].vel.y - SharedConstants::GRAVITY) * SharedConstants::INPUT_FRICTION as f64;
            let aabb = self.entities[i].aabb();
            let (actual, grounded, horiz_hit) =
                world.move_aabb(aabb, self.entities[i].vel, 0.6, &data.blocks);
            self.entities[i].pos += actual;
            self.entities[i].on_ground = grounded;
            if grounded {
                self.entities[i].vel.y = 0.0;
                if kind == EntityKind::FallingBlock {
                    if let Some(st) = self.entities[i].falling_state.take() {
                        world.set_block(BlockPos::from_vec3(self.entities[i].pos), st, &data.blocks);
                    }
                    self.entities[i].removed = true;
                    continue;
                }
            }
            if horiz_hit && grounded {
                self.entities[i].vel.y = 0.42;
            }
        }

        if player_damage > 0.0 {
            player.damage(player_damage, false, data);
        }
        for (center, power, fire) in explosions {
            world.explode(center, power, fire, data);
            if center.distance(player.pos) < (power as f64 * 2.0) {
                let falloff = (1.0 - center.distance(player.pos) / (power as f64 * 2.0)) as f32;
                player.damage(falloff * power * 5.0, false, data);
            }
        }
        for (kind, pos, vel, from_player) in spawns_to_add {
            let id = self.spawn(kind, pos, data);
            if let Some(e) = self.entities.iter_mut().find(|e| e.id == id) {
                e.vel = vel;
                e.owner_projectile_from_player = from_player;
            }
        }

        // Tick particles
        for p in &mut self.particles {
            p.age += 1;
            p.pos += p.vel;
            p.vel.y -= 0.015;
        }
        self.particles.retain(|p| p.age < p.max_age);
        self.entities.retain(|e| !e.removed);
    }
}
