//! Shared vertex lists the geometry generators fill.

use ranger::prelude::*;

use crate::core::buffer_attribute::BufferAttribute;
use crate::core::buffer_geometry::BufferGeometry;
use crate::maths::math_utils::ArrayKind;

/// Position, normal and uv lists plus an index, as three.js's generators
/// build them before handing them to a BufferGeometry.
#[derive(Clone)]
pub struct GeometryBuilder {
    pub vertices: Vec<f64>,
    pub normals: Vec<f64>,
    pub uvs: Vec<f64>,
    pub indices: Vec<f64>,
    pub geometry: BufferGeometry,
}

impl GeometryBuilder {
    pub fn new(type_name: &str) -> GeometryBuilder {
        let mut geometry = BufferGeometry::new();
        geometry.type_name = type_name.to_string();
        GeometryBuilder {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
            geometry,
        }
    }

    pub fn param(&mut self, name: &str, value: f64) {
        self.geometry.parameters.insert(name.to_string(), value);
    }

    pub fn vertex(&mut self, x: f64, y: f64, z: f64) {
        self.vertices.push(x);
        self.vertices.push(y);
        self.vertices.push(z);
    }

    pub fn normal(&mut self, x: f64, y: f64, z: f64) {
        self.normals.push(x);
        self.normals.push(y);
        self.normals.push(z);
    }

    pub fn uv(&mut self, u: f64, v: f64) {
        self.uvs.push(u);
        self.uvs.push(v);
    }

    pub fn face(&mut self, a: f64, b: f64, c: f64) {
        self.indices.push(a);
        self.indices.push(b);
        self.indices.push(c);
    }

    pub fn group(&mut self, start: i64, count: i64, material_index: i64) {
        self.geometry.add_group(start, count, material_index);
    }

    /// The geometry with an index and position, normal and uv attributes.
    pub fn finish(&self) -> BufferGeometry {
        let mut g = self.geometry.clone();
        if !self.indices.is_empty() {
            g.set_index(self.indices.clone());
        }
        g.set_attribute("position", BufferAttribute::new(self.vertices.clone(), 3, ArrayKind::Float32, false).shared());
        g.set_attribute("normal", BufferAttribute::new(self.normals.clone(), 3, ArrayKind::Float32, false).shared());
        g.set_attribute("uv", BufferAttribute::new(self.uvs.clone(), 2, ArrayKind::Float32, false).shared());
        g
    }
}
