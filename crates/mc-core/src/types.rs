use serde::{Deserialize, Serialize};
use std::fmt;

/// Constants directly extracted from Minecraft Java 26.1.2 (`SharedConstants`, `LivingEntity`, `Player`, `dimension_type`).
pub struct SharedConstants;

impl SharedConstants {
    pub const VERSION_NAME: &'static str = "26.1.2";
    pub const WORLD_VERSION: i32 = 4790;
    pub const PROTOCOL_VERSION: i32 = 775;
    pub const RESOURCE_PACK_FORMAT_MAJOR: i32 = 84;
    pub const DATA_PACK_FORMAT_MAJOR: i32 = 101;

    pub const TICKS_PER_SECOND: u32 = 20;
    pub const MS_PER_TICK: f64 = 50.0;
    pub const DAY_TICKS: i64 = 24_000;

    pub const OVERWORLD_MIN_Y: i32 = -64;
    pub const OVERWORLD_HEIGHT: i32 = 384;
    pub const OVERWORLD_MAX_Y: i32 = 319;
    pub const SEA_LEVEL: i32 = 63;

    pub const NETHER_MIN_Y: i32 = 0;
    pub const NETHER_HEIGHT: i32 = 256;
    pub const NETHER_LOGICAL_HEIGHT: i32 = 128;

    pub const END_MIN_Y: i32 = 0;
    pub const END_HEIGHT: i32 = 256;

    // Physics constants verified via javap on LivingEntity & Player
    pub const GRAVITY: f64 = 0.08;
    pub const BASE_JUMP_POWER: f64 = 0.42;
    pub const INPUT_FRICTION: f64 = 0.98;
    pub const WATER_FLOAT_IMPULSE: f64 = 0.04;
    pub const DEFAULT_KNOCKBACK: f64 = 0.4;
    pub const INVULNERABLE_DURATION: i32 = 20;
    pub const DEATH_DURATION: i32 = 20;
    pub const BLOCK_INTERACTION_RANGE: f64 = 4.5;
    pub const ENTITY_INTERACTION_RANGE: f64 = 3.0;
    pub const PLAYER_FLY_SPEED: f64 = 0.05;
    pub const PLAYER_WIDTH: f64 = 0.6;
    pub const PLAYER_HEIGHT: f64 = 1.8;
    pub const PLAYER_EYE_HEIGHT: f64 = 1.62;
    pub const PLAYER_CROUCH_HEIGHT: f64 = 1.5;
    pub const PLAYER_CROUCH_EYE_HEIGHT: f64 = 1.27;
    pub const PISTON_MAX_PUSH_DEPTH: usize = 12;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Identifier {
    pub namespace: String,
    pub path: String,
}

impl Identifier {
    pub fn new(namespace: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            path: path.into(),
        }
    }

    pub fn mc(path: impl Into<String>) -> Self {
        Self {
            namespace: "minecraft".to_string(),
            path: path.into(),
        }
    }

    pub fn parse(s: &str) -> Self {
        if let Some((ns, p)) = s.split_once(':') {
            Self {
                namespace: ns.to_string(),
                path: p.to_string(),
            }
        } else {
            Self::mc(s)
        }
    }

