//! Spherical coordinates (three.js `Spherical`): `phi` from the +y axis,
//! `theta` around it from +z.

use ranger::prelude::*;

use super::math_utils::clamp;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spherical {
    pub radius: f64,
    pub phi: f64,
    pub theta: f64,
}

impl Default for Spherical {
    fn default() -> Spherical {
        Spherical::new(1.0, 0.0, 0.0)
    }
}

impl Spherical {
    pub fn new(radius: f64, phi: f64, theta: f64) -> Spherical {
        Spherical { radius, phi, theta }
    }

    pub fn set(&mut self, radius: f64, phi: f64, theta: f64) -> &mut Spherical {
        self.radius = radius;
        self.phi = phi;
        self.theta = theta;
        self
    }

    pub fn copy(&mut self, s: &Spherical) -> &mut Spherical {
        self.set(s.radius, s.phi, s.theta)
    }

    /// Keeps `phi` off the poles, where `theta` is undefined.
    pub fn make_safe(&mut self) -> &mut Spherical {
        let eps = 0.000001;
        self.phi = clamp(self.phi, eps, std::f64::consts::PI - eps);
        self
    }

    pub fn set_from_vector3(&mut self, v: &Vector3) -> &mut Spherical {
        self.set_from_cartesian_coords(v.x, v.y, v.z)
    }

    pub fn set_from_cartesian_coords(&mut self, x: f64, y: f64, z: f64) -> &mut Spherical {
        self.radius = (x * x + y * y + z * z).sqrt();
        if self.radius == 0.0 {
            self.theta = 0.0;
            self.phi = 0.0;
        } else {
            self.theta = x.atan2(z);
            self.phi = clamp(y / self.radius, -1.0, 1.0).acos();
        }
        self
    }
}
