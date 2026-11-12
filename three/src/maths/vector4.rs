//! A 4D vector (three.js `Vector4`): homogeneous points, viewports, RGBA.

use ranger::prelude::*;

use super::matrix4::Matrix4;
use super::quaternion::Quaternion;
use super::vector2::js_round;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector4 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Default for Vector4 {
    fn default() -> Vector4 {
        Vector4::new(0.0, 0.0, 0.0, 1.0)
    }
}

impl Vector4 {
    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Vector4 {
        Vector4 { x, y, z, w }
    }

    pub fn width(&self) -> f64 {
        self.z
    }

    pub fn height(&self) -> f64 {
        self.w
    }

    pub fn set(&mut self, x: f64, y: f64, z: f64, w: f64) -> &mut Vector4 {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = w;
        self
    }

    pub fn set_scalar(&mut self, s: f64) -> &mut Vector4 {
        self.set(s, s, s, s)
    }

    pub fn set_x(&mut self, x: f64) -> &mut Vector4 {
        self.x = x;
        self
    }

    pub fn set_y(&mut self, y: f64) -> &mut Vector4 {
        self.y = y;
        self
    }

    pub fn set_z(&mut self, z: f64) -> &mut Vector4 {
        self.z = z;
        self
    }

    pub fn set_w(&mut self, w: f64) -> &mut Vector4 {
        self.w = w;
        self
    }

    pub fn set_component(&mut self, index: i64, value: f64) -> &mut Vector4 {
        if index == 0 {
            self.x = value;
        } else if index == 1 {
            self.y = value;
        } else if index == 2 {
            self.z = value;
        } else {
            self.w = value;
        }
        self
    }

    pub fn get_component(&self, index: i64) -> f64 {
        if index == 0 {
            self.x
        } else if index == 1 {
            self.y
        } else if index == 2 {
            self.z
        } else {
            self.w
        }
    }

    pub fn copy(&mut self, v: &Vector4) -> &mut Vector4 {
        self.set(v.x, v.y, v.z, v.w)
    }

    pub fn add(&mut self, v: &Vector4) -> &mut Vector4 {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
        self.w += v.w;
        self
    }

    pub fn add_scalar(&mut self, s: f64) -> &mut Vector4 {
        self.x += s;
        self.y += s;
        self.z += s;
        self.w += s;
        self
    }

    pub fn add_vectors(&mut self, a: &Vector4, b: &Vector4) -> &mut Vector4 {
        self.set(a.x + b.x, a.y + b.y, a.z + b.z, a.w + b.w)
    }

    pub fn add_scaled_vector(&mut self, v: &Vector4, s: f64) -> &mut Vector4 {
        self.x += v.x * s;
        self.y += v.y * s;
        self.z += v.z * s;
        self.w += v.w * s;
        self
    }

    pub fn sub(&mut self, v: &Vector4) -> &mut Vector4 {
        self.x -= v.x;
        self.y -= v.y;
        self.z -= v.z;
        self.w -= v.w;
        self
    }

    pub fn sub_scalar(&mut self, s: f64) -> &mut Vector4 {
        self.x -= s;
        self.y -= s;
        self.z -= s;
        self.w -= s;
        self
    }

    pub fn sub_vectors(&mut self, a: &Vector4, b: &Vector4) -> &mut Vector4 {
        self.set(a.x - b.x, a.y - b.y, a.z - b.z, a.w - b.w)
    }

    pub fn multiply(&mut self, v: &Vector4) -> &mut Vector4 {
        self.x *= v.x;
        self.y *= v.y;
        self.z *= v.z;
        self.w *= v.w;
        self
    }

    pub fn multiply_scalar(&mut self, s: f64) -> &mut Vector4 {
        self.x *= s;
        self.y *= s;
        self.z *= s;
        self.w *= s;
        self
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) -> &mut Vector4 {
        let x = self.x;
        let y = self.y;
        let z = self.z;
        let w = self.w;
        let e = m.elements;
        self.x = e[0] * x + e[4] * y + e[8] * z + e[12] * w;
        self.y = e[1] * x + e[5] * y + e[9] * z + e[13] * w;
        self.z = e[2] * x + e[6] * y + e[10] * z + e[14] * w;
        self.w = e[3] * x + e[7] * y + e[11] * z + e[15] * w;
        self
    }

