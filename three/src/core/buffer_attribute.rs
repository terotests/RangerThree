//! Per-vertex data (three.js `BufferAttribute`): `count` items of
//! `item_size` numbers each, stored in `array`. `kind` says what typed
//! array three.js would hold them in; values are rounded to it when set.

use ranger::cell::RefCell;
use ranger::prelude::*;
use std::rc::Rc;

use crate::maths::math_utils::{denormalize, normalize, ArrayKind};
use crate::maths::matrix3::Matrix3;
use crate::maths::matrix4::Matrix4;
use crate::maths::vector3::Vector3;

/// `gl.STATIC_DRAW`, the default usage hint.
pub const STATIC_DRAW_USAGE: i64 = 35044;
/// `gl.DYNAMIC_DRAW`.
pub const DYNAMIC_DRAW_USAGE: i64 = 35048;

/// A part of the array changed since the last upload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpdateRange {
    pub start: i64,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BufferAttribute {
    pub name: String,
    pub array: Vec<f64>,
    pub item_size: usize,
    pub count: usize,
    pub normalized: bool,
    pub kind: ArrayKind,
    pub usage: i64,
    pub version: i64,
    pub update_ranges: Vec<UpdateRange>,
}

/// `x` as the value an array of `kind` keeps.
pub fn store_as(x: f64, kind: ArrayKind) -> f64 {
    match kind {
        ArrayKind::Float32 => to_f32(x),
        ArrayKind::Uint32 => wrap_int(x, 4294967296.0, false),
        ArrayKind::Uint16 => wrap_int(x, 65536.0, false),
        ArrayKind::Uint8 => wrap_int(x, 256.0, false),
        ArrayKind::Int32 => wrap_int(x, 4294967296.0, true),
        ArrayKind::Int16 => wrap_int(x, 65536.0, true),
        ArrayKind::Int8 => wrap_int(x, 256.0, true),
    }
}

/// The nearest single precision float (what a Float32Array keeps).
pub fn to_f32(x: f64) -> f64 {
    ranger::native!(rust: (x as f32) as f64, es6: "Math.fround({x})")
}

fn wrap_int(x: f64, range: f64, signed: bool) -> f64 {
    if x != x {
        return 0.0;
    }
    let t = x.trunc();
    let mut v = t % range;
    if v < 0.0 {
        v += range;
    }
    if signed && v >= range / 2.0 {
        v -= range;
    }
    v
}

impl BufferAttribute {
    /// An attribute over `array` (rounded to `kind`) with `item_size`
    /// numbers per item.
    pub fn new(array: Vec<f64>, item_size: usize, kind: ArrayKind, normalized: bool) -> BufferAttribute {
        let mut stored: Vec<f64> = Vec::new();
        for v in array.iter() {
            stored.push(store_as(*v, kind));
        }
        let count = if item_size > 0 { stored.len() / item_size } else { 0 };
        BufferAttribute {
            name: String::new(),
            array: stored,
            item_size,
            count,
            normalized,
            kind,
            usage: STATIC_DRAW_USAGE,
            version: 0,
            update_ranges: Vec::new(),
        }
    }

    pub fn float32(array: Vec<f64>, item_size: usize) -> BufferAttribute {
        BufferAttribute::new(array, item_size, ArrayKind::Float32, false)
    }

    pub fn shared(self) -> Rc<RefCell<BufferAttribute>> {
        Rc::new(RefCell::new(self))
    }

    pub fn needs_update(&mut self) {
        self.version += 1;
    }

    pub fn set_usage(&mut self, usage: i64) {
        self.usage = usage;
    }

    pub fn add_update_range(&mut self, start: i64, count: i64) {
        self.update_ranges.push(UpdateRange { start, count });
    }

    pub fn clear_update_ranges(&mut self) {
        self.update_ranges.clear();
    }

    pub fn copy(&mut self, source: &BufferAttribute) {
        self.name = source.name.clone();
        self.array = source.array.clone();
        self.item_size = source.item_size;
        self.count = source.count;
        self.normalized = source.normalized;
        self.kind = source.kind;
        self.usage = source.usage;
    }

    /// Item `index1` of `source` into item `index2` of this attribute... as
    /// three.js: `copyAt(index1, attribute, index2)` writes item index1.
    pub fn copy_at(&mut self, index1: usize, source: &BufferAttribute, index2: usize) {
        let a = index1 * self.item_size;
        let b = index2 * source.item_size;
        for i in 0..self.item_size {
            self.array[a + i] = store_as(source.array[b + i], self.kind);
        }
    }

