//! A 4x4 matrix (three.js `Matrix4`), stored column-major like WebGL.

use ranger::prelude::*;

use super::euler::{Euler, EulerOrder};
use super::matrix3::Matrix3;
use super::quaternion::Quaternion;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4 {
    pub elements: [f64; 16],
}

/// What [`Matrix4::decompose`] answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vector3,
    pub quaternion: Quaternion,
    pub scale: Vector3,
}

impl Default for Matrix4 {
    fn default() -> Matrix4 {
        Matrix4::identity()
    }
}

impl Matrix4 {
    pub fn identity() -> Matrix4 {
        Matrix4 {
            elements: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Takes the entries row by row (as three.js's `set`).
    pub fn set(
        &mut self,
        n11: f64, n12: f64, n13: f64, n14: f64,
        n21: f64, n22: f64, n23: f64, n24: f64,
        n31: f64, n32: f64, n33: f64, n34: f64,
        n41: f64, n42: f64, n43: f64, n44: f64,
    ) -> &mut Matrix4 {
        let te = &mut self.elements;
        te[0] = n11;
        te[4] = n12;
        te[8] = n13;
        te[12] = n14;
        te[1] = n21;
        te[5] = n22;
        te[9] = n23;
        te[13] = n24;
        te[2] = n31;
        te[6] = n32;
        te[10] = n33;
        te[14] = n34;
        te[3] = n41;
        te[7] = n42;
        te[11] = n43;
        te[15] = n44;
        self
    }

    pub fn make_identity(&mut self) -> &mut Matrix4 {
        self.elements = Matrix4::identity().elements;
        self
    }

    pub fn copy(&mut self, m: &Matrix4) -> &mut Matrix4 {
        self.elements = m.elements;
        self
    }

    pub fn copy_position(&mut self, m: &Matrix4) -> &mut Matrix4 {
        self.elements[12] = m.elements[12];
        self.elements[13] = m.elements[13];
        self.elements[14] = m.elements[14];
        self
    }

    pub fn set_from_matrix3(&mut self, m: &Matrix3) -> &mut Matrix4 {
        let me = m.elements;
        self.set(
            me[0], me[3], me[6], 0.0, me[1], me[4], me[7], 0.0, me[2], me[5], me[8], 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// The first three columns as basis vectors.
    pub fn extract_basis(&self) -> Vec<Vector3> {
        let mut x = Vector3::zero();
        let mut y = Vector3::zero();
        let mut z = Vector3::zero();
        x.set_from_matrix_column(self, 0);
        y.set_from_matrix_column(self, 1);
        z.set_from_matrix_column(self, 2);
        vec![x, y, z]
    }

    pub fn make_basis(&mut self, x: &Vector3, y: &Vector3, z: &Vector3) -> &mut Matrix4 {
        self.set(
            x.x, y.x, z.x, 0.0, x.y, y.y, z.y, 0.0, x.z, y.z, z.z, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// The rotation part of `m` (scale divided out, no translation).
    pub fn extract_rotation(&mut self, m: &Matrix4) -> &mut Matrix4 {
        let me = m.elements;
        let mut v = Vector3::zero();
        let scale_x = 1.0 / v.set_from_matrix_column(m, 0).length();
        let scale_y = 1.0 / v.set_from_matrix_column(m, 1).length();
        let scale_z = 1.0 / v.set_from_matrix_column(m, 2).length();
        let te = &mut self.elements;
        te[0] = me[0] * scale_x;
        te[1] = me[1] * scale_x;
        te[2] = me[2] * scale_x;
        te[3] = 0.0;
        te[4] = me[4] * scale_y;
        te[5] = me[5] * scale_y;
        te[6] = me[6] * scale_y;
        te[7] = 0.0;
        te[8] = me[8] * scale_z;
        te[9] = me[9] * scale_z;
        te[10] = me[10] * scale_z;
        te[11] = 0.0;
        te[12] = 0.0;
        te[13] = 0.0;
        te[14] = 0.0;
        te[15] = 1.0;
        self
    }

    pub fn make_rotation_from_euler(&mut self, euler: &Euler) -> &mut Matrix4 {
        let x = euler.x;
        let y = euler.y;
        let z = euler.z;
        let a = x.cos();
        let b = x.sin();
        let c = y.cos();
        let d = y.sin();
        let e = z.cos();
        let f = z.sin();
        let te = &mut self.elements;
        match euler.order {
            EulerOrder::Xyz => {
                let ae = a * e;
                let af = a * f;
                let be = b * e;
                let bf = b * f;
                te[0] = c * e;
                te[4] = -c * f;
                te[8] = d;
                te[1] = af + be * d;
                te[5] = ae - bf * d;
                te[9] = -b * c;
                te[2] = bf - ae * d;
                te[6] = be + af * d;
                te[10] = a * c;
            }
            EulerOrder::Yxz => {
                let ce = c * e;
                let cf = c * f;
                let de = d * e;
                let df = d * f;
                te[0] = ce + df * b;
                te[4] = de * b - cf;
                te[8] = a * d;
                te[1] = a * f;
                te[5] = a * e;
                te[9] = -b;
                te[2] = cf * b - de;
                te[6] = df + ce * b;
                te[10] = a * c;
            }
            EulerOrder::Zxy => {
                let ce = c * e;
                let cf = c * f;
                let de = d * e;
                let df = d * f;
                te[0] = ce - df * b;
                te[4] = -a * f;
                te[8] = de + cf * b;
                te[1] = cf + de * b;
                te[5] = a * e;
                te[9] = df - ce * b;
                te[2] = -a * d;
                te[6] = b;
                te[10] = a * c;
            }
            EulerOrder::Zyx => {
                let ae = a * e;
                let af = a * f;
                let be = b * e;
                let bf = b * f;
                te[0] = c * e;
                te[4] = be * d - af;
                te[8] = ae * d + bf;
                te[1] = c * f;
                te[5] = bf * d + ae;
                te[9] = af * d - be;
                te[2] = -d;
                te[6] = b * c;
                te[10] = a * c;
            }
            EulerOrder::Yzx => {
                let ac = a * c;
                let ad = a * d;
                let bc = b * c;
                let bd = b * d;
                te[0] = c * e;
                te[4] = bd - ac * f;
                te[8] = bc * f + ad;
                te[1] = f;
                te[5] = a * e;
                te[9] = -b * e;
                te[2] = -d * e;
                te[6] = ad * f + bc;
                te[10] = ac - bd * f;
            }
            EulerOrder::Xzy => {
                let ac = a * c;
                let ad = a * d;
                let bc = b * c;
                let bd = b * d;
                te[0] = c * e;
                te[4] = -f;
                te[8] = d * e;
                te[1] = ac * f + bd;
                te[5] = a * e;
                te[9] = ad * f - bc;
                te[2] = bc * f - ad;
                te[6] = b * e;
                te[10] = bd * f + ac;
            }
        }
        te[3] = 0.0;
        te[7] = 0.0;
        te[11] = 0.0;
        te[12] = 0.0;
        te[13] = 0.0;
        te[14] = 0.0;
        te[15] = 1.0;
        self
    }

    pub fn make_rotation_from_quaternion(&mut self, q: &Quaternion) -> &mut Matrix4 {
        let zero = Vector3::zero();
        let one = Vector3::one();
        self.compose(&zero, q, &one)
    }

    /// Rotates so that the -z axis... as three.js: +z points from `target` to `eye`.
    pub fn look_at(&mut self, eye: &Vector3, target: &Vector3, up: &Vector3) -> &mut Matrix4 {
        let mut z = Vector3::zero();
        z.sub_vectors(eye, target);
        if z.length_sq() == 0.0 {
            z.z = 1.0;
        }
        z.normalize();
        let mut x = Vector3::zero();
        x.cross_vectors(up, &z);
        if x.length_sq() == 0.0 {
            if up.z.abs() == 1.0 {
                z.x += 0.0001;
            } else {
                z.z += 0.0001;
            }
            z.normalize();
            x.cross_vectors(up, &z);
        }
        x.normalize();
        let mut y = Vector3::zero();
        y.cross_vectors(&z, &x);
        let te = &mut self.elements;
        te[0] = x.x;
        te[4] = y.x;
        te[8] = z.x;
        te[1] = x.y;
        te[5] = y.y;
        te[9] = z.y;
        te[2] = x.z;
        te[6] = y.z;
        te[10] = z.z;
        self
    }

    pub fn multiply(&mut self, m: &Matrix4) -> &mut Matrix4 {
        let a = *self;
        self.multiply_matrices(&a, m)
    }

    pub fn premultiply(&mut self, m: &Matrix4) -> &mut Matrix4 {
        let b = *self;
        self.multiply_matrices(m, &b)
    }

    pub fn multiply_matrices(&mut self, a: &Matrix4, b: &Matrix4) -> &mut Matrix4 {
        let ae = a.elements;
        let be = b.elements;
        let mut r = [0.0; 16];
        for col in 0..4 {
            for row in 0..4 {
                r[col * 4 + row] = ae[row] * be[col * 4]
                    + ae[4 + row] * be[col * 4 + 1]
                    + ae[8 + row] * be[col * 4 + 2]
                    + ae[12 + row] * be[col * 4 + 3];
            }
        }
        self.elements = r;
        self
    }

    pub fn multiply_scalar(&mut self, s: f64) -> &mut Matrix4 {
        for i in 0..16 {
            self.elements[i] *= s;
        }
        self
    }

    pub fn determinant(&self) -> f64 {
        let te = self.elements.clone();
        let n11 = te[0];
        let n12 = te[4];
        let n13 = te[8];
        let n14 = te[12];
        let n21 = te[1];
        let n22 = te[5];
        let n23 = te[9];
        let n24 = te[13];
        let n31 = te[2];
        let n32 = te[6];
        let n33 = te[10];
        let n34 = te[14];
        let n41 = te[3];
        let n42 = te[7];
        let n43 = te[11];
        let n44 = te[15];
        n41 * (n14 * n23 * n32 - n13 * n24 * n32 - n14 * n22 * n33 + n12 * n24 * n33 + n13 * n22 * n34
            - n12 * n23 * n34)
            + n42
                * (n11 * n23 * n34 - n11 * n24 * n33 + n14 * n21 * n33 - n13 * n21 * n34 + n13 * n24 * n31
                    - n14 * n23 * n31)
            + n43
                * (n11 * n24 * n32 - n11 * n22 * n34 - n14 * n21 * n32 + n12 * n21 * n34 + n14 * n22 * n31
                    - n12 * n24 * n31)
            + n44
                * (-n13 * n22 * n31 - n11 * n23 * n32 + n11 * n22 * n33 + n13 * n21 * n32 - n12 * n21 * n33
                    + n12 * n23 * n31)
    }

    pub fn transpose(&mut self) -> &mut Matrix4 {
        let m = self.elements.clone();
        for row in 0..4 {
            for col in 0..4 {
                self.elements[col * 4 + row] = m[row * 4 + col];
            }
        }
        self
    }

    pub fn set_position(&mut self, x: f64, y: f64, z: f64) -> &mut Matrix4 {
        self.elements[12] = x;
        self.elements[13] = y;
        self.elements[14] = z;
        self
    }

    /// Inverts in place; a singular matrix becomes all zeros (as three.js).
    pub fn invert(&mut self) -> &mut Matrix4 {
        let te = self.elements.clone();
        let n11 = te[0];
        let n21 = te[1];
        let n31 = te[2];
        let n41 = te[3];
        let n12 = te[4];
        let n22 = te[5];
        let n32 = te[6];
        let n42 = te[7];
        let n13 = te[8];
        let n23 = te[9];
        let n33 = te[10];
        let n43 = te[11];
        let n14 = te[12];
        let n24 = te[13];
        let n34 = te[14];
        let n44 = te[15];
        let t11 = n23 * n34 * n42 - n24 * n33 * n42 + n24 * n32 * n43 - n22 * n34 * n43 - n23 * n32 * n44
            + n22 * n33 * n44;
        let t12 = n14 * n33 * n42 - n13 * n34 * n42 - n14 * n32 * n43 + n12 * n34 * n43 + n13 * n32 * n44
            - n12 * n33 * n44;
        let t13 = n13 * n24 * n42 - n14 * n23 * n42 + n14 * n22 * n43 - n12 * n24 * n43 - n13 * n22 * n44
            + n12 * n23 * n44;
        let t14 = n14 * n23 * n32 - n13 * n24 * n32 - n14 * n22 * n33 + n12 * n24 * n33 + n13 * n22 * n34
            - n12 * n23 * n34;
        let det = n11 * t11 + n21 * t12 + n31 * t13 + n41 * t14;
        if det == 0.0 {
            self.elements = [0.0; 16];
            return self;
        }
        let d = 1.0 / det;
        let o = &mut self.elements;
        o[0] = t11 * d;
        o[1] = (n24 * n33 * n41 - n23 * n34 * n41 - n24 * n31 * n43 + n21 * n34 * n43 + n23 * n31 * n44
            - n21 * n33 * n44)
            * d;
        o[2] = (n22 * n34 * n41 - n24 * n32 * n41 + n24 * n31 * n42 - n21 * n34 * n42 - n22 * n31 * n44
            + n21 * n32 * n44)
            * d;
        o[3] = (n23 * n32 * n41 - n22 * n33 * n41 - n23 * n31 * n42 + n21 * n33 * n42 + n22 * n31 * n43
            - n21 * n32 * n43)
            * d;
        o[4] = t12 * d;
        o[5] = (n13 * n34 * n41 - n14 * n33 * n41 + n14 * n31 * n43 - n11 * n34 * n43 - n13 * n31 * n44
            + n11 * n33 * n44)
            * d;
        o[6] = (n14 * n32 * n41 - n12 * n34 * n41 - n14 * n31 * n42 + n11 * n34 * n42 + n12 * n31 * n44
            - n11 * n32 * n44)
            * d;
        o[7] = (n12 * n33 * n41 - n13 * n32 * n41 + n13 * n31 * n42 - n11 * n33 * n42 - n12 * n31 * n43
            + n11 * n32 * n43)
            * d;
        o[8] = t13 * d;
        o[9] = (n14 * n23 * n41 - n13 * n24 * n41 - n14 * n21 * n43 + n11 * n24 * n43 + n13 * n21 * n44
            - n11 * n23 * n44)
            * d;
        o[10] = (n12 * n24 * n41 - n14 * n22 * n41 + n14 * n21 * n42 - n11 * n24 * n42 - n12 * n21 * n44
            + n11 * n22 * n44)
            * d;
        o[11] = (n13 * n22 * n41 - n12 * n23 * n41 - n13 * n21 * n42 + n11 * n23 * n42 + n12 * n21 * n43
            - n11 * n22 * n43)
            * d;
        o[12] = t14 * d;
        o[13] = (n13 * n24 * n31 - n14 * n23 * n31 + n14 * n21 * n33 - n11 * n24 * n33 - n13 * n21 * n34
            + n11 * n23 * n34)
            * d;
        o[14] = (n14 * n22 * n31 - n12 * n24 * n31 - n14 * n21 * n32 + n11 * n24 * n32 + n12 * n21 * n34
            - n11 * n22 * n34)
            * d;
        o[15] = (n12 * n23 * n31 - n13 * n22 * n31 + n13 * n21 * n32 - n11 * n23 * n32 - n12 * n21 * n33
            + n11 * n22 * n33)
            * d;
        self
    }

    pub fn scale(&mut self, v: &Vector3) -> &mut Matrix4 {
        for i in 0..4 {
            self.elements[i] *= v.x;
            self.elements[4 + i] *= v.y;
            self.elements[8 + i] *= v.z;
        }
        self
    }

    pub fn get_max_scale_on_axis(&self) -> f64 {
        let te = self.elements.clone();
        let sx = te[0] * te[0] + te[1] * te[1] + te[2] * te[2];
        let sy = te[4] * te[4] + te[5] * te[5] + te[6] * te[6];
        let sz = te[8] * te[8] + te[9] * te[9] + te[10] * te[10];
        sx.max(sy).max(sz).sqrt()
    }

    pub fn make_translation(&mut self, x: f64, y: f64, z: f64) -> &mut Matrix4 {
        self.set(1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn make_rotation_x(&mut self, theta: f64) -> &mut Matrix4 {
        let c = theta.cos();
        let s = theta.sin();
        self.set(1.0, 0.0, 0.0, 0.0, 0.0, c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn make_rotation_y(&mut self, theta: f64) -> &mut Matrix4 {
        let c = theta.cos();
        let s = theta.sin();
        self.set(c, 0.0, s, 0.0, 0.0, 1.0, 0.0, 0.0, -s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn make_rotation_z(&mut self, theta: f64) -> &mut Matrix4 {
        let c = theta.cos();
        let s = theta.sin();
        self.set(c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0)
    }

    /// Rotation by `angle` around the unit vector `axis`.
    pub fn make_rotation_axis(&mut self, axis: &Vector3, angle: f64) -> &mut Matrix4 {
        let c = angle.cos();
        let s = angle.sin();
        let t = 1.0 - c;
        let x = axis.x;
        let y = axis.y;
        let z = axis.z;
        let tx = t * x;
        let ty = t * y;
        self.set(
            tx * x + c,
            tx * y - s * z,
            tx * z + s * y,
            0.0,
            tx * y + s * z,
            ty * y + c,
            ty * z - s * x,
            0.0,
            tx * z - s * y,
            ty * z + s * x,
            t * z * z + c,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }

    pub fn make_scale(&mut self, x: f64, y: f64, z: f64) -> &mut Matrix4 {
        self.set(x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, z, 0.0, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn make_shear(&mut self, xy: f64, xz: f64, yx: f64, yz: f64, zx: f64, zy: f64) -> &mut Matrix4 {
        self.set(1.0, yx, zx, 0.0, xy, 1.0, zy, 0.0, xz, yz, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn compose(&mut self, position: &Vector3, q: &Quaternion, scale: &Vector3) -> &mut Matrix4 {
        let x = q.x;
        let y = q.y;
        let z = q.z;
        let w = q.w;
        let x2 = x + x;
        let y2 = y + y;
        let z2 = z + z;
        let xx = x * x2;
        let xy = x * y2;
        let xz = x * z2;
        let yy = y * y2;
        let yz = y * z2;
        let zz = z * z2;
        let wx = w * x2;
        let wy = w * y2;
        let wz = w * z2;
        let sx = scale.x;
        let sy = scale.y;
        let sz = scale.z;
        let te = &mut self.elements;
        te[0] = (1.0 - (yy + zz)) * sx;
        te[1] = (xy + wz) * sx;
        te[2] = (xz - wy) * sx;
        te[3] = 0.0;
        te[4] = (xy - wz) * sy;
        te[5] = (1.0 - (xx + zz)) * sy;
        te[6] = (yz + wx) * sy;
        te[7] = 0.0;
        te[8] = (xz + wy) * sz;
        te[9] = (yz - wx) * sz;
        te[10] = (1.0 - (xx + yy)) * sz;
        te[11] = 0.0;
        te[12] = position.x;
        te[13] = position.y;
        te[14] = position.z;
        te[15] = 1.0;
        self
    }

    pub fn decompose(&self) -> Transform {
        let te = self.elements.clone();
        let mut v = Vector3::zero();
        let mut sx = v.set(te[0], te[1], te[2]).length();
        let sy = v.set(te[4], te[5], te[6]).length();
        let sz = v.set(te[8], te[9], te[10]).length();
        if self.determinant() < 0.0 {
            sx = -sx;
        }
        let position = Vector3::new(te[12], te[13], te[14]);
        let mut m = *self;
        let inv_sx = 1.0 / sx;
        let inv_sy = 1.0 / sy;
        let inv_sz = 1.0 / sz;
        m.elements[0] *= inv_sx;
        m.elements[1] *= inv_sx;
        m.elements[2] *= inv_sx;
        m.elements[4] *= inv_sy;
        m.elements[5] *= inv_sy;
        m.elements[6] *= inv_sy;
        m.elements[8] *= inv_sz;
        m.elements[9] *= inv_sz;
        m.elements[10] *= inv_sz;
        let mut quaternion = Quaternion::identity();
        quaternion.set_from_rotation_matrix(&m);
        Transform {
            position,
            quaternion,
            scale: Vector3::new(sx, sy, sz),
        }
    }

    /// A perspective projection of the frustum through the near plane's
    /// `left..right` and `bottom..top` (WebGL clip space, z in -1..1).
    pub fn make_perspective(&mut self, left: f64, right: f64, top: f64, bottom: f64, near: f64, far: f64) -> &mut Matrix4 {
        let x = 2.0 * near / (right - left);
        let y = 2.0 * near / (top - bottom);
        let a = (right + left) / (right - left);
        let b = (top + bottom) / (top - bottom);
        let c = -(far + near) / (far - near);
        let d = (-2.0 * far * near) / (far - near);
        self.set(x, 0.0, a, 0.0, 0.0, y, b, 0.0, 0.0, 0.0, c, d, 0.0, 0.0, -1.0, 0.0)
    }

    pub fn make_orthographic(&mut self, left: f64, right: f64, top: f64, bottom: f64, near: f64, far: f64) -> &mut Matrix4 {
        let w = 1.0 / (right - left);
        let h = 1.0 / (top - bottom);
        let p = 1.0 / (far - near);
        let x = (right + left) * w;
        let y = (top + bottom) * h;
        let z = (far + near) * p;
        self.set(2.0 * w, 0.0, 0.0, -x, 0.0, 2.0 * h, 0.0, -y, 0.0, 0.0, -2.0 * p, -z, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn equals(&self, m: &Matrix4) -> bool {
        for i in 0..16 {
            if self.elements[i] != m.elements[i] {
                return false;
            }
        }
        true
    }

    pub fn from_array(&mut self, array: &[f64], offset: usize) -> &mut Matrix4 {
        for i in 0..16 {
            self.elements[i] = array[i + offset];
        }
        self
    }

    pub fn to_array(&self) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        for i in 0..16 {
            out.push(self.elements[i]);
        }
        out
    }
}