    pub fn divide(&mut self, v: &Vector4) -> &mut Vector4 {
        self.x /= v.x;
        self.y /= v.y;
        self.z /= v.z;
        self.w /= v.w;
        self
    }

    pub fn divide_scalar(&mut self, s: f64) -> &mut Vector4 {
        self.multiply_scalar(1.0 / s)
    }

    /// Axis in xyz and angle in w of the unit quaternion `q`.
    pub fn set_axis_angle_from_quaternion(&mut self, q: &Quaternion) -> &mut Vector4 {
        self.w = 2.0 * q.w.acos();
        let s = (1.0 - q.w * q.w).sqrt();
        if s < 0.0001 {
            self.x = 1.0;
            self.y = 0.0;
            self.z = 0.0;
        } else {
            self.x = q.x / s;
            self.y = q.y / s;
            self.z = q.z / s;
        }
        self
    }

    /// Axis in xyz and angle in w of the rotation in the upper 3x3 of `m`.
    pub fn set_axis_angle_from_rotation_matrix(&mut self, m: &Matrix4) -> &mut Vector4 {
        let epsilon = 0.01;
        let epsilon2 = 0.1;
        let te = m.elements;
        let m11 = te[0];
        let m12 = te[4];
        let m13 = te[8];
        let m21 = te[1];
        let m22 = te[5];
        let m23 = te[9];
        let m31 = te[2];
        let m32 = te[6];
        let m33 = te[10];
        if (m12 - m21).abs() < epsilon && (m13 - m31).abs() < epsilon && (m23 - m32).abs() < epsilon {
            if (m12 + m21).abs() < epsilon2
                && (m13 + m31).abs() < epsilon2
                && (m23 + m32).abs() < epsilon2
                && (m11 + m22 + m33 - 3.0).abs() < epsilon2
            {
                return self.set(1.0, 0.0, 0.0, 0.0);
            }
            let angle = std::f64::consts::PI;
            let xx = (m11 + 1.0) / 2.0;
            let yy = (m22 + 1.0) / 2.0;
            let zz = (m33 + 1.0) / 2.0;
            let xy = (m12 + m21) / 4.0;
            let xz = (m13 + m31) / 4.0;
            let yz = (m23 + m32) / 4.0;
            let x: f64;
            let y: f64;
            let z: f64;
            if xx > yy && xx > zz {
                if xx < epsilon {
                    x = 0.0;
                    y = 0.707106781;
                    z = 0.707106781;
                } else {
                    x = xx.sqrt();
                    y = xy / x;
                    z = xz / x;
                }
            } else if yy > zz {
                if yy < epsilon {
                    x = 0.707106781;
                    y = 0.0;
                    z = 0.707106781;
                } else {
                    y = yy.sqrt();
                    x = xy / y;
                    z = yz / y;
                }
            } else if zz < epsilon {
                x = 0.707106781;
                y = 0.707106781;
                z = 0.0;
            } else {
                z = zz.sqrt();
                x = xz / z;
                y = yz / z;
            }
            return self.set(x, y, z, angle);
        }
        let mut s = ((m32 - m23) * (m32 - m23) + (m13 - m31) * (m13 - m31) + (m21 - m12) * (m21 - m12)).sqrt();
        if s.abs() < 0.001 {
            s = 1.0;
        }
        self.x = (m32 - m23) / s;
        self.y = (m13 - m31) / s;
        self.z = (m21 - m12) / s;
        self.w = ((m11 + m22 + m33 - 1.0) / 2.0).acos();
        self
    }

    pub fn set_from_matrix_position(&mut self, m: &Matrix4) -> &mut Vector4 {
        let e = m.elements;
        self.x = e[12];
        self.y = e[13];
        self.z = e[14];
        self.w = e[15];
        self
    }

