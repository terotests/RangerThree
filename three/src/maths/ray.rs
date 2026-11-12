//! A half-line from `origin` along the unit `direction` (three.js `Ray`).

use ranger::prelude::*;

use super::box3::Box3;
use super::matrix4::Matrix4;
use super::plane::Plane;
use super::sphere::Sphere;
use super::vector3::Vector3;

/// Where a ray passes closest to a segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentHit {
    pub distance_sq: f64,
    pub point_on_ray: Vector3,
    pub point_on_segment: Vector3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: Vector3,
    pub direction: Vector3,
}

impl Default for Ray {
    fn default() -> Ray {
        Ray::new(Vector3::zero(), Vector3::new(0.0, 0.0, -1.0))
    }
}

impl Ray {
    pub fn new(origin: Vector3, direction: Vector3) -> Ray {
        Ray { origin, direction }
    }

    pub fn set(&mut self, origin: &Vector3, direction: &Vector3) -> &mut Ray {
        self.origin = *origin;
        self.direction = *direction;
        self
    }

    pub fn copy(&mut self, r: &Ray) -> &mut Ray {
        self.origin = r.origin;
        self.direction = r.direction;
        self
    }

    pub fn at(&self, t: f64) -> Vector3 {
        let mut p = self.origin;
        p.add_scaled_vector(&self.direction, t);
        p
    }

    pub fn look_at(&mut self, v: &Vector3) -> &mut Ray {
        let mut d = *v;
        d.sub(&self.origin).normalize();
        self.direction = d;
        self
    }

    pub fn recast(&mut self, t: f64) -> &mut Ray {
        self.origin = self.at(t);
        self
    }

    pub fn closest_point_to_point(&self, point: &Vector3) -> Vector3 {
        let mut v = *point;
        v.sub(&self.origin);
        let d = v.dot(&self.direction);
        if d < 0.0 {
            return self.origin;
        }
        self.at(d)
    }

    pub fn distance_to_point(&self, point: &Vector3) -> f64 {
        self.distance_sq_to_point(point).sqrt()
    }

    pub fn distance_sq_to_point(&self, point: &Vector3) -> f64 {
        let mut v = *point;
        v.sub(&self.origin);
        let d = v.dot(&self.direction);
        if d < 0.0 {
            return self.origin.distance_to_squared(point);
        }
        self.at(d).distance_to_squared(point)
    }

    /// Squared distance between the ray and the segment `v0..v1`.
    pub fn distance_sq_to_segment(&self, v0: &Vector3, v1: &Vector3) -> f64 {
        self.segment_hit(v0, v1).distance_sq
    }

