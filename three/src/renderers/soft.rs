//! A software renderer: draws a scene into an RGBA pixel buffer with a
//! depth buffer. Meshes (also instanced) are lit per vertex (flat when the
//! material asks for it or the geometry has no normals) with ambient,
//! hemisphere, directional, point and spot lights, Blinn-Phong highlights
//! for Phong and Standard materials, vertex and instance colours and fog;
//! lines, line loops, segments, points and wireframes are drawn too.
//! Triangles are clipped at the camera's near plane. It is the renderer the
//! playground and tests use; the GL backend comes later.

use ranger::prelude::*;

use crate::core::object3d::{descendants, update_matrix_world, Background, ObjectKind, ObjectRef};
use crate::materials::material::{Material, MaterialKind, Side};
use crate::maths::color::{linear_to_srgb, Color};
use crate::maths::matrix3::Matrix3;
use crate::maths::matrix4::Matrix4;
use crate::maths::vector3::Vector3;
use crate::scenes::fog::Fog;

#[derive(Clone, Copy, Debug, PartialEq)]
struct LightIn {
    /// 0 ambient, 1 hemisphere, 2 directional, 3 point, 4 spot
    kind: i64,
    color: Color,
    ground: Color,
    intensity: f64,
    position: Vector3,
    direction: Vector3,
    distance: f64,
    decay: f64,
    cos_outer: f64,
    cos_inner: f64,
}

/// What shading needs from a material.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Surface {
    kind: MaterialKind,
    color: Color,
    emissive: Color,
    specular: Color,
    shininess: f64,
    diffuse_scale: f64,
    lit: bool,
    specular_on: bool,
}

/// A vertex after lighting, in view space.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Vert {
    view: Vector3,
    color: Color,
}

/// Per-frame state the draw calls share.
#[derive(Clone, Debug)]
struct Frame {
    view: Matrix4,
    projection: Matrix4,
    near: f64,
    camera_position: Vector3,
    lights: Vec<LightIn>,
    fog: Option<Fog>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoftRenderer {
    pub width: i64,
    pub height: i64,
    /// RGBA, 0..255, row by row from the top.
    pub pixels: Vec<f64>,
    pub depth: Vec<f64>,
    pub clear_color: Color,
    pub clear_alpha: f64,
    /// Triangles, lines and points drawn by the last render.
    pub triangles: i64,
    pub lines: i64,
    pub points: i64,
}

fn to_byte(c: f64) -> f64 {
    let v = (linear_to_srgb(c.max(0.0).min(1.0)) * 255.0 + 0.5).floor();
    v.max(0.0).min(255.0)
}

fn mix_color(a: &Color, b: &Color, t: f64) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t)
}

fn mul_color(a: &Color, b: &Color) -> Color {
    Color::new(a.r * b.r, a.g * b.g, a.b * b.b)
}

fn lerp_vert(a: &Vert, b: &Vert, t: f64) -> Vert {
    let mut v = a.view;
    v.lerp(&b.view, t);
    Vert { view: v, color: mix_color(&a.color, &b.color, t) }
}

fn surface_of(m: &Material) -> Surface {
    let lit = match m.kind {
        MaterialKind::MeshLambert
        | MaterialKind::MeshPhong
        | MaterialKind::MeshStandard
        | MaterialKind::MeshPhysical
        | MaterialKind::MeshToon => true,
        _ => false,
    };
    let mut specular = m.specular;
    let mut shininess = m.shininess;
    let mut diffuse_scale = 1.0;
    let mut specular_on = m.kind == MaterialKind::MeshPhong;
    if m.kind == MaterialKind::MeshStandard || m.kind == MaterialKind::MeshPhysical {
        // a Blinn-Phong stand-in for the metal/rough model
        let r = m.roughness.max(0.04);
        shininess = (2.0 / (r * r * r * r) - 2.0).max(1.0).min(2048.0);
        let base = Color::new(0.04, 0.04, 0.04);
        specular = mix_color(&base, &m.color, m.metalness);
        let gloss = 1.0 - r;
        specular = Color::new(specular.r * gloss, specular.g * gloss, specular.b * gloss);
        diffuse_scale = 1.0 - m.metalness;
        specular_on = true;
    }
    let mut emissive = m.emissive;
    emissive = Color::new(emissive.r * m.emissive_intensity, emissive.g * m.emissive_intensity, emissive.b * m.emissive_intensity);
    Surface { kind: m.kind, color: m.color, emissive, specular, shininess, diffuse_scale, lit, specular_on }
}

