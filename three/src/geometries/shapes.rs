//! Geometries built from curves and from other geometries: three.js
//! `ShapeGeometry`, `ExtrudeGeometry`, `TubeGeometry`, `EdgesGeometry` and
//! `WireframeGeometry`.

use ranger::prelude::*;

use super::builder::GeometryBuilder;
use crate::core::buffer_attribute::BufferAttribute;
use crate::core::buffer_geometry::BufferGeometry;
use crate::extras::curves::{frames_from_tangents, Curve, FrenetFrames, EPSILON};
use crate::extras::shape_utils::{is_clock_wise, triangulate_shape, without_dup_end};
use crate::maths::math_utils::{ArrayKind, DEG2RAD};
use crate::maths::triangle::Triangle;
use crate::maths::vector2::{js_round, Vector2};
use crate::maths::vector3::Vector3;

const PI: f64 = std::f64::consts::PI;

fn to_2d(points: &Vec<Vector3>) -> Vec<Vector2> {
    let mut out: Vec<Vector2> = Vec::new();
    for p in points.iter() {
        out.push(Vector2::new(p.x, p.y));
    }
    out
}

fn reversed(points: &Vec<Vector2>) -> Vec<Vector2> {
    let mut out = points.clone();
    out.reverse();
    out
}

/// A shape's outline and holes at `divisions` per curve, as `Vector2`.
fn extract_points(shape: &mut Curve, divisions: i64) -> (Vec<Vector2>, Vec<Vec<Vector2>>) {
    let outline = to_2d(&shape.get_points(divisions));
    let mut holes: Vec<Vec<Vector2>> = Vec::new();
    for h in shape.get_points_holes(divisions).iter() {
        holes.push(to_2d(h));
    }
    (outline, holes)
}

// ---- ShapeGeometry ------------------------------------------------------

/// three.js `ShapeGeometry`: the shapes filled flat at z = 0. `grouped` is
/// three.js's "shapes is an array" (one group per shape).
pub fn shape_geometry(mut shapes: Vec<Curve>, curve_segments: f64, grouped: bool) -> BufferGeometry {
    let mut b = GeometryBuilder::new("ShapeGeometry");
    b.param("curveSegments", curve_segments);
    let mut group_start: i64 = 0;
    for s in 0..shapes.len() {
        let (outline, holes_in) = extract_points(&mut shapes[s], curve_segments as i64);
        let index_offset = (b.vertices.len() / 3) as f64;
        let mut outline = outline;
        if !is_clock_wise(&outline) {
            outline = reversed(&outline);
        }
        let mut holes: Vec<Vec<Vector2>> = Vec::new();
        for h in holes_in.iter() {
            holes.push(if is_clock_wise(h) { reversed(h) } else { h.clone() });
        }
        // triangulate_shape leaves out a repeated end point; so do the vertices
        let faces = triangulate_shape(&outline, &holes);
        let mut all = without_dup_end(&outline);
        for h in holes.iter() {
            for p in without_dup_end(h).iter() {
                all.push(*p);
            }
        }
        for v in all.iter() {
            b.vertex(v.x, v.y, 0.0);
            b.normal(0.0, 0.0, 1.0);
            b.uv(v.x, v.y);
        }
        let mut count: i64 = 0;
        for f in faces.iter() {
            b.face(f[0] as f64 + index_offset, f[1] as f64 + index_offset, f[2] as f64 + index_offset);
            count += 3;
        }
        if grouped {
            b.group(group_start, count, s as i64);
            group_start += count;
        }
    }
    b.finish()
}

// ---- ExtrudeGeometry ----------------------------------------------------

/// three.js `ExtrudeGeometry` options. For extrusion along a path,
/// `path_points` and `path_frames` hold the path's spaced points and Frenet
/// frames at `steps` (see `extrude_along`).
#[derive(Clone, Debug, PartialEq)]
pub struct ExtrudeOptions {
    pub curve_segments: i64,
    pub steps: i64,
    pub depth: f64,
    pub bevel_enabled: bool,
    pub bevel_thickness: f64,
    pub bevel_size: f64,
    pub bevel_offset: f64,
    pub bevel_segments: i64,
    pub extrude_by_path: bool,
    pub path_points: Vec<Vector3>,
    pub path_frames: FrenetFrames,
}

