//! A mesh's vertex data (three.js `BufferGeometry`): named attributes,
//! an optional index, material groups and cached bounds.

use ranger::cell::RefCell;
use ranger::prelude::*;
use std::rc::Rc;

use super::buffer_attribute::BufferAttribute;
use crate::maths::box3::Box3;
use crate::maths::math_utils::ArrayKind;
use crate::maths::matrix3::Matrix3;
use crate::maths::matrix4::Matrix4;
use crate::maths::quaternion::Quaternion;
use crate::maths::sphere::Sphere;
use crate::maths::vector3::Vector3;
use crate::maths::math_utils::INFINITY;

pub type AttributeRef = Rc<RefCell<BufferAttribute>>;
pub type GeometryRef = Rc<RefCell<BufferGeometry>>;

/// A range of the index (or of the vertices) drawn with one material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryGroup {
    pub start: i64,
    pub count: i64,
    pub material_index: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawRange {
    pub start: f64,
    pub count: f64,
}

#[derive(Clone)]
pub struct BufferGeometry {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    /// three.js's `type`: "BufferGeometry", "BoxGeometry", ...
    pub type_name: String,
    /// The constructor arguments of a generated geometry, by name.
    pub parameters: Map<String, f64>,
    pub attributes: Map<String, AttributeRef>,
    pub morph_attributes: Map<String, Vec<AttributeRef>>,
    pub morph_targets_relative: bool,
    pub index: Option<AttributeRef>,
    pub groups: Vec<GeometryGroup>,
    pub bounding_box: Option<Box3>,
    pub bounding_sphere: Option<Sphere>,
    pub draw_range: DrawRange,
}

impl Default for BufferGeometry {
    fn default() -> BufferGeometry {
        BufferGeometry::new()
    }
}

impl BufferGeometry {
    pub fn new() -> BufferGeometry {
        BufferGeometry {
            id: 0,
            uuid: String::new(),
            name: String::new(),
            type_name: "BufferGeometry".to_string(),
            parameters: Map::new(),
            attributes: Map::new(),
            morph_attributes: Map::new(),
            morph_targets_relative: false,
            index: None,
            groups: Vec::new(),
            bounding_box: None,
            bounding_sphere: None,
            draw_range: DrawRange { start: 0.0, count: INFINITY },
        }
    }

    pub fn shared(self) -> GeometryRef {
        Rc::new(RefCell::new(self))
    }

    pub fn get_index(&self) -> Option<AttributeRef> {
        match &self.index {
            Some(i) => Some(i.clone()),
            None => None,
        }
    }

    /// An index over `indices`, 16-bit when every index fits (as three.js).
    pub fn set_index(&mut self, indices: Vec<f64>) {
        let mut max = 0.0;
        for v in indices.iter() {
            if *v > max {
                max = *v;
            }
        }
        let kind = if max > 65535.0 { ArrayKind::Uint32 } else { ArrayKind::Uint16 };
        self.index = Some(BufferAttribute::new(indices, 1, kind, false).shared());
    }

    pub fn set_index_attribute(&mut self, index: Option<AttributeRef>) {
        self.index = index;
    }

    pub fn get_attribute(&self, name: &str) -> Option<AttributeRef> {
        match self.attributes.get(name) {
            Some(a) => Some(a.clone()),
            None => None,
        }
    }

    pub fn set_attribute(&mut self, name: &str, attribute: AttributeRef) {
        self.attributes.insert(name.to_string(), attribute);
    }

    pub fn delete_attribute(&mut self, name: &str) {
        self.attributes.remove(name);
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.contains_key(name)
    }

    pub fn add_group(&mut self, start: i64, count: i64, material_index: i64) {
        self.groups.push(GeometryGroup { start, count, material_index });
    }

    pub fn clear_groups(&mut self) {
        self.groups.clear();
    }

    pub fn set_draw_range(&mut self, start: f64, count: f64) {
        self.draw_range = DrawRange { start, count };
    }

