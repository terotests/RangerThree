//! A line segment (three.js `Line3`).

use ranger::prelude::*;

use super::math_utils::clamp;
use super::matrix4::Matrix4;
use super::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line3 {
    pub start: Vector3,
    pub end: Vector3,
}

impl Default for Line3 {
    fn default() -> Line3 {
        Line3::new(Vector3::zero(), Vector3::zero())
    }
}

impl Line3 {
    pub fn new(start: Vector3, end: Vector3) -> Line3 {
        Line3 { start, end }
    }

    pub fn set(&mut self, start: &Vector3, end: &Vector3) -> &mut Line3 {
        self.start = *start;
        self.end = *end;
        self
    }

    pub fn copy(&mut self, l: &Line3) -> &mut Line3 {
        self.start = l.start;
        self.end = l.end;
        self
    }

    pub fn get_center(&self) -> Vector3 {
        let mut c = Vector3::zero();
        c.add_vectors(&self.start, &self.end).multiply_scalar(0.5);
        c
    }

    pub fn delta(&self) -> Vector3 {
        let mut d = Vector3::zero();
        d.sub_vectors(&self.end, &self.start);
        d
    }

    pub fn distance_sq(&self) -> f64 {
        self.start.distance_to_squared(&self.end)
    }

    pub fn distance(&self) -> f64 {
        self.start.distance_to(&self.end)
    }

    /// The point `t` of the way from start to end (t may leave 0..1).
    pub fn at(&self, t: f64) -> Vector3 {
        let mut p = self.delta();
        p.multiply_scalar(t).add(&self.start);
        p
    }

    pub fn closest_point_to_point_parameter(&self, point: &Vector3, clamp_to_line: bool) -> f64 {
        let mut start_p = *point;
        start_p.sub(&self.start);
        let start_end = self.delta();
        let start_end2 = start_end.dot(&start_end);
        let t = start_end.dot(&start_p) / start_end2;
        if clamp_to_line {
            clamp(t, 0.0, 1.0)
        } else {
            t
        }
    }

    pub fn closest_point_to_point(&self, point: &Vector3, clamp_to_line: bool) -> Vector3 {
        let t = self.closest_point_to_point_parameter(point, clamp_to_line);
        self.at(t)
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) -> &mut Line3 {
        self.start.apply_matrix4(m);
        self.end.apply_matrix4(m);
        self
    }

    pub fn equals(&self, l: &Line3) -> bool {
        self.start.equals(&l.start) && self.end.equals(&l.end)
    }
}
