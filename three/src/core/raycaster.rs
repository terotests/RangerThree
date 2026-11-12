//! Picking: a ray from a camera through the screen, and what it hits
//! (three.js `Raycaster` with `Mesh`, `InstancedMesh`, `Points`, `Line`
//! and `Sprite` raycasting).

use ranger::prelude::*;

use super::bounds::{instance_bounding_sphere, instance_matrix_at};
use super::buffer_attribute::BufferAttribute;
use super::layers::Layers;
use super::object3d::{vertex_position, MeshData, ObjectKind, ObjectRef};
use crate::materials::material::Side;
use crate::maths::matrix4::Matrix4;
use crate::maths::ray::Ray;
use crate::maths::sphere::Sphere;
use crate::maths::triangle::Triangle;
use crate::maths::vector2::Vector2;
use crate::maths::vector3::Vector3;

/// The triangle a mesh hit lies on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub a: i64,
    pub b: i64,
    pub c: i64,
    pub normal: Vector3,
    pub material_index: i64,
}

/// One hit, nearest first in a raycaster's answer. Which optional parts
/// are there depends on what was hit, as in three.js.
#[derive(Clone)]
pub struct Intersection {
    pub distance: f64,
    pub point: Vector3,
    pub object: ObjectRef,
    pub face: Option<Face>,
    pub face_index: Option<i64>,
    pub uv: Option<Vector2>,
    pub uv1: Option<Vector2>,
    pub normal: Option<Vector3>,
    pub barycoord: Option<Vector3>,
    /// Points and lines: the vertex (or segment start) hit.
    pub index: Option<i64>,
    pub distance_to_ray: Option<f64>,
    pub instance_id: Option<i64>,
}

impl Intersection {
    fn at(distance: f64, point: Vector3, object: &ObjectRef) -> Intersection {
        Intersection {
            distance,
            point,
            object: object.clone(),
            face: None,
            face_index: None,
            uv: None,
            uv1: None,
            normal: None,
            barycoord: None,
            index: None,
            distance_to_ray: None,
            instance_id: None,
        }
    }
}

#[derive(Clone)]
pub struct Raycaster {
    pub ray: Ray,
    pub near: f64,
    pub far: f64,
    /// Needed for sprites, which face the camera.
    pub camera: Option<ObjectRef>,
    pub layers: Layers,
    pub line_threshold: f64,
    pub points_threshold: f64,
}

impl Raycaster {
    pub fn new(origin: Vector3, direction: Vector3, near: f64, far: f64) -> Raycaster {
        Raycaster {
            ray: Ray::new(origin, direction),
            near,
            far,
            camera: None,
            layers: Layers::new(),
            line_threshold: 1.0,
            points_threshold: 1.0,
        }
    }

    pub fn set(&mut self, origin: &Vector3, direction: &Vector3) {
        self.ray.set(origin, direction);
    }

    /// The ray from `camera` through `coords` (normalized device
    /// coordinates, -1..1 with y up).
    pub fn set_from_camera(&mut self, coords: &Vector2, camera: &ObjectRef) {
        let c = camera.borrow();
        let cd = match c.camera() {
            Some(cd) => cd.clone(),
            None => return,
        };
        if matches!(c.kind, ObjectKind::PerspectiveCamera(_)) {
            self.ray.origin.set_from_matrix_position(&c.matrix_world);
            let mut d = Vector3::new(coords.x, coords.y, 0.5);
            d.apply_matrix4(&cd.projection_matrix_inverse);
            d.apply_matrix4(&c.matrix_world);
            d.sub(&self.ray.origin);
            d.normalize();
            self.ray.direction = d;
        } else {
            let mut o = Vector3::new(coords.x, coords.y, cd.projection_matrix.elements[14]);
            o.apply_matrix4(&cd.projection_matrix_inverse);
            o.apply_matrix4(&c.matrix_world);
            self.ray.origin = o;
            let mut d = Vector3::new(0.0, 0.0, -1.0);
            d.transform_direction(&c.matrix_world);
            self.ray.direction = d;
        }
        drop(c);
        self.camera = Some(camera.clone());
    }

