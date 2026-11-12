//! A rotation as a unit quaternion (three.js `Quaternion`).

use ranger::prelude::*;

use super::euler::{Euler, EulerOrder};
use super::math_utils::Rng;
use super::matrix4::Matrix4;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Default for Quaternion {
    fn default() -> Quaternion {
        Quaternion::identity()
    }
}

impl Quaternion {
    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Quaternion {
        Quaternion { x, y, z, w }
    }

    pub fn identity() -> Quaternion {
        Quaternion::new(0.0, 0.0, 0.0, 1.0)
    }

    pub fn from_euler(e: &Euler) -> Quaternion {
        let mut q = Quaternion::identity();
        q.set_from_euler(e);
        q
    }

    pub fn from_axis_angle(axis: &Vector3, angle: f64) -> Quaternion {
        let mut q = Quaternion::identity();
        q.set_from_axis_angle(axis, angle);
        q
    }

    pub fn set(&mut self, x: f64, y: f64, z: f64, w: f64) -> &mut Quaternion {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = w;
        self
    }

    pub fn copy(&mut self, q: &Quaternion) -> &mut Quaternion {
        self.set(q.x, q.y, q.z, q.w)
    }

    pub fn set_from_euler(&mut self, e: &Euler) -> &mut Quaternion {
        let c1 = (e.x / 2.0).cos();
        let c2 = (e.y / 2.0).cos();
        let c3 = (e.z / 2.0).cos();
        let s1 = (e.x / 2.0).sin();
        let s2 = (e.y / 2.0).sin();
        let s3 = (e.z / 2.0).sin();
        match e.order {
            EulerOrder::Xyz => self.set(
                s1 * c2 * c3 + c1 * s2 * s3,
                c1 * s2 * c3 - s1 * c2 * s3,
                c1 * c2 * s3 + s1 * s2 * c3,
                c1 * c2 * c3 - s1 * s2 * s3,
            ),
            EulerOrder::Yxz => self.set(
                s1 * c2 * c3 + c1 * s2 * s3,
                c1 * s2 * c3 - s1 * c2 * s3,
                c1 * c2 * s3 - s1 * s2 * c3,
                c1 * c2 * c3 + s1 * s2 * s3,
            ),
            EulerOrder::Zxy => self.set(
                s1 * c2 * c3 - c1 * s2 * s3,
                c1 * s2 * c3 + s1 * c2 * s3,
                c1 * c2 * s3 + s1 * s2 * c3,
                c1 * c2 * c3 - s1 * s2 * s3,
            ),
            EulerOrder::Zyx => self.set(
                s1 * c2 * c3 - c1 * s2 * s3,
                c1 * s2 * c3 + s1 * c2 * s3,
                c1 * c2 * s3 - s1 * s2 * c3,
                c1 * c2 * c3 + s1 * s2 * s3,
            ),
            EulerOrder::Yzx => self.set(
                s1 * c2 * c3 + c1 * s2 * s3,
                c1 * s2 * c3 + s1 * c2 * s3,
                c1 * c2 * s3 - s1 * s2 * c3,
                c1 * c2 * c3 - s1 * s2 * s3,
            ),
            EulerOrder::Xzy => self.set(
                s1 * c2 * c3 - c1 * s2 * s3,
                c1 * s2 * c3 - s1 * c2 * s3,
                c1 * c2 * s3 + s1 * s2 * c3,
                c1 * c2 * c3 + s1 * s2 * s3,
            ),
        }
    }

    /// Rotation by `angle` around the unit vector `axis`.
    pub fn set_from_axis_angle(&mut self, axis: &Vector3, angle: f64) -> &mut Quaternion {
        let half = angle / 2.0;
        let s = half.sin();
        self.set(axis.x * s, axis.y * s, axis.z * s, half.cos())
    }