    /// The closest points of the ray and the segment `v0`-`v1`, and their
    /// squared distance.
    pub fn segment_hit(&self, v0: &Vector3, v1: &Vector3) -> SegmentHit {
        let mut seg_center = *v0;
        seg_center.add(v1).multiply_scalar(0.5);
        let mut seg_dir = *v1;
        seg_dir.sub(v0).normalize();
        let mut diff = self.origin;
        diff.sub(&seg_center);
        let seg_extent = v0.distance_to(v1) * 0.5;
        let a01 = -self.direction.dot(&seg_dir);
        let b0 = diff.dot(&self.direction);
        let b1 = -diff.dot(&seg_dir);
        let c = diff.length_sq();
        let det = (1.0 - a01 * a01).abs();
        let mut s0: f64;
        let mut s1: f64;
        let sqr_dist: f64;
        if det > 0.0 {
            s0 = a01 * b1 - b0;
            s1 = a01 * b0 - b1;
            let ext_det = seg_extent * det;
            if s0 >= 0.0 {
                if s1 >= -ext_det {
                    if s1 <= ext_det {
                        let inv_det = 1.0 / det;
                        s0 *= inv_det;
                        s1 *= inv_det;
                        sqr_dist = s0 * (s0 + a01 * s1 + 2.0 * b0) + s1 * (a01 * s0 + s1 + 2.0 * b1) + c;
                    } else {
                        s1 = seg_extent;
                        s0 = (-(a01 * s1 + b0)).max(0.0);
                        sqr_dist = -s0 * s0 + s1 * (s1 + 2.0 * b1) + c;
                    }
                } else {
                    s1 = -seg_extent;
                    s0 = (-(a01 * s1 + b0)).max(0.0);
                    sqr_dist = -s0 * s0 + s1 * (s1 + 2.0 * b1) + c;
                }
            } else if s1 <= -ext_det {
                s0 = (-(-a01 * seg_extent + b0)).max(0.0);
                s1 = if s0 > 0.0 { -seg_extent } else { (-seg_extent).max(seg_extent.min(-b1)) };
                sqr_dist = -s0 * s0 + s1 * (s1 + 2.0 * b1) + c;
            } else if s1 <= ext_det {
                s0 = 0.0;
                s1 = (-seg_extent).max(seg_extent.min(-b1));
                sqr_dist = s1 * (s1 + 2.0 * b1) + c;
            } else {
                s0 = (-(a01 * seg_extent + b0)).max(0.0);
                s1 = if s0 > 0.0 { seg_extent } else { (-seg_extent).max(seg_extent.min(-b1)) };
                sqr_dist = -s0 * s0 + s1 * (s1 + 2.0 * b1) + c;
            }
        } else {
            s1 = if a01 > 0.0 { -seg_extent } else { seg_extent };
            s0 = (-(a01 * s1 + b0)).max(0.0);
            sqr_dist = -s0 * s0 + s1 * (s1 + 2.0 * b1) + c;
        }
        let mut on_ray = self.direction;
        on_ray.multiply_scalar(s0);
        on_ray.add(&self.origin);
        let mut on_segment = seg_dir;
        on_segment.multiply_scalar(s1);
        on_segment.add(&seg_center);
        SegmentHit { distance_sq: sqr_dist, point_on_ray: on_ray, point_on_segment: on_segment }
    }

    pub fn intersect_sphere(&self, sphere: &Sphere) -> Option<Vector3> {
        let mut v = sphere.center;
        v.sub(&self.origin);
        let tca = v.dot(&self.direction);
        let d2 = v.dot(&v) - tca * tca;
        let radius2 = sphere.radius * sphere.radius;
        if d2 > radius2 {
            return None;
        }
        let thc = (radius2 - d2).sqrt();
        let t0 = tca - thc;
        let t1 = tca + thc;
        if t1 < 0.0 {
            return None;
        }
        if t0 < 0.0 {
            return Some(self.at(t1));
        }
        Some(self.at(t0))
    }

    pub fn intersects_sphere(&self, sphere: &Sphere) -> bool {
        if sphere.radius < 0.0 {
            return false;
        }
        self.distance_sq_to_point(&sphere.center) <= sphere.radius * sphere.radius
    }

    /// Distance along the ray to the plane; `None` when parallel or behind.
    pub fn distance_to_plane(&self, plane: &Plane) -> Option<f64> {
        let denominator = plane.normal.dot(&self.direction);
        if denominator == 0.0 {
            if plane.distance_to_point(&self.origin) == 0.0 {
                return Some(0.0);
            }
            return None;
        }
        let t = -(self.origin.dot(&plane.normal) + plane.constant) / denominator;
        if t >= 0.0 {
            Some(t)
        } else {
            None
        }
    }

    pub fn intersect_plane(&self, plane: &Plane) -> Option<Vector3> {
        match self.distance_to_plane(plane) {
            Some(t) => Some(self.at(t)),
            None => None,
        }
    }

    pub fn intersects_plane(&self, plane: &Plane) -> bool {
        let dist = plane.distance_to_point(&self.origin);
        if dist == 0.0 {
            return true;
        }
        let denominator = plane.normal.dot(&self.direction);
        denominator * dist < 0.0
    }

