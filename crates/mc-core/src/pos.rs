use glam::{DVec3, IVec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Direction {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    pub const HORIZONTAL: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(idx: u8) -> Self {
        match idx % 6 {
            0 => Direction::Down,
            1 => Direction::Up,
            2 => Direction::North,
            3 => Direction::South,
            4 => Direction::West,
            _ => Direction::East,
        }
    }

    #[inline]
    pub const fn opposite(self) -> Self {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    #[inline]
    pub const fn axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }

    #[inline]
    pub const fn normal(self) -> IVec3 {
        match self {
            Direction::Down => IVec3::new(0, -1, 0),
            Direction::Up => IVec3::new(0, 1, 0),
            Direction::North => IVec3::new(0, 0, -1),
            Direction::South => IVec3::new(0, 0, 1),
            Direction::West => IVec3::new(-1, 0, 0),
            Direction::East => IVec3::new(1, 0, 0),
        }
    }

    #[inline]
    pub const fn step_x(self) -> i32 {
        match self {
            Direction::West => -1,
            Direction::East => 1,
            _ => 0,
        }
    }

    #[inline]
    pub const fn step_y(self) -> i32 {
        match self {
            Direction::Down => -1,
            Direction::Up => 1,
            _ => 0,
        }
    }

    #[inline]
    pub const fn step_z(self) -> i32 {
        match self {
            Direction::North => -1,
            Direction::South => 1,
            _ => 0,
        }
    }

    #[inline]
    pub const fn clock_wise(self) -> Self {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
            other => other,
        }
    }

    #[inline]
    pub const fn counter_clock_wise(self) -> Self {
        match self {
            Direction::North => Direction::West,
            Direction::West => Direction::South,
            Direction::South => Direction::East,
            Direction::East => Direction::North,
            other => other,
        }
    }

    pub fn from_yaw(yaw_rad: f64) -> Self {
        let deg = yaw_rad.to_degrees().rem_euclid(360.0);
        if !(45.0..315.0).contains(&deg) {
            Direction::South
        } else if deg < 135.0 {
            Direction::West
        } else if deg < 225.0 {
            Direction::North
        } else {
            Direction::East
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::North => "north",
            Direction::South => "south",
            Direction::West => "west",
            Direction::East => "east",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, PartialOrd, Ord)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn from_vec3(v: DVec3) -> Self {
        Self {
            x: v.x.floor() as i32,
            y: v.y.floor() as i32,
            z: v.z.floor() as i32,
        }
    }

    #[inline]
    pub fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            z: self.z + dz,
        }
    }

    #[inline]
    pub fn relative(self, dir: Direction) -> Self {
        let n = dir.normal();
        Self {
            x: self.x + n.x,
            y: self.y + n.y,
            z: self.z + n.z,
        }
    }

    #[inline]
    pub fn relative_n(self, dir: Direction, dist: i32) -> Self {
        let n = dir.normal();
        Self {
            x: self.x + n.x * dist,
            y: self.y + n.y * dist,
            z: self.z + n.z * dist,
        }
    }

    #[inline]
    pub fn chunk_pos(self) -> ChunkPos {
        ChunkPos {
            x: self.x.div_euclid(16),
            z: self.z.div_euclid(16),
        }
    }

    #[inline]
    pub fn section_y(self) -> i32 {
        self.y.div_euclid(16)
    }

    #[inline]
    pub fn local_x(self) -> usize {
        self.x.rem_euclid(16) as usize
    }

    #[inline]
    pub fn local_y(self) -> usize {
        self.y.rem_euclid(16) as usize
    }

    #[inline]
    pub fn local_z(self) -> usize {
        self.z.rem_euclid(16) as usize
    }

    #[inline]
    pub fn center(self) -> DVec3 {
        DVec3::new(self.x as f64 + 0.5, self.y as f64 + 0.5, self.z as f64 + 0.5)
    }

    #[inline]
    pub fn dist_sq(self, other: Self) -> i64 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        let dz = (self.z - other.z) as i64;
        dx * dx + dy * dy + dz * dz
    }

    /// Pack into Minecraft 64-bit BlockPos long (26-bit X, 12-bit Y, 26-bit Z).
    #[inline]
    pub const fn as_long(self) -> i64 {
        let x = (self.x as i64) & 0x3FF_FFFF;
        let z = (self.z as i64) & 0x3FF_FFFF;
        let y = (self.y as i64) & 0xFFF;
        (x << 38) | (z << 12) | y
    }

    #[inline]
    pub const fn from_long(v: i64) -> Self {
        let x = (v >> 38) as i32;
        let y = ((v << 52) >> 52) as i32;
        let z = ((v << 26) >> 38) as i32;
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    #[inline]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    #[inline]
    pub const fn min_block_x(self) -> i32 {
        self.x * 16
    }

    #[inline]
    pub const fn min_block_z(self) -> i32 {
        self.z * 16
    }

    #[inline]
    pub const fn region_x(self) -> i32 {
        self.x.div_euclid(32)
    }

    #[inline]
    pub const fn region_z(self) -> i32 {
        self.z.div_euclid(32)
    }

    #[inline]
    pub const fn region_local_x(self) -> usize {
        self.x.rem_euclid(32) as usize
    }

    #[inline]
    pub const fn region_local_z(self) -> usize {
        self.z.rem_euclid(32) as usize
    }

    #[inline]
    pub const fn as_long(self) -> i64 {
        ((self.x as i64) & 0xFFFF_FFFF) | (((self.z as i64) & 0xFFFF_FFFF) << 32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SectionPos {
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn from_block(pos: BlockPos) -> Self {
        Self {
            x: pos.x.div_euclid(16),
            y: pos.y.div_euclid(16),
            z: pos.z.div_euclid(16),
        }
    }
}