/// The colour of a surface point `at` (world) with normal `n` (world).
fn shade(s: &Surface, albedo: &Color, n: &Vector3, at: &Vector3, frame: &Frame) -> Color {
    if s.kind == MaterialKind::MeshNormal {
        let mut vn = *n;
        vn.transform_direction(&frame.view);
        return Color::new(vn.x * 0.5 + 0.5, vn.y * 0.5 + 0.5, vn.z * 0.5 + 0.5);
    }
    if !s.lit {
        return *albedo;
    }
    let mut to_eye = frame.camera_position;
    to_eye.sub(at);
    to_eye.normalize();
    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;
    let mut sr = 0.0;
    let mut sg = 0.0;
    let mut sb = 0.0;
    for l in frame.lights.iter() {
        let mut c = l.color;
        let mut k = 1.0;
        let mut dir = l.direction;
        let mut directional = true;
        if l.kind == 0 {
            directional = false;
        } else if l.kind == 1 {
            let t = n.dot(&l.direction) * 0.5 + 0.5;
            c = mix_color(&l.ground, &l.color, t);
            directional = false;
        } else if l.kind >= 3 {
            dir = l.position;
            dir.sub(at);
            let d = dir.length();
            dir.normalize();
            // three.js's punctual light falloff
            let mut fall = 1.0 / d.powf(l.decay).max(0.01);
            if l.distance > 0.0 {
                let ratio = d / l.distance;
                let w = (1.0 - ratio * ratio * ratio * ratio).max(0.0).min(1.0);
                fall *= w * w;
            }
            k = fall;
            if l.kind == 4 {
                let mut to_light = l.direction;
                to_light.negate();
                let cos = dir.dot(&to_light);
                let edge = if l.cos_inner > l.cos_outer { ((cos - l.cos_outer) / (l.cos_inner - l.cos_outer)).max(0.0).min(1.0) } else if cos >= l.cos_outer { 1.0 } else { 0.0 };
                k *= edge * edge * (3.0 - 2.0 * edge);
            }
        }
        let mut diffuse = 1.0;
        if directional {
            let nl = n.dot(&dir);
            diffuse = nl.max(0.0);
            if s.kind == MaterialKind::MeshToon {
                diffuse = if nl > 0.5 { 1.0 } else if nl > 0.0 { 0.5 } else { 0.0 };
            }
            if s.specular_on && nl > 0.0 {
                let mut h = dir;
                h.add(&to_eye);
                h.normalize();
                let spec = n.dot(&h).max(0.0).powf(s.shininess) * (s.shininess + 2.0) / 8.0;
                let e = l.intensity * k * spec * nl;
                sr += c.r * e;
                sg += c.g * e;
                sb += c.b * e;
            }
        }
        let e = l.intensity * k * diffuse;
        r += c.r * e;
        g += c.g * e;
        b += c.b * e;
    }
    let d = s.diffuse_scale;
    Color::new(
        albedo.r * r * d + s.specular.r * sr + s.emissive.r,
        albedo.g * g * d + s.specular.g * sg + s.emissive.g,
        albedo.b * b * d + s.specular.b * sb + s.emissive.b,
    )
}

