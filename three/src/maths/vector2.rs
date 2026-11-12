//! A 2D vector (three.js `Vector2`).

use ranger::prelude::*;

use super::math_utils::Rng;
use super::matrix3::Matrix3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Default for Vector2 {
    fn default() -> Vector2 {
        Vector2::new(0.0, 0.0)
    }
}

impl Vector2 {
    pub fn new(x: f64, y: f64) -> Vector2 {
        Vector2 { x, y }
    }



    pub fn set(&mut self, x: f64, y: f64) -> &mut Vector2 {
        self.x = x;
        self.y = y;
        self
    }

    pub fn set_scalar(&mut self, s: f64) -> &mut Vector2 {
        self.x = s;
        self.y = s;
        self
    }

    pub fn set_x(&mut self, x: f64) -> &mut Vector2 {
        self.x = x;
        self
    }

    pub fn set_y(&mut self, y: f64) -> &mut Vector2 {
        self.y = y;
        self
    }

    pub fn set_component(&mut self, index: i64, value: f64) -> &mut Vector2 {
        if index == 0 {
            self.x = value;
        } else {
            self.y = value;
        }
        self
    }

    pub fn get_component(&self, index: i64) -> f64 {
        if index == 0 {
            self.x
        } else {
            self.y
        }
    }

    pub fn copy(&mut self, v: &Vector2) -> &mut Vector2 {
        self.x = v.x;
        self.y = v.y;
        self
    }

    pub fn add(&mut self, v: &Vector2) -> &mut Vector2 {
        self.x += v.x;
        self.y += v.y;
        self
    }

    pub fn add_scalar(&mut self, s: f64) -> &mut Vector2 {
        self.x += s;
        self.y += s;
        self
    }

    pub fn add_vectors(&mut self, a: &Vector2, b: &Vector2) -> &mut Vector2 {
        self.x = a.x + b.x;
        self.y = a.y + b.y;
        self
    }

    pub fn add_scaled_vector(&mut self, v: &Vector2, s: f64) -> &mut Vector2 {
        self.x += v.x * s;
        self.y += v.y * s;
        self
    }

    pub fn sub(&mut self, v: &Vector2) -> &mut Vector2 {
        self.x -= v.x;
        self.y -= v.y;
        self
    }

    pub fn sub_scalar(&mut self, s: f64) -> &mut Vector2 {
        self.x -= s;
        self.y -= s;
        self
    }

    pub fn sub_vectors(&mut self, a: &Vector2, b: &Vector2) -> &mut Vector2 {
        self.x = a.x - b.x;
        self.y = a.y - b.y;
        self
    }

    pub fn multiply(&mut self, v: &Vector2) -> &mut Vector2 {
        self.x *= v.x;
        self.y *= v.y;
        self
    }

    pub fn multiply_scalar(&mut self, s: f64) -> &mut Vector2 {
        self.x *= s;
        self.y *= s;
        self
    }

    pub fn divide(&mut self, v: &Vector2) -> &mut Vector2 {
        self.x /= v.x;
        self.y /= v.y;
        self
    }

    pub fn divide_scalar(&mut self, s: f64) -> &mut Vector2 {
        self.multiply_scalar(1.0 / s)
    }

    pub fn apply_matrix3(&mut self, m: &Matrix3) -> &mut Vector2 {
        let x = self.x;
        let y = self.y;
        let e = m.elements;
        self.x = e[0] * x + e[3] * y + e[6];
        self.y = e[1] * x + e[4] * y + e[7];
        self
    }

    pub fn min(&mut self, v: &Vector2) -> &mut Vector2 {
        self.x = self.x.min(v.x);
        self.y = self.y.min(v.y);
        self
    }

    pub fn max(&mut self, v: &Vector2) -> &mut Vector2 {
        self.x = self.x.max(v.x);
        self.y = self.y.max(v.y);
        self
    }

    pub fn clamp(&mut self, min: &Vector2, max: &Vector2) -> &mut Vector2 {
        self.x = min.x.max(max.x.min(self.x));
        self.y = min.y.max(max.y.min(self.y));
        self
    }

    pub fn clamp_scalar(&mut self, min: f64, max: f64) -> &mut Vector2 {
        self.x = min.max(max.min(self.x));
        self.y = min.max(max.min(self.y));
        self
    }

