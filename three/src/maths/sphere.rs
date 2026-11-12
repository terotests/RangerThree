//! A sphere (three.js `Sphere`). A negative radius is the empty sphere.

use ranger::prelude::*;

use super::box3::Box3;
use super::matrix4::Matrix4;
use super::plane::Plane;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub center: Vector3,
    pub radius: f64,
}

impl Default for Sphere {
    fn default() -> Sphere {
        Sphere::empty()
    }
}

impl Sphere {
    pub fn new(center: Vector3, radius: f64) -> Sphere {
        Sphere { center, radius }
    }

    pub fn empty() -> Sphere {
        Sphere::new(Vector3::zero(), -1.0)
    }

    pub fn set(&mut self, center: &Vector3, radius: f64) -> &mut Sphere {
        self.center = *center;
        self.radius = radius;
        self
    }

    /// The sphere around `points`, centred on their bounding box's centre.
    pub fn set_from_points(&mut self, points: &Vec<Vector3>) -> &mut Sphere {
        let mut b = Box3::empty();
        b.set_from_points(points);
        let center = b.get_center();
        self.set_from_points_around(points, &center)
    }

    /// The smallest sphere centred on `center` holding `points`.
    pub fn set_from_points_around(&mut self, points: &Vec<Vector3>, center: &Vector3) -> &mut Sphere {
        let mut max_sq: f64 = 0.0;
        for p in points.iter() {
            max_sq = max_sq.max(center.distance_to_squared(p));
        }
        self.center = *center;
        self.radius = max_sq.sqrt();
        self
    }

    pub fn copy(&mut self, s: &Sphere) -> &mut Sphere {
        self.center = s.center;
        self.radius = s.radius;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.radius < 0.0
    }

    pub fn make_empty(&mut self) -> &mut Sphere {
        self.center.set(0.0, 0.0, 0.0);
        self.radius = -1.0;
        self
    }

    pub fn contains_point(&self, p: &Vector3) -> bool {
        p.distance_to_squared(&self.center) <= self.radius * self.radius
    }

    pub fn distance_to_point(&self, p: &Vector3) -> f64 {
        p.distance_to(&self.center) - self.radius
    }

    pub fn intersects_sphere(&self, s: &Sphere) -> bool {
        let r = self.radius + s.radius;
        s.center.distance_to_squared(&self.center) <= r * r
    }

    pub fn intersects_box(&self, b: &Box3) -> bool {
        b.intersects_sphere(self)
    }

    pub fn intersects_plane(&self, plane: &Plane) -> bool {
        plane.distance_to_point(&self.center).abs() <= self.radius
    }

    pub fn clamp_point(&self, p: &Vector3) -> Vector3 {
        let delta_sq = self.center.distance_to_squared(p);
        let mut out = *p;
        if delta_sq > self.radius * self.radius {
            out.sub(&self.center).normalize();
            out.multiply_scalar(self.radius).add(&self.center);
        }
        out
    }

    pub fn get_bounding_box(&self) -> Box3 {
        if self.is_empty() {
            return Box3::empty();
        }
        let mut b = Box3::new(self.center, self.center);
        b.expand_by_scalar(self.radius);
        b
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) -> &mut Sphere {
        self.center.apply_matrix4(m);
        self.radius *= m.get_max_scale_on_axis();
        self
    }

    pub fn translate(&mut self, offset: &Vector3) -> &mut Sphere {
        self.center.add(offset);
        self
    }

    pub fn expand_by_point(&mut self, p: &Vector3) -> &mut Sphere {
        if self.is_empty() {
            self.center = *p;
            self.radius = 0.0;
            return self;
        }
        let mut v = *p;
        v.sub(&self.center);
        let length_sq = v.length_sq();
        if length_sq > self.radius * self.radius {
            let length = length_sq.sqrt();
            let delta = (length - self.radius) * 0.5;
            v.multiply_scalar(delta / length);
            self.center.add(&v);
            self.radius += delta;
        }
        self
    }

    pub fn union(&mut self, s: &Sphere) -> &mut Sphere {
        if s.is_empty() {
            return self;
        }
        if self.is_empty() {
            return self.copy(s);
        }
        if self.center.equals(&s.center) {
            self.radius = self.radius.max(s.radius);
        } else {
            let mut v = s.center;
            v.sub(&self.center);
            v.set_length(s.radius);
            let mut far = s.center;
            far.add(&v);
            self.expand_by_point(&far);
            let mut near = s.center;
            near.sub(&v);
            self.expand_by_point(&near);
        }
        self
    }

    pub fn equals(&self, s: &Sphere) -> bool {
        self.center.equals(&s.center) && self.radius == s.radius
    }
}
