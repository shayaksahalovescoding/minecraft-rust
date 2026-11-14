/// Java `LegacyRandomSource` (48-bit linear congruential generator).
#[derive(Debug, Clone)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULTIPLIER: u64 = 0x5DEECE66D;
    const ADDEND: u64 = 0xB;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self {
            seed: ((seed as u64) ^ Self::MULTIPLIER) & Self::MASK,
        }
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = ((seed as u64) ^ Self::MULTIPLIER) & Self::MASK;
    }

    #[inline]
    pub fn next_bits(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND)
            & Self::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    #[inline]
    pub fn next_i32(&mut self) -> i32 {
        self.next_bits(32)
    }

    pub fn next_int_bounded(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if (bound & -bound) == bound {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }
        loop {
            let bits = self.next_bits(31);
            let val = bits % bound;
            if bits - val + (bound - 1) >= 0 {
                return val;
            }
        }
    }

    #[inline]
    pub fn next_i64(&mut self) -> i64 {
        ((self.next_bits(32) as i64) << 32) + (self.next_bits(32) as i64)
    }

    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.next_bits(24) as f32 / ((1u32 << 24) as f32)
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        let high = (self.next_bits(26) as i64) << 27;
        let low = self.next_bits(27) as i64;
        (high + low) as f64 / ((1u64 << 53) as f64)
    }

    #[inline]
    pub fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }
}

/// Xoroshiro128++ matching Minecraft modern `XoroshiroRandomSource`.
#[derive(Debug, Clone)]
pub struct Xoroshiro128pp {
    lo: u64,
    hi: u64,
}

impl Xoroshiro128pp {
    #[inline]
    fn stafford13(mut z: u64) -> u64 {
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    pub fn from_seed(seed: i64) -> Self {
        let s = (seed as u64) ^ 0x6A09E667F3BCC909;
        let lo = Self::stafford13(s);
        let hi = Self::stafford13(s.wrapping_add(0x9E3779B97F4A7C15));
        let lo = if lo == 0 && hi == 0 { 0x9E3779B97F4A7C15 } else { lo };
        Self { lo, hi }
    }

    pub fn from_pair(lo: u64, hi: u64) -> Self {
        if lo == 0 && hi == 0 {
            Self {
                lo: 0x9E3779B97F4A7C15,
                hi: 0x6A09E667F3BCC909,
            }
        } else {
            Self { lo, hi }
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let s0 = self.lo;
        let mut s1 = self.hi;
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.lo = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.hi = s1.rotate_left(28);
        result
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    #[inline]
    pub fn next_int_bounded(&mut self, bound: u32) -> u32 {
        let mut r = self.next_u32() as u64;
        let mut m = r * (bound as u64);
        let mut lo = (m as u32) as u64;
        if lo < (bound as u64) {
            let t = (u32::MAX - bound + 1) % bound;
            while lo < t as u64 {
                r = self.next_u32() as u64;
                m = r * (bound as u64);
                lo = (m as u32) as u64;
            }
        }
        (m >> 32) as u32
    }

    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) * (1.0 / ((1u32 << 24) as f32))
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64))
    }
}
