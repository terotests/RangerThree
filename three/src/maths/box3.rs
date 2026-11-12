//! An axis-aligned box (three.js `Box3`).

use ranger::prelude::*;

use super::math_utils::INFINITY;

use super::matrix4::Matrix4;
use super::plane::Plane;
use super::sphere::Sphere;
use super::triangle::Triangle;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box3 {
    pub min: Vector3,
    pub max: Vector3,
}

impl Default for Box3 {
    fn default() -> Box3 {
        Box3::empty()
    }
}

impl Box3 {
    pub fn new(min: Vector3, max: Vector3) -> Box3 {
        Box3 { min, max }
    }

    /// The empty box: min at +infinity, max at -infinity.
    pub fn empty() -> Box3 {
        Box3::new(
            Vector3::new(INFINITY, INFINITY, INFINITY),
            Vector3::new(-INFINITY, -INFINITY, -INFINITY),
        )
    }

    pub fn set(&mut self, min: &Vector3, max: &Vector3) -> &mut Box3 {
        self.min = *min;
        self.max = *max;
        self
    }

    /// The box around flat `[x, y, z, x, y, z, ...]` coordinates.
    pub fn set_from_array(&mut self, array: &[f64]) -> &mut Box3 {
        self.make_empty();
        let mut i = 0;
        while i + 2 < array.len() {
            let p = Vector3::new(array[i], array[i + 1], array[i + 2]);
            self.expand_by_point(&p);
            i += 3;
        }
        self
    }

    pub fn set_from_points(&mut self, points: &Vec<Vector3>) -> &mut Box3 {
        self.make_empty();
        for p in points.iter() {
            self.expand_by_point(p);
        }
        self
    }

    pub fn set_from_center_and_size(&mut self, center: &Vector3, size: &Vector3) -> &mut Box3 {
        let hx = size.x * 0.5;
        let hy = size.y * 0.5;
        let hz = size.z * 0.5;
        self.min.set(center.x - hx, center.y - hy, center.z - hz);
        self.max.set(center.x + hx, center.y + hy, center.z + hz);
        self
    }

    pub fn copy(&mut self, b: &Box3) -> &mut Box3 {
        self.min = b.min;
        self.max = b.max;
        self
    }

    pub fn make_empty(&mut self) -> &mut Box3 {
        self.min.set(INFINITY, INFINITY, INFINITY);
        self.max.set(-INFINITY, -INFINITY, -INFINITY);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.max.x < self.min.x || self.max.y < self.min.y || self.max.z < self.min.z
    }

    pub fn get_center(&self) -> Vector3 {
        if self.is_empty() {
            Vector3::zero()
        } else {
            Vector3::new(
                (self.min.x + self.max.x) * 0.5,
                (self.min.y + self.max.y) * 0.5,
                (self.min.z + self.max.z) * 0.5,
            )
        }
    }

    pub fn get_size(&self) -> Vector3 {
        if self.is_empty() {
            Vector3::zero()
        } else {
            Vector3::new(self.max.x - self.min.x, self.max.y - self.min.y, self.max.z - self.min.z)
        }
    }

    pub fn expand_by_point(&mut self, p: &Vector3) -> &mut Box3 {
        self.min.min(p);
        self.max.max(p);
        self
    }

    pub fn expand_by_vector(&mut self, v: &Vector3) -> &mut Box3 {
        self.min.sub(v);
        self.max.add(v);
        self
    }

    pub fn expand_by_scalar(&mut self, s: f64) -> &mut Box3 {
        self.min.add_scalar(-s);
        self.max.add_scalar(s);
        self
    }

