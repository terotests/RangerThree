//! Outline helpers (three.js `ShapeUtils`).

use ranger::prelude::*;

use super::earcut::triangulate;
use crate::maths::vector2::Vector2;

/// Signed area of a closed outline; negative when it runs clockwise.
pub fn area(contour: &Vec<Vector2>) -> f64 {
    let n = contour.len();
    let mut a = 0.0;
    if n == 0 {
        return a;
    }
    let mut p = n - 1;
    for q in 0..n {
        a += contour[p].x * contour[q].y - contour[q].x * contour[p].y;
        p = q;
    }
    a * 0.5
}

pub fn is_clock_wise(pts: &Vec<Vector2>) -> bool {
    area(pts) < 0.0
}

/// `points` without its last point when that repeats the first (three.js
/// drops it from the caller's array; the facade does that part).
pub fn without_dup_end(points: &Vec<Vector2>) -> Vec<Vector2> {
    let mut out = points.clone();
    let l = out.len();
    if l > 2 && out[l - 1].equals(&out[0]) {
        out.pop();
    }
    out
}

/// Triangles (three vertex indices each) covering `contour` minus `holes`;
/// hole vertices are numbered after the contour's.
pub fn triangulate_shape(contour: &Vec<Vector2>, holes: &Vec<Vec<Vector2>>) -> Vec<Vec<i64>> {
    let mut vertices: Vec<f64> = Vec::new();
    let mut hole_indices: Vec<i64> = Vec::new();
    let c = without_dup_end(contour);
    for p in c.iter() {
        vertices.push(p.x);
        vertices.push(p.y);
    }
    let mut hole_index = c.len() as i64;
    for h in holes.iter() {
        let hole = without_dup_end(h);
        hole_indices.push(hole_index);
        hole_index += hole.len() as i64;
        for p in hole.iter() {
            vertices.push(p.x);
            vertices.push(p.y);
        }
    }
    let triangles = triangulate(&vertices, &hole_indices);
    let mut faces: Vec<Vec<i64>> = Vec::new();
    let mut i = 0;
    while i + 2 < triangles.len() {
        faces.push(vec![triangles[i], triangles[i + 1], triangles[i + 2]]);
        i += 3;
    }
    faces
}