impl Default for ExtrudeOptions {
    fn default() -> ExtrudeOptions {
        ExtrudeOptions::new()
    }
}

impl ExtrudeOptions {
    pub fn new() -> ExtrudeOptions {
        ExtrudeOptions {
            curve_segments: 12,
            steps: 1,
            depth: 1.0,
            bevel_enabled: true,
            bevel_thickness: 0.2,
            bevel_size: 0.1,
            bevel_offset: 0.0,
            bevel_segments: 3,
            extrude_by_path: false,
            path_points: Vec::new(),
            path_frames: FrenetFrames { tangents: Vec::new(), normals: Vec::new(), binormals: Vec::new() },
        }
    }

    /// Extrude along `path` (three.js's `extrudePath`): closed Catmull-Rom
    /// paths pass `closed`.
    pub fn extrude_along(&mut self, path: &mut Curve, closed: bool) {
        self.extrude_by_path = true;
        self.path_points = path.get_spaced_points(self.steps);
        self.path_frames = path.compute_frenet_frames(self.steps, closed);
    }
}

fn merge_overlapping_points(points: &mut Vec<Vector2>) {
    if points.is_empty() {
        return;
    }
    let threshold_sq = 1e-10 * 1e-10;
    let mut prev = points[0];
    let mut i = 1;
    while i <= points.len() && !points.is_empty() {
        let current_index = i % points.len();
        let cur = points[current_index];
        let dx = cur.x - prev.x;
        let dy = cur.y - prev.y;
        let dist_sq = dx * dx + dy * dy;
        let scale = cur.x.abs().max(cur.y.abs()).max(prev.x.abs()).max(prev.y.abs());
        if dist_sq <= threshold_sq * scale * scale {
            points.remove(current_index);
            continue;
        }
        prev = cur;
        i += 1;
    }
}

fn js_sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn bevel_vec(pt: &Vector2, prev: &Vector2, next: &Vector2) -> Vector2 {
    let v_prev_x = pt.x - prev.x;
    let v_prev_y = pt.y - prev.y;
    let v_next_x = next.x - pt.x;
    let v_next_y = next.y - pt.y;
    let v_prev_lensq = v_prev_x * v_prev_x + v_prev_y * v_prev_y;
    let collinear0 = v_prev_x * v_next_y - v_prev_y * v_next_x;
    let mut v_trans_x = 0.0;
    let mut v_trans_y = 0.0;
    let mut shrink_by = 1.0;
    if collinear0.abs() > EPSILON {
        let v_prev_len = v_prev_lensq.sqrt();
        let v_next_len = (v_next_x * v_next_x + v_next_y * v_next_y).sqrt();
        let prev_shift_x = prev.x - v_prev_y / v_prev_len;
        let prev_shift_y = prev.y + v_prev_x / v_prev_len;
        let next_shift_x = next.x - v_next_y / v_next_len;
        let next_shift_y = next.y + v_next_x / v_next_len;
        let sf = ((next_shift_x - prev_shift_x) * v_next_y - (next_shift_y - prev_shift_y) * v_next_x)
            / (v_prev_x * v_next_y - v_prev_y * v_next_x);
        v_trans_x = prev_shift_x + v_prev_x * sf - pt.x;
        v_trans_y = prev_shift_y + v_prev_y * sf - pt.y;
        let v_trans_lensq = v_trans_x * v_trans_x + v_trans_y * v_trans_y;
        if v_trans_lensq <= 2.0 {
            return Vector2::new(v_trans_x, v_trans_y);
        }
        shrink_by = (v_trans_lensq / 2.0).sqrt();
    } else {
        let mut direction_eq = false;
        if v_prev_x > EPSILON {
            if v_next_x > EPSILON {
                direction_eq = true;
            }
        } else if v_prev_x < -EPSILON {
            if v_next_x < -EPSILON {
                direction_eq = true;
            }
        } else if js_sign(v_prev_y) == js_sign(v_next_y) {
            direction_eq = true;
        }
        if direction_eq {
            v_trans_x = -v_prev_y;
            v_trans_y = v_prev_x;
            shrink_by = v_prev_lensq.sqrt();
        } else {
            v_trans_x = v_prev_x;
            v_trans_y = v_prev_y;
            shrink_by = (v_prev_lensq / 2.0).sqrt();
        }
    }
    Vector2::new(v_trans_x / shrink_by, v_trans_y / shrink_by)
}

