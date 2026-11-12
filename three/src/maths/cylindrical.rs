//! Cylindrical coordinates (three.js `Cylindrical`).

use ranger::prelude::*;

use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cylindrical {
    pub radius: f64,
    pub theta: f64,
    pub y: f64,
}

impl Default for Cylindrical {
    fn default() -> Cylindrical {
        Cylindrical::new(1.0, 0.0, 0.0)
    }
}

impl Cylindrical {
    pub fn new(radius: f64, theta: f64, y: f64) -> Cylindrical {
        Cylindrical { radius, theta, y }
    }

    pub fn set(&mut self, radius: f64, theta: f64, y: f64) -> &mut Cylindrical {
        self.radius = radius;
        self.theta = theta;
        self.y = y;
        self
    }

    pub fn copy(&mut self, c: &Cylindrical) -> &mut Cylindrical {
        self.set(c.radius, c.theta, c.y)
    }

    pub fn set_from_vector3(&mut self, v: &Vector3) -> &mut Cylindrical {
        self.set_from_cartesian_coords(v.x, v.y, v.z)
    }

    pub fn set_from_cartesian_coords(&mut self, x: f64, y: f64, z: f64) -> &mut Cylindrical {
        self.radius = (x * x + z * z).sqrt();
        self.theta = x.atan2(z);
        self.y = y;
        self
    }
}