    pub fn min(&mut self, v: &Vector4) -> &mut Vector4 {
        self.x = self.x.min(v.x);
        self.y = self.y.min(v.y);
        self.z = self.z.min(v.z);
        self.w = self.w.min(v.w);
        self
    }

    pub fn max(&mut self, v: &Vector4) -> &mut Vector4 {
        self.x = self.x.max(v.x);
        self.y = self.y.max(v.y);
        self.z = self.z.max(v.z);
        self.w = self.w.max(v.w);
        self
    }

    pub fn clamp(&mut self, min: &Vector4, max: &Vector4) -> &mut Vector4 {
        self.x = min.x.max(max.x.min(self.x));
        self.y = min.y.max(max.y.min(self.y));
        self.z = min.z.max(max.z.min(self.z));
        self.w = min.w.max(max.w.min(self.w));
        self
    }

    pub fn clamp_scalar(&mut self, min: f64, max: f64) -> &mut Vector4 {
        self.x = min.max(max.min(self.x));
        self.y = min.max(max.min(self.y));
        self.z = min.max(max.min(self.z));
        self.w = min.max(max.min(self.w));
        self
    }

    pub fn clamp_length(&mut self, min: f64, max: f64) -> &mut Vector4 {
        let length = self.length();
        let d = if length == 0.0 { 1.0 } else { length };
        self.divide_scalar(d)
            .multiply_scalar(min.max(max.min(length)))
    }

    pub fn floor(&mut self) -> &mut Vector4 {
        self.set(self.x.floor(), self.y.floor(), self.z.floor(), self.w.floor())
    }

    pub fn ceil(&mut self) -> &mut Vector4 {
        self.set(self.x.ceil(), self.y.ceil(), self.z.ceil(), self.w.ceil())
    }

    pub fn round(&mut self) -> &mut Vector4 {
        self.set(js_round(self.x), js_round(self.y), js_round(self.z), js_round(self.w))
    }

    pub fn round_to_zero(&mut self) -> &mut Vector4 {
        self.set(self.x.trunc(), self.y.trunc(), self.z.trunc(), self.w.trunc())
    }

    pub fn negate(&mut self) -> &mut Vector4 {
        self.set(-self.x, -self.y, -self.z, -self.w)
    }

    pub fn dot(&self, v: &Vector4) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z + self.w * v.w
    }

    pub fn length_sq(&self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    pub fn length(&self) -> f64 {
        self.length_sq().sqrt()
    }

    pub fn manhattan_length(&self) -> f64 {
        self.x.abs() + self.y.abs() + self.z.abs() + self.w.abs()
    }

    pub fn normalize(&mut self) -> &mut Vector4 {
        let length = self.length();
        let d = if length == 0.0 { 1.0 } else { length };
        self.divide_scalar(d)
    }

    pub fn set_length(&mut self, length: f64) -> &mut Vector4 {
        self.normalize().multiply_scalar(length)
    }

    pub fn lerp(&mut self, v: &Vector4, alpha: f64) -> &mut Vector4 {
        self.x += (v.x - self.x) * alpha;
        self.y += (v.y - self.y) * alpha;
        self.z += (v.z - self.z) * alpha;
        self.w += (v.w - self.w) * alpha;
        self
    }

    pub fn lerp_vectors(&mut self, v1: &Vector4, v2: &Vector4, alpha: f64) -> &mut Vector4 {
        self.x = v1.x + (v2.x - v1.x) * alpha;
        self.y = v1.y + (v2.y - v1.y) * alpha;
        self.z = v1.z + (v2.z - v1.z) * alpha;
        self.w = v1.w + (v2.w - v1.w) * alpha;
        self
    }

    pub fn equals(&self, v: &Vector4) -> bool {
        self.x == v.x && self.y == v.y && self.z == v.z && self.w == v.w
    }

    pub fn from_array(&mut self, array: &[f64], offset: usize) -> &mut Vector4 {
        self.x = array[offset];
        self.y = array[offset + 1];
        self.z = array[offset + 2];
        self.w = array[offset + 3];
        self
    }

    pub fn to_array(&self) -> Vec<f64> {
        vec![self.x, self.y, self.z, self.w]
    }
}
