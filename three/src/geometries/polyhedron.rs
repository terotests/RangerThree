//! three.js `PolyhedronGeometry` and its tetra-, octa-, icosa- and
//! dodecahedron presets: faces split `detail` times and pushed out to a
//! sphere of `radius`, non-indexed.

use ranger::prelude::*;

use super::builder::GeometryBuilder;
use crate::core::buffer_attribute::BufferAttribute;
use crate::core::buffer_geometry::BufferGeometry;
use crate::maths::math_utils::ArrayKind;
use crate::maths::vector3::Vector3;

const PI: f64 = std::f64::consts::PI;

fn azimuth(v: &Vector3) -> f64 {
    v.z.atan2(-v.x)
}

fn inclination(v: &Vector3) -> f64 {
    (-v.y).atan2((v.x * v.x + v.z * v.z).sqrt())
}

fn vertex_at(vertices: &Vec<f64>, index: f64) -> Vector3 {
    let s = (index as usize) * 3;
    Vector3::new(vertices[s], vertices[s + 1], vertices[s + 2])
}

fn lerped(a: &Vector3, b: &Vector3, t: f64) -> Vector3 {
    let mut r = a.clone();
    r.lerp(b, t);
    r
}

fn push_vertex(buffer: &mut Vec<f64>, v: &Vector3) {
    buffer.push(v.x);
    buffer.push(v.y);
    buffer.push(v.z);
}

fn subdivide_face(buffer: &mut Vec<f64>, a: &Vector3, b: &Vector3, c: &Vector3, detail: i64) {
    let cols = detail + 1;
    let mut grid: Vec<Vec<Vector3>> = Vec::new();
    for i in 0..(cols + 1) {
        let t = i as f64 / cols as f64;
        let aj = lerped(a, c, t);
        let bj = lerped(b, c, t);
        let rows = cols - i;
        let mut row: Vec<Vector3> = Vec::new();
        for j in 0..(rows + 1) {
            if j == 0 && i == cols {
                row.push(aj.clone());
            } else {
                row.push(lerped(&aj, &bj, j as f64 / rows as f64));
            }
        }
        grid.push(row);
    }
    for i in 0..cols {
        let i0 = i as usize;
        let i1 = (i + 1) as usize;
        for j in 0..(2 * (cols - i) - 1) {
            let k = (j / 2) as usize;
            if j % 2 == 0 {
                push_vertex(buffer, &grid[i0][k + 1]);
                push_vertex(buffer, &grid[i1][k]);
                push_vertex(buffer, &grid[i0][k]);
            } else {
                push_vertex(buffer, &grid[i0][k + 1]);
                push_vertex(buffer, &grid[i1][k + 1]);
                push_vertex(buffer, &grid[i1][k]);
            }
        }
    }
}

fn polyhedron_uvs(buffer: &Vec<f64>) -> Vec<f64> {
    let mut uvs: Vec<f64> = Vec::new();
    let mut i = 0;
    while i < buffer.len() {
        let v = Vector3::new(buffer[i], buffer[i + 1], buffer[i + 2]);
        let u = azimuth(&v) / 2.0 / PI + 0.5;
        let w = inclination(&v) / PI + 0.5;
        uvs.push(u);
        uvs.push(1.0 - w);
        i += 3;
    }
    // correctUVs: the seam where azimuth wraps, and the poles
    let mut i = 0;
    let mut j = 0;
    while i < buffer.len() {
        let a = Vector3::new(buffer[i], buffer[i + 1], buffer[i + 2]);
        let b = Vector3::new(buffer[i + 3], buffer[i + 4], buffer[i + 5]);
        let c = Vector3::new(buffer[i + 6], buffer[i + 7], buffer[i + 8]);
        let mut centroid = a.clone();
        centroid.add(&b);
        centroid.add(&c);
        centroid.divide_scalar(3.0);
        let azi = azimuth(&centroid);
        let corners = vec![a, b, c];
        for (n, corner) in corners.iter().enumerate() {
            let stride = j + n * 2;
            let ux = uvs[stride];
            if azi < 0.0 && ux == 1.0 {
                uvs[stride] = ux - 1.0;
            }
            if corner.x == 0.0 && corner.z == 0.0 {
                uvs[stride] = azi / 2.0 / PI + 0.5;
            }
        }
        i += 9;
        j += 6;
    }
    // correctSeam
    let mut i = 0;
    while i < uvs.len() {
        let x0 = uvs[i];
        let x1 = uvs[i + 2];
        let x2 = uvs[i + 4];
        let max = x0.max(x1).max(x2);
        let min = x0.min(x1).min(x2);
        if max > 0.9 && min < 0.1 {
            if x0 < 0.2 {
                uvs[i] += 1.0;
            }
            if x1 < 0.2 {
                uvs[i + 2] += 1.0;
            }
            if x2 < 0.2 {
                uvs[i + 4] += 1.0;
            }
        }
        i += 6;
    }
    uvs
}