fn collect_lights(objects: &Vec<ObjectRef>) -> Vec<LightIn> {
    let mut out: Vec<LightIn> = Vec::new();
    for o in objects.iter() {
        let ob = o.borrow();
        if !ob.visible {
            continue;
        }
        let kind = match &ob.kind {
            ObjectKind::AmbientLight(_) => 0,
            ObjectKind::HemisphereLight(_) => 1,
            ObjectKind::DirectionalLight(_) => 2,
            ObjectKind::PointLight(_) => 3,
            ObjectKind::SpotLight(_) => 4,
            _ => -1,
        };
        if kind < 0 {
            continue;
        }
        let l = match ob.light() {
            Some(l) => l,
            None => continue,
        };
        let position = ob.world_position();
        let mut target = Vector3::zero();
        if let Some(t) = &l.target {
            target.set_from_matrix_position(&t.borrow().matrix_world);
        }
        let mut direction = position;
        if kind == 1 {
            direction = Vector3::new(0.0, 1.0, 0.0);
            direction.apply_matrix4(&ob.matrix_world);
            direction.sub(&position);
        } else if kind == 4 {
            // spot: from the light towards its target
            direction = target;
            direction.sub(&position);
        } else {
            direction.sub(&target);
        }
        direction.normalize();
        out.push(LightIn {
            kind,
            color: l.color,
            ground: l.ground_color,
            intensity: l.intensity,
            position,
            direction,
            distance: l.distance,
            decay: l.decay,
            cos_outer: l.angle.cos(),
            cos_inner: (l.angle * (1.0 - l.penumbra)).cos(),
        });
    }
    out
}

impl SoftRenderer {
    pub fn new(width: i64, height: i64) -> SoftRenderer {
        let mut r = SoftRenderer {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            depth: Vec::new(),
            clear_color: Color::new(0.0, 0.0, 0.0),
            clear_alpha: 1.0,
            triangles: 0,
            lines: 0,
            points: 0,
        };
        r.set_size(width, height);
        r
    }

    pub fn set_size(&mut self, width: i64, height: i64) {
        self.width = width.max(1);
        self.height = height.max(1);
        let n = (self.width * self.height) as usize;
        self.pixels = vec![0.0; n * 4];
        self.depth = vec![1.0; n];
    }

    pub fn clear(&mut self, color: &Color, alpha: f64) {
        let r = to_byte(color.r);
        let g = to_byte(color.g);
        let b = to_byte(color.b);
        let a = (alpha * 255.0).max(0.0).min(255.0);
        let n = (self.width * self.height) as usize;
        for i in 0..n {
            self.pixels[i * 4] = r;
            self.pixels[i * 4 + 1] = g;
            self.pixels[i * 4 + 2] = b;
            self.pixels[i * 4 + 3] = a;
            self.depth[i] = 1.0;
        }
    }

    pub fn render(&mut self, scene: &ObjectRef, camera: &ObjectRef) {
        update_matrix_world(scene, false);
        if camera.borrow().parent.is_none() {
            update_matrix_world(camera, false);
        }
        let mut clear = self.clear_color;
        let mut fog: Option<Fog> = None;
        if let ObjectKind::Scene(s) = &scene.borrow().kind {
            if let Background::Color(c) = &s.background {
                clear = *c;
            }
            fog = s.fog.clone();
        }
        let alpha = self.clear_alpha;
        self.clear(&clear, alpha);
        self.triangles = 0;
        self.lines = 0;
        self.points = 0;

        let mut view = camera.borrow().matrix_world;
        view.invert();
        let mut projection = Matrix4::identity();
        let mut near = 0.1;
        if let Some(c) = camera.borrow().camera() {
            projection = c.projection_matrix;
            near = c.near;
        }
        let camera_position = camera.borrow().world_position();
        let objects = descendants(scene);
        let lights = collect_lights(&objects);
        let frame = Frame { view, projection, near, camera_position, lights, fog };
        // opaque first, then transparent ones back to front
        let mut transparent: Vec<ObjectRef> = Vec::new();
        for o in objects.iter() {
            if !self.visible_chain(o) {
                continue;
            }
            if self.is_transparent(o) {
                transparent.push(o.clone());
                continue;
            }
            self.draw_object(o, &frame);
        }
        let mut depths: Vec<f64> = Vec::new();
        for o in transparent.iter() {
            let mut p = o.borrow().world_position();
            p.apply_matrix4(&frame.view);
            depths.push(p.z);
        }
        for i in 1..transparent.len() {
            let mut j = i;
            while j > 0 && depths[j - 1] > depths[j] {
                depths.swap(j - 1, j);
                transparent.swap(j - 1, j);
                j -= 1;
            }
        }
        for o in transparent.iter() {
            self.draw_object(o, &frame);
        }
    }

