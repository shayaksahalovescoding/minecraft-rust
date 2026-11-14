use mc_core::JavaRandom;

/// Improved Perlin noise matching Minecraft's `ImprovedNoise`.
#[derive(Debug, Clone)]
pub struct PerlinNoise {
    perm: [u8; 512],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
}

impl PerlinNoise {
    pub fn new(rng: &mut JavaRandom) -> Self {
        let xo = rng.next_f64() * 256.0;
        let yo = rng.next_f64() * 256.0;
        let zo = rng.next_f64() * 256.0;
        let mut p = [0u8; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        for i in 0..256 {
            let j = i + rng.next_int_bounded((256 - i) as i32) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Self { perm, xo, yo, zo }
    }

    #[inline]
    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    #[inline]
    fn lerp(t: f64, a: f64, b: f64) -> f64 {
        a + t * (b - a)
    }

    #[inline]
    fn grad(hash: u8, x: f64, y: f64, z: f64) -> f64 {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 {
            y
        } else if h == 12 || h == 14 {
            x
        } else {
            z
        };
        (if (h & 1) == 0 { u } else { -u }) + (if (h & 2) == 0 { v } else { -v })
    }

    pub fn noise3d(&self, x: f64, y: f64, z: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;

        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let zi = z.floor() as i32;

        let dx = x - xi as f64;
        let dy = y - yi as f64;
        let dz = z - zi as f64;

        let u = Self::fade(dx);
        let v = Self::fade(dy);
        let w = Self::fade(dz);

        let x0 = (xi & 255) as usize;
        let y0 = (yi & 255) as usize;
        let z0 = (zi & 255) as usize;

        let a = self.perm[x0] as usize + y0;
        let aa = self.perm[a] as usize + z0;
        let ab = self.perm[a + 1] as usize + z0;
        let b = self.perm[x0 + 1] as usize + y0;
        let ba = self.perm[b] as usize + z0;
        let bb = self.perm[b + 1] as usize + z0;

        Self::lerp(
            w,
            Self::lerp(
                v,
                Self::lerp(
                    u,
                    Self::grad(self.perm[aa], dx, dy, dz),
                    Self::grad(self.perm[ba], dx - 1.0, dy, dz),
                ),
                Self::lerp(
                    u,
                    Self::grad(self.perm[ab], dx, dy - 1.0, dz),
                    Self::grad(self.perm[bb], dx - 1.0, dy - 1.0, dz),
                ),
            ),
            Self::lerp(
                v,
                Self::lerp(
                    u,
                    Self::grad(self.perm[aa + 1], dx, dy, dz - 1.0),
                    Self::grad(self.perm[ba + 1], dx - 1.0, dy, dz - 1.0),
                ),
                Self::lerp(
                    u,
                    Self::grad(self.perm[ab + 1], dx, dy - 1.0, dz - 1.0),
                    Self::grad(self.perm[bb + 1], dx - 1.0, dy - 1.0, dz - 1.0),
                ),
            ),
        )
    }

    #[inline]
    pub fn noise2d(&self, x: f64, z: f64) -> f64 {
        self.noise3d(x, 0.0, z)
    }
}

#[derive(Debug, Clone)]
pub struct OctaveNoise {
    octaves: Vec<PerlinNoise>,
    amplitudes: Vec<f64>,
    max_amp: f64,
}

impl OctaveNoise {
    pub fn new(seed: i64, num_octaves: usize) -> Self {
        let mut rng = JavaRandom::new(seed);
        let mut octaves = Vec::with_capacity(num_octaves);
        let mut amplitudes = Vec::with_capacity(num_octaves);
        let mut amp = 1.0;
        let mut max_amp = 0.0;
        for _ in 0..num_octaves {
            octaves.push(PerlinNoise::new(&mut rng));
            amplitudes.push(amp);
            max_amp += amp;
            amp *= 0.5;
        }
        Self {
            octaves,
            amplitudes,
            max_amp: max_amp.max(1e-6),
        }
    }

    pub fn sample2d(&self, x: f64, z: f64, base_freq: f64) -> f64 {
        let mut sum = 0.0;
        let mut freq = base_freq;
        for (oct, &amp) in self.octaves.iter().zip(&self.amplitudes) {
            sum += oct.noise2d(x * freq, z * freq) * amp;
            freq *= 2.0;
        }
        sum / self.max_amp
    }

    pub fn sample3d(&self, x: f64, y: f64, z: f64, base_freq: f64, y_scale: f64) -> f64 {
        let mut sum = 0.0;
        let mut freq = base_freq;
        for (oct, &amp) in self.octaves.iter().zip(&self.amplitudes) {
            sum += oct.noise3d(x * freq, y * freq * y_scale, z * freq) * amp;
            freq *= 2.0;
        }
        sum / self.max_amp
    }
}

#[inline]
pub fn hash_3d(x: i32, y: i32, z: i32, seed: i64) -> u64 {
    let mut h = (seed as u64)
        ^ (x as u64).wrapping_mul(0x9E3779B97F4A7C15)
        ^ (y as u64).wrapping_mul(0xBF58476D1CE4E5B9)
        ^ (z as u64).wrapping_mul(0x94D049BB133111EB);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D049BB133111EB);
    h ^ (h >> 31)
}