    /// What the ray hits in `object` (and, with `recursive`, under it),
    /// nearest first.
    pub fn intersect_object(&self, object: &ObjectRef, recursive: bool) -> Vec<Intersection> {
        let mut out: Vec<Intersection> = Vec::new();
        self.intersect(object, recursive, &mut out);
        sort_by_distance(&mut out);
        out
    }

    pub fn intersect_objects(&self, objects: &Vec<ObjectRef>, recursive: bool) -> Vec<Intersection> {
        let mut out: Vec<Intersection> = Vec::new();
        for o in objects.iter() {
            self.intersect(o, recursive, &mut out);
        }
        sort_by_distance(&mut out);
        out
    }

    fn intersect(&self, object: &ObjectRef, recursive: bool, out: &mut Vec<Intersection>) {
        let test = object.borrow().layers.test(&self.layers);
        if test {
            self.raycast(object, out);
        }
        if recursive {
            let kids: Vec<ObjectRef> = object.borrow().children.clone();
            for c in kids.iter() {
                self.intersect(c, true, out);
            }
        }
    }

    /// The hits on one object (not its children).
    pub fn raycast(&self, object: &ObjectRef, out: &mut Vec<Intersection>) {
        let o = object.borrow();
        let matrix_world = o.matrix_world;
        let scale = o.scale;
        match &o.kind {
            ObjectKind::Mesh(d) => {
                let d = d.clone();
                drop(o);
                self.raycast_mesh(object, &d, &matrix_world, None, out);
            }
            ObjectKind::InstancedMesh(d) => {
                let d = d.clone();
                drop(o);
                let mut sphere = instance_bounding_sphere(&d);
                sphere.apply_matrix4(&matrix_world);
                if !self.ray.intersects_sphere(&sphere) {
                    return;
                }
                for i in 0..(d.count as usize) {
                    let mut m = matrix_world;
                    m.multiply(&instance_matrix_at(&d, i));
                    self.raycast_mesh(object, &d, &m, Some(i as i64), out);
                }
            }
            ObjectKind::Points(d) => {
                let d = d.clone();
                drop(o);
                self.raycast_points(object, &d, &matrix_world, &scale, out);
            }
            ObjectKind::Line(d) => {
                let d = d.clone();
                drop(o);
                self.raycast_line(object, &d, &matrix_world, &scale, 1, false, out);
            }
            ObjectKind::LineSegments(d) => {
                let d = d.clone();
                drop(o);
                self.raycast_line(object, &d, &matrix_world, &scale, 2, false, out);
            }
            ObjectKind::LineLoop(d) => {
                let d = d.clone();
                drop(o);
                self.raycast_line(object, &d, &matrix_world, &scale, 1, true, out);
            }
            ObjectKind::Sprite(_) => {
                drop(o);
                self.raycast_sprite(object, out);
            }
            _ => {}
        }
    }

    fn world_sphere(&self, data: &MeshData, matrix_world: &Matrix4) -> Sphere {
        let mut g = data.geometry.borrow_mut();
        if g.bounding_sphere.is_none() {
            g.compute_bounding_sphere();
        }
        let mut s = g.bounding_sphere.unwrap_or(Sphere::empty());
        s.apply_matrix4(matrix_world);
        s
    }

    fn local_ray(&self, matrix_world: &Matrix4) -> Ray {
        let mut inverse = *matrix_world;
        inverse.invert();
        let mut r = self.ray;
        r.apply_matrix4(&inverse);
        r
    }