    pub fn short_name(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

impl GameMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "survival" | "s" | "0" => Some(Self::Survival),
            "creative" | "c" | "1" => Some(Self::Creative),
            "adventure" | "a" | "2" => Some(Self::Adventure),
            "spectator" | "sp" | "3" => Some(Self::Spectator),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
            Self::Spectator => "spectator",
        }
    }

    pub fn is_survival_or_adventure(self) -> bool {
        matches!(self, Self::Survival | Self::Adventure)
    }

    pub fn can_fly(self) -> bool {
        matches!(self, Self::Creative | Self::Spectator)
    }

    pub fn insta_build(self) -> bool {
        matches!(self, Self::Creative)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Difficulty {
    Peaceful = 0,
    Easy = 1,
    Normal = 2,
    Hard = 3,
}

impl Difficulty {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "peaceful" | "p" | "0" => Some(Self::Peaceful),
            "easy" | "e" | "1" => Some(Self::Easy),
            "normal" | "n" | "2" => Some(Self::Normal),
            "hard" | "h" | "3" => Some(Self::Hard),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DimensionId {
    Overworld,
    TheNether,
    TheEnd,
}

impl DimensionId {
    pub fn min_y(self) -> i32 {
        match self {
            Self::Overworld => SharedConstants::OVERWORLD_MIN_Y,
            Self::TheNether => SharedConstants::NETHER_MIN_Y,
            Self::TheEnd => SharedConstants::END_MIN_Y,
        }
    }

    pub fn height(self) -> i32 {
        match self {
            Self::Overworld => SharedConstants::OVERWORLD_HEIGHT,
            Self::TheNether => SharedConstants::NETHER_HEIGHT,
            Self::TheEnd => SharedConstants::END_HEIGHT,
        }
    }

    pub fn max_y(self) -> i32 {
        self.min_y() + self.height() - 1
    }

    pub fn has_skylight(self) -> bool {
        matches!(self, Self::Overworld | Self::TheEnd)
    }

    pub fn has_ceiling(self) -> bool {
        matches!(self, Self::TheNether)
    }

    pub fn water_evaporates(self) -> bool {
        matches!(self, Self::TheNether)
    }

    pub fn fast_lava(self) -> bool {
        matches!(self, Self::TheNether)
    }

    pub fn bed_explodes(self) -> bool {
        matches!(self, Self::TheNether | Self::TheEnd)
    }

    pub fn id_str(self) -> &'static str {
        match self {
            Self::Overworld => "minecraft:overworld",
            Self::TheNether => "minecraft:the_nether",
            Self::TheEnd => "minecraft:the_end",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRules {
    pub do_daylight_cycle: bool,
    pub do_weather_cycle: bool,
    pub do_mob_spawning: bool,
    pub do_fire_tick: bool,
    pub do_tile_drops: bool,
    pub do_mob_loot: bool,
    pub keep_inventory: bool,
    pub mob_griefing: bool,
    pub natural_regeneration: bool,
    pub random_tick_speed: u32,
    pub fall_damage: bool,
    pub fire_damage: bool,
    pub drowning_damage: bool,
    pub pvp: bool,
    pub show_coordinates: bool,
}

impl Default for GameRules {
    fn default() -> Self {
        Self {
            do_daylight_cycle: true,
            do_weather_cycle: true,
            do_mob_spawning: true,
            do_fire_tick: true,
            do_tile_drops: true,
            do_mob_loot: true,
            keep_inventory: false,
            mob_griefing: true,
            natural_regeneration: true,
            random_tick_speed: 3,
            fall_damage: true,
            fire_damage: true,
            drowning_damage: true,
            pvp: true,
            show_coordinates: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::pos::{BlockPos, Direction};
    use crate::aabb::AABB;
    use crate::random::{JavaRandom, Xoroshiro128pp};
    use glam::DVec3;

    #[test]
    fn block_pos_packing_roundtrip() {
        for p in [
            BlockPos::new(0, 64, 0),
            BlockPos::new(-12345, -64, 98765),
            BlockPos::new(100000, 319, -100000),
        ] {
            assert_eq!(BlockPos::from_long(p.as_long()), p);
        }
    }

    #[test]
    fn direction_opposites_and_rotations() {
        for d in Direction::ALL {
            assert_eq!(d.opposite().opposite(), d);
        }
        assert_eq!(Direction::North.clock_wise(), Direction::East);
        assert_eq!(Direction::North.counter_clock_wise(), Direction::West);
    }

    #[test]
    fn aabb_collision_and_raycast() {
        let player = AABB::from_feet(DVec3::new(0.5, 64.0, 0.5), 0.6, 1.8);
        let floor = AABB::unit_block(0, 63, 0);
        assert_eq!(player.clip_y_collide(floor, -0.5), 0.0);
        let block = AABB::unit_block(2, 64, 0);
        assert!(block.ray_intersect(DVec3::new(0.5, 64.5, 0.5), DVec3::X, 5.0).is_some());
    }

    #[test]
    fn rng_determinism() {
        let mut r1 = JavaRandom::new(123456789);
        let mut r2 = JavaRandom::new(123456789);
        for _ in 0..10 {
            assert_eq!(r1.next_i64(), r2.next_i64());
        }
        let mut x1 = Xoroshiro128pp::from_seed(42);
        let mut x2 = Xoroshiro128pp::from_seed(42);
        for _ in 0..10 {
            assert_eq!(x1.next_u64(), x2.next_u64());
        }
    }
}