    pub fn clamp_length(&mut self, min: f64, max: f64) -> &mut Vector2 {
        let length = self.length();
        let d = if length == 0.0 { 1.0 } else { length };
        self.divide_scalar(d)
            .multiply_scalar(min.max(max.min(length)))
    }

    pub fn floor(&mut self) -> &mut Vector2 {
        self.x = self.x.floor();
        self.y = self.y.floor();
        self
    }

    pub fn ceil(&mut self) -> &mut Vector2 {
        self.x = self.x.ceil();
        self.y = self.y.ceil();
        self
    }

    pub fn round(&mut self) -> &mut Vector2 {
        self.x = js_round(self.x);
        self.y = js_round(self.y);
        self
    }

    pub fn round_to_zero(&mut self) -> &mut Vector2 {
        self.x = self.x.trunc();
        self.y = self.y.trunc();
        self
    }

    pub fn negate(&mut self) -> &mut Vector2 {
        self.x = -self.x;
        self.y = -self.y;
        self
    }

    pub fn dot(&self, v: &Vector2) -> f64 {
        self.x * v.x + self.y * v.y
    }

    pub fn cross(&self, v: &Vector2) -> f64 {
        self.x * v.y - self.y * v.x
    }

    pub fn length_sq(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn manhattan_length(&self) -> f64 {
        self.x.abs() + self.y.abs()
    }

    pub fn normalize(&mut self) -> &mut Vector2 {
        let length = self.length();
        let d = if length == 0.0 { 1.0 } else { length };
        self.divide_scalar(d)
    }

    /// The angle of the vector against the positive x axis, in `[0, 2π)`.
    pub fn angle(&self) -> f64 {
        (-self.y).atan2(-self.x) + std::f64::consts::PI
    }

    pub fn angle_to(&self, v: &Vector2) -> f64 {
        let denominator = (self.length_sq() * v.length_sq()).sqrt();
        if denominator == 0.0 {
            return std::f64::consts::PI / 2.0;
        }
        let theta = self.dot(v) / denominator;
        theta.max(-1.0).min(1.0).acos()
    }

    pub fn distance_to(&self, v: &Vector2) -> f64 {
        self.distance_to_squared(v).sqrt()
    }

    pub fn distance_to_squared(&self, v: &Vector2) -> f64 {
        let dx = self.x - v.x;
        let dy = self.y - v.y;
        dx * dx + dy * dy
    }

    pub fn manhattan_distance_to(&self, v: &Vector2) -> f64 {
        (self.x - v.x).abs() + (self.y - v.y).abs()
    }

    pub fn set_length(&mut self, length: f64) -> &mut Vector2 {
        self.normalize().multiply_scalar(length)
    }

    pub fn lerp(&mut self, v: &Vector2, alpha: f64) -> &mut Vector2 {
        self.x += (v.x - self.x) * alpha;
        self.y += (v.y - self.y) * alpha;
        self
    }

    pub fn lerp_vectors(&mut self, v1: &Vector2, v2: &Vector2, alpha: f64) -> &mut Vector2 {
        self.x = v1.x + (v2.x - v1.x) * alpha;
        self.y = v1.y + (v2.y - v1.y) * alpha;
        self
    }

    pub fn equals(&self, v: &Vector2) -> bool {
        self.x == v.x && self.y == v.y
    }

    pub fn from_array(&mut self, array: &[f64], offset: usize) -> &mut Vector2 {
        self.x = array[offset];
        self.y = array[offset + 1];
        self
    }

    pub fn to_array(&self) -> Vec<f64> {
        vec![self.x, self.y]
    }

    pub fn rotate_around(&mut self, center: &Vector2, angle: f64) -> &mut Vector2 {
        let c = angle.cos();
        let s = angle.sin();
        let x = self.x - center.x;
        let y = self.y - center.y;
        self.x = x * c - y * s + center.x;
        self.y = x * s + y * c + center.y;
        self
    }

    pub fn random(&mut self, rng: &mut Rng) -> &mut Vector2 {
        self.x = rng.next();
        self.y = rng.next();
        self
    }
}

/// JavaScript's `Math.round`: halves go towards positive infinity.
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}
