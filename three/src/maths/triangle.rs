//! A triangle (three.js `Triangle`).

use ranger::prelude::*;

use super::box3::Box3;
use super::plane::Plane;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    pub a: Vector3,
    pub b: Vector3,
    pub c: Vector3,
}

impl Default for Triangle {
    fn default() -> Triangle {
        Triangle::new(Vector3::zero(), Vector3::zero(), Vector3::zero())
    }
}

impl Triangle {
    pub fn new(a: Vector3, b: Vector3, c: Vector3) -> Triangle {
        Triangle { a, b, c }
    }

    /// The unit normal of `a b c` wound counter-clockwise (zero if degenerate).
    pub fn normal_of(a: &Vector3, b: &Vector3, c: &Vector3) -> Vector3 {
        let mut target = *c;
        target.sub(b);
        let mut v0 = *a;
        v0.sub(b);
        target.cross(&v0);
        let length_sq = target.length_sq();
        if length_sq > 0.0 {
            target.multiply_scalar(1.0 / length_sq.sqrt());
            return target;
        }
        Vector3::zero()
    }

    /// Barycentric coordinates of `point` in `a b c`; `None` if degenerate.
    pub fn barycoord_of(point: &Vector3, a: &Vector3, b: &Vector3, c: &Vector3) -> Option<Vector3> {
        let mut v0 = *c;
        v0.sub(a);
        let mut v1 = *b;
        v1.sub(a);
        let mut v2 = *point;
        v2.sub(a);
        let dot00 = v0.dot(&v0);
        let dot01 = v0.dot(&v1);
        let dot02 = v0.dot(&v2);
        let dot11 = v1.dot(&v1);
        let dot12 = v1.dot(&v2);
        let denom = dot00 * dot11 - dot01 * dot01;
        if denom == 0.0 {
            return None;
        }
        let inv_denom = 1.0 / denom;
        let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
        let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
        Some(Vector3::new(1.0 - u - v, v, u))
    }

    pub fn contains_point_of(point: &Vector3, a: &Vector3, b: &Vector3, c: &Vector3) -> bool {
        match Triangle::barycoord_of(point, a, b, c) {
            Some(v) => v.x >= 0.0 && v.y >= 0.0 && v.x + v.y <= 1.0,
            None => false,
        }
    }

    pub fn set(&mut self, a: &Vector3, b: &Vector3, c: &Vector3) -> &mut Triangle {
        self.a = *a;
        self.b = *b;
        self.c = *c;
        self
    }

    pub fn set_from_points_and_indices(&mut self, points: &Vec<Vector3>, i0: usize, i1: usize, i2: usize) -> &mut Triangle {
        self.a = points[i0];
        self.b = points[i1];
        self.c = points[i2];
        self
    }

    pub fn copy(&mut self, t: &Triangle) -> &mut Triangle {
        self.a = t.a;
        self.b = t.b;
        self.c = t.c;
        self
    }

    pub fn get_area(&self) -> f64 {
        let mut v0 = self.c;
        v0.sub(&self.b);
        let mut v1 = self.a;
        v1.sub(&self.b);
        v0.cross(&v1).length() * 0.5
    }

    pub fn get_midpoint(&self) -> Vector3 {
        let mut m = Vector3::zero();
        m.add_vectors(&self.a, &self.b).add(&self.c).multiply_scalar(1.0 / 3.0);
        m
    }

    pub fn get_normal(&self) -> Vector3 {
        Triangle::normal_of(&self.a, &self.b, &self.c)
    }

    pub fn get_plane(&self) -> Plane {
        let mut p = Plane::default();
        p.set_from_coplanar_points(&self.a, &self.b, &self.c);
        p
    }

    pub fn get_barycoord(&self, point: &Vector3) -> Option<Vector3> {
        Triangle::barycoord_of(point, &self.a, &self.b, &self.c)
    }

    /// `v1 v2 v3` (values at the corners) interpolated at `point`.
    pub fn get_interpolation(&self, point: &Vector3, v1: &Vector3, v2: &Vector3, v3: &Vector3) -> Option<Vector3> {
        match self.get_barycoord(point) {
            Some(w) => {
                let mut out = Vector3::zero();
                out.add_scaled_vector(v1, w.x);
                out.add_scaled_vector(v2, w.y);
                out.add_scaled_vector(v3, w.z);
                Some(out)
            }
            None => None,
        }
    }

    pub fn contains_point(&self, point: &Vector3) -> bool {
        Triangle::contains_point_of(point, &self.a, &self.b, &self.c)
    }

    /// True when the triangle faces against `direction`.
    pub fn is_front_facing(&self, direction: &Vector3) -> bool {
        let mut v0 = self.c;
        v0.sub(&self.b);
        let mut v1 = self.a;
        v1.sub(&self.b);
        v0.cross(&v1).dot(direction) < 0.0
    }

    pub fn intersects_box(&self, b: &Box3) -> bool {
        b.intersects_triangle(self)
    }

    /// The point of the triangle nearest `p` (Ericson, Real-Time Collision
    /// Detection 5.1.5).
    pub fn closest_point_to_point(&self, p: &Vector3) -> Vector3 {
        let a = self.a;
        let b = self.b;
        let c = self.c;
        let mut ab = b;
        ab.sub(&a);
        let mut ac = c;
        ac.sub(&a);
        let mut ap = *p;
        ap.sub(&a);
        let d1 = ab.dot(&ap);
        let d2 = ac.dot(&ap);
        if d1 <= 0.0 && d2 <= 0.0 {
            return a;
        }
        let mut bp = *p;
        bp.sub(&b);
        let d3 = ab.dot(&bp);
        let d4 = ac.dot(&bp);
        if d3 >= 0.0 && d4 <= d3 {
            return b;
        }
        let vc = d1 * d4 - d3 * d2;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            let v = d1 / (d1 - d3);
            let mut out = a;
            out.add_scaled_vector(&ab, v);
            return out;
        }
        let mut cp = *p;
        cp.sub(&c);
        let d5 = ab.dot(&cp);
        let d6 = ac.dot(&cp);
        if d6 >= 0.0 && d5 <= d6 {
            return c;
        }
        let vb = d5 * d2 - d1 * d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            let w = d2 / (d2 - d6);
            let mut out = a;
            out.add_scaled_vector(&ac, w);
            return out;
        }
        let va = d3 * d6 - d5 * d4;
        if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
            let mut bc = c;
            bc.sub(&b);
            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            let mut out = b;
            out.add_scaled_vector(&bc, w);
            return out;
        }
        let denom = 1.0 / (va + vb + vc);
        let v = vb * denom;
        let w = vc * denom;
        let mut out = a;
        out.add_scaled_vector(&ab, v).add_scaled_vector(&ac, w);
        out
    }

    pub fn equals(&self, t: &Triangle) -> bool {
        self.a.equals(&t.a) && self.b.equals(&t.b) && self.c.equals(&t.c)
    }
}
