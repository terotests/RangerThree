//! A rotation as three angles applied in a given axis order (three.js `Euler`).

use ranger::prelude::*;

use super::math_utils::clamp;
use super::matrix4::Matrix4;
use super::quaternion::Quaternion;
use super::vector3::Vector3;

/// The order the axis rotations apply in (three.js's order strings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EulerOrder {
    Xyz,
    Yxz,
    Zxy,
    Zyx,
    Yzx,
    Xzy,
}

impl EulerOrder {
    pub fn name(&self) -> String {
        match self {
            EulerOrder::Xyz => "XYZ".to_string(),
            EulerOrder::Yxz => "YXZ".to_string(),
            EulerOrder::Zxy => "ZXY".to_string(),
            EulerOrder::Zyx => "ZYX".to_string(),
            EulerOrder::Yzx => "YZX".to_string(),
            EulerOrder::Xzy => "XZY".to_string(),
        }
    }

    /// The order a three.js order string names; anything else is XYZ.
    pub fn parse(name: &str) -> EulerOrder {
        match name {
            "YXZ" => EulerOrder::Yxz,
            "ZXY" => EulerOrder::Zxy,
            "ZYX" => EulerOrder::Zyx,
            "YZX" => EulerOrder::Yzx,
            "XZY" => EulerOrder::Xzy,
            _ => EulerOrder::Xyz,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Euler {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub order: EulerOrder,
}

impl Default for Euler {
    fn default() -> Euler {
        Euler::new(0.0, 0.0, 0.0, EulerOrder::Xyz)
    }
}

impl Euler {
    pub fn new(x: f64, y: f64, z: f64, order: EulerOrder) -> Euler {
        Euler { x, y, z, order }
    }

    pub fn set(&mut self, x: f64, y: f64, z: f64, order: EulerOrder) -> &mut Euler {
        self.x = x;
        self.y = y;
        self.z = z;
        self.order = order;
        self
    }

    pub fn copy(&mut self, e: &Euler) -> &mut Euler {
        self.set(e.x, e.y, e.z, e.order)
    }

    /// The angles of the rotation in the upper 3x3 of `m` (unscaled), in `order`.
    pub fn set_from_rotation_matrix(&mut self, m: &Matrix4, order: EulerOrder) -> &mut Euler {
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
        let limit = 0.9999999;
        match order {
            EulerOrder::Xyz => {
                self.y = clamp(m13, -1.0, 1.0).asin();
                if m13.abs() < limit {
                    self.x = (-m23).atan2(m33);
                    self.z = (-m12).atan2(m11);
                } else {
                    self.x = m32.atan2(m22);
                    self.z = 0.0;
                }
            }
            EulerOrder::Yxz => {
                self.x = (-clamp(m23, -1.0, 1.0)).asin();
                if m23.abs() < limit {
                    self.y = m13.atan2(m33);
                    self.z = m21.atan2(m22);
                } else {
                    self.y = (-m31).atan2(m11);
                    self.z = 0.0;
                }
            }
            EulerOrder::Zxy => {
                self.x = clamp(m32, -1.0, 1.0).asin();
                if m32.abs() < limit {
                    self.y = (-m31).atan2(m33);
                    self.z = (-m12).atan2(m22);
                } else {
                    self.y = 0.0;
                    self.z = m21.atan2(m11);
                }
            }
            EulerOrder::Zyx => {
                self.y = (-clamp(m31, -1.0, 1.0)).asin();
                if m31.abs() < limit {
                    self.x = m32.atan2(m33);
                    self.z = m21.atan2(m11);
                } else {
                    self.x = 0.0;
                    self.z = (-m12).atan2(m22);
                }
            }
            EulerOrder::Yzx => {
                self.z = clamp(m21, -1.0, 1.0).asin();
                if m21.abs() < limit {
                    self.x = (-m23).atan2(m22);
                    self.y = (-m31).atan2(m11);
                } else {
                    self.x = 0.0;
                    self.y = m13.atan2(m33);
                }
            }
            EulerOrder::Xzy => {
                self.z = (-clamp(m12, -1.0, 1.0)).asin();
                if m12.abs() < limit {
                    self.x = m32.atan2(m22);
                    self.y = m13.atan2(m11);
                } else {
                    self.x = (-m23).atan2(m33);
                    self.y = 0.0;
                }
            }
        }
        self.order = order;
        self
    }

    pub fn set_from_quaternion(&mut self, q: &Quaternion, order: EulerOrder) -> &mut Euler {
        let mut m = Matrix4::identity();
        m.make_rotation_from_quaternion(q);
        self.set_from_rotation_matrix(&m, order)
    }

    pub fn set_from_vector3(&mut self, v: &Vector3, order: EulerOrder) -> &mut Euler {
        self.set(v.x, v.y, v.z, order)
    }

    /// The same rotation expressed in another order.
    pub fn reorder(&mut self, order: EulerOrder) -> &mut Euler {
        let q = Quaternion::from_euler(self);
        self.set_from_quaternion(&q, order)
    }

    pub fn equals(&self, e: &Euler) -> bool {
        self.x == e.x && self.y == e.y && self.z == e.z && self.order == e.order
    }

    pub fn to_vector3(&self) -> Vector3 {
        Vector3::new(self.x, self.y, self.z)
    }
}
