//! Six planes bounding what a camera sees (three.js `Frustum`).

use ranger::prelude::*;

use super::box3::Box3;
use super::matrix4::Matrix4;
use super::plane::Plane;
use super::sphere::Sphere;
use super::vector3::Vector3;

#[derive(Clone, Debug, PartialEq)]
pub struct Frustum {
    pub planes: Vec<Plane>,
}

impl Default for Frustum {
    fn default() -> Frustum {
        Frustum::new()
    }
}

impl Frustum {
    pub fn new() -> Frustum {
        let mut planes: Vec<Plane> = Vec::new();
        for _ in 0..6 {
            planes.push(Plane::default());
        }
        Frustum { planes }
    }

    pub fn set(&mut self, planes: &Vec<Plane>) -> &mut Frustum {
        for i in 0..6 {
            self.planes[i] = planes[i];
        }
        self
    }

    pub fn copy(&mut self, f: &Frustum) -> &mut Frustum {
        for i in 0..6 {
            self.planes[i] = f.planes[i];
        }
        self
    }

    /// The planes of a projection (times view) matrix, WebGL clip space.
    pub fn set_from_projection_matrix(&mut self, m: &Matrix4) -> &mut Frustum {
        let me = m.elements;
        let me0 = me[0];
        let me1 = me[1];
        let me2 = me[2];
        let me3 = me[3];
        let me4 = me[4];
        let me5 = me[5];
        let me6 = me[6];
        let me7 = me[7];
        let me8 = me[8];
        let me9 = me[9];
        let me10 = me[10];
        let me11 = me[11];
        let me12 = me[12];
        let me13 = me[13];
        let me14 = me[14];
        let me15 = me[15];
        self.planes[0].set_components(me3 - me0, me7 - me4, me11 - me8, me15 - me12).normalize();
        self.planes[1].set_components(me3 + me0, me7 + me4, me11 + me8, me15 + me12).normalize();
        self.planes[2].set_components(me3 + me1, me7 + me5, me11 + me9, me15 + me13).normalize();
        self.planes[3].set_components(me3 - me1, me7 - me5, me11 - me9, me15 - me13).normalize();
        self.planes[4].set_components(me3 - me2, me7 - me6, me11 - me10, me15 - me14).normalize();
        self.planes[5].set_components(me3 + me2, me7 + me6, me11 + me10, me15 + me14).normalize();
        self
    }

    pub fn intersects_sphere(&self, sphere: &Sphere) -> bool {
        let neg_radius = -sphere.radius;
        for i in 0..6 {
            if self.planes[i].distance_to_point(&sphere.center) < neg_radius {
                return false;
            }
        }
        true
    }

    pub fn intersects_box(&self, b: &Box3) -> bool {
        for i in 0..6 {
            let plane = self.planes[i];
            let p = Vector3::new(
                if plane.normal.x > 0.0 { b.max.x } else { b.min.x },
                if plane.normal.y > 0.0 { b.max.y } else { b.min.y },
                if plane.normal.z > 0.0 { b.max.z } else { b.min.z },
            );
            if plane.distance_to_point(&p) < 0.0 {
                return false;
            }
        }
        true
    }

    pub fn contains_point(&self, point: &Vector3) -> bool {
        for i in 0..6 {
            if self.planes[i].distance_to_point(point) < 0.0 {
                return false;
            }
        }
        true
    }
}