    /// The rotation in the upper 3x3 of `m`, which must be unscaled.
    pub fn set_from_rotation_matrix(&mut self, m: &Matrix4) -> &mut Quaternion {
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
        let trace = m11 + m22 + m33;
        if trace > 0.0 {
            let s = 0.5 / (trace + 1.0).sqrt();
            self.set((m32 - m23) * s, (m13 - m31) * s, (m21 - m12) * s, 0.25 / s)
        } else if m11 > m22 && m11 > m33 {
            let s = 2.0 * (1.0 + m11 - m22 - m33).sqrt();
            self.set(0.25 * s, (m12 + m21) / s, (m13 + m31) / s, (m32 - m23) / s)
        } else if m22 > m33 {
            let s = 2.0 * (1.0 + m22 - m11 - m33).sqrt();
            self.set((m12 + m21) / s, 0.25 * s, (m23 + m32) / s, (m13 - m31) / s)
        } else {
            let s = 2.0 * (1.0 + m33 - m11 - m22).sqrt();
            self.set((m13 + m31) / s, (m23 + m32) / s, 0.25 * s, (m21 - m12) / s)
        }
    }

    /// The rotation that turns the unit vector `from` onto `to`.
    pub fn set_from_unit_vectors(&mut self, from: &Vector3, to: &Vector3) -> &mut Quaternion {
        let mut r = from.dot(to) + 1.0;
        if r < 1e-8 {
            r = 0.0;
            if from.x.abs() > from.z.abs() {
                self.set(-from.y, from.x, 0.0, r);
            } else {
                self.set(0.0, -from.z, from.y, r);
            }
        } else {
            self.set(
                from.y * to.z - from.z * to.y,
                from.z * to.x - from.x * to.z,
                from.x * to.y - from.y * to.x,
                r,
            );
        }
        self.normalize()
    }

    pub fn angle_to(&self, q: &Quaternion) -> f64 {
        let d = self.dot(q).abs().max(-1.0).min(1.0);
        2.0 * d.acos()
    }

    pub fn rotate_towards(&mut self, q: &Quaternion, step: f64) -> &mut Quaternion {
        let angle = self.angle_to(q);
        if angle == 0.0 {
            return self;
        }
        let t = (1.0f64).min(step / angle);
        self.slerp(q, t)
    }

    pub fn make_identity(&mut self) -> &mut Quaternion {
        self.set(0.0, 0.0, 0.0, 1.0)
    }

    /// The inverse of a unit quaternion (its conjugate).
    pub fn invert(&mut self) -> &mut Quaternion {
        self.conjugate()
    }

    pub fn conjugate(&mut self) -> &mut Quaternion {
        self.x = -self.x;
        self.y = -self.y;
        self.z = -self.z;
        self
    }