    pub fn contains_point(&self, p: &Vector3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    pub fn contains_box(&self, b: &Box3) -> bool {
        self.min.x <= b.min.x
            && b.max.x <= self.max.x
            && self.min.y <= b.min.y
            && b.max.y <= self.max.y
            && self.min.z <= b.min.z
            && b.max.z <= self.max.z
    }

    /// Where `p` lies in the box, 0..1 on each axis.
    pub fn get_parameter(&self, p: &Vector3) -> Vector3 {
        Vector3::new(
            (p.x - self.min.x) / (self.max.x - self.min.x),
            (p.y - self.min.y) / (self.max.y - self.min.y),
            (p.z - self.min.z) / (self.max.z - self.min.z),
        )
    }

    pub fn intersects_box(&self, b: &Box3) -> bool {
        b.max.x >= self.min.x
            && b.min.x <= self.max.x
            && b.max.y >= self.min.y
            && b.min.y <= self.max.y
            && b.max.z >= self.min.z
            && b.min.z <= self.max.z
    }

    pub fn intersects_sphere(&self, s: &Sphere) -> bool {
        let p = self.clamp_point(&s.center);
        p.distance_to_squared(&s.center) <= s.radius * s.radius
    }

    pub fn intersects_plane(&self, plane: &Plane) -> bool {
        let n = plane.normal;
        let mut min: f64;
        let mut max: f64;
        if n.x > 0.0 {
            min = n.x * self.min.x;
            max = n.x * self.max.x;
        } else {
            min = n.x * self.max.x;
            max = n.x * self.min.x;
        }
        if n.y > 0.0 {
            min += n.y * self.min.y;
            max += n.y * self.max.y;
        } else {
            min += n.y * self.max.y;
            max += n.y * self.min.y;
        }
        if n.z > 0.0 {
            min += n.z * self.min.z;
            max += n.z * self.max.z;
        } else {
            min += n.z * self.max.z;
            max += n.z * self.min.z;
        }
        min <= -plane.constant && max >= -plane.constant
    }

    /// Separating axis test of the box against the triangle.
    pub fn intersects_triangle(&self, t: &Triangle) -> bool {
        if self.is_empty() {
            return false;
        }
        let center = self.get_center();
        let mut extents = self.max;
        extents.sub(&center);
        let mut v0 = t.a;
        v0.sub(&center);
        let mut v1 = t.b;
        v1.sub(&center);
        let mut v2 = t.c;
        v2.sub(&center);
        let mut f0 = v1;
        f0.sub(&v0);
        let mut f1 = v2;
        f1.sub(&v1);
        let mut f2 = v0;
        f2.sub(&v2);
        let axes = vec![
            0.0, -f0.z, f0.y, 0.0, -f1.z, f1.y, 0.0, -f2.z, f2.y, f0.z, 0.0, -f0.x, f1.z, 0.0, -f1.x, f2.z,
            0.0, -f2.x, -f0.y, f0.x, 0.0, -f1.y, f1.x, 0.0, -f2.y, f2.x, 0.0,
        ];
        if !sat_for_axes(&axes, &v0, &v1, &v2, &extents) {
            return false;
        }
        let unit = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        if !sat_for_axes(&unit, &v0, &v1, &v2, &extents) {
            return false;
        }
        let mut normal = Vector3::zero();
        normal.cross_vectors(&f0, &f1);
        let n = vec![normal.x, normal.y, normal.z];
        sat_for_axes(&n, &v0, &v1, &v2, &extents)
    }

    pub fn clamp_point(&self, p: &Vector3) -> Vector3 {
        let mut c = *p;
        c.clamp(&self.min, &self.max);
        c
    }

    pub fn distance_to_point(&self, p: &Vector3) -> f64 {
        self.clamp_point(p).distance_to(p)
    }

    pub fn get_bounding_sphere(&self) -> Sphere {
        if self.is_empty() {
            return Sphere::empty();
        }
        Sphere::new(self.get_center(), self.get_size().length() * 0.5)
    }

    pub fn intersect(&mut self, b: &Box3) -> &mut Box3 {
        self.min.max(&b.min);
        self.max.min(&b.max);
        if self.is_empty() {
            self.make_empty();
        }
        self
    }

    pub fn union(&mut self, b: &Box3) -> &mut Box3 {
        self.min.min(&b.min);
        self.max.max(&b.max);
        self
    }

    /// The box around the eight transformed corners.
    pub fn apply_matrix4(&mut self, m: &Matrix4) -> &mut Box3 {
        if self.is_empty() {
            return self;
        }
        let lo = self.min;
        let hi = self.max;
        self.make_empty();
        for i in 0..8 {
            let mut p = Vector3::new(
                if (i & 4) != 0 { hi.x } else { lo.x },
                if (i & 2) != 0 { hi.y } else { lo.y },
                if (i & 1) != 0 { hi.z } else { lo.z },
            );
            p.apply_matrix4(m);
            self.expand_by_point(&p);
        }
        self
    }

    pub fn translate(&mut self, offset: &Vector3) -> &mut Box3 {
        self.min.add(offset);
        self.max.add(offset);
        self
    }

    pub fn equals(&self, b: &Box3) -> bool {
        self.min.equals(&b.min) && self.max.equals(&b.max)
    }
}

fn sat_for_axes(axes: &Vec<f64>, v0: &Vector3, v1: &Vector3, v2: &Vector3, extents: &Vector3) -> bool {
    let mut i = 0;
    while i + 2 < axes.len() {
        let axis = Vector3::new(axes[i], axes[i + 1], axes[i + 2]);
        let r = extents.x * axis.x.abs() + extents.y * axis.y.abs() + extents.z * axis.z.abs();
        let p0 = v0.dot(&axis);
        let p1 = v1.dot(&axis);
        let p2 = v2.dot(&axis);
        if (-(p0.max(p1).max(p2))).max(p0.min(p1).min(p2)) > r {
            return false;
        }
        i += 3;
    }
    true
}