    fn raycast_mesh(&self, object: &ObjectRef, data: &MeshData, matrix_world: &Matrix4, instance_id: Option<i64>, out: &mut Vec<Intersection>) {
        let sphere = self.world_sphere(data, matrix_world);
        let mut near_ray = self.ray;
        near_ray.recast(self.near);
        if !sphere.contains_point(&near_ray.origin) {
            match near_ray.intersect_sphere(&sphere) {
                Some(hit) => {
                    let span = self.far - self.near;
                    if near_ray.origin.distance_to_squared(&hit) > span * span {
                        return;
                    }
                }
                None => return,
            }
        }
        let local = self.local_ray(matrix_world);
        let g = data.geometry.borrow();
        if let Some(b) = &g.bounding_box {
            if !local.intersects_box(b) {
                return;
            }
        }
        let uv = g.get_attribute("uv");
        let uv1 = g.get_attribute("uv1");
        let normal = g.get_attribute("normal");
        let position = g.get_attribute("position");
        let dr_start = g.draw_range.start;
        let dr_end = g.draw_range.start + g.draw_range.count;
        let groups = g.groups.clone();
        let index = g.get_index();
        drop(g);
        let count = match &index {
            Some(ix) => ix.borrow().count as f64,
            None => match &position {
                Some(p) => p.borrow().count as f64,
                None => return,
            },
        };
        let corner = |i: f64| -> i64 {
            match &index {
                Some(ix) => ix.borrow().get_x(i as usize) as i64,
                None => i as i64,
            }
        };
        if data.material_is_array {
            for group in groups.iter() {
                let mi = group.material_index as usize;
                if mi >= data.materials.len() {
                    continue;
                }
                let side = data.materials[mi].borrow().side;
                let start = (group.start as f64).max(dr_start);
                let end = count.min(((group.start + group.count) as f64).min(dr_end));
                let mut j = start;
                while j < end {
                    let hit = self.triangle_hit(object, data, matrix_world, &local, side, &uv, &uv1, &normal, corner(j), corner(j + 1.0), corner(j + 2.0));
                    if let Some(mut h) = hit {
                        h.face_index = Some((j / 3.0).floor() as i64);
                        if let Some(mut f) = h.face {
                            f.material_index = group.material_index;
                            h.face = Some(f);
                        }
                        h.instance_id = instance_id;
                        out.push(h);
                    }
                    j += 3.0;
                }
            }
        } else {
            let side = data.materials[0].borrow().side;
            let start = dr_start.max(0.0);
            let end = count.min(dr_end);
            let mut i = start;
            while i < end {
                let hit = self.triangle_hit(object, data, matrix_world, &local, side, &uv, &uv1, &normal, corner(i), corner(i + 1.0), corner(i + 2.0));
                if let Some(mut h) = hit {
                    h.face_index = Some((i / 3.0).floor() as i64);
                    h.instance_id = instance_id;
                    out.push(h);
                }
                i += 3.0;
            }
        }
    }

    fn triangle_hit(
        &self,
        object: &ObjectRef,
        data: &MeshData,
        matrix_world: &Matrix4,
        local: &Ray,
        side: Side,
        uv: &Option<super::buffer_geometry::AttributeRef>,
        uv1: &Option<super::buffer_geometry::AttributeRef>,
        normal: &Option<super::buffer_geometry::AttributeRef>,
        a: i64,
        b: i64,
        c: i64,
    ) -> Option<Intersection> {
        let va = vertex_position(data, a as usize);
        let vb = vertex_position(data, b as usize);
        let vc = vertex_position(data, c as usize);
        let point = match side {
            Side::Back => local.intersect_triangle(&vc, &vb, &va, true),
            Side::Front => local.intersect_triangle(&va, &vb, &vc, true),
            Side::Double => local.intersect_triangle(&va, &vb, &vc, false),
        };
        let p = match point {
            Some(p) => p,
            None => return None,
        };
        let mut world = p;
        world.apply_matrix4(matrix_world);
        let distance = self.ray.origin.distance_to(&world);
        if distance < self.near || distance > self.far {
            return None;
        }
        let mut hit = Intersection::at(distance, world, object);
        let bary = Triangle::barycoord_of(&p, &va, &vb, &vc).unwrap_or(Vector3::zero());
        if let Some(u) = uv {
            hit.uv = Some(interpolate2(&u.borrow(), a, b, c, &bary));
        }
        if let Some(u) = uv1 {
            hit.uv1 = Some(interpolate2(&u.borrow(), a, b, c, &bary));
        }
        if let Some(n) = normal {
            let mut v = interpolate3(&n.borrow(), a, b, c, &bary);
            if v.dot(&local.direction) > 0.0 {
                v.multiply_scalar(-1.0);
            }
            hit.normal = Some(v);
        }
        hit.face = Some(Face { a, b, c, normal: Triangle::normal_of(&va, &vb, &vc), material_index: 0 });
        hit.barycoord = Some(bary);
        Some(hit)
    }