    pub fn intersect_box(&self, b: &Box3) -> Option<Vector3> {
        let inv_x = 1.0 / self.direction.x;
        let inv_y = 1.0 / self.direction.y;
        let inv_z = 1.0 / self.direction.z;
        let o = self.origin;
        let mut tmin: f64;
        let mut tmax: f64;
        if inv_x >= 0.0 {
            tmin = (b.min.x - o.x) * inv_x;
            tmax = (b.max.x - o.x) * inv_x;
        } else {
            tmin = (b.max.x - o.x) * inv_x;
            tmax = (b.min.x - o.x) * inv_x;
        }
        let tymin: f64;
        let tymax: f64;
        if inv_y >= 0.0 {
            tymin = (b.min.y - o.y) * inv_y;
            tymax = (b.max.y - o.y) * inv_y;
        } else {
            tymin = (b.max.y - o.y) * inv_y;
            tymax = (b.min.y - o.y) * inv_y;
        }
        if tmin > tymax || tymin > tmax {
            return None;
        }
        if tymin > tmin || tmin != tmin {
            tmin = tymin;
        }
        if tymax < tmax || tmax != tmax {
            tmax = tymax;
        }
        let tzmin: f64;
        let tzmax: f64;
        if inv_z >= 0.0 {
            tzmin = (b.min.z - o.z) * inv_z;
            tzmax = (b.max.z - o.z) * inv_z;
        } else {
            tzmin = (b.max.z - o.z) * inv_z;
            tzmax = (b.min.z - o.z) * inv_z;
        }
        if tmin > tzmax || tzmin > tmax {
            return None;
        }
        if tzmin > tmin || tmin != tmin {
            tmin = tzmin;
        }
        if tzmax < tmax || tmax != tmax {
            tmax = tzmax;
        }
        if tmax < 0.0 {
            return None;
        }
        Some(self.at(if tmin >= 0.0 { tmin } else { tmax }))
    }

    pub fn intersects_box(&self, b: &Box3) -> bool {
        self.intersect_box(b).is_some()
    }

    /// Where the ray meets the triangle `a b c`; with `backface_culling`
    /// a triangle facing away (clockwise as seen from the ray) is missed.
    pub fn intersect_triangle(&self, a: &Vector3, b: &Vector3, c: &Vector3, backface_culling: bool) -> Option<Vector3> {
        let mut edge1 = *b;
        edge1.sub(a);
        let mut edge2 = *c;
        edge2.sub(a);
        let mut normal = Vector3::zero();
        normal.cross_vectors(&edge1, &edge2);
        let mut ddn = self.direction.dot(&normal);
        let sign: f64;
        if ddn > 0.0 {
            if backface_culling {
                return None;
            }
            sign = 1.0;
        } else if ddn < 0.0 {
            sign = -1.0;
            ddn = -ddn;
        } else {
            return None;
        }
        let mut diff = self.origin;
        diff.sub(a);
        let mut e2 = edge2;
        let dd_qx_e2 = sign * self.direction.dot(e2.cross_vectors(&diff, &edge2));
        if dd_qx_e2 < 0.0 {
            return None;
        }
        let mut e1 = edge1;
        let dd_e1x_q = sign * self.direction.dot(e1.cross(&diff));
        if dd_e1x_q < 0.0 {
            return None;
        }
        if dd_qx_e2 + dd_e1x_q > ddn {
            return None;
        }
        let qdn = -sign * diff.dot(&normal);
        if qdn < 0.0 {
            return None;
        }
        Some(self.at(qdn / ddn))
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) -> &mut Ray {
        self.origin.apply_matrix4(m);
        self.direction.transform_direction(m);
        self
    }

    pub fn equals(&self, r: &Ray) -> bool {
        self.origin.equals(&r.origin) && self.direction.equals(&r.direction)
    }
}