    pub fn dot(&self, v: &Quaternion) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z + self.w * v.w
    }

    pub fn length_sq(&self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    pub fn length(&self) -> f64 {
        self.length_sq().sqrt()
    }

    pub fn normalize(&mut self) -> &mut Quaternion {
        let l = self.length();
        if l == 0.0 {
            self.set(0.0, 0.0, 0.0, 1.0)
        } else {
            let inv = 1.0 / l;
            self.set(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
        }
    }

    pub fn multiply(&mut self, q: &Quaternion) -> &mut Quaternion {
        let a = *self;
        self.multiply_quaternions(&a, q)
    }

    pub fn premultiply(&mut self, q: &Quaternion) -> &mut Quaternion {
        let b = *self;
        self.multiply_quaternions(q, &b)
    }

    pub fn multiply_quaternions(&mut self, a: &Quaternion, b: &Quaternion) -> &mut Quaternion {
        let qax = a.x;
        let qay = a.y;
        let qaz = a.z;
        let qaw = a.w;
        let qbx = b.x;
        let qby = b.y;
        let qbz = b.z;
        let qbw = b.w;
        self.set(
            qax * qbw + qaw * qbx + qay * qbz - qaz * qby,
            qay * qbw + qaw * qby + qaz * qbx - qax * qbz,
            qaz * qbw + qaw * qbz + qax * qby - qay * qbx,
            qaw * qbw - qax * qbx - qay * qby - qaz * qbz,
        )
    }

    pub fn slerp(&mut self, qb: &Quaternion, t: f64) -> &mut Quaternion {
        let mut d = self.dot(qb);
        let mut x = qb.x;
        let mut y = qb.y;
        let mut z = qb.z;
        let mut w = qb.w;
        if d < 0.0 {
            x = -x;
            y = -y;
            z = -z;
            w = -w;
            d = -d;
        }
        let mut s = 1.0 - t;
        let mut tt = t;
        if d < 0.9995 {
            let theta = d.acos();
            let sin = theta.sin();
            s = (s * theta).sin() / sin;
            tt = (tt * theta).sin() / sin;
            self.set(
                self.x * s + x * tt,
                self.y * s + y * tt,
                self.z * s + z * tt,
                self.w * s + w * tt,
            )
        } else {
            self.set(
                self.x * s + x * tt,
                self.y * s + y * tt,
                self.z * s + z * tt,
                self.w * s + w * tt,
            );
            self.normalize()
        }
    }

    pub fn slerp_quaternions(&mut self, qa: &Quaternion, qb: &Quaternion, t: f64) -> &mut Quaternion {
        self.copy(qa).slerp(qb, t)
    }

    /// Slerp over flat arrays: `dst[dst_offset..+4]` becomes the rotation
    /// `t` of the way from `src0[o0..]` to `src1[o1..]` (three.js `slerpFlat`).
    pub fn slerp_flat(
        dst: &mut Vec<f64>,
        dst_offset: usize,
        src0: &[f64],
        offset0: usize,
        src1: &[f64],
        offset1: usize,
        t: f64,
    ) {
        let mut a = Quaternion::new(src0[offset0], src0[offset0 + 1], src0[offset0 + 2], src0[offset0 + 3]);
        let b = Quaternion::new(src1[offset1], src1[offset1 + 1], src1[offset1 + 2], src1[offset1 + 3]);
        if t <= 0.0 {
            // a stays
        } else if t >= 1.0 {
            a.copy(&b);
        } else if !a.equals(&b) {
            let mut s = 1.0 - t;
            let cos = a.dot(&b);
            let dir = if cos >= 0.0 { 1.0 } else { -1.0 };
            let sqr_sin = 1.0 - cos * cos;
            let mut tt = t;
            if sqr_sin > 1e-15 {
                let sin = sqr_sin.sqrt();
                let len = sin.atan2(cos * dir);
                s = (s * len).sin() / sin;
                tt = (tt * len).sin() / sin;
            }
            let t_dir = tt * dir;
            a.set(
                a.x * s + b.x * t_dir,
                a.y * s + b.y * t_dir,
                a.z * s + b.z * t_dir,
                a.w * s + b.w * t_dir,
            );
            if s == 1.0 - t {
                a.normalize();
            }
        }
        dst[dst_offset] = a.x;
        dst[dst_offset + 1] = a.y;
        dst[dst_offset + 2] = a.z;
        dst[dst_offset + 3] = a.w;
    }

    /// A uniformly distributed random rotation.
    pub fn random(&mut self, rng: &mut Rng) -> &mut Quaternion {
        let theta1 = 2.0 * std::f64::consts::PI * rng.next();
        let theta2 = 2.0 * std::f64::consts::PI * rng.next();
        let x0 = rng.next();
        let r1 = (1.0 - x0).sqrt();
        let r2 = x0.sqrt();
        self.set(r1 * theta1.sin(), r1 * theta1.cos(), r2 * theta2.sin(), r2 * theta2.cos())
    }

    pub fn equals(&self, q: &Quaternion) -> bool {
        self.x == q.x && self.y == q.y && self.z == q.z && self.w == q.w
    }

    pub fn from_array(&mut self, array: &[f64], offset: usize) -> &mut Quaternion {
        self.set(array[offset], array[offset + 1], array[offset + 2], array[offset + 3])
    }

    pub fn to_array(&self) -> Vec<f64> {
        vec![self.x, self.y, self.z, self.w]
    }
}
