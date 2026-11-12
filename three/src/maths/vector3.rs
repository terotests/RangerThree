//! A point or direction in 3D space (three.js `Vector3`).

use ranger::prelude::*;

use super::color::Color;
use super::cylindrical::Cylindrical;
use super::euler::Euler;
use super::math_utils::Rng;
use super::matrix3::Matrix3;
use super::matrix4::Matrix4;
use super::quaternion::Quaternion;
use super::spherical::Spherical;
use super::vector2::js_round;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for Vector3 {
    fn default() -> Vector3 {
        Vector3::new(0.0, 0.0, 0.0)
    }
}

impl Vector3 {
    pub fn new(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3 { x, y, z }
    }

    pub fn zero() -> Vector3 {
        Vector3::new(0.0, 0.0, 0.0)
    }

    pub fn one() -> Vector3 {
        Vector3::new(1.0, 1.0, 1.0)
    }

    pub fn set(&mut self, x: f64, y: f64, z: f64) -> &mut Vector3 {
        self.x = x;
        self.y = y;
        self.z = z;
        self
    }

    pub fn set_scalar(&mut self, s: f64) -> &mut Vector3 {
        self.x = s;
        self.y = s;
        self.z = s;
        self
    }

    pub fn set_x(&mut self, x: f64) -> &mut Vector3 {
        self.x = x;
        self
    }

    pub fn set_y(&mut self, y: f64) -> &mut Vector3 {
        self.y = y;
        self
    }

    pub fn set_z(&mut self, z: f64) -> &mut Vector3 {
        self.z = z;
        self
    }

    pub fn set_component(&mut self, index: i64, value: f64) -> &mut Vector3 {
        if index == 0 {
            self.x = value;
        } else if index == 1 {
            self.y = value;
        } else {
            self.z = value;
        }
        self
    }

    pub fn get_component(&self, index: i64) -> f64 {
        if index == 0 {
            self.x
        } else if index == 1 {
            self.y
        } else {
            self.z
        }
    }

    pub fn copy(&mut self, v: &Vector3) -> &mut Vector3 {
        self.x = v.x;
        self.y = v.y;
        self.z = v.z;
        self
    }

    pub fn add(&mut self, v: &Vector3) -> &mut Vector3 {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
        self
    }

    pub fn add_scalar(&mut self, s: f64) -> &mut Vector3 {
        self.x += s;
        self.y += s;
        self.z += s;
        self
    }

    pub fn add_vectors(&mut self, a: &Vector3, b: &Vector3) -> &mut Vector3 {
        self.x = a.x + b.x;
        self.y = a.y + b.y;
        self.z = a.z + b.z;
        self
    }

    pub fn add_scaled_vector(&mut self, v: &Vector3, s: f64) -> &mut Vector3 {
        self.x += v.x * s;
        self.y += v.y * s;
        self.z += v.z * s;
        self
    }

    pub fn sub(&mut self, v: &Vector3) -> &mut Vector3 {
        self.x -= v.x;
        self.y -= v.y;
        self.z -= v.z;
        self
    }

    pub fn sub_scalar(&mut self, s: f64) -> &mut Vector3 {
        self.x -= s;
        self.y -= s;
        self.z -= s;
        self
    }

    pub fn sub_vectors(&mut self, a: &Vector3, b: &Vector3) -> &mut Vector3 {
        self.x = a.x - b.x;
        self.y = a.y - b.y;
        self.z = a.z - b.z;
        self
    }

    pub fn multiply(&mut self, v: &Vector3) -> &mut Vector3 {
        self.x *= v.x;
        self.y *= v.y;
        self.z *= v.z;
        self
    }

    pub fn multiply_scalar(&mut self, s: f64) -> &mut Vector3 {
        self.x *= s;
        self.y *= s;
        self.z *= s;
        self
    }

    pub fn multiply_vectors(&mut self, a: &Vector3, b: &Vector3) -> &mut Vector3 {
        self.x = a.x * b.x;
        self.y = a.y * b.y;
        self.z = a.z * b.z;
        self
    }