    fn raycast_points(&self, object: &ObjectRef, data: &MeshData, matrix_world: &Matrix4, scale: &Vector3, out: &mut Vec<Intersection>) {
        let threshold = self.points_threshold;
        let mut sphere = self.world_sphere(data, matrix_world);
        sphere.radius += threshold;
        if !self.ray.intersects_sphere(&sphere) {
            return;
        }
        let local = self.local_ray(matrix_world);
        let local_threshold = threshold / ((scale.x + scale.y + scale.z) / 3.0);
        let threshold_sq = local_threshold * local_threshold;
        let g = data.geometry.borrow();
        let position = match g.get_attribute("position") {
            Some(p) => p,
            None => return,
        };
        let pos = position.borrow();
        let index = g.get_index();
        let count = match &index {
            Some(ix) => ix.borrow().count as f64,
            None => pos.count as f64,
        };
        let start = g.draw_range.start.max(0.0);
        let end = count.min(g.draw_range.start + g.draw_range.count);
        let mut i = start;
        while i < end {
            let vi = match &index {
                Some(ix) => ix.borrow().get_x(i as usize) as i64,
                None => i as i64,
            };
            let point = pos.get_vector3(vi as usize);
            let d_sq = local.distance_sq_to_point(&point);
            if d_sq < threshold_sq {
                let mut p = local.closest_point_to_point(&point);
                p.apply_matrix4(matrix_world);
                let distance = self.ray.origin.distance_to(&p);
                if distance >= self.near && distance <= self.far {
                    let mut hit = Intersection::at(distance, p, object);
                    hit.distance_to_ray = Some(d_sq.sqrt());
                    hit.index = Some(vi);
                    out.push(hit);
                }
            }
            i += 1.0;
        }
    }

    fn raycast_line(&self, object: &ObjectRef, data: &MeshData, matrix_world: &Matrix4, scale: &Vector3, step: i64, closed: bool, out: &mut Vec<Intersection>) {
        let threshold = self.line_threshold;
        let mut sphere = self.world_sphere(data, matrix_world);
        sphere.radius += threshold;
        if !self.ray.intersects_sphere(&sphere) {
            return;
        }
        let local = self.local_ray(matrix_world);
        let local_threshold = threshold / ((scale.x + scale.y + scale.z) / 3.0);
        let threshold_sq = local_threshold * local_threshold;
        let g = data.geometry.borrow();
        let position = match g.get_attribute("position") {
            Some(p) => p,
            None => return,
        };
        let pos = position.borrow();
        let index = g.get_index();
        let count = match &index {
            Some(ix) => ix.borrow().count as f64,
            None => pos.count as f64,
        };
        let start = g.draw_range.start.max(0.0);
        let end = count.min(g.draw_range.start + g.draw_range.count);
        let vertex = |i: f64| -> usize {
            match &index {
                Some(ix) => ix.borrow().get_x(i as usize) as usize,
                None => i as usize,
            }
        };
        let mut i = start;
        while i < end - 1.0 {
            if let Some(h) = self.segment_hit(object, &pos, matrix_world, &local, threshold_sq, vertex(i), vertex(i + 1.0), i as i64) {
                out.push(h);
            }
            i += step as f64;
        }
        if closed && end > start {
            if let Some(h) = self.segment_hit(object, &pos, matrix_world, &local, threshold_sq, vertex(end - 1.0), vertex(start), (end - 1.0) as i64) {
                out.push(h);
            }
        }
    }

    fn segment_hit(&self, object: &ObjectRef, pos: &BufferAttribute, matrix_world: &Matrix4, local: &Ray, threshold_sq: f64, a: usize, b: usize, i: i64) -> Option<Intersection> {
        let start = pos.get_vector3(a);
        let end = pos.get_vector3(b);
        let s = local.segment_hit(&start, &end);
        if s.distance_sq > threshold_sq {
            return None;
        }
        let mut on_ray = s.point_on_ray;
        on_ray.apply_matrix4(matrix_world);
        let distance = self.ray.origin.distance_to(&on_ray);
        if distance < self.near || distance > self.far {
            return None;
        }
        let mut point = s.point_on_segment;
        point.apply_matrix4(matrix_world);
        let mut hit = Intersection::at(distance, point, object);
        hit.index = Some(i);
        Some(hit)
    }