fn polyhedron_into(type_name: &str, vertices: &Vec<f64>, indices: &Vec<f64>, radius: f64, detail: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new(type_name);
    b.param("radius", radius);
    b.param("detail", detail);
    let d = detail.floor() as i64;
    let mut buffer: Vec<f64> = Vec::new();
    let mut i = 0;
    while i + 2 < indices.len() {
        let a = vertex_at(vertices, indices[i]);
        let bv = vertex_at(vertices, indices[i + 1]);
        let c = vertex_at(vertices, indices[i + 2]);
        subdivide_face(&mut buffer, &a, &bv, &c, d);
        i += 3;
    }
    let mut i = 0;
    while i < buffer.len() {
        let mut v = Vector3::new(buffer[i], buffer[i + 1], buffer[i + 2]);
        v.normalize();
        v.multiply_scalar(radius);
        buffer[i] = v.x;
        buffer[i + 1] = v.y;
        buffer[i + 2] = v.z;
        i += 3;
    }
    let uvs = polyhedron_uvs(&buffer);
    let mut g = b.geometry.clone();
    g.set_attribute("position", BufferAttribute::new(buffer.clone(), 3, ArrayKind::Float32, false).shared());
    g.set_attribute("normal", BufferAttribute::new(buffer.clone(), 3, ArrayKind::Float32, false).shared());
    g.set_attribute("uv", BufferAttribute::new(uvs, 2, ArrayKind::Float32, false).shared());
    if d == 0 {
        g.compute_vertex_normals();
    } else {
        g.normalize_normals();
    }
    g
}

/// three.js `PolyhedronGeometry` from a vertex list and triangle indices.
pub fn polyhedron_geometry(vertices: &Vec<f64>, indices: &Vec<f64>, radius: f64, detail: f64) -> BufferGeometry {
    polyhedron_into("PolyhedronGeometry", vertices, indices, radius, detail)
}

pub fn tetrahedron_geometry(radius: f64, detail: f64) -> BufferGeometry {
    let vertices = vec![1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0];
    let indices = vec![2.0, 1.0, 0.0, 0.0, 3.0, 2.0, 1.0, 3.0, 0.0, 2.0, 3.0, 1.0];
    polyhedron_into("TetrahedronGeometry", &vertices, &indices, radius, detail)
}

pub fn octahedron_geometry(radius: f64, detail: f64) -> BufferGeometry {
    let vertices = vec![1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0];
    let indices = vec![
        0.0, 2.0, 4.0, 0.0, 4.0, 3.0, 0.0, 3.0, 5.0, 0.0, 5.0, 2.0, 1.0, 2.0, 5.0, 1.0, 5.0, 3.0, 1.0, 3.0, 4.0, 1.0, 4.0, 2.0,
    ];
    polyhedron_into("OctahedronGeometry", &vertices, &indices, radius, detail)
}

pub fn icosahedron_geometry(radius: f64, detail: f64) -> BufferGeometry {
    let t = (1.0 + (5.0_f64).sqrt()) / 2.0;
    let vertices = vec![
        -1.0, t, 0.0, 1.0, t, 0.0, -1.0, -t, 0.0, 1.0, -t, 0.0, 0.0, -1.0, t, 0.0, 1.0, t, 0.0, -1.0, -t, 0.0, 1.0, -t, t, 0.0, -1.0, t, 0.0, 1.0, -t, 0.0, -1.0, -t, 0.0, 1.0,
    ];
    let indices = vec![
        0.0, 11.0, 5.0, 0.0, 5.0, 1.0, 0.0, 1.0, 7.0, 0.0, 7.0, 10.0, 0.0, 10.0, 11.0, 1.0, 5.0, 9.0, 5.0, 11.0, 4.0, 11.0, 10.0, 2.0, 10.0, 7.0, 6.0, 7.0, 1.0, 8.0, 3.0, 9.0, 4.0, 3.0, 4.0, 2.0, 3.0, 2.0, 6.0, 3.0, 6.0, 8.0, 3.0, 8.0, 9.0, 4.0, 9.0, 5.0, 2.0, 4.0, 11.0, 6.0, 2.0, 10.0, 8.0, 6.0, 7.0, 9.0, 8.0, 1.0,
    ];
    polyhedron_into("IcosahedronGeometry", &vertices, &indices, radius, detail)
}

pub fn dodecahedron_geometry(radius: f64, detail: f64) -> BufferGeometry {
    let t = (1.0 + (5.0_f64).sqrt()) / 2.0;
    let r = 1.0 / t;
    let vertices = vec![
        -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 0.0, -r, -t, 0.0, -r, t, 0.0, r, -t, 0.0, r, t, -r, -t, 0.0, -r, t, 0.0, r, -t, 0.0, r, t, 0.0, -t, 0.0, -r, t, 0.0, -r, -t, 0.0, r, t, 0.0, r,
    ];
    let indices = vec![
        3.0, 11.0, 7.0, 3.0, 7.0, 15.0, 3.0, 15.0, 13.0, 7.0, 19.0, 17.0, 7.0, 17.0, 6.0, 7.0, 6.0, 15.0, 17.0, 4.0, 8.0, 17.0, 8.0, 10.0, 17.0, 10.0, 6.0, 8.0, 0.0, 16.0, 8.0, 16.0, 2.0, 8.0, 2.0, 10.0, 0.0, 12.0, 1.0, 0.0, 1.0, 18.0, 0.0, 18.0, 16.0, 6.0, 10.0, 2.0, 6.0, 2.0, 13.0, 6.0, 13.0, 15.0, 2.0, 16.0, 18.0, 2.0, 18.0, 3.0, 2.0, 3.0, 13.0, 18.0, 1.0, 9.0, 18.0, 9.0, 11.0, 18.0, 11.0, 3.0, 4.0, 14.0, 12.0, 4.0, 12.0, 0.0, 4.0, 0.0, 8.0, 11.0, 9.0, 5.0, 11.0, 5.0, 19.0, 11.0, 19.0, 7.0, 19.0, 5.0, 14.0, 19.0, 14.0, 4.0, 19.0, 4.0, 17.0, 1.0, 12.0, 14.0, 1.0, 14.0, 5.0, 1.0, 5.0, 9.0,
    ];
    polyhedron_into("DodecahedronGeometry", &vertices, &indices, radius, detail)
}