    pub fn apply_euler(&mut self, euler: &Euler) -> &mut Vector3 {
        let q = Quaternion::from_euler(euler);
        self.apply_quaternion(&q)
    }

    pub fn apply_axis_angle(&mut self, axis: &Vector3, angle: f64) -> &mut Vector3 {
        let q = Quaternion::from_axis_angle(axis, angle);
        self.apply_quaternion(&q)
    }

    pub fn apply_matrix3(&mut self, m: &Matrix3) -> &mut Vector3 {
        let x = self.x;
        let y = self.y;
        let z = self.z;
        let e = m.elements;
        self.x = e[0] * x + e[3] * y + e[6] * z;
        self.y = e[1] * x + e[4] * y + e[7] * z;
        self.z = e[2] * x + e[5] * y + e[8] * z;
        self
    }

    pub fn apply_normal_matrix(&mut self, m: &Matrix3) -> &mut Vector3 {
        self.apply_matrix3(m).normalize()
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) -> &mut Vector3 {
        let x = self.x;
        let y = self.y;
        let z = self.z;
        let e = m.elements;
        let w = 1.0 / (e[3] * x + e[7] * y + e[11] * z + e[15]);
        self.x = (e[0] * x + e[4] * y + e[8] * z + e[12]) * w;
        self.y = (e[1] * x + e[5] * y + e[9] * z + e[13]) * w;
        self.z = (e[2] * x + e[6] * y + e[10] * z + e[14]) * w;
        self
    }

    pub fn apply_quaternion(&mut self, q: &Quaternion) -> &mut Vector3 {
        let vx = self.x;
        let vy = self.y;
        let vz = self.z;
        let qx = q.x;
        let qy = q.y;
        let qz = q.z;
        let qw = q.w;
        let tx = 2.0 * (qy * vz - qz * vy);
        let ty = 2.0 * (qz * vx - qx * vz);
        let tz = 2.0 * (qx * vy - qy * vx);
        self.x = vx + qw * tx + qy * tz - qz * ty;
        self.y = vy + qw * ty + qz * tx - qx * tz;
        self.z = vz + qw * tz + qx * ty - qy * tx;
        self
    }

    /// Transforms the direction by the upper 3x3 of `m` and normalises it.
    pub fn transform_direction(&mut self, m: &Matrix4) -> &mut Vector3 {
        let x = self.x;
        let y = self.y;
        let z = self.z;
        let e = m.elements;
        self.x = e[0] * x + e[4] * y + e[8] * z;
        self.y = e[1] * x + e[5] * y + e[9] * z;
        self.z = e[2] * x + e[6] * y + e[10] * z;
        self.normalize()
    }

    pub fn divide(&mut self, v: &Vector3) -> &mut Vector3 {
        self.x /= v.x;
        self.y /= v.y;
        self.z /= v.z;
        self
    }

    pub fn divide_scalar(&mut self, s: f64) -> &mut Vector3 {
        self.multiply_scalar(1.0 / s)
    }

    pub fn min(&mut self, v: &Vector3) -> &mut Vector3 {
        self.x = self.x.min(v.x);
        self.y = self.y.min(v.y);
        self.z = self.z.min(v.z);
        self
    }

    pub fn max(&mut self, v: &Vector3) -> &mut Vector3 {
        self.x = self.x.max(v.x);
        self.y = self.y.max(v.y);
        self.z = self.z.max(v.z);
        self
    }

    pub fn clamp(&mut self, min: &Vector3, max: &Vector3) -> &mut Vector3 {
        self.x = min.x.max(max.x.min(self.x));
        self.y = min.y.max(max.y.min(self.y));
        self.z = min.z.max(max.z.min(self.z));
        self
    }

    pub fn clamp_scalar(&mut self, min: f64, max: f64) -> &mut Vector3 {
        self.x = min.max(max.min(self.x));
        self.y = min.max(max.min(self.y));
        self.z = min.max(max.min(self.z));
        self
    }