    fn raycast_sprite(&self, object: &ObjectRef, out: &mut Vec<Intersection>) {
        let camera = match &self.camera {
            Some(c) => c.clone(),
            None => return,
        };
        let o = object.borrow();
        let sd = match &o.kind {
            ObjectKind::Sprite(s) => s.clone(),
            _ => return,
        };
        let c = camera.borrow();
        let cd = match c.camera() {
            Some(cd) => cd.clone(),
            None => return,
        };
        let mut world_scale = Vector3::zero();
        world_scale.set_from_matrix_scale(&o.matrix_world);
        let view_world = c.matrix_world;
        let mut model_view = cd.matrix_world_inverse;
        model_view.multiply(&o.matrix_world);
        let mut mv_position = Vector3::zero();
        mv_position.set_from_matrix_position(&model_view);
        let material = sd.material.borrow();
        if matches!(c.kind, ObjectKind::PerspectiveCamera(_)) && !material.size_attenuation {
            world_scale.multiply_scalar(-mv_position.z);
        }
        let rotation = material.rotation;
        let corner = |x: f64, y: f64| -> Vector3 {
            let ax = (x - sd.center.x + 0.5) * world_scale.x;
            let ay = (y - sd.center.y + 0.5) * world_scale.y;
            let (rx, ry) = if rotation != 0.0 {
                (rotation.cos() * ax - rotation.sin() * ay, rotation.sin() * ax + rotation.cos() * ay)
            } else {
                (ax, ay)
            };
            let mut v = mv_position;
            v.x += rx;
            v.y += ry;
            v.apply_matrix4(&view_world);
            v
        };
        let va = corner(-0.5, -0.5);
        let mut vb = corner(0.5, -0.5);
        let vc = corner(0.5, 0.5);
        let mut uvb = Vector2::new(1.0, 0.0);
        let mut hit = self.ray.intersect_triangle(&va, &vb, &vc, false);
        if hit.is_none() {
            vb = corner(-0.5, 0.5);
            uvb = Vector2::new(0.0, 1.0);
            hit = self.ray.intersect_triangle(&va, &vc, &vb, false);
        }
        let p = match hit {
            Some(p) => p,
            None => return,
        };
        let distance = self.ray.origin.distance_to(&p);
        if distance < self.near || distance > self.far {
            return;
        }
        let uva = Vector2::new(0.0, 0.0);
        let uvc = Vector2::new(1.0, 1.0);
        // uv from the corners A, B, C as last set (B is the replaced one
        // after the second try), as three.js interpolates
        let bary = Triangle::barycoord_of(&p, &va, &vb, &vc);
        let mut out_hit = Intersection::at(distance, p, object);
        if let Some(w) = bary {
            out_hit.uv = Some(Vector2::new(
                uva.x * w.x + uvb.x * w.y + uvc.x * w.z,
                uva.y * w.x + uvb.y * w.y + uvc.y * w.z,
            ));
        }
        out.push(out_hit);
    }
}

fn interpolate2(attr: &BufferAttribute, a: i64, b: i64, c: i64, w: &Vector3) -> Vector2 {
    let ax = attr.get_x(a as usize);
    let ay = attr.get_y(a as usize);
    let bx = attr.get_x(b as usize);
    let by = attr.get_y(b as usize);
    let cx = attr.get_x(c as usize);
    let cy = attr.get_y(c as usize);
    Vector2::new(ax * w.x + bx * w.y + cx * w.z, ay * w.x + by * w.y + cy * w.z)
}

fn interpolate3(attr: &BufferAttribute, a: i64, b: i64, c: i64, w: &Vector3) -> Vector3 {
    let va = attr.get_vector3(a as usize);
    let vb = attr.get_vector3(b as usize);
    let vc = attr.get_vector3(c as usize);
    Vector3::new(
        va.x * w.x + vb.x * w.y + vc.x * w.z,
        va.y * w.x + vb.y * w.y + vc.y * w.z,
        va.z * w.x + vb.z * w.y + vc.z * w.z,
    )
}

/// Stable insertion sort, nearest first (JavaScript's sort is stable too).
fn sort_by_distance(hits: &mut Vec<Intersection>) {
    let n = hits.len();
    let mut i = 1;
    while i < n {
        let mut j = i;
        while j > 0 && hits[j - 1].distance > hits[j].distance {
            hits.swap(j - 1, j);
            j -= 1;
        }
        i += 1;
    }
}