    fn visible_chain(&self, o: &ObjectRef) -> bool {
        if !o.borrow().visible {
            return false;
        }
        let mut at = match &o.borrow().parent {
            Some(w) => w.upgrade(),
            None => None,
        };
        while let Some(p) = at {
            if !p.borrow().visible {
                return false;
            }
            at = match &p.borrow().parent {
                Some(w) => w.upgrade(),
                None => None,
            };
        }
        true
    }

    fn is_transparent(&self, o: &ObjectRef) -> bool {
        let ob = o.borrow();
        match ob.mesh_data() {
            Some(d) => {
                if d.materials.is_empty() {
                    return false;
                }
                let m = d.materials[0].borrow();
                m.transparent && m.opacity < 1.0
            }
            None => false,
        }
    }

    fn draw_object(&mut self, o: &ObjectRef, frame: &Frame) {
        let kind = match &o.borrow().kind {
            ObjectKind::Mesh(_) => 1,
            ObjectKind::InstancedMesh(_) => 2,
            ObjectKind::Line(_) => 3,
            ObjectKind::LineSegments(_) => 4,
            ObjectKind::LineLoop(_) => 5,
            ObjectKind::Points(_) => 6,
            _ => 0,
        };
        if kind == 0 {
            return;
        }
        let ob = o.borrow();
        let data = match ob.mesh_data() {
            Some(d) => d,
            None => return,
        };
        if data.materials.is_empty() {
            return;
        }
        let material = data.materials[0].borrow();
        if !material.visible {
            return;
        }
        let geometry = data.geometry.borrow();
        let world = ob.matrix_world;
        if kind == 1 {
            self.draw_mesh(&geometry, &material, &world, None, frame);
        } else if kind == 2 {
            let count = data.count;
            if let Some(im) = &data.instance_matrix {
                let ia = im.borrow();
                for i in 0..count {
                    let mut local = Matrix4::identity();
                    local.from_array(&ia.array, (i * 16) as usize);
                    let mut m = Matrix4::identity();
                    m.multiply_matrices(&world, &local);
                    let mut tint: Option<Color> = None;
                    if let Some(ic) = &data.instance_color {
                        let c = ic.borrow().get_vector3(i as usize);
                        tint = Some(Color::new(c.x, c.y, c.z));
                    }
                    self.draw_mesh(&geometry, &material, &m, tint, frame);
                }
            }
        } else if kind == 6 {
            self.draw_points(&geometry, &material, &world, frame);
        } else {
            self.draw_lines(&geometry, &material, &world, kind, frame);
        }
    }

    fn fogged(&self, c: &Color, view_z: f64, frame: &Frame, uses_fog: bool) -> Color {
        if !uses_fog {
            return *c;
        }
        match &frame.fog {
            Some(f) => mix_color(c, &f.color, f.factor(-view_z)),
            None => *c,
        }
    }

    fn to_screen(&self, v: &Vector3, frame: &Frame) -> Vector3 {
        let mut c = *v;
        c.apply_matrix4(&frame.projection);
        Vector3::new((c.x + 1.0) * 0.5 * self.width as f64, (1.0 - c.y) * 0.5 * self.height as f64, c.z)
    }

