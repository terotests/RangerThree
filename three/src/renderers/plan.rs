//! What a GPU backend needs for one frame, worked out on the CPU: the
//! camera matrices, the lights and fog packed as flat uniform arrays, each
//! material's shading inputs, and the draw list (opaque objects first,
//! then transparent ones back to front). The WebGL2 backend in the JS
//! facade only uploads buffers and issues draws from this, so the frame
//! logic stays in the crate and other targets can drive their own GPU API
//! with it.

use ranger::prelude::*;

use crate::core::object3d::{descendants, update_matrix_world, Background, ObjectKind, ObjectRef};
use crate::materials::material::{Material, MaterialKind, Side};
use crate::maths::color::Color;
use crate::maths::matrix4::Matrix4;
use crate::maths::vector3::Vector3;
use crate::renderers::soft::{collect_lights, surface_of};
use crate::scenes::fog::FogKind;
use crate::textures::texture::UV_MAPPING;

/// Floats per light in `RenderPlan::lights` (five vec4s).
pub const LIGHT_STRIDE: i64 = 20;
/// Floats per material in `material_uniforms` (five vec4s).
pub const MATERIAL_STRIDE: i64 = 24;

/// One object to draw.
#[derive(Clone)]
pub struct DrawItem {
    pub object: ObjectRef,
    /// 1 mesh, 2 instanced mesh, 3 line, 4 line segments, 5 line loop, 6 points, 7 sprite
    pub kind: i64,
    pub transparent: bool,
    /// view-space z of the object's origin (for sorting)
    pub depth: f64,
    /// the object's renderOrder, and that of the nearest group above it
    pub render_order: f64,
    pub group_order: f64,
}

#[derive(Clone)]
pub struct RenderPlan {
    pub view: Matrix4,
    pub projection: Matrix4,
    pub camera_position: Vector3,
    pub clear: Color,
    pub clear_alpha: f64,
    /// kind (0 none, 1 linear, 2 exponential), r, g, b, near, far, density, 0
    pub fog: Vec<f64>,
    /// `LIGHT_STRIDE` floats per light:
    /// color.rgb, kind | ground.rgb, intensity | position.xyz, distance |
    /// direction.xyz, decay | cos_outer, cos_inner, 0, 0
    pub lights: Vec<f64>,
    pub light_count: i64,
    pub items: Vec<DrawItem>,
}

fn draw_kind(o: &ObjectRef) -> i64 {
    match &o.borrow().kind {
        ObjectKind::Mesh(_) => 1,
        ObjectKind::InstancedMesh(_) => 2,
        ObjectKind::Line(_) => 3,
        ObjectKind::LineSegments(_) => 4,
        ObjectKind::LineLoop(_) => 5,
        ObjectKind::Points(_) => 6,
        ObjectKind::Sprite(_) => 7,
        _ => 0,
    }
}

/// three.js sorts by the renderOrder of the nearest enclosing Group first
fn group_order(o: &ObjectRef) -> f64 {
    let mut at = Some(o.clone());
    while let Some(p) = at {
        let is_group = match &p.borrow().kind {
            ObjectKind::Group => true,
            _ => false,
        };
        if is_group {
            return p.borrow().render_order;
        }
        at = match &p.borrow().parent {
            Some(w) => w.upgrade(),
            None => None,
        };
    }
    0.0
}

/// whether `a` is drawn after `b`: opaque by group order then render order
/// (otherwise in scene order), transparent also farthest first
fn after(a: &DrawItem, b: &DrawItem, transparent: bool) -> bool {
    if a.group_order != b.group_order {
        return a.group_order > b.group_order;
    }
    if a.render_order != b.render_order {
        return a.render_order > b.render_order;
    }
    transparent && a.depth > b.depth
}

