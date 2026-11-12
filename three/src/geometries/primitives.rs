//! Generated shapes with three.js's vertex order, normals, uvs and groups:
//! box, plane, sphere, cylinder, cone, circle, ring, torus, torus knot,
//! capsule and lathe.

use ranger::prelude::*;

use super::builder::GeometryBuilder;
use crate::core::buffer_geometry::BufferGeometry;
use crate::maths::vector2::Vector2;
use crate::maths::vector3::Vector3;

const PI: f64 = std::f64::consts::PI;

fn bool_value(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

/// One side of a box: `u`, `v`, `w` are axis numbers (0 = x, 1 = y, 2 = z).
struct BoxSide {
    u: i64,
    v: i64,
    w: i64,
    udir: f64,
    vdir: f64,
    width: f64,
    height: f64,
    depth: f64,
    grid_x: i64,
    grid_y: i64,
    material_index: i64,
}

fn build_box_side(b: &mut GeometryBuilder, s: &BoxSide, number_of_vertices: i64, group_start: i64) -> i64 {
    let segment_width = s.width / s.grid_x as f64;
    let segment_height = s.height / s.grid_y as f64;
    let width_half = s.width / 2.0;
    let height_half = s.height / 2.0;
    let depth_half = s.depth / 2.0;
    let grid_x1 = s.grid_x + 1;
    let grid_y1 = s.grid_y + 1;
    let mut group_count: i64 = 0;
    let mut vector = Vector3::zero();
    for iy in 0..grid_y1 {
        let y = iy as f64 * segment_height - height_half;
        for ix in 0..grid_x1 {
            let x = ix as f64 * segment_width - width_half;
            vector.set_component(s.u, x * s.udir);
            vector.set_component(s.v, y * s.vdir);
            vector.set_component(s.w, depth_half);
            b.vertex(vector.x, vector.y, vector.z);
            vector.set_component(s.u, 0.0);
            vector.set_component(s.v, 0.0);
            vector.set_component(s.w, if s.depth > 0.0 { 1.0 } else { -1.0 });
            b.normal(vector.x, vector.y, vector.z);
            b.uv(ix as f64 / s.grid_x as f64, 1.0 - (iy as f64 / s.grid_y as f64));
        }
    }
    for iy in 0..s.grid_y {
        for ix in 0..s.grid_x {
            let a = (number_of_vertices + ix + grid_x1 * iy) as f64;
            let bb = (number_of_vertices + ix + grid_x1 * (iy + 1)) as f64;
            let c = (number_of_vertices + (ix + 1) + grid_x1 * (iy + 1)) as f64;
            let d = (number_of_vertices + (ix + 1) + grid_x1 * iy) as f64;
            b.face(a, bb, d);
            b.face(bb, c, d);
            group_count += 6;
        }
    }
    b.group(group_start, group_count, s.material_index);
    group_count
}

/// three.js `BoxGeometry`: six sides, one group (material index) each.
pub fn box_geometry(width: f64, height: f64, depth: f64, width_segments: f64, height_segments: f64, depth_segments: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("BoxGeometry");
    b.param("width", width);
    b.param("height", height);
    b.param("depth", depth);
    b.param("widthSegments", width_segments);
    b.param("heightSegments", height_segments);
    b.param("depthSegments", depth_segments);
    let ws = width_segments.floor() as i64;
    let hs = height_segments.floor() as i64;
    let ds = depth_segments.floor() as i64;
    let sides = vec![
        BoxSide { u: 2, v: 1, w: 0, udir: -1.0, vdir: -1.0, width: depth, height, depth: width, grid_x: ds, grid_y: hs, material_index: 0 },
        BoxSide { u: 2, v: 1, w: 0, udir: 1.0, vdir: -1.0, width: depth, height, depth: -width, grid_x: ds, grid_y: hs, material_index: 1 },
        BoxSide { u: 0, v: 2, w: 1, udir: 1.0, vdir: 1.0, width, height: depth, depth: height, grid_x: ws, grid_y: ds, material_index: 2 },
        BoxSide { u: 0, v: 2, w: 1, udir: 1.0, vdir: -1.0, width, height: depth, depth: -height, grid_x: ws, grid_y: ds, material_index: 3 },
        BoxSide { u: 0, v: 1, w: 2, udir: 1.0, vdir: -1.0, width, height, depth, grid_x: ws, grid_y: hs, material_index: 4 },
        BoxSide { u: 0, v: 1, w: 2, udir: -1.0, vdir: -1.0, width, height, depth: -depth, grid_x: ws, grid_y: hs, material_index: 5 },
    ];
    let mut number_of_vertices: i64 = 0;
    let mut group_start: i64 = 0;
    for s in sides.iter() {
        let count = build_box_side(&mut b, s, number_of_vertices, group_start);
        group_start += count;
        number_of_vertices += (s.grid_x + 1) * (s.grid_y + 1);
    }
    b.finish()
}

/// three.js `PlaneGeometry`, facing +z.
pub fn plane_geometry(width: f64, height: f64, width_segments: f64, height_segments: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("PlaneGeometry");
    b.param("width", width);
    b.param("height", height);
    b.param("widthSegments", width_segments);
    b.param("heightSegments", height_segments);
    let width_half = width / 2.0;
    let height_half = height / 2.0;
    let grid_x = width_segments.floor() as i64;
    let grid_y = height_segments.floor() as i64;
    let grid_x1 = grid_x + 1;
    let grid_y1 = grid_y + 1;
    let segment_width = width / grid_x as f64;
    let segment_height = height / grid_y as f64;
    for iy in 0..grid_y1 {
        let y = iy as f64 * segment_height - height_half;
        for ix in 0..grid_x1 {
            let x = ix as f64 * segment_width - width_half;
            b.vertex(x, -y, 0.0);
            b.normal(0.0, 0.0, 1.0);
            b.uv(ix as f64 / grid_x as f64, 1.0 - (iy as f64 / grid_y as f64));
        }
    }
    for iy in 0..grid_y {
        for ix in 0..grid_x {
            let a = (ix + grid_x1 * iy) as f64;
            let bb = (ix + grid_x1 * (iy + 1)) as f64;
            let c = ((ix + 1) + grid_x1 * (iy + 1)) as f64;
            let d = ((ix + 1) + grid_x1 * iy) as f64;
            b.face(a, bb, d);
            b.face(bb, c, d);
        }
    }
    b.finish()
}

/// three.js `SphereGeometry`.
pub fn sphere_geometry(radius: f64, width_segments: f64, height_segments: f64, phi_start: f64, phi_length: f64, theta_start: f64, theta_length: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("SphereGeometry");
    b.param("radius", radius);
    b.param("widthSegments", width_segments);
    b.param("heightSegments", height_segments);
    b.param("phiStart", phi_start);
    b.param("phiLength", phi_length);
    b.param("thetaStart", theta_start);
    b.param("thetaLength", theta_length);
    let ws = (width_segments.floor() as i64).max(3);
    let hs = (height_segments.floor() as i64).max(2);
    let theta_end = (theta_start + theta_length).min(PI);
    let mut normal = Vector3::zero();
    for iy in 0..(hs + 1) {
        let v = iy as f64 / hs as f64;
        let theta = theta_start + v * theta_length;
        let y = radius * theta.cos();
        let ring_radius = (radius * radius - y * y).sqrt();
        let mut u_offset = 0.0;
        if iy == 0 && theta_start == 0.0 {
            u_offset = 0.5 / ws as f64;
        } else if iy == hs && theta_end == PI {
            u_offset = -0.5 / ws as f64;
        }
        for ix in 0..(ws + 1) {
            let u = ix as f64 / ws as f64;
            let phi = phi_start + u * phi_length;
            let x = -ring_radius * phi.cos();
            let z = ring_radius * phi.sin();
            b.vertex(x, y, z);
            normal.set(x, y, z);
            normal.normalize();
            b.normal(normal.x, normal.y, normal.z);
            b.uv(u + u_offset, 1.0 - v);
        }
    }
    let row = ws + 1;
    for iy in 0..hs {
        for ix in 0..ws {
            let a = (iy * row + ix + 1) as f64;
            let bb = (iy * row + ix) as f64;
            let c = ((iy + 1) * row + ix) as f64;
            let d = ((iy + 1) * row + ix + 1) as f64;
            if iy != 0 || theta_start > 0.0 {
                b.face(a, bb, d);
            }
            if iy != hs - 1 || theta_end < PI {
                b.face(bb, c, d);
            }
        }
    }
    b.finish()
}

fn cylinder_into(b: &mut GeometryBuilder, radius_top: f64, radius_bottom: f64, height: f64, radial_segments: f64, height_segments: f64, open_ended: bool, theta_start: f64, theta_length: f64) {
    let rs = radial_segments.floor() as i64;
    let hs = height_segments.floor() as i64;
    let half_height = height / 2.0;
    let mut index: i64 = 0;
    let mut group_start: i64 = 0;
    // torso
    let mut group_count: i64 = 0;
    let slope = (radius_bottom - radius_top) / height;
    let mut normal = Vector3::zero();
    for y in 0..(hs + 1) {
        let v = y as f64 / hs as f64;
        let radius = v * (radius_bottom - radius_top) + radius_top;
        for x in 0..(rs + 1) {
            let u = x as f64 / rs as f64;
            let theta = u * theta_length + theta_start;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();
            b.vertex(radius * sin_theta, -v * height + half_height, radius * cos_theta);
            normal.set(sin_theta, slope, cos_theta);
            normal.normalize();
            b.normal(normal.x, normal.y, normal.z);
            b.uv(u, 1.0 - v);
            index += 1;
        }
    }
    let row = rs + 1;
    for x in 0..rs {
        for y in 0..hs {
            let a = (y * row + x) as f64;
            let bb = ((y + 1) * row + x) as f64;
            let c = ((y + 1) * row + x + 1) as f64;
            let d = (y * row + x + 1) as f64;
            if radius_top > 0.0 || y != 0 {
                b.face(a, bb, d);
                group_count += 3;
            }
            if radius_bottom > 0.0 || y != hs - 1 {
                b.face(bb, c, d);
                group_count += 3;
            }
        }
    }
    b.group(group_start, group_count, 0);
    group_start += group_count;
    if open_ended {
        return;
    }
    let mut cap = 0;
    while cap < 2 {
        let top = cap == 0;
        cap += 1;
        let radius = if top { radius_top } else { radius_bottom };
        if radius <= 0.0 {
            continue;
        }
        let sign = if top { 1.0 } else { -1.0 };
        let center_index_start = index;
        let mut count: i64 = 0;
        for _x in 0..rs {
            b.vertex(0.0, half_height * sign, 0.0);
            b.normal(0.0, sign, 0.0);
            b.uv(0.5, 0.5);
            index += 1;
        }
        let center_index_end = index;
        for x in 0..(rs + 1) {
            let u = x as f64 / rs as f64;
            let theta = u * theta_length + theta_start;
            let cos_theta = theta.cos();
            let sin_theta = theta.sin();
            b.vertex(radius * sin_theta, half_height * sign, radius * cos_theta);
            b.normal(0.0, sign, 0.0);
            b.uv(cos_theta * 0.5 + 0.5, sin_theta * 0.5 * sign + 0.5);
            index += 1;
        }
        for x in 0..rs {
            let c = (center_index_start + x) as f64;
            let i = (center_index_end + x) as f64;
            if top {
                b.face(i, i + 1.0, c);
            } else {
                b.face(i + 1.0, i, c);
            }
            count += 3;
        }
        b.group(group_start, count, if top { 1 } else { 2 });
        group_start += count;
    }
}

/// three.js `CylinderGeometry`: groups 0 (side), 1 (top cap), 2 (bottom cap).
pub fn cylinder_geometry(radius_top: f64, radius_bottom: f64, height: f64, radial_segments: f64, height_segments: f64, open_ended: bool, theta_start: f64, theta_length: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("CylinderGeometry");
    b.param("radiusTop", radius_top);
    b.param("radiusBottom", radius_bottom);
    b.param("height", height);
    b.param("radialSegments", radial_segments);
    b.param("heightSegments", height_segments);
    b.param("openEnded", bool_value(open_ended));
    b.param("thetaStart", theta_start);
    b.param("thetaLength", theta_length);
    cylinder_into(&mut b, radius_top, radius_bottom, height, radial_segments, height_segments, open_ended, theta_start, theta_length);
    b.finish()
}

/// three.js `ConeGeometry`: a cylinder with a zero top radius.
pub fn cone_geometry(radius: f64, height: f64, radial_segments: f64, height_segments: f64, open_ended: bool, theta_start: f64, theta_length: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("ConeGeometry");
    b.param("radius", radius);
    b.param("height", height);
    b.param("radialSegments", radial_segments);
    b.param("heightSegments", height_segments);
    b.param("openEnded", bool_value(open_ended));
    b.param("thetaStart", theta_start);
    b.param("thetaLength", theta_length);
    cylinder_into(&mut b, 0.0, radius, height, radial_segments, height_segments, open_ended, theta_start, theta_length);
    b.finish()
}

/// three.js `CircleGeometry`, facing +z.
pub fn circle_geometry(radius: f64, segments: f64, theta_start: f64, theta_length: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("CircleGeometry");
    b.param("radius", radius);
    b.param("segments", segments);
    b.param("thetaStart", theta_start);
    b.param("thetaLength", theta_length);
    let n = segments.max(3.0);
    b.vertex(0.0, 0.0, 0.0);
    b.normal(0.0, 0.0, 1.0);
    b.uv(0.5, 0.5);
    let mut s = 0.0;
    while s <= n {
        let segment = theta_start + s / n * theta_length;
        let x = radius * segment.cos();
        let y = radius * segment.sin();
        b.vertex(x, y, 0.0);
        b.normal(0.0, 0.0, 1.0);
        let len = b.vertices.len();
        let vx = b.vertices[len - 3];
        let vy = b.vertices[len - 2];
        b.uv((vx / radius + 1.0) / 2.0, (vy / radius + 1.0) / 2.0);
        s += 1.0;
    }
    let mut i = 1.0;
    while i <= n {
        b.face(i, i + 1.0, 0.0);
        i += 1.0;
    }
    b.finish()
}

/// three.js `RingGeometry`, facing +z.
pub fn ring_geometry(inner_radius: f64, outer_radius: f64, theta_segments: f64, phi_segments: f64, theta_start: f64, theta_length: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("RingGeometry");
    b.param("innerRadius", inner_radius);
    b.param("outerRadius", outer_radius);
    b.param("thetaSegments", theta_segments);
    b.param("phiSegments", phi_segments);
    b.param("thetaStart", theta_start);
    b.param("thetaLength", theta_length);
    let ts = theta_segments.max(3.0);
    let ps = phi_segments.max(1.0);
    let mut radius = inner_radius;
    let radius_step = (outer_radius - inner_radius) / ps;
    let mut j = 0.0;
    while j <= ps {
        let mut i = 0.0;
        while i <= ts {
            let segment = theta_start + i / ts * theta_length;
            let x = radius * segment.cos();
            let y = radius * segment.sin();
            b.vertex(x, y, 0.0);
            b.normal(0.0, 0.0, 1.0);
            b.uv((x / outer_radius + 1.0) / 2.0, (y / outer_radius + 1.0) / 2.0);
            i += 1.0;
        }
        radius += radius_step;
        j += 1.0;
    }
    let mut j = 0.0;
    while j < ps {
        let level = j * (ts + 1.0);
        let mut i = 0.0;
        while i < ts {
            let segment = i + level;
            b.face(segment, segment + ts + 1.0, segment + 1.0);
            b.face(segment + ts + 1.0, segment + ts + 2.0, segment + 1.0);
            i += 1.0;
        }
        j += 1.0;
    }
    b.finish()
}

/// three.js `TorusGeometry` around the z axis.
pub fn torus_geometry(radius: f64, tube: f64, radial_segments: f64, tubular_segments: f64, arc: f64, theta_start: f64, theta_length: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("TorusGeometry");
    b.param("radius", radius);
    b.param("tube", tube);
    b.param("radialSegments", radial_segments);
    b.param("tubularSegments", tubular_segments);
    b.param("arc", arc);
    b.param("thetaStart", theta_start);
    b.param("thetaLength", theta_length);
    let rs = radial_segments.floor() as i64;
    let ts = tubular_segments.floor() as i64;
    let mut vertex = Vector3::zero();
    let mut center = Vector3::zero();
    let mut normal = Vector3::zero();
    for j in 0..(rs + 1) {
        let v = theta_start + (j as f64 / rs as f64) * theta_length;
        for i in 0..(ts + 1) {
            let u = i as f64 / ts as f64 * arc;
            vertex.set((radius + tube * v.cos()) * u.cos(), (radius + tube * v.cos()) * u.sin(), tube * v.sin());
            b.vertex(vertex.x, vertex.y, vertex.z);
            center.set(radius * u.cos(), radius * u.sin(), 0.0);
            normal.sub_vectors(&vertex, &center);
            normal.normalize();
            b.normal(normal.x, normal.y, normal.z);
            b.uv(i as f64 / ts as f64, j as f64 / rs as f64);
        }
    }
    for j in 1..(rs + 1) {
        for i in 1..(ts + 1) {
            let a = ((ts + 1) * j + i - 1) as f64;
            let bb = ((ts + 1) * (j - 1) + i - 1) as f64;
            let c = ((ts + 1) * (j - 1) + i) as f64;
            let d = ((ts + 1) * j + i) as f64;
            b.face(a, bb, d);
            b.face(bb, c, d);
        }
    }
    b.finish()
}

fn torus_knot_point(u: f64, p: f64, q: f64, radius: f64) -> Vector3 {
    let cu = u.cos();
    let su = u.sin();
    let qu_over_p = q / p * u;
    let cs = qu_over_p.cos();
    Vector3::new(radius * (2.0 + cs) * 0.5 * cu, radius * (2.0 + cs) * su * 0.5, radius * qu_over_p.sin() * 0.5)
}

/// three.js `TorusKnotGeometry`.
pub fn torus_knot_geometry(radius: f64, tube: f64, tubular_segments: f64, radial_segments: f64, p: f64, q: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("TorusKnotGeometry");
    b.param("radius", radius);
    b.param("tube", tube);
    b.param("tubularSegments", tubular_segments);
    b.param("radialSegments", radial_segments);
    b.param("p", p);
    b.param("q", q);
    let ts = tubular_segments.floor() as i64;
    let rs = radial_segments.floor() as i64;
    let mut vertex = Vector3::zero();
    let mut normal = Vector3::zero();
    let mut t = Vector3::zero();
    let mut n = Vector3::zero();
    let mut bn = Vector3::zero();
    for i in 0..(ts + 1) {
        let u = i as f64 / ts as f64 * p * PI * 2.0;
        let p1 = torus_knot_point(u, p, q, radius);
        let p2 = torus_knot_point(u + 0.01, p, q, radius);
        t.sub_vectors(&p2, &p1);
        n.add_vectors(&p2, &p1);
        bn.cross_vectors(&t, &n);
        n.cross_vectors(&bn, &t);
        bn.normalize();
        n.normalize();
        for j in 0..(rs + 1) {
            let v = j as f64 / rs as f64 * PI * 2.0;
            let cx = -tube * v.cos();
            let cy = tube * v.sin();
            vertex.set(p1.x + (cx * n.x + cy * bn.x), p1.y + (cx * n.y + cy * bn.y), p1.z + (cx * n.z + cy * bn.z));
            b.vertex(vertex.x, vertex.y, vertex.z);
            normal.sub_vectors(&vertex, &p1);
            normal.normalize();
            b.normal(normal.x, normal.y, normal.z);
            b.uv(i as f64 / ts as f64, j as f64 / rs as f64);
        }
    }
    for j in 1..(ts + 1) {
        for i in 1..(rs + 1) {
            let a = ((rs + 1) * (j - 1) + (i - 1)) as f64;
            let bb = ((rs + 1) * j + (i - 1)) as f64;
            let c = ((rs + 1) * j + i) as f64;
            let d = ((rs + 1) * (j - 1) + i) as f64;
            b.face(a, bb, d);
            b.face(bb, c, d);
        }
    }
    b.finish()
}

/// three.js `CapsuleGeometry`: a cylinder of `height` with half-sphere caps.
pub fn capsule_geometry(radius: f64, height: f64, cap_segments: f64, radial_segments: f64, height_segments: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("CapsuleGeometry");
    b.param("radius", radius);
    b.param("height", height);
    b.param("capSegments", cap_segments);
    b.param("radialSegments", radial_segments);
    b.param("heightSegments", height_segments);
    let height = height.max(0.0);
    let cs = (cap_segments.floor() as i64).max(1);
    let rs = (radial_segments.floor() as i64).max(3);
    let hs = (height_segments.floor() as i64).max(1);
    let half_height = height / 2.0;
    let cap_arc_length = (PI / 2.0) * radius;
    let total_arc_length = 2.0 * cap_arc_length + height;
    let num_vertical = cs * 2 + hs;
    let per_row = rs + 1;
    let mut normal = Vector3::zero();
    for iy in 0..(num_vertical + 1) {
        let profile_y: f64;
        let profile_radius: f64;
        let normal_y: f64;
        let arc_length: f64;
        if iy <= cs {
            let progress = iy as f64 / cs as f64;
            let angle = (progress * PI) / 2.0;
            profile_y = -half_height - radius * angle.cos();
            profile_radius = radius * angle.sin();
            normal_y = -radius * angle.cos();
            arc_length = progress * cap_arc_length;
        } else if iy <= cs + hs {
            let progress = (iy - cs) as f64 / hs as f64;
            profile_y = -half_height + progress * height;
            profile_radius = radius;
            normal_y = 0.0;
            arc_length = cap_arc_length + progress * height;
        } else {
            let progress = (iy - cs - hs) as f64 / cs as f64;
            let angle = (progress * PI) / 2.0;
            profile_y = half_height + radius * angle.sin();
            profile_radius = radius * angle.cos();
            normal_y = radius * angle.sin();
            arc_length = cap_arc_length + height + progress * cap_arc_length;
        }
        let v = (arc_length / total_arc_length).min(1.0).max(0.0);
        let mut u_offset = 0.0;
        if iy == 0 {
            u_offset = 0.5 / rs as f64;
        } else if iy == num_vertical {
            u_offset = -0.5 / rs as f64;
        }
        for ix in 0..(rs + 1) {
            let u = ix as f64 / rs as f64;
            let theta = u * PI * 2.0;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();
            b.vertex(-profile_radius * cos_theta, profile_y, profile_radius * sin_theta);
            normal.set(-profile_radius * cos_theta, normal_y, profile_radius * sin_theta);
            normal.normalize();
            b.normal(normal.x, normal.y, normal.z);
            b.uv(u + u_offset, v);
        }
        if iy > 0 {
            let prev = (iy - 1) * per_row;
            for ix in 0..rs {
                let i1 = (prev + ix) as f64;
                let i2 = (prev + ix + 1) as f64;
                let i3 = (iy * per_row + ix) as f64;
                let i4 = (iy * per_row + ix + 1) as f64;
                b.face(i1, i2, i3);
                b.face(i2, i4, i3);
            }
        }
    }
    b.finish()
}

/// three.js's default lathe profile.
pub fn lathe_default_points() -> Vec<Vector2> {
    vec![Vector2::new(0.0, -0.5), Vector2::new(0.5, 0.0), Vector2::new(0.0, 0.5)]
}

/// three.js `LatheGeometry`: `points` (x = radius, y = height) turned
/// around the y axis.
pub fn lathe_geometry(points: &Vec<Vector2>, segments: f64, phi_start: f64, phi_length: f64) -> BufferGeometry {
    let mut b = GeometryBuilder::new("LatheGeometry");
    b.param("segments", segments);
    b.param("phiStart", phi_start);
    b.param("phiLength", phi_length);
    let segs = segments.floor() as i64;
    let phi_length = phi_length.max(0.0).min(PI * 2.0);
    let inverse_segments = 1.0 / segs as f64;
    let count = points.len();
    let last = count - 1;
    let mut init_normals: Vec<f64> = Vec::new();
    let mut normal = Vector3::zero();
    let mut cur_normal = Vector3::zero();
    let mut prev_normal = Vector3::zero();
    for j in 0..count {
        if j == 0 {
            let dx = points[j + 1].x - points[j].x;
            let dy = points[j + 1].y - points[j].y;
            normal.set(dy * 1.0, -dx, dy * 0.0);
            prev_normal.copy(&normal);
            normal.normalize();
            init_normals.push(normal.x);
            init_normals.push(normal.y);
            init_normals.push(normal.z);
        } else if j == last {
            init_normals.push(prev_normal.x);
            init_normals.push(prev_normal.y);
            init_normals.push(prev_normal.z);
        } else {
            let dx = points[j + 1].x - points[j].x;
            let dy = points[j + 1].y - points[j].y;
            normal.set(dy * 1.0, -dx, dy * 0.0);
            cur_normal.copy(&normal);
            normal.add(&prev_normal);
            normal.normalize();
            init_normals.push(normal.x);
            init_normals.push(normal.y);
            init_normals.push(normal.z);
            prev_normal.copy(&cur_normal);
        }
    }
    for i in 0..(segs + 1) {
        let phi = phi_start + i as f64 * inverse_segments * phi_length;
        let sin = phi.sin();
        let cos = phi.cos();
        for j in 0..count {
            b.vertex(points[j].x * sin, points[j].y, points[j].x * cos);
            b.uv(i as f64 / segs as f64, j as f64 / last as f64);
            b.normal(init_normals[3 * j] * sin, init_normals[3 * j + 1], init_normals[3 * j] * cos);
        }
    }
    let n = count as i64;
    for i in 0..segs {
        for j in 0..(n - 1) {
            let base = j + i * n;
            let a = base as f64;
            let bb = (base + n) as f64;
            let c = (base + n + 1) as f64;
            let d = (base + 1) as f64;
            b.face(a, bb, d);
            b.face(c, d, bb);
        }
    }
    b.finish()
}