fn ring_movements(ring: &Vec<Vector2>) -> Vec<Vector2> {
    let il = ring.len();
    let mut out: Vec<Vector2> = Vec::new();
    for i in 0..il {
        let j = if i == 0 { il - 1 } else { i - 1 };
        let k = if i + 1 == il { 0 } else { i + 1 };
        out.push(bevel_vec(&ring[i], &ring[j], &ring[k]));
    }
    out
}

fn scaled(pt: &Vector2, vec: &Vector2, size: f64) -> Vector2 {
    let mut p = *pt;
    p.add_scaled_vector(vec, size);
    p
}

/// The flat lists ExtrudeGeometry fills: vertex positions by layer
/// (`placeholder`), then the triangle soup and its uvs.
struct ExtrudeLists {
    placeholder: Vec<f64>,
    vertices: Vec<f64>,
    uvs: Vec<f64>,
}

impl ExtrudeLists {
    fn v(&mut self, x: f64, y: f64, z: f64) {
        self.placeholder.push(x);
        self.placeholder.push(y);
        self.placeholder.push(z);
    }

    fn add_vertex(&mut self, index: i64) {
        let i = (index * 3) as usize;
        let (x, y, z) = (self.placeholder[i], self.placeholder[i + 1], self.placeholder[i + 2]);
        self.vertices.push(x);
        self.vertices.push(y);
        self.vertices.push(z);
    }

    fn at(&self, index: usize, k: usize) -> f64 {
        self.vertices[index * 3 + k]
    }

    // three.js's WorldUVGenerator
    fn f3(&mut self, a: i64, b: i64, c: i64) {
        self.add_vertex(a);
        self.add_vertex(b);
        self.add_vertex(c);
        let next = self.vertices.len() / 3;
        for n in (next - 3)..next {
            let (x, y) = (self.at(n, 0), self.at(n, 1));
            self.uvs.push(x);
            self.uvs.push(y);
        }
    }

    fn f4(&mut self, a: i64, b: i64, c: i64, d: i64) {
        self.add_vertex(a);
        self.add_vertex(b);
        self.add_vertex(d);
        self.add_vertex(b);
        self.add_vertex(c);
        self.add_vertex(d);
        let next = self.vertices.len() / 3;
        let ia = next - 6;
        let ib = next - 3;
        let ic = next - 2;
        let id = next - 1;
        let k = if (self.at(ia, 1) - self.at(ib, 1)).abs() < (self.at(ia, 0) - self.at(ib, 0)).abs() { 0 } else { 1 };
        let order = vec![ia, ib, id, ib, ic, id];
        for n in order.iter() {
            let (u, z) = (self.at(*n, k), self.at(*n, 2));
            self.uvs.push(u);
            self.uvs.push(1.0 - z);
        }
    }
}

/// three.js `ExtrudeGeometry` with its World UV generator.
pub fn extrude_geometry(mut shapes: Vec<Curve>, options: &ExtrudeOptions) -> BufferGeometry {
    let mut g = BufferGeometry::new();
    g.type_name = "ExtrudeGeometry".to_string();
    let mut lists = ExtrudeLists { placeholder: Vec::new(), vertices: Vec::new(), uvs: Vec::new() };
    for s in 0..shapes.len() {
        lists.placeholder = Vec::new();
        extrude_shape(&mut g, &mut lists, &mut shapes[s], options);
    }
    g.set_attribute("position", BufferAttribute::new(lists.vertices.clone(), 3, ArrayKind::Float32, false).shared());
    g.set_attribute("uv", BufferAttribute::new(lists.uvs.clone(), 2, ArrayKind::Float32, false).shared());
    g.compute_vertex_normals();
    g
}