    fn draw_mesh(&mut self, geometry: &crate::core::buffer_geometry::BufferGeometry, material: &Material, world: &Matrix4, tint: Option<Color>, frame: &Frame) {
        let position = match geometry.get_attribute("position") {
            Some(p) => p,
            None => return,
        };
        let pos = position.borrow();
        let mut normals = geometry.get_attribute("normal");
        if material.flat_shading {
            normals = None;
        }
        let mut colors = geometry.get_attribute("color");
        if !material.vertex_colors {
            colors = None;
        }
        let index = geometry.get_index();
        let count = match &index {
            Some(ix) => ix.borrow().count,
            None => pos.count,
        };
        let mut normal_matrix = Matrix3::identity();
        normal_matrix.get_normal_matrix(world);
        let surface = surface_of(material);
        let mut base = material.color;
        if let Some(t) = &tint {
            base = mul_color(&base, t);
        }
        let opacity = material.opacity;
        let blend = material.transparent && opacity < 1.0;
        let mut i = 0;
        while i + 2 < count {
            let mut ids: Vec<usize> = vec![i, i + 1, i + 2];
            if let Some(ix) = &index {
                let ib = ix.borrow();
                ids = vec![ib.get_x(i) as usize, ib.get_x(i + 1) as usize, ib.get_x(i + 2) as usize];
            }
            i += 3;
            let mut wp: Vec<Vector3> = Vec::new();
            for k in 0..3 {
                let mut p = pos.get_vector3(ids[k]);
                p.apply_matrix4(world);
                wp.push(p);
            }
            let mut e1 = wp[1];
            e1.sub(&wp[0]);
            let mut e2 = wp[2];
            e2.sub(&wp[0]);
            let mut face_n = Vector3::zero();
            face_n.cross_vectors(&e1, &e2);
            face_n.normalize();
            // facing: the face normal against the direction to the camera
            let mut to_cam = frame.camera_position;
            to_cam.sub(&wp[0]);
            let front = face_n.dot(&to_cam) > 0.0;
            if material.side == Side::Front && !front {
                continue;
            }
            if material.side == Side::Back && front {
                continue;
            }
            let flip = !front;
            if material.wireframe {
                let mut vs: Vec<Vert> = Vec::new();
                for k in 0..3 {
                    let mut v = wp[k];
                    v.apply_matrix4(&frame.view);
                    vs.push(Vert { view: v, color: self.fogged(&base, v.z, frame, material.fog) });
                }
                for k in 0..3 {
                    let k2 = (k + 1) % 3;
                    self.line_3d(&vs[k], &vs[k2], frame);
                }
                continue;
            }
            let mut verts: Vec<Vert> = Vec::new();
            let mut centre = wp[0];
            centre.add(&wp[1]);
            centre.add(&wp[2]);
            centre.multiply_scalar(1.0 / 3.0);
            let mut flat_color = Color::new(0.0, 0.0, 0.0);
            if normals.is_none() {
                let mut n = face_n;
                if flip {
                    n.negate();
                }
                flat_color = shade(&surface, &base, &n, &centre, frame);
            }
            for k in 0..3 {
                let mut albedo = base;
                if let Some(ca) = &colors {
                    let c = ca.borrow().get_vector3(ids[k]);
                    albedo = mul_color(&albedo, &Color::new(c.x, c.y, c.z));
                }
                let mut color = flat_color;
                if let Some(na) = &normals {
                    let mut n = na.borrow().get_vector3(ids[k]);
                    n.apply_normal_matrix(&normal_matrix);
                    if flip {
                        n.negate();
                    }
                    color = shade(&surface, &albedo, &n, &wp[k], frame);
                } else if colors.is_some() {
                    let mut n = face_n;
                    if flip {
                        n.negate();
                    }
                    color = shade(&surface, &albedo, &n, &wp[k], frame);
                }
                let mut v = wp[k];
                v.apply_matrix4(&frame.view);
                color = self.fogged(&color, v.z, frame, material.fog);
                verts.push(Vert { view: v, color });
            }
            let clipped = clip_near(&verts, -frame.near);
            if clipped.len() < 3 {
                continue;
            }
            let mut screen: Vec<Vector3> = Vec::new();
            for v in clipped.iter() {
                screen.push(self.to_screen(&v.view, frame));
            }
            for k in 1..(clipped.len() - 1) {
                self.fill_triangle(&screen[0], &screen[k], &screen[k + 1], &clipped[0].color, &clipped[k].color, &clipped[k + 1].color, opacity, blend);
            }
            self.triangles += 1;
        }
    }

