use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AABB {
    pub min: DVec3,
    pub max: DVec3,
}

impl AABB {
    #[inline]
    pub const fn new(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> Self {
        Self {
            min: DVec3::new(min_x, min_y, min_z),
            max: DVec3::new(max_x, max_y, max_z),
        }
    }

    #[inline]
    pub fn unit_block(x: i32, y: i32, z: i32) -> Self {
        Self::new(
            x as f64,
            y as f64,
            z as f64,
            (x + 1) as f64,
            (y + 1) as f64,
            (z + 1) as f64,
        )
    }

    #[inline]
    pub fn from_feet(pos: DVec3, width: f64, height: f64) -> Self {
        let half = width * 0.5;
        Self {
            min: DVec3::new(pos.x - half, pos.y, pos.z - half),
            max: DVec3::new(pos.x + half, pos.y + height, pos.z + half),
        }
    }

    #[inline]
    pub fn move_by(self, dx: f64, dy: f64, dz: f64) -> Self {
        let delta = DVec3::new(dx, dy, dz);
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    #[inline]
    pub fn inflate(self, dx: f64, dy: f64, dz: f64) -> Self {
        let d = DVec3::new(dx, dy, dz);
        Self {
            min: self.min - d,
            max: self.max + d,
        }
    }

    #[inline]
    pub fn expand_towards(self, dx: f64, dy: f64, dz: f64) -> Self {
        let mut min = self.min;
        let mut max = self.max;
        if dx < 0.0 {
            min.x += dx;
        } else {
            max.x += dx;
        }
        if dy < 0.0 {
            min.y += dy;
        } else {
            max.y += dy;
        }
        if dz < 0.0 {
            min.z += dz;
        } else {
            max.z += dz;
        }
        Self { min, max }
    }

    #[inline]
    pub fn intersects(self, other: Self) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }

    #[inline]
    pub fn contains_point(self, p: DVec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    pub fn clip_y_collide(self, block: Self, mut dy: f64) -> f64 {
        if self.max.x <= block.min.x || self.min.x >= block.max.x {
            return dy;
        }
        if self.max.z <= block.min.z || self.min.z >= block.max.z {
            return dy;
        }
        if dy > 0.0 && self.max.y <= block.min.y + 1e-7 {
            let max_dy = block.min.y - self.max.y;
            if max_dy < dy {
                dy = max_dy;
            }
        } else if dy < 0.0 && self.min.y >= block.max.y - 1e-7 {
            let max_dy = block.max.y - self.min.y;
            if max_dy > dy {
                dy = max_dy;
            }
        }
        dy
    }

    pub fn clip_x_collide(self, block: Self, mut dx: f64) -> f64 {
        if self.max.y <= block.min.y || self.min.y >= block.max.y {
            return dx;
        }
        if self.max.z <= block.min.z || self.min.z >= block.max.z {
            return dx;
        }
        if dx > 0.0 && self.max.x <= block.min.x + 1e-7 {
            let max_dx = block.min.x - self.max.x;
            if max_dx < dx {
                dx = max_dx;
            }
        } else if dx < 0.0 && self.min.x >= block.max.x - 1e-7 {
            let max_dx = block.max.x - self.min.x;
            if max_dx > dx {
                dx = max_dx;
            }
        }
        dx
    }

    pub fn clip_z_collide(self, block: Self, mut dz: f64) -> f64 {
        if self.max.x <= block.min.x || self.min.x >= block.max.x {
            return dz;
        }
        if self.max.y <= block.min.y || self.min.y >= block.max.y {
            return dz;
        }
        if dz > 0.0 && self.max.z <= block.min.z + 1e-7 {
            let max_dz = block.min.z - self.max.z;
            if max_dz < dz {
                dz = max_dz;
            }
        } else if dz < 0.0 && self.min.z >= block.max.z - 1e-7 {
            let max_dz = block.max.z - self.min.z;
            if max_dz > dz {
                dz = max_dz;
            }
        }
        dz
    }

    pub fn ray_intersect(self, origin: DVec3, dir: DVec3, max_dist: f64) -> Option<f64> {
        let mut tmin = 0.0f64;
        let mut tmax = max_dist;
        for i in 0..3 {
            let o = origin[i];
            let d = dir[i];
            let bmin = self.min[i];
            let bmax = self.max[i];
            if d.abs() < 1e-12 {
                if o < bmin || o > bmax {
                    return None;
                }
            } else {
                let inv = 1.0 / d;
                let mut t1 = (bmin - o) * inv;
                let mut t2 = (bmax - o) * inv;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                tmin = tmin.max(t1);
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some(tmin)
    }
}
