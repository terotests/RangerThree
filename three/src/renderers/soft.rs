//! A software renderer: draws a scene's meshes into an RGBA pixel buffer
//! with a depth buffer, flat shading, and ambient, hemisphere, directional,
//! point and spot lights as Lambert light. It is the first renderer the
//! playground and tests use; the GL backend comes later.

use ranger::prelude::*;

use crate::core::object3d::{descendants, update_matrix_world, Background, ObjectKind, ObjectRef};
use crate::materials::material::{MaterialKind, Side};
use crate::maths::color::{linear_to_srgb, Color};
use crate::maths::matrix4::Matrix4;
use crate::maths::vector3::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
struct LightIn {
    /// 0 ambient, 1 hemisphere, 2 directional, 3 point
    kind: i64,
    color: Color,
    ground: Color,
    intensity: f64,
    position: Vector3,
    direction: Vector3,
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
    /// Triangles drawn by the last render.
    pub triangles: i64,
}

fn to_byte(c: f64) -> f64 {
    let v = (linear_to_srgb(c.max(0.0).min(1.0)) * 255.0 + 0.5).floor();
    v.max(0.0).min(255.0)
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
        if let ObjectKind::Scene(s) = &scene.borrow().kind {
            if let Background::Color(c) = &s.background {
                clear = *c;
            }
        }
        let alpha = self.clear_alpha;
        self.clear(&clear, alpha);
        self.triangles = 0;

        let mut view = camera.borrow().matrix_world;
        view.invert();
        let projection = match camera.borrow().camera() {
            Some(c) => c.projection_matrix,
            None => Matrix4::identity(),
        };
        let mut view_projection = Matrix4::identity();
        view_projection.multiply_matrices(&projection, &view);

        let objects = descendants(scene);
        let lights = collect_lights(&objects);
        for o in objects.iter() {
            if !o.borrow().visible {
                continue;
            }
            let is_mesh = match &o.borrow().kind {
                ObjectKind::Mesh(_) => true,
                _ => false,
            };
            if is_mesh {
                self.draw_mesh(o, &view, &view_projection, &lights);
            }
        }
    }

    fn draw_mesh(&mut self, object: &ObjectRef, view: &Matrix4, view_projection: &Matrix4, lights: &Vec<LightIn>) {
        let o = object.borrow();
        let data = match o.mesh_data() {
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
        let position = match geometry.get_attribute("position") {
            Some(p) => p,
            None => return,
        };
        let pos = position.borrow();
        let index = geometry.get_index();
        let world = o.matrix_world;
        let count = match &index {
            Some(ix) => ix.borrow().count,
            None => pos.count,
        };
        let w = self.width as f64;
        let h = self.height as f64;
        let mut i = 0;
        while i + 2 < count {
            let mut ids: Vec<usize> = vec![i, i + 1, i + 2];
            if let Some(ix) = &index {
                let ib = ix.borrow();
                ids = vec![ib.get_x(i) as usize, ib.get_x(i + 1) as usize, ib.get_x(i + 2) as usize];
            }
            i += 3;
            let mut wp: Vec<Vector3> = Vec::new();
            let mut sp: Vec<Vector3> = Vec::new();
            let mut behind = false;
            for k in 0..3 {
                let mut p = pos.get_vector3(ids[k]);
                p.apply_matrix4(&world);
                let mut v = p;
                v.apply_matrix4(view);
                if v.z > -1e-6 {
                    behind = true;
                }
                let mut c = p;
                c.apply_matrix4(view_projection);
                sp.push(Vector3::new((c.x + 1.0) * 0.5 * w, (1.0 - c.y) * 0.5 * h, c.z));
                wp.push(p);
            }
            if behind {
                continue;
            }
            // screen-space winding: positive is counter-clockwise on screen (y down)
            let area = (sp[1].x - sp[0].x) * (sp[2].y - sp[0].y) - (sp[2].x - sp[0].x) * (sp[1].y - sp[0].y);
            let front = area < 0.0;
            if material.side == Side::Front && !front {
                continue;
            }
            if material.side == Side::Back && front {
                continue;
            }
            let mut e1 = wp[1];
            e1.sub(&wp[0]);
            let mut e2 = wp[2];
            e2.sub(&wp[0]);
            let mut n = Vector3::zero();
            n.cross_vectors(&e1, &e2);
            n.normalize();
            if !front {
                n.negate();
            }
            let mut centre = wp[0];
            centre.add(&wp[1]);
            centre.add(&wp[2]);
            centre.multiply_scalar(1.0 / 3.0);
            let color = shade(material.kind, &material.color, &material.emissive, material.emissive_intensity, &n, &centre, lights);
            self.fill_triangle(&sp[0], &sp[1], &sp[2], &color, material.opacity, material.transparent);
            self.triangles += 1;
        }
    }

    fn fill_triangle(&mut self, a: &Vector3, b: &Vector3, c: &Vector3, color: &Color, opacity: f64, transparent: bool) {
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i64;
        let max_x = a.x.max(b.x).max(c.x).ceil().min(self.width as f64 - 1.0) as i64;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i64;
        let max_y = a.y.max(b.y).max(c.y).ceil().min(self.height as f64 - 1.0) as i64;
        let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        if area == 0.0 {
            return;
        }
        let r = to_byte(color.r);
        let g = to_byte(color.g);
        let bl = to_byte(color.b);
        let blend = transparent && opacity < 1.0;
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
            ObjectKind::SpotLight(_) => 3,
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
        } else {
            direction.sub(&target);
        }
        direction.normalize();
        out.push(LightIn { kind, color: l.color, ground: l.ground_color, intensity: l.intensity, position, direction });
    }
    out
}

fn shade(kind: MaterialKind, color: &Color, emissive: &Color, emissive_intensity: f64, n: &Vector3, at: &Vector3, lights: &Vec<LightIn>) -> Color {
    match kind {
        MaterialKind::MeshNormal => {
            return Color::new(n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5);
        }
        MaterialKind::MeshBasic | MaterialKind::LineBasic | MaterialKind::LineDashed | MaterialKind::Points | MaterialKind::Sprite => {
            return *color;
        }
        _ => {}
    }
    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;
    for l in lights.iter() {
        let mut k = 0.0;
        let mut c = l.color;
        if l.kind == 0 {
            k = 1.0;
        } else if l.kind == 1 {
            let t = n.dot(&l.direction) * 0.5 + 0.5;
            c = Color::new(
                l.ground.r + (l.color.r - l.ground.r) * t,
                l.ground.g + (l.color.g - l.ground.g) * t,
                l.ground.b + (l.color.b - l.ground.b) * t,
            );
            k = 1.0;
        } else if l.kind == 2 {
            k = n.dot(&l.direction).max(0.0);
        } else {
            let mut d = l.position;
            d.sub(at);
            d.normalize();
            k = n.dot(&d).max(0.0);
        }
        r += c.r * l.intensity * k;
        g += c.g * l.intensity * k;
        b += c.b * l.intensity * k;
    }
    Color::new(
        color.r * r + emissive.r * emissive_intensity,
        color.g * g + emissive.g * emissive_intensity,
        color.b * b + emissive.b * emissive_intensity,
    )
}