    fn draw_lines(&mut self, geometry: &crate::core::buffer_geometry::BufferGeometry, material: &Material, world: &Matrix4, kind: i64, frame: &Frame) {
        let position = match geometry.get_attribute("position") {
            Some(p) => p,
            None => return,
        };
        let pos = position.borrow();
        let mut colors = geometry.get_attribute("color");
        if !material.vertex_colors {
            colors = None;
        }
        let index = geometry.get_index();
        let count = match &index {
            Some(ix) => ix.borrow().count,
            None => pos.count,
        };
        let mut verts: Vec<Vert> = Vec::new();
        for i in 0..count {
            let id = match &index {
                Some(ix) => ix.borrow().get_x(i) as usize,
                None => i,
            };
            let mut p = pos.get_vector3(id);
            p.apply_matrix4(world);
            p.apply_matrix4(&frame.view);
            let mut c = material.color;
            if let Some(ca) = &colors {
                let v = ca.borrow().get_vector3(id);
                c = mul_color(&c, &Color::new(v.x, v.y, v.z));
            }
            verts.push(Vert { view: p, color: self.fogged(&c, p.z, frame, material.fog) });
        }
        let n = verts.len();
        if n < 2 {
            return;
        }
        if kind == 4 {
            let mut i = 0;
            while i + 1 < n {
                self.line_3d(&verts[i], &verts[i + 1], frame);
                i += 2;
            }
        } else {
            for i in 0..(n - 1) {
                self.line_3d(&verts[i], &verts[i + 1], frame);
            }
            if kind == 5 {
                self.line_3d(&verts[n - 1], &verts[0], frame);
            }
        }
    }

    fn draw_points(&mut self, geometry: &crate::core::buffer_geometry::BufferGeometry, material: &Material, world: &Matrix4, frame: &Frame) {
        let position = match geometry.get_attribute("position") {
            Some(p) => p,
            None => return,
        };
        let pos = position.borrow();
        let mut colors = geometry.get_attribute("color");
        if !material.vertex_colors {
            colors = None;
        }
        let scale = frame.projection.elements[5] * self.height as f64 * 0.5;
        for i in 0..pos.count {
            let mut p = pos.get_vector3(i);
            p.apply_matrix4(world);
            p.apply_matrix4(&frame.view);
            if p.z > -frame.near {
                continue;
            }
            let mut c = material.color;
            if let Some(ca) = &colors {
                let v = ca.borrow().get_vector3(i);
                c = mul_color(&c, &Color::new(v.x, v.y, v.z));
            }
            c = self.fogged(&c, p.z, frame, material.fog);
            let s = self.to_screen(&p, frame);
            let size = if material.size_attenuation { material.size * scale / -p.z } else { material.size };
            let half = (size * 0.5).max(0.5);
            let x0 = (s.x - half).floor() as i64;
            let x1 = (s.x + half).ceil() as i64;
            let y0 = (s.y - half).floor() as i64;
            let y1 = (s.y + half).ceil() as i64;
            for y in y0.max(0)..y1.min(self.height) {
                for x in x0.max(0)..x1.min(self.width) {
                    self.plot(x, y, s.z, &c);
                }
            }
            self.points += 1;
        }
    }

