//! An axis-aligned rectangle (three.js `Box2`).

use ranger::prelude::*;

use super::math_utils::INFINITY;

use super::vector2::Vector2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box2 {
    pub min: Vector2,
    pub max: Vector2,
}

impl Default for Box2 {
    fn default() -> Box2 {
        Box2::empty()
    }
}

impl Box2 {
    pub fn new(min: Vector2, max: Vector2) -> Box2 {
        Box2 { min, max }
    }

    /// The empty box: min at +infinity, max at -infinity.
    pub fn empty() -> Box2 {
        Box2::new(
            Vector2::new(INFINITY, INFINITY),
            Vector2::new(-INFINITY, -INFINITY),
        )
    }

    pub fn set(&mut self, min: &Vector2, max: &Vector2) -> &mut Box2 {
        self.min = *min;
        self.max = *max;
        self
    }

    pub fn set_from_points(&mut self, points: &Vec<Vector2>) -> &mut Box2 {
        self.make_empty();
        for p in points.iter() {
            self.expand_by_point(p);
        }
        self
    }

    pub fn set_from_center_and_size(&mut self, center: &Vector2, size: &Vector2) -> &mut Box2 {
        let hx = size.x * 0.5;
        let hy = size.y * 0.5;
        self.min.set(center.x - hx, center.y - hy);
        self.max.set(center.x + hx, center.y + hy);
        self
    }

    pub fn copy(&mut self, b: &Box2) -> &mut Box2 {
        self.min = b.min;
        self.max = b.max;
        self
    }

    pub fn make_empty(&mut self) -> &mut Box2 {
        self.min.set(INFINITY, INFINITY);
        self.max.set(-INFINITY, -INFINITY);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.max.x < self.min.x || self.max.y < self.min.y
    }

    pub fn get_center(&self) -> Vector2 {
        if self.is_empty() {
            Vector2::new(0.0, 0.0)
        } else {
            Vector2::new((self.min.x + self.max.x) * 0.5, (self.min.y + self.max.y) * 0.5)
        }
    }

    pub fn get_size(&self) -> Vector2 {
        if self.is_empty() {
            Vector2::new(0.0, 0.0)
        } else {
            Vector2::new(self.max.x - self.min.x, self.max.y - self.min.y)
        }
    }

    pub fn expand_by_point(&mut self, p: &Vector2) -> &mut Box2 {
        self.min.min(p);
        self.max.max(p);
        self
    }

    pub fn expand_by_vector(&mut self, v: &Vector2) -> &mut Box2 {
        self.min.sub(v);
        self.max.add(v);
        self
    }

    pub fn expand_by_scalar(&mut self, s: f64) -> &mut Box2 {
        self.min.add_scalar(-s);
        self.max.add_scalar(s);
        self
    }

    pub fn contains_point(&self, p: &Vector2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn contains_box(&self, b: &Box2) -> bool {
        self.min.x <= b.min.x && b.max.x <= self.max.x && self.min.y <= b.min.y && b.max.y <= self.max.y
    }

    /// Where `p` lies in the box, 0..1 on each axis.
    pub fn get_parameter(&self, p: &Vector2) -> Vector2 {
        Vector2::new(
            (p.x - self.min.x) / (self.max.x - self.min.x),
            (p.y - self.min.y) / (self.max.y - self.min.y),
        )
    }

    pub fn intersects_box(&self, b: &Box2) -> bool {
        b.max.x >= self.min.x && b.min.x <= self.max.x && b.max.y >= self.min.y && b.min.y <= self.max.y
    }

    pub fn clamp_point(&self, p: &Vector2) -> Vector2 {
        let mut c = *p;
        c.clamp(&self.min, &self.max);
        c
    }

    pub fn distance_to_point(&self, p: &Vector2) -> f64 {
        self.clamp_point(p).distance_to(p)
    }

    pub fn intersect(&mut self, b: &Box2) -> &mut Box2 {
        self.min.max(&b.min);
        self.max.min(&b.max);
        if self.is_empty() {
            self.make_empty();
        }
        self
    }

    pub fn union(&mut self, b: &Box2) -> &mut Box2 {
        self.min.min(&b.min);
        self.max.max(&b.max);
        self
    }

    pub fn translate(&mut self, offset: &Vector2) -> &mut Box2 {
        self.min.add(offset);
        self.max.add(offset);
        self
    }

    pub fn equals(&self, b: &Box2) -> bool {
        self.min.equals(&b.min) && self.max.equals(&b.max)
    }
}
