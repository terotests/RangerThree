//! A 3x3 matrix (three.js `Matrix3`), stored column-major like WebGL.

use ranger::prelude::*;

use super::matrix4::Matrix4;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix3 {
    pub elements: [f64; 9],
}

impl Default for Matrix3 {
    fn default() -> Matrix3 {
        Matrix3::identity()
    }
}

impl Matrix3 {
    pub fn identity() -> Matrix3 {
        Matrix3 {
            elements: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        }
    }

    /// Takes the entries row by row (as three.js's `set`).
    pub fn from_rows(
        n11: f64, n12: f64, n13: f64,
        n21: f64, n22: f64, n23: f64,
        n31: f64, n32: f64, n33: f64,
    ) -> Matrix3 {
        let mut m = Matrix3::identity();
        m.set(n11, n12, n13, n21, n22, n23, n31, n32, n33);
        m
    }

    pub fn set(
        &mut self,
        n11: f64, n12: f64, n13: f64,
        n21: f64, n22: f64, n23: f64,
        n31: f64, n32: f64, n33: f64,
    ) -> &mut Matrix3 {
        let te = &mut self.elements;
        te[0] = n11;
        te[1] = n21;
        te[2] = n31;
        te[3] = n12;
        te[4] = n22;
        te[5] = n32;
        te[6] = n13;
        te[7] = n23;
        te[8] = n33;
        self
    }