fn extrude_shape(g: &mut BufferGeometry, lists: &mut ExtrudeLists, shape: &mut Curve, o: &ExtrudeOptions) {
    let steps = o.steps;
    let depth = o.depth;
    let by_path = o.extrude_by_path;
    let bevel_enabled = o.bevel_enabled && !by_path;
    let mut bevel_segments = o.bevel_segments;
    let mut bevel_thickness = o.bevel_thickness;
    let mut bevel_size = o.bevel_size;
    let mut bevel_offset = o.bevel_offset;
    if !bevel_enabled {
        bevel_segments = 0;
        bevel_thickness = 0.0;
        bevel_size = 0.0;
        bevel_offset = 0.0;
    }
    let (outline, holes_in) = extract_points(shape, o.curve_segments);
    let mut contour = outline;
    let mut holes: Vec<Vec<Vector2>> = holes_in;
    if !is_clock_wise(&contour) {
        contour = reversed(&contour);
        for h in 0..holes.len() {
            if is_clock_wise(&holes[h]) {
                holes[h] = reversed(&holes[h]);
            }
        }
    }
    merge_overlapping_points(&mut contour);
    for h in 0..holes.len() {
        merge_overlapping_points(&mut holes[h]);
    }
    let num_holes = holes.len();
    let mut vertices = contour.clone();
    for h in holes.iter() {
        for p in h.iter() {
            vertices.push(*p);
        }
    }
    let vlen = vertices.len() as i64;
    let contour_movements = ring_movements(&contour);
    let mut holes_movements: Vec<Vec<Vector2>> = Vec::new();
    let mut vertices_movements = contour_movements.clone();
    for h in holes.iter() {
        let m = ring_movements(h);
        for v in m.iter() {
            vertices_movements.push(*v);
        }
        holes_movements.push(m);
    }
    let mut faces: Vec<Vec<i64>> = Vec::new();
    if bevel_segments == 0 {
        faces = triangulate_shape(&contour, &holes);
    } else {
        let mut contracted: Vec<Vector2> = Vec::new();
        let mut expanded: Vec<Vec<Vector2>> = Vec::new();
        for b in 0..bevel_segments {
            let t = b as f64 / bevel_segments as f64;
            let z = bevel_thickness * (t * PI / 2.0).cos();
            let bs = bevel_size * (t * PI / 2.0).sin() + bevel_offset;
            for i in 0..contour.len() {
                let vert = scaled(&contour[i], &contour_movements[i], bs);
                lists.v(vert.x, vert.y, -z);
                if t == 0.0 {
                    contracted.push(vert);
                }
            }
            for h in 0..num_holes {
                let mut one: Vec<Vector2> = Vec::new();
                for i in 0..holes[h].len() {
                    let vert = scaled(&holes[h][i], &holes_movements[h][i], bs);
                    lists.v(vert.x, vert.y, -z);
                    if t == 0.0 {
                        one.push(vert);
                    }
                }
                if t == 0.0 {
                    expanded.push(one);
                }
            }
        }
        faces = triangulate_shape(&contracted, &expanded);
    }
    let bs = bevel_size + bevel_offset;
    for s in 0..(steps + 1) {
        for i in 0..(vlen as usize) {
            let vert = if bevel_enabled { scaled(&vertices[i], &vertices_movements[i], bs) } else { vertices[i] };
            if !by_path {
                let z = if s == 0 { 0.0 } else { depth / steps as f64 * s as f64 };
                lists.v(vert.x, vert.y, z);
            } else {
                let su = s as usize;
                let mut normal = o.path_frames.normals[su];
                normal.multiply_scalar(vert.x);
                let mut binormal = o.path_frames.binormals[su];
                binormal.multiply_scalar(vert.y);
                let mut p = o.path_points[su];
                p.add(&normal);
                p.add(&binormal);
                lists.v(p.x, p.y, p.z);
            }
        }
    }
    let mut b = bevel_segments - 1;
    while b >= 0 {
        let t = b as f64 / bevel_segments as f64;
        let z = bevel_thickness * (t * PI / 2.0).cos();
        let bs = bevel_size * (t * PI / 2.0).sin() + bevel_offset;
        for i in 0..contour.len() {
            let vert = scaled(&contour[i], &contour_movements[i], bs);
            lists.v(vert.x, vert.y, depth + z);
        }
        for h in 0..num_holes {
            for i in 0..holes[h].len() {
                let vert = scaled(&holes[h][i], &holes_movements[h][i], bs);
                lists.v(vert.x, vert.y, depth + z);
            }
        }
        b -= 1;
    }
    // lids
    let start = (lists.vertices.len() / 3) as i64;
    if bevel_enabled {
        let offset = 0;
        for f in faces.iter() {
            lists.f3(f[2] + offset, f[1] + offset, f[0] + offset);
        }
        let offset = vlen * (steps + bevel_segments * 2);
        for f in faces.iter() {
            lists.f3(f[0] + offset, f[1] + offset, f[2] + offset);
        }
    } else {
        for f in faces.iter() {
            lists.f3(f[2], f[1], f[0]);
        }
        for f in faces.iter() {
            lists.f3(f[0] + vlen * steps, f[1] + vlen * steps, f[2] + vlen * steps);
        }
    }
    g.add_group(start, (lists.vertices.len() / 3) as i64 - start, 0);
    // side walls
    let start = (lists.vertices.len() / 3) as i64;
    let layers = steps + bevel_segments * 2;
    let mut layer_offset: i64 = 0;
    sidewalls(lists, &contour, layer_offset, vlen, layers);
    layer_offset += contour.len() as i64;
    for h in holes.iter() {
        sidewalls(lists, h, layer_offset, vlen, layers);
        layer_offset += h.len() as i64;
    }
    g.add_group(start, (lists.vertices.len() / 3) as i64 - start, 1);
}

