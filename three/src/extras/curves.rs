//! Curves (three.js `Curve` and its kinds, `CurvePath`, `Path`, `Shape`).
//!
//! One `Curve` struct holds what every kind shares (the arc-length cache)
//! and a `CurveKind` with the kind's own data. Points are `Vector3`; a 2D
//! curve (`is_2d`) keeps z at 0, and the JavaScript facade hands those out
//! as `Vector2`.

use ranger::prelude::*;

use crate::maths::math_utils::clamp;
use crate::maths::matrix4::Matrix4;
use crate::maths::vector3::Vector3;

/// Number.EPSILON.
pub const EPSILON: f64 = 2.220446049250313e-16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CatmullRomType {
    Centripetal,
    Chordal,
    CatmullRom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineData {
    pub v1: Vector3,
    pub v2: Vector3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezierData {
    pub v0: Vector3,
    pub v1: Vector3,
    pub v2: Vector3,
    pub v3: Vector3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EllipseData {
    pub x: f64,
    pub y: f64,
    pub x_radius: f64,
    pub y_radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub clockwise: bool,
    pub rotation: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CatmullRomData {
    pub points: Vec<Vector3>,
    pub closed: bool,
    pub curve_type: CatmullRomType,
    pub tension: f64,
}

/// A `CurvePath`: curves joined end to end. `Path` and `Shape` add a pen
/// (`current_point`) and holes.
#[derive(Clone, Debug, PartialEq)]
pub struct PathData {
    pub curves: Vec<Curve>,
    pub auto_close: bool,
    pub current_point: Vector3,
    pub cache_lengths: Vec<f64>,
    pub holes: Vec<Curve>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CurveKind {
    Line(LineData),
    QuadraticBezier(BezierData),
    CubicBezier(BezierData),
    Ellipse(EllipseData),
    Spline(Vec<Vector3>),
    CatmullRom(CatmullRomData),
    Path(PathData),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub kind: CurveKind,
    pub is_2d: bool,
    pub arc_length_divisions: i64,
    pub needs_update: bool,
    pub cache_arc_lengths: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrenetFrames {
    pub tangents: Vec<Vector3>,
    pub normals: Vec<Vector3>,
    pub binormals: Vec<Vector3>,
}

fn catmull_rom(t: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let v0 = (p2 - p0) * 0.5;
    let v1 = (p3 - p1) * 0.5;
    let t2 = t * t;
    let t3 = t * t2;
    (2.0 * p1 - 2.0 * p2 + v0 + v1) * t3 + (-3.0 * p1 + 3.0 * p2 - 2.0 * v0 - v1) * t2 + v0 * t + p1
}

fn quadratic_bezier(t: f64, p0: f64, p1: f64, p2: f64) -> f64 {
    let k = 1.0 - t;
    k * k * p0 + 2.0 * (1.0 - t) * t * p1 + t * t * p2
}

fn cubic_bezier(t: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let k = 1.0 - t;
    k * k * k * p0 + 3.0 * k * k * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t * t * t * p3
}

/// The cubic c0 + c1 t + c2 t² + c3 t³ through x0, x1 with slopes t0, t1.
fn cubic_poly(x0: f64, x1: f64, t0: f64, t1: f64) -> Vec<f64> {
    vec![x0, t0, -3.0 * x0 + 3.0 * x1 - 2.0 * t0 - t1, 2.0 * x0 - 2.0 * x1 + t0 + t1]
}

fn nonuniform_catmull_rom(x0: f64, x1: f64, x2: f64, x3: f64, dt0: f64, dt1: f64, dt2: f64) -> Vec<f64> {
    let mut t1 = (x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1;
    let mut t2 = (x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2;
    t1 *= dt1;
    t2 *= dt1;
    cubic_poly(x1, x2, t1, t2)
}

fn poly_at(c: &Vec<f64>, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    c[0] + c[1] * t + c[2] * t2 + c[3] * t3
}

fn same_xy(a: &Vector3, b: &Vector3) -> bool {
    a.x == b.x && a.y == b.y
}

impl Curve {
    fn with_kind(kind: CurveKind, is_2d: bool) -> Curve {
        Curve { kind, is_2d, arc_length_divisions: 200, needs_update: false, cache_arc_lengths: Vec::new() }
    }

    pub fn line(v1: Vector3, v2: Vector3, is_2d: bool) -> Curve {
        Curve::with_kind(CurveKind::Line(LineData { v1, v2 }), is_2d)
    }

    pub fn quadratic_bezier(v0: Vector3, v1: Vector3, v2: Vector3, is_2d: bool) -> Curve {
        Curve::with_kind(CurveKind::QuadraticBezier(BezierData { v0, v1, v2, v3: Vector3::zero() }), is_2d)
    }

    pub fn cubic_bezier(v0: Vector3, v1: Vector3, v2: Vector3, v3: Vector3, is_2d: bool) -> Curve {
        Curve::with_kind(CurveKind::CubicBezier(BezierData { v0, v1, v2, v3 }), is_2d)
    }

    pub fn ellipse(x: f64, y: f64, x_radius: f64, y_radius: f64, start_angle: f64, end_angle: f64, clockwise: bool, rotation: f64) -> Curve {
        let e = EllipseData { x, y, x_radius, y_radius, start_angle, end_angle, clockwise, rotation };
        Curve::with_kind(CurveKind::Ellipse(e), true)
    }

    pub fn spline(points: Vec<Vector3>) -> Curve {
        Curve::with_kind(CurveKind::Spline(points), true)
    }

    pub fn catmull_rom(points: Vec<Vector3>, closed: bool, curve_type: CatmullRomType, tension: f64) -> Curve {
        Curve::with_kind(CurveKind::CatmullRom(CatmullRomData { points, closed, curve_type, tension }), false)
    }

    /// An empty `CurvePath` (`is_2d` false) or `Path`/`Shape` (true).
    pub fn path(is_2d: bool) -> Curve {
        let p = PathData {
            curves: Vec::new(),
            auto_close: false,
            current_point: Vector3::zero(),
            cache_lengths: Vec::new(),
            holes: Vec::new(),
        };
        Curve::with_kind(CurveKind::Path(p), is_2d)
    }

    /// A `Path` through `points`: a move to the first, lines to the rest.
    pub fn path_from_points(points: &Vec<Vector3>) -> Curve {
        let mut c = Curve::path(true);
        c.set_from_points(points);
        c
    }

    pub fn is_line(&self) -> bool {
        match &self.kind {
            CurveKind::Line(_) => true,
            _ => false,
        }
    }

    pub fn is_ellipse(&self) -> bool {
        match &self.kind {
            CurveKind::Ellipse(_) => true,
            _ => false,
        }
    }

    /// The point at `t` in [0, 1] along the curve's parameter.
    pub fn get_point(&mut self, t: f64) -> Vector3 {
        if let CurveKind::Path(_) = &self.kind {
            return self.path_point(t);
        }
        match &self.kind {
            CurveKind::Line(l) => {
                if t == 1.0 {
                    l.v2
                } else {
                    let mut p = l.v2;
                    p.sub(&l.v1);
                    p.multiply_scalar(t);
                    p.add(&l.v1);
                    p
                }
            }
            CurveKind::QuadraticBezier(b) => Vector3::new(
                quadratic_bezier(t, b.v0.x, b.v1.x, b.v2.x),
                quadratic_bezier(t, b.v0.y, b.v1.y, b.v2.y),
                if self.is_2d { 0.0 } else { quadratic_bezier(t, b.v0.z, b.v1.z, b.v2.z) },
            ),
            CurveKind::CubicBezier(b) => Vector3::new(
                cubic_bezier(t, b.v0.x, b.v1.x, b.v2.x, b.v3.x),
                cubic_bezier(t, b.v0.y, b.v1.y, b.v2.y, b.v3.y),
                if self.is_2d { 0.0 } else { cubic_bezier(t, b.v0.z, b.v1.z, b.v2.z, b.v3.z) },
            ),
            CurveKind::Ellipse(e) => ellipse_point(e, t),
            CurveKind::Spline(points) => spline_point(points, t),
            CurveKind::CatmullRom(c) => catmull_rom_point(c, t),
            CurveKind::Path(_) => Vector3::zero(),
        }
    }

    /// The point at `u` in [0, 1] along the curve's length.
    pub fn get_point_at(&mut self, u: f64) -> Vector3 {
        if self.is_line() {
            return self.get_point(u);
        }
        let t = self.get_u_to_t_mapping(u, 0.0);
        self.get_point(t)
    }

    pub fn get_points(&mut self, divisions: i64) -> Vec<Vector3> {
        if let CurveKind::Path(_) = &self.kind {
            return self.path_points(divisions);
        }
        let mut points: Vec<Vector3> = Vec::new();
        for d in 0..(divisions + 1) {
            points.push(self.get_point(d as f64 / divisions as f64));
        }
        points
    }

    pub fn get_spaced_points(&mut self, divisions: i64) -> Vec<Vector3> {
        let mut points: Vec<Vector3> = Vec::new();
        if let CurveKind::Path(_) = &self.kind {
            for i in 0..(divisions + 1) {
                points.push(self.get_point(i as f64 / divisions as f64));
            }
            let auto_close = match &self.kind {
                CurveKind::Path(p) => p.auto_close,
                _ => false,
            };
            if auto_close {
                let first = points[0];
                points.push(first);
            }
            return points;
        }
        for d in 0..(divisions + 1) {
            points.push(self.get_point_at(d as f64 / divisions as f64));
        }
        points
    }

    pub fn get_length(&mut self) -> f64 {
        if let CurveKind::Path(_) = &self.kind {
            let lens = self.get_curve_lengths();
            if lens.is_empty() {
                return 0.0;
            }
            return lens[lens.len() - 1];
        }
        let lengths = self.get_lengths(self.arc_length_divisions);
        lengths[lengths.len() - 1]
    }

    /// The summed lengths at `divisions + 1` evenly spaced parameters; kept
    /// until `needs_update` is set (as three.js does).
    pub fn get_lengths(&mut self, divisions: i64) -> Vec<f64> {
        if self.cache_arc_lengths.len() as i64 == divisions + 1 && !self.needs_update {
            return self.cache_arc_lengths.clone();
        }
        self.needs_update = false;
        let mut cache: Vec<f64> = Vec::new();
        let mut last = self.get_point(0.0);
        let mut sum = 0.0;
        cache.push(0.0);
        for p in 1..(divisions + 1) {
            let current = self.get_point(p as f64 / divisions as f64);
            sum += current.distance_to(&last);
            cache.push(sum);
            last = current;
        }
        self.cache_arc_lengths = cache.clone();
        cache
    }

    pub fn update_arc_lengths(&mut self) {
        self.needs_update = true;
        if let CurveKind::Path(p) = &mut self.kind {
            p.cache_lengths = Vec::new();
            self.get_curve_lengths();
            return;
        }
        self.get_lengths(self.arc_length_divisions);
    }

    /// The parameter `t` at the fraction `u` of the length, or at the length
    /// `distance` when that is not 0.
    pub fn get_u_to_t_mapping(&mut self, u: f64, distance: f64) -> f64 {
        let arc_lengths = self.get_lengths(self.arc_length_divisions);
        let il = arc_lengths.len() as i64;
        let target = if distance != 0.0 { distance } else { u * arc_lengths[(il - 1) as usize] };
        let mut low: i64 = 0;
        let mut high: i64 = il - 1;
        while low <= high {
            let i = low + (high - low) / 2;
            let comparison = arc_lengths[i as usize] - target;
            if comparison < 0.0 {
                low = i + 1;
            } else if comparison > 0.0 {
                high = i - 1;
            } else {
                high = i;
                break;
            }
        }
        let i = high;
        if arc_lengths[i as usize] == target {
            return i as f64 / (il - 1) as f64;
        }
        let before = arc_lengths[i as usize];
        let after = arc_lengths[(i + 1) as usize];
        let fraction = (target - before) / (after - before);
        (i as f64 + fraction) / (il - 1) as f64
    }

    pub fn get_tangent(&mut self, t: f64) -> Vector3 {
        if let CurveKind::Line(l) = &self.kind {
            let mut v = l.v2;
            v.sub(&l.v1);
            v.normalize();
            return v;
        }
        let delta = 0.0001;
        let mut t1 = t - delta;
        let mut t2 = t + delta;
        if t1 < 0.0 {
            t1 = 0.0;
        }
        if t2 > 1.0 {
            t2 = 1.0;
        }
        let pt1 = self.get_point(t1);
        let mut tangent = self.get_point(t2);
        tangent.sub(&pt1);
        tangent.normalize();
        tangent
    }

    pub fn get_tangent_at(&mut self, u: f64) -> Vector3 {
        if self.is_line() {
            return self.get_tangent(u);
        }
        let t = self.get_u_to_t_mapping(u, 0.0);
        self.get_tangent(t)
    }

    /// Tangent, normal and binormal at `segments + 1` evenly spaced lengths,
    /// with normals carried along by parallel transport.
    pub fn compute_frenet_frames(&mut self, segments: i64, closed: bool) -> FrenetFrames {
        let mut tangents: Vec<Vector3> = Vec::new();
        for i in 0..(segments + 1) {
            let u = i as f64 / segments as f64;
            tangents.push(self.get_tangent_at(u));
        }
        frames_from_tangents(tangents, segments, closed)
    }

    // ---- CurvePath ----------------------------------------------------

    pub fn add(&mut self, curve: Curve) {
        if let CurveKind::Path(p) = &mut self.kind {
            p.curves.push(curve);
        }
    }

    /// Adds a line back to the start when the path does not end there.
    pub fn close_path(&mut self) {
        let count = match &self.kind {
            CurveKind::Path(p) => p.curves.len(),
            _ => 0,
        };
        if count == 0 {
            return;
        }
        let mut start = Vector3::zero();
        let mut end = Vector3::zero();
        if let CurveKind::Path(p) = &mut self.kind {
            start = p.curves[0].get_point(0.0);
            end = p.curves[count - 1].get_point(1.0);
        }
        if !start.equals(&end) {
            let is_2d = self.is_2d;
            self.add(Curve::line(end, start, is_2d));
        }
    }

    /// The summed lengths of the curves; kept while their count is the same.
    pub fn get_curve_lengths(&mut self) -> Vec<f64> {
        if let CurveKind::Path(p) = &mut self.kind {
            if !p.cache_lengths.is_empty() && p.cache_lengths.len() == p.curves.len() {
                return p.cache_lengths.clone();
            }
            let mut lengths: Vec<f64> = Vec::new();
            let mut sums = 0.0;
            for i in 0..p.curves.len() {
                sums += p.curves[i].get_length();
                lengths.push(sums);
            }
            p.cache_lengths = lengths.clone();
            return lengths;
        }
        Vec::new()
    }

    fn path_point(&mut self, t: f64) -> Vector3 {
        let d = t * self.get_length();
        let curve_lengths = self.get_curve_lengths();
        if let CurveKind::Path(p) = &mut self.kind {
            for i in 0..curve_lengths.len() {
                if curve_lengths[i] >= d {
                    let diff = curve_lengths[i] - d;
                    let segment_length = p.curves[i].get_length();
                    let u = if segment_length == 0.0 { 0.0 } else { 1.0 - diff / segment_length };
                    return p.curves[i].get_point_at(u);
                }
            }
            let n = p.curves.len();
            if n > 0 {
                return p.curves[n - 1].get_point(1.0);
            }
        }
        Vector3::zero()
    }

    fn path_points(&mut self, divisions: i64) -> Vec<Vector3> {
        let mut points: Vec<Vector3> = Vec::new();
        if let CurveKind::Path(p) = &mut self.kind {
            let mut has_last = false;
            let mut last = Vector3::zero();
            for i in 0..p.curves.len() {
                let resolution = match &p.curves[i].kind {
                    CurveKind::Ellipse(_) => divisions * 2,
                    CurveKind::Line(_) => 1,
                    CurveKind::Spline(sp) => divisions * sp.len() as i64,
                    _ => divisions,
                };
                let pts = p.curves[i].get_points(resolution);
                for point in pts.iter() {
                    if has_last && last.equals(point) {
                        continue;
                    }
                    points.push(*point);
                    last = *point;
                    has_last = true;
                }
            }
            let n = points.len();
            if p.auto_close && n > 1 && !points[n - 1].equals(&points[0]) {
                let first = points[0];
                points.push(first);
            }
        }
        points
    }

    // ---- Path -------------------------------------------------------------

    fn pen(&self) -> Vector3 {
        match &self.kind {
            CurveKind::Path(p) => p.current_point,
            _ => Vector3::zero(),
        }
    }

    fn set_pen(&mut self, x: f64, y: f64) {
        if let CurveKind::Path(p) = &mut self.kind {
            p.current_point.set(x, y, 0.0);
        }
    }

    pub fn set_from_points(&mut self, points: &Vec<Vector3>) {
        if points.is_empty() {
            return;
        }
        self.move_to(points[0].x, points[0].y);
        for i in 1..points.len() {
            self.line_to(points[i].x, points[i].y);
        }
    }

    pub fn move_to(&mut self, x: f64, y: f64) {
        self.set_pen(x, y);
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        let from = self.pen();
        self.add(Curve::line(from, Vector3::new(x, y, 0.0), true));
        self.set_pen(x, y);
    }

    pub fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        let from = self.pen();
        self.add(Curve::quadratic_bezier(from, Vector3::new(cpx, cpy, 0.0), Vector3::new(x, y, 0.0), true));
        self.set_pen(x, y);
    }

    pub fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        let from = self.pen();
        let c = Curve::cubic_bezier(from, Vector3::new(cp1x, cp1y, 0.0), Vector3::new(cp2x, cp2y, 0.0), Vector3::new(x, y, 0.0), true);
        self.add(c);
        self.set_pen(x, y);
    }

    pub fn spline_thru(&mut self, pts: &Vec<Vector3>) {
        if pts.is_empty() {
            return;
        }
        let mut npts: Vec<Vector3> = vec![self.pen()];
        for p in pts.iter() {
            npts.push(*p);
        }
        let last = pts[pts.len() - 1];
        self.add(Curve::spline(npts));
        self.set_pen(last.x, last.y);
    }

    pub fn arc(&mut self, x: f64, y: f64, radius: f64, start_angle: f64, end_angle: f64, clockwise: bool) {
        let pen = self.pen();
        self.absellipse(x + pen.x, y + pen.y, radius, radius, start_angle, end_angle, clockwise, 0.0);
    }

    pub fn absarc(&mut self, x: f64, y: f64, radius: f64, start_angle: f64, end_angle: f64, clockwise: bool) {
        self.absellipse(x, y, radius, radius, start_angle, end_angle, clockwise, 0.0);
    }

    pub fn ellipse_to(&mut self, x: f64, y: f64, x_radius: f64, y_radius: f64, start_angle: f64, end_angle: f64, clockwise: bool, rotation: f64) {
        let pen = self.pen();
        self.absellipse(x + pen.x, y + pen.y, x_radius, y_radius, start_angle, end_angle, clockwise, rotation);
    }

    pub fn absellipse(&mut self, x: f64, y: f64, x_radius: f64, y_radius: f64, start_angle: f64, end_angle: f64, clockwise: bool, rotation: f64) {
        let mut curve = Curve::ellipse(x, y, x_radius, y_radius, start_angle, end_angle, clockwise, rotation);
        let count = match &self.kind {
            CurveKind::Path(p) => p.curves.len(),
            _ => 0,
        };
        if count > 0 {
            let first = curve.get_point(0.0);
            if !same_xy(&first, &self.pen()) {
                self.line_to(first.x, first.y);
            }
        }
        let last = curve.get_point(1.0);
        self.add(curve);
        self.set_pen(last.x, last.y);
    }

    // ---- Shape ------------------------------------------------------------

    pub fn add_hole(&mut self, hole: Curve) {
        if let CurveKind::Path(p) = &mut self.kind {
            p.holes.push(hole);
        }
    }

    pub fn get_points_holes(&mut self, divisions: i64) -> Vec<Vec<Vector3>> {
        let mut out: Vec<Vec<Vector3>> = Vec::new();
        if let CurveKind::Path(p) = &mut self.kind {
            for i in 0..p.holes.len() {
                out.push(p.holes[i].get_points(divisions));
            }
        }
        out
    }
}

/// Normals and binormals carried along `tangents` by parallel transport
/// (the second half of three.js's `computeFrenetFrames`).
pub fn frames_from_tangents(tangents: Vec<Vector3>, segments: i64, closed: bool) -> FrenetFrames {
    let mut normals: Vec<Vector3> = Vec::new();
    let mut binormals: Vec<Vector3> = Vec::new();
    let mut vec = Vector3::zero();
    let mut mat = Matrix4::identity();
    let mut normal = Vector3::zero();
    let mut min = 1.7976931348623157e308;
    let tx = tangents[0].x.abs();
    let ty = tangents[0].y.abs();
    let tz = tangents[0].z.abs();
    if tx <= min {
        min = tx;
        normal.set(1.0, 0.0, 0.0);
    }
    if ty <= min {
        min = ty;
        normal.set(0.0, 1.0, 0.0);
    }
    if tz <= min {
        normal.set(0.0, 0.0, 1.0);
    }
    vec.cross_vectors(&tangents[0], &normal);
    vec.normalize();
    let mut n0 = Vector3::zero();
    n0.cross_vectors(&tangents[0], &vec);
    let mut b0 = Vector3::zero();
    b0.cross_vectors(&tangents[0], &n0);
    normals.push(n0);
    binormals.push(b0);
    for i in 1..(segments + 1) {
        let iu = i as usize;
        let mut n = normals[iu - 1];
        vec.cross_vectors(&tangents[iu - 1], &tangents[iu]);
        if vec.length() > EPSILON {
            vec.normalize();
            let theta = clamp(tangents[iu - 1].dot(&tangents[iu]), -1.0, 1.0).acos();
            mat.make_rotation_axis(&vec, theta);
            n.apply_matrix4(&mat);
        }
        let mut b = Vector3::zero();
        b.cross_vectors(&tangents[iu], &n);
        normals.push(n);
        binormals.push(b);
    }
    if closed {
        let last = segments as usize;
        let mut theta = clamp(normals[0].dot(&normals[last]), -1.0, 1.0).acos();
        theta /= segments as f64;
        vec.cross_vectors(&normals[0], &normals[last]);
        if tangents[0].dot(&vec) > 0.0 {
            theta = -theta;
        }
        for i in 1..(segments + 1) {
            let iu = i as usize;
            mat.make_rotation_axis(&tangents[iu], theta * i as f64);
            let mut n = normals[iu];
            n.apply_matrix4(&mat);
            let mut b = Vector3::zero();
            b.cross_vectors(&tangents[iu], &n);
            normals[iu] = n;
            binormals[iu] = b;
        }
    }
    FrenetFrames { tangents, normals, binormals }
}

fn ellipse_point(e: &EllipseData, t: f64) -> Vector3 {
    let two_pi = std::f64::consts::PI * 2.0;
    let mut delta = e.end_angle - e.start_angle;
    let same_points = delta.abs() < EPSILON;
    while delta < 0.0 {
        delta += two_pi;
    }
    while delta > two_pi {
        delta -= two_pi;
    }
    if delta < EPSILON {
        delta = if same_points { 0.0 } else { two_pi };
    }
    if e.clockwise && !same_points {
        delta = if delta == two_pi { -two_pi } else { delta - two_pi };
    }
    let angle = e.start_angle + t * delta;
    let mut x = e.x + e.x_radius * angle.cos();
    let mut y = e.y + e.y_radius * angle.sin();
    if e.rotation != 0.0 {
        let cos = e.rotation.cos();
        let sin = e.rotation.sin();
        let tx = x - e.x;
        let ty = y - e.y;
        x = tx * cos - ty * sin + e.x;
        y = tx * sin + ty * cos + e.y;
    }
    Vector3::new(x, y, 0.0)
}

fn spline_point(points: &Vec<Vector3>, t: f64) -> Vector3 {
    let n = points.len() as i64;
    let p = (n - 1) as f64 * t;
    let int_point = p.floor() as i64;
    let weight = p - int_point as f64;
    let p0 = points[(if int_point == 0 { int_point } else { int_point - 1 }) as usize];
    let p1 = points[int_point as usize];
    let p2 = points[(if int_point > n - 2 { n - 1 } else { int_point + 1 }) as usize];
    let p3 = points[(if int_point > n - 3 { n - 1 } else { int_point + 2 }) as usize];
    Vector3::new(
        catmull_rom(weight, p0.x, p1.x, p2.x, p3.x),
        catmull_rom(weight, p0.y, p1.y, p2.y, p3.y),
        0.0,
    )
}

fn catmull_rom_point(c: &CatmullRomData, t: f64) -> Vector3 {
    let points = &c.points;
    let l = points.len() as i64;
    let p = (l - if c.closed { 0 } else { 1 }) as f64 * t;
    let mut int_point = p.floor() as i64;
    let mut weight = p - int_point as f64;
    if c.closed {
        if int_point <= 0 {
            int_point += ((int_point.abs() / l) + 1) * l;
        }
    } else if weight == 0.0 && int_point == l - 1 {
        int_point = l - 2;
        weight = 1.0;
    }
    let p0 = if c.closed || int_point > 0 {
        points[((int_point - 1) % l) as usize]
    } else {
        let mut v = Vector3::zero();
        v.sub_vectors(&points[0], &points[1]);
        v.add(&points[0]);
        v
    };
    let p1 = points[(int_point % l) as usize];
    let p2 = points[((int_point + 1) % l) as usize];
    let p3 = if c.closed || int_point + 2 < l {
        points[((int_point + 2) % l) as usize]
    } else {
        let mut v = Vector3::zero();
        v.sub_vectors(&points[(l - 1) as usize], &points[(l - 2) as usize]);
        v.add(&points[(l - 1) as usize]);
        v
    };
    let pow = if c.curve_type == CatmullRomType::Chordal { 0.5 } else { 0.25 };
    let mut dt0 = p0.distance_to_squared(&p1).powf(pow);
    let mut dt1 = p1.distance_to_squared(&p2).powf(pow);
    let mut dt2 = p2.distance_to_squared(&p3).powf(pow);
    if dt1 < 1e-4 {
        dt1 = 1.0;
    }
    if dt0 < 1e-4 {
        dt0 = dt1;
    }
    if dt2 < 1e-4 {
        dt2 = dt1;
    }
    let uniform = c.curve_type == CatmullRomType::CatmullRom;
    let k = c.tension;
    let px = if uniform {
        cubic_poly(p1.x, p2.x, k * (p2.x - p0.x), k * (p3.x - p1.x))
    } else {
        nonuniform_catmull_rom(p0.x, p1.x, p2.x, p3.x, dt0, dt1, dt2)
    };
    let py = if uniform {
        cubic_poly(p1.y, p2.y, k * (p2.y - p0.y), k * (p3.y - p1.y))
    } else {
        nonuniform_catmull_rom(p0.y, p1.y, p2.y, p3.y, dt0, dt1, dt2)
    };
    let pz = if uniform {
        cubic_poly(p1.z, p2.z, k * (p2.z - p0.z), k * (p3.z - p1.z))
    } else {
        nonuniform_catmull_rom(p0.z, p1.z, p2.z, p3.z, dt0, dt1, dt2)
    };
    Vector3::new(poly_at(&px, weight), poly_at(&py, weight), poly_at(&pz, weight))
}