    /// Number of vertices (the position attribute's count).
    pub fn vertex_count(&self) -> usize {
        match self.attributes.get("position") {
            Some(p) => p.borrow().count,
            None => 0,
        }
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) {
        if let Some(p) = self.attributes.get("position") {
            p.borrow_mut().apply_matrix4(m);
            p.borrow_mut().needs_update();
        }
        if let Some(n) = self.attributes.get("normal") {
            let mut nm = Matrix3::identity();
            nm.get_normal_matrix(m);
            n.borrow_mut().apply_normal_matrix(&nm);
            n.borrow_mut().needs_update();
        }
        if let Some(t) = self.attributes.get("tangent") {
            t.borrow_mut().transform_direction(m);
            t.borrow_mut().needs_update();
        }
        if self.bounding_box.is_some() {
            self.compute_bounding_box();
        }
        if self.bounding_sphere.is_some() {
            self.compute_bounding_sphere();
        }
    }

    pub fn apply_quaternion(&mut self, q: &Quaternion) {
        let mut m = Matrix4::identity();
        m.make_rotation_from_quaternion(q);
        self.apply_matrix4(&m);
    }

    pub fn rotate_x(&mut self, angle: f64) {
        let mut m = Matrix4::identity();
        m.make_rotation_x(angle);
        self.apply_matrix4(&m);
    }

    pub fn rotate_y(&mut self, angle: f64) {
        let mut m = Matrix4::identity();
        m.make_rotation_y(angle);
        self.apply_matrix4(&m);
    }

    pub fn rotate_z(&mut self, angle: f64) {
        let mut m = Matrix4::identity();
        m.make_rotation_z(angle);
        self.apply_matrix4(&m);
    }

    pub fn translate(&mut self, x: f64, y: f64, z: f64) {
        let mut m = Matrix4::identity();
        m.make_translation(x, y, z);
        self.apply_matrix4(&m);
    }

    pub fn scale(&mut self, x: f64, y: f64, z: f64) {
        let mut m = Matrix4::identity();
        m.make_scale(x, y, z);
        self.apply_matrix4(&m);
    }

    /// Turns the geometry so that its +z faces `v`.
    pub fn look_at(&mut self, v: &Vector3) {
        let mut m = Matrix4::identity();
        let origin = Vector3::zero();
        let up = Vector3::new(0.0, 1.0, 0.0);
        m.look_at(v, &origin, &up);
        let t = m.decompose();
        let mut r = Matrix4::identity();
        r.make_rotation_from_quaternion(&t.quaternion);
        self.apply_matrix4(&r);
    }

    /// Moves the geometry so its bounding box is centred on the origin.
    pub fn center(&mut self) {
        self.compute_bounding_box();
        let c = match &self.bounding_box {
            Some(b) => b.get_center(),
            None => Vector3::zero(),
        };
        self.translate(-c.x, -c.y, -c.z);
    }

    pub fn set_from_points(&mut self, points: &Vec<Vector3>) {
        let mut array: Vec<f64> = Vec::new();
        for p in points.iter() {
            array.push(p.x);
            array.push(p.y);
            array.push(p.z);
        }
        self.set_attribute("position", BufferAttribute::float32(array, 3).shared());
    }

    pub fn compute_bounding_box(&mut self) {
        let mut b = Box3::empty();
        match self.attributes.get("position") {
            Some(p) => {
                let p = p.borrow();
                for i in 0..p.count {
                    let v = p.get_vector3(i);
                    b.expand_by_point(&v);
                }
            }
            None => {
                b.make_empty();
            }
        }
        self.bounding_box = Some(b);
    }

    pub fn compute_bounding_sphere(&mut self) {
        let mut s = Sphere::empty();
        if let Some(p) = self.attributes.get("position") {
            let p = p.borrow();
            let mut b = Box3::empty();
            for i in 0..p.count {
                b.expand_by_point(&p.get_vector3(i));
            }
            let center = b.get_center();
            let mut max_sq: f64 = 0.0;
            for i in 0..p.count {
                max_sq = max_sq.max(center.distance_to_squared(&p.get_vector3(i)));
            }
            s = Sphere::new(center, max_sq.sqrt());
        }
        self.bounding_sphere = Some(s);
    }