fn sidewalls(lists: &mut ExtrudeLists, ring: &Vec<Vector2>, layer_offset: i64, vlen: i64, layers: i64) {
    let n = ring.len() as i64;
    let mut i = n - 1;
    while i >= 0 {
        let j = i;
        let k = if i - 1 < 0 { n - 1 } else { i - 1 };
        for s in 0..layers {
            let slen1 = vlen * s;
            let slen2 = vlen * (s + 1);
            lists.f4(layer_offset + j + slen1, layer_offset + k + slen1, layer_offset + k + slen2, layer_offset + j + slen2);
        }
        i -= 1;
    }
}

// ---- TubeGeometry -------------------------------------------------------

/// The tube's centre points and frames along `path` (three.js
/// `TubeGeometry` samples the path at equal lengths).
pub fn tube_geometry(path: &mut Curve, tubular_segments: f64, radius: f64, radial_segments: f64, closed: bool) -> BufferGeometry {
    let segs = tubular_segments as i64;
    let frames = path.compute_frenet_frames(segs, closed);
    let mut points: Vec<Vector3> = Vec::new();
    for i in 0..(segs + 1) {
        points.push(path.get_point_at(i as f64 / tubular_segments));
    }
    tube_from_frames(&points, &frames, tubular_segments, radius, radial_segments, closed)
}

/// Frames for a curve known only by its tangents (a JavaScript subclass of
/// `Curve`): see `frames_from_tangents`.
pub fn frames_for_tangents(tangents: Vec<Vector3>, segments: i64, closed: bool) -> FrenetFrames {
    frames_from_tangents(tangents, segments, closed)
}