    fn plot(&mut self, x: i64, y: i64, z: f64, c: &Color) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height || z < -1.0 || z > 1.0 {
            return;
        }
        let at = (y * self.width + x) as usize;
        if z >= self.depth[at] {
            return;
        }
        self.depth[at] = z;
        let p = at * 4;
        self.pixels[p] = to_byte(c.r);
        self.pixels[p + 1] = to_byte(c.g);
        self.pixels[p + 2] = to_byte(c.b);
        self.pixels[p + 3] = 255.0;
    }

    /// A line between two view-space vertices, clipped at the near plane.
    fn line_3d(&mut self, a_in: &Vert, b_in: &Vert, frame: &Frame) {
        let zn = -frame.near;
        let mut a = *a_in;
        let mut b = *b_in;
        if a.view.z > zn && b.view.z > zn {
            return;
        }
        if a.view.z > zn {
            let t = (zn - a.view.z) / (b.view.z - a.view.z);
            a = lerp_vert(&a, &b, t);
        } else if b.view.z > zn {
            let t = (zn - b.view.z) / (a.view.z - b.view.z);
            b = lerp_vert(&b, &a, t);
        }
        let sa = self.to_screen(&a.view, frame);
        let sb = self.to_screen(&b.view, frame);
        let dx = sb.x - sa.x;
        let dy = sb.y - sa.y;
        let steps = dx.abs().max(dy.abs()).ceil().max(1.0).min(4096.0) as i64;
        for s in 0..(steps + 1) {
            let t = s as f64 / steps as f64;
            let x = (sa.x + dx * t).floor() as i64;
            let y = (sa.y + dy * t).floor() as i64;
            let z = sa.z + (sb.z - sa.z) * t - 1e-5;
            let c = mix_color(&a.color, &b.color, t);
            self.plot(x, y, z, &c);
        }
        self.lines += 1;
    }

    fn fill_triangle(&mut self, a: &Vector3, b: &Vector3, c: &Vector3, ca: &Color, cb: &Color, cc: &Color, opacity: f64, blend: bool) {
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i64;
        let max_x = a.x.max(b.x).max(c.x).ceil().min(self.width as f64 - 1.0) as i64;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i64;
        let max_y = a.y.max(b.y).max(c.y).ceil().min(self.height as f64 - 1.0) as i64;
        let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        if area == 0.0 {
            return;
        }
        let mut y = min_y;
        while y <= max_y {
            let py = y as f64 + 0.5;
            let mut x = min_x;
            while x <= max_x {
                let px = x as f64 + 0.5;
                let w0 = ((b.x - px) * (c.y - py) - (c.x - px) * (b.y - py)) / area;
                let w1 = ((c.x - px) * (a.y - py) - (a.x - px) * (c.y - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let z = w0 * a.z + w1 * b.z + w2 * c.z;
                    let at = (y * self.width + x) as usize;
                    if z >= -1.0 && z <= 1.0 && z < self.depth[at] {
                        let p = at * 4;
                        let r = to_byte(w0 * ca.r + w1 * cb.r + w2 * cc.r);
                        let g = to_byte(w0 * ca.g + w1 * cb.g + w2 * cc.g);
                        let bl = to_byte(w0 * ca.b + w1 * cb.b + w2 * cc.b);
                        if blend {
                            self.pixels[p] = r * opacity + self.pixels[p] * (1.0 - opacity);
                            self.pixels[p + 1] = g * opacity + self.pixels[p + 1] * (1.0 - opacity);
                            self.pixels[p + 2] = bl * opacity + self.pixels[p + 2] * (1.0 - opacity);
                        } else {
                            self.depth[at] = z;
                            self.pixels[p] = r;
                            self.pixels[p + 1] = g;
                            self.pixels[p + 2] = bl;
                        }
                        self.pixels[p + 3] = 255.0;
                    }
                }
                x += 1;
            }
            y += 1;
        }
    }
}

/// The polygon `verts` cut at view-space z = `zn` (keeps z <= zn).
fn clip_near(verts: &Vec<Vert>, zn: f64) -> Vec<Vert> {
    let mut out: Vec<Vert> = Vec::new();
    let n = verts.len();
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let a_in = a.view.z <= zn;
        let b_in = b.view.z <= zn;
        if a_in {
            out.push(a);
        }
        if a_in != b_in {
            let t = (zn - a.view.z) / (b.view.z - a.view.z);
            out.push(lerp_vert(&a, &b, t));
        }
    }
    out
}