    pub fn clamp_length(&mut self, min: f64, max: f64) -> &mut Vector3 {
        let length = self.length();
        let d = if length == 0.0 { 1.0 } else { length };
        self.divide_scalar(d)
            .multiply_scalar(min.max(max.min(length)))
    }

    pub fn floor(&mut self) -> &mut Vector3 {
        self.x = self.x.floor();
        self.y = self.y.floor();
        self.z = self.z.floor();
        self
    }

    pub fn ceil(&mut self) -> &mut Vector3 {
        self.x = self.x.ceil();
        self.y = self.y.ceil();
        self.z = self.z.ceil();
        self
    }

    pub fn round(&mut self) -> &mut Vector3 {
        self.x = js_round(self.x);
        self.y = js_round(self.y);
        self.z = js_round(self.z);
        self
    }

    pub fn round_to_zero(&mut self) -> &mut Vector3 {
        self.x = self.x.trunc();
        self.y = self.y.trunc();
        self.z = self.z.trunc();
        self
    }

    pub fn negate(&mut self) -> &mut Vector3 {
        self.x = -self.x;
        self.y = -self.y;
        self.z = -self.z;
        self
    }

    pub fn dot(&self, v: &Vector3) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    pub fn length_sq(&self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn manhattan_length(&self) -> f64 {
        self.x.abs() + self.y.abs() + self.z.abs()
    }

    pub fn normalize(&mut self) -> &mut Vector3 {
        let length = self.length();
        let d = if length == 0.0 { 1.0 } else { length };
        self.divide_scalar(d)
    }

    pub fn set_length(&mut self, length: f64) -> &mut Vector3 {
        self.normalize().multiply_scalar(length)
    }

    pub fn lerp(&mut self, v: &Vector3, alpha: f64) -> &mut Vector3 {
        self.x += (v.x - self.x) * alpha;
        self.y += (v.y - self.y) * alpha;
        self.z += (v.z - self.z) * alpha;
        self
    }

    pub fn lerp_vectors(&mut self, v1: &Vector3, v2: &Vector3, alpha: f64) -> &mut Vector3 {
        self.x = v1.x + (v2.x - v1.x) * alpha;
        self.y = v1.y + (v2.y - v1.y) * alpha;
        self.z = v1.z + (v2.z - v1.z) * alpha;
        self
    }

    pub fn cross(&mut self, v: &Vector3) -> &mut Vector3 {
        let a = *self;
        self.cross_vectors(&a, v)
    }

    pub fn cross_vectors(&mut self, a: &Vector3, b: &Vector3) -> &mut Vector3 {
        let ax = a.x;
        let ay = a.y;
        let az = a.z;
        let bx = b.x;
        let by = b.y;
        let bz = b.z;
        self.x = ay * bz - az * by;
        self.y = az * bx - ax * bz;
        self.z = ax * by - ay * bx;
        self
    }

    pub fn project_on_vector(&mut self, v: &Vector3) -> &mut Vector3 {
        let denominator = v.length_sq();
        if denominator == 0.0 {
            return self.set(0.0, 0.0, 0.0);
        }
        let scalar = v.dot(self) / denominator;
        let d = *v;
        self.copy(&d).multiply_scalar(scalar)
    }

    pub fn project_on_plane(&mut self, plane_normal: &Vector3) -> &mut Vector3 {
        let mut p = *self;
        p.project_on_vector(plane_normal);
        self.sub(&p)
    }

    /// Reflects off the plane through the origin with the unit `normal`.
    pub fn reflect(&mut self, normal: &Vector3) -> &mut Vector3 {
        let mut n = *normal;
        let d = 2.0 * self.dot(normal);
        n.multiply_scalar(d);
        self.sub(&n)
    }

    pub fn angle_to(&self, v: &Vector3) -> f64 {
        let denominator = (self.length_sq() * v.length_sq()).sqrt();
        if denominator == 0.0 {
            return std::f64::consts::PI / 2.0;
        }
        let theta = self.dot(v) / denominator;
        theta.max(-1.0).min(1.0).acos()
    }

    pub fn distance_to(&self, v: &Vector3) -> f64 {
        self.distance_to_squared(v).sqrt()
    }

    pub fn distance_to_squared(&self, v: &Vector3) -> f64 {
        let dx = self.x - v.x;
        let dy = self.y - v.y;
        let dz = self.z - v.z;
        dx * dx + dy * dy + dz * dz
    }

    pub fn manhattan_distance_to(&self, v: &Vector3) -> f64 {
        (self.x - v.x).abs() + (self.y - v.y).abs() + (self.z - v.z).abs()
    }

    pub fn set_from_spherical(&mut self, s: &Spherical) -> &mut Vector3 {
        self.set_from_spherical_coords(s.radius, s.phi, s.theta)
    }

    pub fn set_from_spherical_coords(&mut self, radius: f64, phi: f64, theta: f64) -> &mut Vector3 {
        let sin_phi_radius = phi.sin() * radius;
        self.x = sin_phi_radius * theta.sin();
        self.y = phi.cos() * radius;
        self.z = sin_phi_radius * theta.cos();
        self
    }

    pub fn set_from_cylindrical(&mut self, c: &Cylindrical) -> &mut Vector3 {
        self.set_from_cylindrical_coords(c.radius, c.theta, c.y)
    }

    pub fn set_from_cylindrical_coords(&mut self, radius: f64, theta: f64, y: f64) -> &mut Vector3 {
        self.x = radius * theta.sin();
        self.y = y;
        self.z = radius * theta.cos();
        self
    }

    pub fn set_from_matrix_position(&mut self, m: &Matrix4) -> &mut Vector3 {
        let e = m.elements;
        self.x = e[12];
        self.y = e[13];
        self.z = e[14];
        self
    }

    pub fn set_from_matrix_scale(&mut self, m: &Matrix4) -> &mut Vector3 {
        let mut c = Vector3::zero();
        let sx = c.set_from_matrix_column(m, 0).length();
        let sy = c.set_from_matrix_column(m, 1).length();
        let sz = c.set_from_matrix_column(m, 2).length();
        self.x = sx;
        self.y = sy;
        self.z = sz;
        self
    }

    pub fn set_from_matrix_column(&mut self, m: &Matrix4, index: usize) -> &mut Vector3 {
        let e = m.elements;
        self.x = e[index * 4];
        self.y = e[index * 4 + 1];
        self.z = e[index * 4 + 2];
        self
    }

    pub fn set_from_matrix3_column(&mut self, m: &Matrix3, index: usize) -> &mut Vector3 {
        let e = m.elements;
        self.x = e[index * 3];
        self.y = e[index * 3 + 1];
        self.z = e[index * 3 + 2];
        self
    }

    pub fn set_from_euler(&mut self, e: &Euler) -> &mut Vector3 {
        self.x = e.x;
        self.y = e.y;
        self.z = e.z;
        self
    }

    pub fn set_from_color(&mut self, c: &Color) -> &mut Vector3 {
        self.x = c.r;
        self.y = c.g;
        self.z = c.b;
        self
    }

    pub fn equals(&self, v: &Vector3) -> bool {
        self.x == v.x && self.y == v.y && self.z == v.z
    }

    pub fn from_array(&mut self, array: &[f64], offset: usize) -> &mut Vector3 {
        self.x = array[offset];
        self.y = array[offset + 1];
        self.z = array[offset + 2];
        self
    }

    pub fn to_array(&self) -> Vec<f64> {
        vec![self.x, self.y, self.z]
    }

    pub fn random(&mut self, rng: &mut Rng) -> &mut Vector3 {
        self.x = rng.next();
        self.y = rng.next();
        self.z = rng.next();
        self
    }

    /// A uniformly distributed unit vector.
    pub fn random_direction(&mut self, rng: &mut Rng) -> &mut Vector3 {
        let theta = rng.next() * std::f64::consts::PI * 2.0;
        let u = rng.next() * 2.0 - 1.0;
        let c = (1.0 - u * u).sqrt();
        self.x = c * theta.cos();
        self.y = u;
        self.z = c * theta.sin();
        self
    }
}