/// `TubeGeometry` around `points` (`tubular_segments + 1` of them, at equal
/// lengths along the path) with `frames` from `compute_frenet_frames`.
pub fn tube_from_frames(points: &Vec<Vector3>, frames: &FrenetFrames, tubular_segments: f64, radius: f64, radial_segments: f64, closed: bool) -> BufferGeometry {
    let mut b = GeometryBuilder::new("TubeGeometry");
    b.param("tubularSegments", tubular_segments);
    b.param("radius", radius);
    b.param("radialSegments", radial_segments);
    let segs = tubular_segments as i64;
    let radial = radial_segments as i64;
    for i in 0..(segs + 1) {
        let at: usize = if i == segs && closed { 0 } else { i as usize };
        let p = points[at];
        let n = frames.normals[at];
        let bn = frames.binormals[at];
        for j in 0..(radial + 1) {
            let v = j as f64 / radial_segments * PI * 2.0;
            let sin = v.sin();
            let cos = -v.cos();
            let mut normal = Vector3::new(cos * n.x + sin * bn.x, cos * n.y + sin * bn.y, cos * n.z + sin * bn.z);
            normal.normalize();
            b.normal(normal.x, normal.y, normal.z);
            b.vertex(p.x + radius * normal.x, p.y + radius * normal.y, p.z + radius * normal.z);
        }
    }
    for i in 0..(segs + 1) {
        for j in 0..(radial + 1) {
            b.uv(i as f64 / tubular_segments, j as f64 / radial_segments);
        }
    }
    for j in 1..(segs + 1) {
        for i in 1..(radial + 1) {
            let a = (radial + 1) * (j - 1) + (i - 1);
            let bb = (radial + 1) * j + (i - 1);
            let c = (radial + 1) * j + i;
            let d = (radial + 1) * (j - 1) + i;
            b.face(a as f64, bb as f64, d as f64);
            b.face(bb as f64, c as f64, d as f64);
        }
    }
    b.finish()
}

// ---- EdgesGeometry / WireframeGeometry ----------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct EdgeEntry {
    index0: i64,
    index1: i64,
    normal: Vector3,
    alive: bool,
}

fn edge_hash(v: &Vector3, precision: f64) -> String {
    let x = js_round(v.x * precision) as i64;
    let y = js_round(v.y * precision) as i64;
    let z = js_round(v.z * precision) as i64;
    format!("{},{},{}", x, y, z)
}

/// three.js `EdgesGeometry`: line segments along the edges where the faces
/// meet at more than `threshold_angle` degrees (and the open borders).
pub fn edges_geometry(geometry: &BufferGeometry, threshold_angle: f64) -> BufferGeometry {
    let mut out = BufferGeometry::new();
    out.type_name = "EdgesGeometry".to_string();
    out.parameters.insert("thresholdAngle".to_string(), threshold_angle);
    let position = match geometry.get_attribute("position") {
        Some(p) => p,
        None => return out,
    };
    let pos = position.borrow();
    let index = geometry.get_index();
    let index_count = match &index {
        Some(ix) => ix.borrow().count,
        None => pos.count,
    };
    let precision = 10000.0;
    let threshold_dot = (DEG2RAD * threshold_angle).cos();
    let mut keys: Map<String, i64> = Map::new();
    let mut entries: Vec<EdgeEntry> = Vec::new();
    let mut vertices: Vec<f64> = Vec::new();
    let mut i = 0;
    while i < index_count {
        let mut idx: Vec<i64> = vec![i as i64, i as i64 + 1, i as i64 + 2];
        if let Some(ix) = &index {
            let ib = ix.borrow();
            idx = vec![ib.get_x(i) as i64, ib.get_x(i + 1) as i64, ib.get_x(i + 2) as i64];
        }
        let tri = Triangle::new(pos.get_vector3(idx[0] as usize), pos.get_vector3(idx[1] as usize), pos.get_vector3(idx[2] as usize));
        let normal = tri.get_normal();
        let verts = vec![tri.a, tri.b, tri.c];
        let hashes = vec![edge_hash(&tri.a, precision), edge_hash(&tri.b, precision), edge_hash(&tri.c, precision)];
        i += 3;
        if hashes[0] == hashes[1] || hashes[1] == hashes[2] || hashes[2] == hashes[0] {
            continue;
        }
        for j in 0..3 {
            let jn = (j + 1) % 3;
            let hash = format!("{}_{}", hashes[j], hashes[jn]);
            let reverse = format!("{}_{}", hashes[jn], hashes[j]);
            let rev_at = match keys.get(&reverse) {
                Some(k) => *k,
                None => -1,
            };
            if rev_at >= 0 && entries[rev_at as usize].alive {
                if normal.dot(&entries[rev_at as usize].normal) <= threshold_dot {
                    let (v0, v1) = (verts[j], verts[jn]);
                    vertices.push(v0.x);
                    vertices.push(v0.y);
                    vertices.push(v0.z);
                    vertices.push(v1.x);
                    vertices.push(v1.y);
                    vertices.push(v1.z);
                }
                entries[rev_at as usize].alive = false;
            } else if !keys.contains_key(&hash) {
                keys.insert(hash, entries.len() as i64);
                entries.push(EdgeEntry { index0: idx[j], index1: idx[jn], normal, alive: true });
            }
        }
    }
    for e in entries.iter() {
        if e.alive {
            let v0 = pos.get_vector3(e.index0 as usize);
            let v1 = pos.get_vector3(e.index1 as usize);
            vertices.push(v0.x);
            vertices.push(v0.y);
            vertices.push(v0.z);
            vertices.push(v1.x);
            vertices.push(v1.y);
            vertices.push(v1.z);
        }
    }
    out.set_attribute("position", BufferAttribute::new(vertices, 3, ArrayKind::Float32, false).shared());
    out
}