    pub fn copy_array(&mut self, array: &Vec<f64>) {
        for i in 0..array.len() {
            if i < self.array.len() {
                self.array[i] = store_as(array[i], self.kind);
            }
        }
    }

    /// Writes `values` from item offset `offset` (in numbers, as three.js).
    pub fn set(&mut self, values: &Vec<f64>, offset: usize) {
        for i in 0..values.len() {
            self.array[offset + i] = store_as(values[i], self.kind);
        }
    }

    pub fn get_component(&self, index: usize, component: usize) -> f64 {
        let v = self.array[index * self.item_size + component];
        if self.normalized {
            denormalize(v, self.kind)
        } else {
            v
        }
    }

    pub fn set_component(&mut self, index: usize, component: usize, value: f64) {
        let v = if self.normalized { normalize(value, self.kind) } else { value };
        self.array[index * self.item_size + component] = store_as(v, self.kind);
    }

    pub fn get_x(&self, index: usize) -> f64 {
        self.get_component(index, 0)
    }

    pub fn get_y(&self, index: usize) -> f64 {
        self.get_component(index, 1)
    }

    pub fn get_z(&self, index: usize) -> f64 {
        self.get_component(index, 2)
    }

    pub fn get_w(&self, index: usize) -> f64 {
        self.get_component(index, 3)
    }

    pub fn set_x(&mut self, index: usize, x: f64) {
        self.set_component(index, 0, x);
    }

    pub fn set_y(&mut self, index: usize, y: f64) {
        self.set_component(index, 1, y);
    }

    pub fn set_z(&mut self, index: usize, z: f64) {
        self.set_component(index, 2, z);
    }

    pub fn set_w(&mut self, index: usize, w: f64) {
        self.set_component(index, 3, w);
    }

    pub fn set_xy(&mut self, index: usize, x: f64, y: f64) {
        self.set_component(index, 0, x);
        self.set_component(index, 1, y);
    }

    pub fn set_xyz(&mut self, index: usize, x: f64, y: f64, z: f64) {
        self.set_component(index, 0, x);
        self.set_component(index, 1, y);
        self.set_component(index, 2, z);
    }

    pub fn set_xyzw(&mut self, index: usize, x: f64, y: f64, z: f64, w: f64) {
        self.set_component(index, 0, x);
        self.set_component(index, 1, y);
        self.set_component(index, 2, z);
        self.set_component(index, 3, w);
    }

    /// Item `index` as a vector (missing components are zero).
    pub fn get_vector3(&self, index: usize) -> Vector3 {
        let x = self.get_x(index);
        let y = if self.item_size > 1 { self.get_y(index) } else { 0.0 };
        let z = if self.item_size > 2 { self.get_z(index) } else { 0.0 };
        Vector3::new(x, y, z)
    }

    pub fn set_vector3(&mut self, index: usize, v: &Vector3) {
        self.set_component(index, 0, v.x);
        if self.item_size > 1 {
            self.set_component(index, 1, v.y);
        }
        if self.item_size > 2 {
            self.set_component(index, 2, v.z);
        }
    }

    pub fn apply_matrix3(&mut self, m: &Matrix3) {
        if self.item_size == 2 {
            for i in 0..self.count {
                let mut v = crate::maths::vector2::Vector2::new(self.get_x(i), self.get_y(i));
                v.apply_matrix3(m);
                self.set_xy(i, v.x, v.y);
            }
        } else if self.item_size == 3 {
            for i in 0..self.count {
                let mut v = self.get_vector3(i);
                v.apply_matrix3(m);
                self.set_vector3(i, &v);
            }
        }
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) {
        for i in 0..self.count {
            let mut v = self.get_vector3(i);
            v.apply_matrix4(m);
            self.set_vector3(i, &v);
        }
    }

    pub fn apply_normal_matrix(&mut self, m: &Matrix3) {
        for i in 0..self.count {
            let mut v = self.get_vector3(i);
            v.apply_normal_matrix(m);
            self.set_vector3(i, &v);
        }
    }

    pub fn transform_direction(&mut self, m: &Matrix4) {
        for i in 0..self.count {
            let mut v = self.get_vector3(i);
            v.transform_direction(m);
            self.set_vector3(i, &v);
        }
    }
}
