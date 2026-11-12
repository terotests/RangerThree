//! A plane `normal · p + constant = 0` (three.js `Plane`).

use ranger::prelude::*;

use super::box3::Box3;
use super::line3::Line3;
use super::matrix3::Matrix3;
use super::matrix4::Matrix4;
use super::sphere::Sphere;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: Vector3,
    pub constant: f64,
}

impl Default for Plane {
    fn default() -> Plane {
        Plane::new(Vector3::new(1.0, 0.0, 0.0), 0.0)
    }
}

impl Plane {
    pub fn new(normal: Vector3, constant: f64) -> Plane {
        Plane { normal, constant }
    }

    pub fn set(&mut self, normal: &Vector3, constant: f64) -> &mut Plane {
        self.normal = *normal;
        self.constant = constant;
        self
    }

    pub fn set_components(&mut self, x: f64, y: f64, z: f64, w: f64) -> &mut Plane {
        self.normal.set(x, y, z);
        self.constant = w;
        self
    }

    pub fn set_from_normal_and_coplanar_point(&mut self, normal: &Vector3, point: &Vector3) -> &mut Plane {
        self.normal = *normal;
        self.constant = -point.dot(&self.normal);
        self
    }

    /// The plane through three points, wound counter-clockwise.
    pub fn set_from_coplanar_points(&mut self, a: &Vector3, b: &Vector3, c: &Vector3) -> &mut Plane {
        let mut v1 = *c;
        v1.sub(b);
        let mut v2 = *a;
        v2.sub(b);
        let mut normal = v1;
        normal.cross(&v2).normalize();
        self.set_from_normal_and_coplanar_point(&normal, a)
    }

    pub fn copy(&mut self, p: &Plane) -> &mut Plane {
        self.normal = p.normal;
        self.constant = p.constant;
        self
    }

    pub fn normalize(&mut self) -> &mut Plane {
        let inv = 1.0 / self.normal.length();
        self.normal.multiply_scalar(inv);
        self.constant *= inv;
        self
    }

    pub fn negate(&mut self) -> &mut Plane {
        self.constant *= -1.0;
        self.normal.negate();
        self
    }

    pub fn distance_to_point(&self, p: &Vector3) -> f64 {
        self.normal.dot(p) + self.constant
    }

    pub fn distance_to_sphere(&self, s: &Sphere) -> f64 {
        self.distance_to_point(&s.center) - s.radius
    }

    pub fn project_point(&self, p: &Vector3) -> Vector3 {
        let mut out = self.normal;
        out.multiply_scalar(-self.distance_to_point(p)).add(p);
        out
    }

    /// Where the segment crosses the plane; `None` when it does not.
    pub fn intersect_line(&self, line: &Line3) -> Option<Vector3> {
        let direction = line.delta();
        let denominator = self.normal.dot(&direction);
        if denominator == 0.0 {
            if self.distance_to_point(&line.start) == 0.0 {
                return Some(line.start);
            }
            return None;
        }
        let t = -(line.start.dot(&self.normal) + self.constant) / denominator;
        if t < 0.0 || t > 1.0 {
            return None;
        }
        let mut out = line.start;
        out.add_scaled_vector(&direction, t);
        Some(out)
    }

    pub fn intersects_line(&self, line: &Line3) -> bool {
        let start = self.distance_to_point(&line.start);
        let end = self.distance_to_point(&line.end);
        (start < 0.0 && end > 0.0) || (end < 0.0 && start > 0.0)
    }

    pub fn intersects_box(&self, b: &Box3) -> bool {
        b.intersects_plane(self)
    }

    pub fn intersects_sphere(&self, s: &Sphere) -> bool {
        s.intersects_plane(self)
    }

    pub fn coplanar_point(&self) -> Vector3 {
        let mut out = self.normal;
        out.multiply_scalar(-self.constant);
        out
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) -> &mut Plane {
        let mut normal_matrix = Matrix3::identity();
        normal_matrix.get_normal_matrix(m);
        let mut reference = self.coplanar_point();
        reference.apply_matrix4(m);
        let mut normal = self.normal;
        normal.apply_matrix3(&normal_matrix).normalize();
        self.normal = normal;
        self.constant = -reference.dot(&normal);
        self
    }

    pub fn translate(&mut self, offset: &Vector3) -> &mut Plane {
        self.constant -= offset.dot(&self.normal);
        self
    }

    pub fn equals(&self, p: &Plane) -> bool {
        self.normal.equals(&p.normal) && self.constant == p.constant
    }
}