fn wire_key(a: &Vector3, b: &Vector3) -> String {
    // + 0.0 turns -0 into 0, as JavaScript's number-to-string does
    format!("{},{},{}-{},{},{}", a.x + 0.0, a.y + 0.0, a.z + 0.0, b.x + 0.0, b.y + 0.0, b.z + 0.0)
}

fn unique_edge(start: &Vector3, end: &Vector3, edges: &mut Map<String, bool>) -> bool {
    let h1 = wire_key(start, end);
    let h2 = wire_key(end, start);
    if edges.contains_key(&h1) || edges.contains_key(&h2) {
        return false;
    }
    edges.insert(h1, true);
    edges.insert(h2, true);
    true
}

fn push_edge(vertices: &mut Vec<f64>, a: &Vector3, b: &Vector3) {
    vertices.push(a.x);
    vertices.push(a.y);
    vertices.push(a.z);
    vertices.push(b.x);
    vertices.push(b.y);
    vertices.push(b.z);
}

/// three.js `WireframeGeometry`: each triangle edge once, as line segments.
pub fn wireframe_geometry(geometry: &BufferGeometry) -> BufferGeometry {
    let mut out = BufferGeometry::new();
    out.type_name = "WireframeGeometry".to_string();
    let position = match geometry.get_attribute("position") {
        Some(p) => p,
        None => return out,
    };
    let pos = position.borrow();
    let mut vertices: Vec<f64> = Vec::new();
    let mut edges: Map<String, bool> = Map::new();
    if let Some(ix) = geometry.get_index() {
        let indices = ix.borrow();
        let mut groups = geometry.groups.clone();
        if groups.is_empty() {
            groups.push(crate::core::buffer_geometry::GeometryGroup { start: 0, count: indices.count as i64, material_index: 0 });
        }
        for g in groups.iter() {
            let mut i = g.start;
            while i < g.start + g.count {
                for j in 0..3 {
                    let i1 = indices.get_x((i + j) as usize) as usize;
                    let i2 = indices.get_x((i + (j + 1) % 3) as usize) as usize;
                    let start = pos.get_vector3(i1);
                    let end = pos.get_vector3(i2);
                    if unique_edge(&start, &end, &mut edges) {
                        push_edge(&mut vertices, &start, &end);
                    }
                }
                i += 3;
            }
        }
    } else {
        let tris = pos.count / 3;
        for i in 0..tris {
            for j in 0..3 {
                let start = pos.get_vector3(3 * i + j);
                let end = pos.get_vector3(3 * i + (j + 1) % 3);
                if unique_edge(&start, &end, &mut edges) {
                    push_edge(&mut vertices, &start, &end);
                }
            }
        }
    }
    out.set_attribute("position", BufferAttribute::new(vertices, 3, ArrayKind::Float32, false).shared());
    out
}