fn visible_chain(o: &ObjectRef) -> bool {
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

/// The frame for `scene` seen from `camera`; `clear_color` is used when
/// the scene has no colour background.
pub fn plan_frame(scene: &ObjectRef, camera: &ObjectRef, clear_color: &Color, clear_alpha: f64) -> RenderPlan {
    update_matrix_world(scene, false);
    if camera.borrow().parent.is_none() {
        update_matrix_world(camera, false);
    }
    let mut clear = *clear_color;
    let mut clear_alpha = clear_alpha;
    let mut fog: Vec<f64> = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    if let ObjectKind::Scene(s) = &scene.borrow().kind {
        if let Background::Color(c) = &s.background {
            // three.js draws a colour background opaque
            clear = *c;
            clear_alpha = 1.0;
        }
        if let Some(f) = &s.fog {
            let kind = if f.kind == FogKind::Linear { 1.0 } else { 2.0 };
            fog = vec![kind, f.color.r, f.color.g, f.color.b, f.near, f.far, f.density, 0.0];
        }
    }
    let mut view = camera.borrow().matrix_world;
    view.invert();
    let mut projection = Matrix4::identity();
    if let Some(c) = camera.borrow().camera() {
        projection = c.projection_matrix;
    }
    let camera_position = camera.borrow().world_position();
    let objects = descendants(scene);
    let lights_in = collect_lights(&objects);
    let mut lights: Vec<f64> = Vec::new();
    for l in lights_in.iter() {
        lights.push(l.color.r);
        lights.push(l.color.g);
        lights.push(l.color.b);
        lights.push(l.kind as f64);
        lights.push(l.ground.r);
        lights.push(l.ground.g);
        lights.push(l.ground.b);
        lights.push(l.intensity);
        lights.push(l.position.x);
        lights.push(l.position.y);
        lights.push(l.position.z);
        lights.push(l.distance);
        lights.push(l.direction.x);
        lights.push(l.direction.y);
        lights.push(l.direction.z);
        lights.push(l.decay);
        lights.push(l.cos_outer);
        lights.push(l.cos_inner);
        lights.push(0.0);
        lights.push(0.0);
    }
    let mut opaque: Vec<DrawItem> = Vec::new();
    let mut transparent: Vec<DrawItem> = Vec::new();
    for o in objects.iter() {
        let kind = draw_kind(o);
        if kind == 0 || !visible_chain(o) {
            continue;
        }
        let mut is_transparent = false;
        let mut has_material = false;
        if let Some(d) = o.borrow().mesh_data() {
            if !d.materials.is_empty() {
                let m = d.materials[0].borrow();
                has_material = m.visible;
                is_transparent = m.transparent;
            }
        }
        if let ObjectKind::Sprite(sd) = &o.borrow().kind {
            let m = sd.material.borrow();
            has_material = m.visible;
            is_transparent = m.transparent;
        }
        if !has_material {
            continue;
        }
        let mut p = o.borrow().world_position();
        p.apply_matrix4(&view);
        let render_order = o.borrow().render_order;
        let item = DrawItem { object: o.clone(), kind, transparent: is_transparent, depth: p.z, render_order, group_order: group_order(o) };
        if is_transparent {
            transparent.push(item);
        } else {
            opaque.push(item);
        }
    }
    // stable insertion sorts; transparent: farthest (most negative z) first
    for i in 1..opaque.len() {
        let mut j = i;
        while j > 0 && after(&opaque[j - 1], &opaque[j], false) {
            opaque.swap(j - 1, j);
            j -= 1;
        }
    }
    for i in 1..transparent.len() {
        let mut j = i;
        while j > 0 && after(&transparent[j - 1], &transparent[j], true) {
            transparent.swap(j - 1, j);
            j -= 1;
        }
    }
    let mut items = opaque;
    for t in transparent.into_iter() {
        items.push(t);
    }
    RenderPlan {
        view,
        projection,
        camera_position,
        clear,
        clear_alpha,
        fog,
        light_count: lights_in.len() as i64,
        lights,
        items,
    }
}

/// A material's shading inputs as `MATERIAL_STRIDE` floats:
/// color.rgb, opacity | emissive.rgb, alpha_test |
/// specular.rgb, shininess | diffuse_scale, lit, specular_on, mode |
/// side (0 front, 1 back, 2 double), flat, vertex_colors, fog |
/// roughness, metalness (as set; the shader applies the maps and clamps),
/// physical (GGX), 0.
/// mode: 0 plain, 1 toon, 2 normal, 3 depth, 4 matcap.
pub fn material_uniforms(m: &Material) -> Vec<f64> {
    let s = surface_of(m);
    let mode = if m.kind == MaterialKind::MeshToon {
        1.0
    } else if m.kind == MaterialKind::MeshNormal {
        2.0
    } else if m.kind == MaterialKind::MeshDepth {
        3.0
    } else if m.kind == MaterialKind::MeshMatcap {
        4.0
    } else {
        0.0
    };
    let side = match m.side {
        Side::Front => 0.0,
        Side::Back => 1.0,
        Side::Double => 2.0,
    };
    vec![
        s.color.r,
        s.color.g,
        s.color.b,
        m.opacity,
        s.emissive.r,
        s.emissive.g,
        s.emissive.b,
        m.alpha_test,
        s.specular.r,
        s.specular.g,
        s.specular.b,
        s.shininess,
        s.diffuse_scale,
        if s.lit { 1.0 } else { 0.0 },
        if s.specular_on { 1.0 } else { 0.0 },
        mode,
        side,
        if m.flat_shading { 1.0 } else { 0.0 },
        if m.vertex_colors { 1.0 } else { 0.0 },
        if m.fog { 1.0 } else { 0.0 },
        m.roughness,
        m.metalness,
        if s.physical { 1.0 } else { 0.0 },
        0.0,
    ]
}

/// The 3x3 uv transform (column-major) of a material's map, identity when
/// there is none or it does not use uv mapping.
pub fn map_transform(m: &Material) -> Vec<f64> {
    let mut out: Vec<f64> = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    if let Some(t) = &m.map {
        let tb = t.borrow();
        if tb.mapping != UV_MAPPING {
            return out;
        }
        let mut mat = tb.matrix;
        if tb.matrix_auto_update {
            mat.set_uv_transform(tb.offset.x, tb.offset.y, tb.repeat.x, tb.repeat.y, tb.rotation, tb.center.x, tb.center.y);
        }
        for i in 0..9 {
            out[i] = mat.elements[i];
        }
    }
    out
}
