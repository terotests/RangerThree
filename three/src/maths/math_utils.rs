//! Scalar helpers (three.js `MathUtils`) and a small deterministic random
//! source. Randomness is an explicit [`Rng`] value rather than global state,
//! so every Ranger target draws the same numbers from the same seed.

use ranger::prelude::*;

pub const DEG2RAD: f64 = std::f64::consts::PI / 180.0;
/// Positive infinity, spelled so that every Ranger target reads it.
pub const INFINITY: f64 = 1.0 / 0.0;

pub const RAD2DEG: f64 = 180.0 / std::f64::consts::PI;

pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// `n mod m` that is never negative (for positive `m`).
pub fn euclidean_modulo(n: f64, m: f64) -> f64 {
    ((n % m) + m) % m
}

/// Maps `x` from the range `[a1, a2]` to `[b1, b2]`.
pub fn map_linear(x: f64, a1: f64, a2: f64, b1: f64, b2: f64) -> f64 {
    b1 + (x - a1) * (b2 - b1) / (a2 - a1)
}

pub fn inverse_lerp(x: f64, y: f64, value: f64) -> f64 {
    if x != y {
        (value - x) / (y - x)
    } else {
        0.0
    }
}

pub fn lerp(x: f64, y: f64, t: f64) -> f64 {
    (1.0 - t) * x + t * y
}

/// Frame-rate independent smoothing towards `y` (`lambda` is the rate).
pub fn damp(x: f64, y: f64, lambda: f64, dt: f64) -> f64 {
    lerp(x, y, 1.0 - (-lambda * dt).exp())
}

pub fn pingpong(x: f64, length: f64) -> f64 {
    length - (euclidean_modulo(x, length * 2.0) - length).abs()
}

pub fn smoothstep(x: f64, min: f64, max: f64) -> f64 {
    if x <= min {
        return 0.0;
    }
    if x >= max {
        return 1.0;
    }
    let t = (x - min) / (max - min);
    t * t * (3.0 - 2.0 * t)
}

pub fn smootherstep(x: f64, min: f64, max: f64) -> f64 {
    if x <= min {
        return 0.0;
    }
    if x >= max {
        return 1.0;
    }
    let t = (x - min) / (max - min);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn deg_to_rad(degrees: f64) -> f64 {
    degrees * DEG2RAD
}

pub fn rad_to_deg(radians: f64) -> f64 {
    radians * RAD2DEG
}

pub fn is_power_of_two(value: i64) -> bool {
    value > 0 && (value & (value - 1)) == 0
}

pub fn ceil_power_of_two(value: f64) -> f64 {
    (2.0f64).powf((value.ln() / (2.0f64).ln()).ceil())
}

pub fn floor_power_of_two(value: f64) -> f64 {
    (2.0f64).powf((value.ln() / (2.0f64).ln()).floor())
}

/// Integer storage kinds a normalised attribute may hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArrayKind {
    Float32,
    Uint32,
    Uint16,
    Uint8,
    Int32,
    Int16,
    Int8,
}

/// A value stored in an integer array, as the float it stands for.
pub fn denormalize(value: f64, kind: ArrayKind) -> f64 {
    match kind {
        ArrayKind::Float32 => value,
        ArrayKind::Uint32 => value / 4294967295.0,
        ArrayKind::Uint16 => value / 65535.0,
        ArrayKind::Uint8 => value / 255.0,
        ArrayKind::Int32 => (value / 2147483647.0).max(-1.0),
        ArrayKind::Int16 => (value / 32767.0).max(-1.0),
        ArrayKind::Int8 => (value / 127.0).max(-1.0),
    }
}

/// A float as the integer an array of `kind` stores for it.
pub fn normalize(value: f64, kind: ArrayKind) -> f64 {
    match kind {
        ArrayKind::Float32 => value,
        ArrayKind::Uint32 => (value * 4294967295.0).round(),
        ArrayKind::Uint16 => (value * 65535.0).round(),
        ArrayKind::Uint8 => (value * 255.0).round(),
        ArrayKind::Int32 => (value * 2147483647.0).round(),
        ArrayKind::Int16 => (value * 32767.0).round(),
        ArrayKind::Int8 => (value * 127.0).round(),
    }
}

/// `(a * b) mod 2^32` for 32-bit operands, without a 64-bit overflow on
/// any target (JavaScript's `Math.imul`, unsigned).
pub fn mul_u32(a: i64, b: i64) -> i64 {
    let a = a & 0xffffffff;
    let b = b & 0xffffffff;
    let lo = (a & 0xffff) * b;
    let hi = (((a >> 16) & 0xffff) * b) & 0xffff;
    (lo + (hi << 16)) & 0xffffffff
}

/// A deterministic pseudo random source (mulberry32, as three.js's
/// `seededRandom`). [`Rng::next`] answers a float in `[0, 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rng {
    pub seed: i64,
}

impl Rng {
    pub fn new(seed: i64) -> Rng {
        Rng { seed: seed & 0xffffffff }
    }

    pub fn next(&mut self) -> f64 {
        self.seed = (self.seed + 0x6D2B79F5) & 0xffffffff;
        let mut t = self.seed;
        t = mul_u32(t ^ (t >> 15), t | 1);
        t ^= (t + mul_u32(t ^ (t >> 7), t | 61)) & 0xffffffff;
        ((t ^ (t >> 14)) & 0xffffffff) as f64 / 4294967296.0
    }

    pub fn rand_int(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() * ((high - low + 1) as f64)).floor() as i64
    }

    pub fn rand_float(&mut self, low: f64, high: f64) -> f64 {
        low + self.next() * (high - low)
    }

    pub fn rand_float_spread(&mut self, range: f64) -> f64 {
        range * (0.5 - self.next())
    }

    /// A version 4 UUID in upper case, as three.js's `generateUUID`.
    pub fn uuid(&mut self) -> String {
        let hex = "0123456789ABCDEF";
        let digits: Vec<char> = hex.chars().collect();
        let mut out = String::new();
        for i in 0..36 {
            if i == 8 || i == 13 || i == 18 || i == 23 {
                out.push('-');
            } else if i == 14 {
                out.push('4');
            } else {
                let mut d = (self.next() * 16.0).floor() as usize;
                if i == 19 {
                    d = (d & 3) | 8;
                }
                out.push(digits[d]);
            }
        }
        out
    }
}