    /// Smooth normals: each vertex gets the sum of its faces' normals.
    pub fn compute_vertex_normals(&mut self) {
        let position = match self.attributes.get("position") {
            Some(p) => p.clone(),
            None => return,
        };
        let count = position.borrow().count;
        let normal = match self.attributes.get("normal") {
            Some(n) if n.borrow().count == count => n.clone(),
            _ => {
                let n = BufferAttribute::float32(vec![0.0; count * 3], 3).shared();
                self.attributes.insert("normal".to_string(), n.clone());
                n
            }
        };
        let mut sums: Vec<f64> = vec![0.0; count * 3];
        let pos = position.borrow();
        let mut add_face = |a: usize, b: usize, c: usize, sums: &mut Vec<f64>| {
            let pa = pos.get_vector3(a);
            let pb = pos.get_vector3(b);
            let pc = pos.get_vector3(c);
            let mut cb = pc;
            cb.sub(&pb);
            let mut ab = pa;
            ab.sub(&pb);
            cb.cross(&ab);
            for k in 0..3 {
                let idx = if k == 0 { a } else if k == 1 { b } else { c };
                sums[idx * 3] += cb.x;
                sums[idx * 3 + 1] += cb.y;
                sums[idx * 3 + 2] += cb.z;
            }
        };
        match &self.index {
            Some(index) => {
                let index = index.borrow();
                let mut i = 0;
                while i + 2 < index.count {
                    add_face(index.get_x(i) as usize, index.get_x(i + 1) as usize, index.get_x(i + 2) as usize, &mut sums);
                    i += 3;
                }
            }
            None => {
                let mut i = 0;
                while i + 2 < count {
                    add_face(i, i + 1, i + 2, &mut sums);
                    i += 3;
                }
            }
        }
        let mut n = normal.borrow_mut();
        for i in 0..count {
            n.set_xyz(i, sums[i * 3], sums[i * 3 + 1], sums[i * 3 + 2]);
        }
        drop(n);
        drop(pos);
        self.normalize_normals();
        normal.borrow_mut().needs_update();
    }

    pub fn normalize_normals(&mut self) {
        if let Some(n) = self.attributes.get("normal") {
            let mut n = n.borrow_mut();
            for i in 0..n.count {
                let mut v = n.get_vector3(i);
                v.normalize();
                n.set_xyz(i, v.x, v.y, v.z);
            }
        }
    }

    /// The same triangles with every corner its own vertex.
    pub fn to_non_indexed(&self) -> BufferGeometry {
        let mut out = BufferGeometry::new();
        let index = match &self.index {
            Some(i) => i.clone(),
            None => {
                out.copy(self);
                return out;
            }
        };
        let index = index.borrow();
        for (name, attr) in self.attributes.iter() {
            let a = attr.borrow();
            let mut array: Vec<f64> = Vec::new();
            for i in 0..index.count {
                let src = index.get_x(i) as usize * a.item_size;
                for k in 0..a.item_size {
                    array.push(a.array[src + k]);
                }
            }
            let mut na = BufferAttribute::new(array, a.item_size, a.kind, a.normalized);
            na.name = a.name.clone();
            out.attributes.insert(name.clone(), na.shared());
        }
        for g in self.groups.iter() {
            out.groups.push(*g);
        }
        out
    }

    /// A deep copy: attributes are copied, not shared.
    pub fn copy(&mut self, source: &BufferGeometry) {
        self.name = source.name.clone();
        self.type_name = source.type_name.clone();
        self.parameters = source.parameters.clone();
        self.attributes = Map::new();
        for (name, attr) in source.attributes.iter() {
            let a = attr.borrow().clone();
            self.attributes.insert(name.clone(), a.shared());
        }
        self.morph_attributes = Map::new();
        for (name, list) in source.morph_attributes.iter() {
            let mut copies: Vec<AttributeRef> = Vec::new();
            for attr in list.iter() {
                copies.push(attr.borrow().clone().shared());
            }
            self.morph_attributes.insert(name.clone(), copies);
        }
        self.morph_targets_relative = source.morph_targets_relative;
        self.index = match &source.index {
            Some(i) => Some(i.borrow().clone().shared()),
            None => None,
        };
        self.groups = source.groups.clone();
        self.bounding_box = source.bounding_box;
        self.bounding_sphere = source.bounding_sphere;
        self.draw_range = source.draw_range;
    }

    pub fn deep_clone(&self) -> BufferGeometry {
        let mut g = BufferGeometry::new();
        g.copy(self);
        g
    }
}