    pub fn make_identity(&mut self) -> &mut Matrix3 {
        self.set(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn copy(&mut self, m: &Matrix3) -> &mut Matrix3 {
        self.elements = m.elements;
        self
    }

    /// The three columns as basis vectors.
    pub fn extract_basis(&self) -> Vec<Vector3> {
        let mut x = Vector3::zero();
        let mut y = Vector3::zero();
        let mut z = Vector3::zero();
        x.set_from_matrix3_column(self, 0);
        y.set_from_matrix3_column(self, 1);
        z.set_from_matrix3_column(self, 2);
        vec![x, y, z]
    }

    /// The upper left 3x3 of `m`.
    pub fn set_from_matrix4(&mut self, m: &Matrix4) -> &mut Matrix3 {
        let me = m.elements;
        self.set(me[0], me[4], me[8], me[1], me[5], me[9], me[2], me[6], me[10])
    }

    pub fn multiply(&mut self, m: &Matrix3) -> &mut Matrix3 {
        let a = *self;
        self.multiply_matrices(&a, m)
    }

    pub fn premultiply(&mut self, m: &Matrix3) -> &mut Matrix3 {
        let b = *self;
        self.multiply_matrices(m, &b)
    }

    pub fn multiply_matrices(&mut self, a: &Matrix3, b: &Matrix3) -> &mut Matrix3 {
        let ae = a.elements;
        let be = b.elements;
        let a11 = ae[0];
        let a12 = ae[3];
        let a13 = ae[6];
        let a21 = ae[1];
        let a22 = ae[4];
        let a23 = ae[7];
        let a31 = ae[2];
        let a32 = ae[5];
        let a33 = ae[8];
        let b11 = be[0];
        let b12 = be[3];
        let b13 = be[6];
        let b21 = be[1];
        let b22 = be[4];
        let b23 = be[7];
        let b31 = be[2];
        let b32 = be[5];
        let b33 = be[8];
        let te = &mut self.elements;
        te[0] = a11 * b11 + a12 * b21 + a13 * b31;
        te[3] = a11 * b12 + a12 * b22 + a13 * b32;
        te[6] = a11 * b13 + a12 * b23 + a13 * b33;
        te[1] = a21 * b11 + a22 * b21 + a23 * b31;
        te[4] = a21 * b12 + a22 * b22 + a23 * b32;
        te[7] = a21 * b13 + a22 * b23 + a23 * b33;
        te[2] = a31 * b11 + a32 * b21 + a33 * b31;
        te[5] = a31 * b12 + a32 * b22 + a33 * b32;
        te[8] = a31 * b13 + a32 * b23 + a33 * b33;
        self
    }

    pub fn multiply_scalar(&mut self, s: f64) -> &mut Matrix3 {
        for i in 0..9 {
            self.elements[i] *= s;
        }
        self
    }

    pub fn determinant(&self) -> f64 {
        let te = self.elements.clone();
        let a = te[0];
        let b = te[1];
        let c = te[2];
        let d = te[3];
        let e = te[4];
        let f = te[5];
        let g = te[6];
        let h = te[7];
        let i = te[8];
        a * e * i - a * f * h - b * d * i + b * f * g + c * d * h - c * e * g
    }

    /// Inverts in place; a singular matrix becomes all zeros (as three.js).
    pub fn invert(&mut self) -> &mut Matrix3 {
        let te = self.elements.clone();
        let n11 = te[0];
        let n21 = te[1];
        let n31 = te[2];
        let n12 = te[3];
        let n22 = te[4];
        let n32 = te[5];
        let n13 = te[6];
        let n23 = te[7];
        let n33 = te[8];
        let t11 = n33 * n22 - n32 * n23;
        let t12 = n32 * n13 - n33 * n12;
        let t13 = n23 * n12 - n22 * n13;
        let det = n11 * t11 + n21 * t12 + n31 * t13;
        if det == 0.0 {
            return self.set(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        }
        let det_inv = 1.0 / det;
        let o = &mut self.elements;
        o[0] = t11 * det_inv;
        o[1] = (n31 * n23 - n33 * n21) * det_inv;
        o[2] = (n32 * n21 - n31 * n22) * det_inv;
        o[3] = t12 * det_inv;
        o[4] = (n33 * n11 - n31 * n13) * det_inv;
        o[5] = (n31 * n12 - n32 * n11) * det_inv;
        o[6] = t13 * det_inv;
        o[7] = (n21 * n13 - n23 * n11) * det_inv;
        o[8] = (n22 * n11 - n21 * n12) * det_inv;
        self
    }

    pub fn transpose(&mut self) -> &mut Matrix3 {
        let m = self.elements.clone();
        let t = &mut self.elements;
        t[1] = m[3];
        t[3] = m[1];
        t[2] = m[6];
        t[6] = m[2];
        t[5] = m[7];
        t[7] = m[5];
        self
    }

    /// The inverse transpose of the upper 3x3 of `m`: normals' transform.
    pub fn get_normal_matrix(&mut self, m: &Matrix4) -> &mut Matrix3 {
        self.set_from_matrix4(m).invert().transpose()
    }

    pub fn transpose_into_array(&self) -> Vec<f64> {
        let m = self.elements.clone();
        vec![m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]]
    }

    pub fn set_uv_transform(&mut self, tx: f64, ty: f64, sx: f64, sy: f64, rotation: f64, cx: f64, cy: f64) -> &mut Matrix3 {
        let c = rotation.cos();
        let s = rotation.sin();
        self.set(
            sx * c,
            sx * s,
            -sx * (c * cx + s * cy) + cx + tx,
            -sy * s,
            sy * c,
            -sy * (-s * cx + c * cy) + cy + ty,
            0.0,
            0.0,
            1.0,
        )
    }

    pub fn scale(&mut self, sx: f64, sy: f64) -> &mut Matrix3 {
        let m = Matrix3::scaling(sx, sy);
        self.premultiply(&m)
    }

    pub fn rotate(&mut self, theta: f64) -> &mut Matrix3 {
        let m = Matrix3::rotation(-theta);
        self.premultiply(&m)
    }

    pub fn translate(&mut self, tx: f64, ty: f64) -> &mut Matrix3 {
        let m = Matrix3::translation(tx, ty);
        self.premultiply(&m)
    }

    pub fn translation(x: f64, y: f64) -> Matrix3 {
        Matrix3::from_rows(1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0)
    }

    pub fn rotation(theta: f64) -> Matrix3 {
        let c = theta.cos();
        let s = theta.sin();
        Matrix3::from_rows(c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn scaling(x: f64, y: f64) -> Matrix3 {
        Matrix3::from_rows(x, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 1.0)
    }

    pub fn make_translation(&mut self, x: f64, y: f64) -> &mut Matrix3 {
        let m = Matrix3::translation(x, y);
        self.copy(&m)
    }

    pub fn make_rotation(&mut self, theta: f64) -> &mut Matrix3 {
        let m = Matrix3::rotation(theta);
        self.copy(&m)
    }

    pub fn make_scale(&mut self, x: f64, y: f64) -> &mut Matrix3 {
        let m = Matrix3::scaling(x, y);
        self.copy(&m)
    }

    pub fn equals(&self, m: &Matrix3) -> bool {
        for i in 0..9 {
            if self.elements[i] != m.elements[i] {
                return false;
            }
        }
        true
    }

    pub fn from_array(&mut self, array: &[f64], offset: usize) -> &mut Matrix3 {
        for i in 0..9 {
            self.elements[i] = array[i + offset];
        }
        self
    }

    pub fn to_array(&self) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        for i in 0..9 {
            out.push(self.elements[i]);
        }
        out
    }
}
