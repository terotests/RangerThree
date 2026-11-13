//! What a GPU backend needs for one frame, worked out on the CPU: the
//! camera matrices, the lights and fog packed as flat uniform arrays, each
//! material's shading inputs, and the draw list (opaque objects first,
//! then transparent ones back to front). The WebGL2 backend in the JS
//! facade only uploads buffers and issues draws from this, so the frame
//! logic stays in the crate and other targets can drive their own GPU API
//! with it.

use ranger::prelude::*;

use crate::core::object3d::{descendants, update_matrix_world, Background, ObjectKind, ObjectRef};
use crate::materials::material::{Blending, Material, MaterialKind, Side};
use crate::maths::color::Color;
use crate::maths::matrix4::Matrix4;
use crate::maths::math_utils::RAD2DEG;
use crate::maths::vector3::Vector3;
use crate::renderers::shaders::key_names;
use crate::renderers::soft::{collect_lights, surface_of};
use crate::scenes::fog::FogKind;
use crate::textures::texture::UV_MAPPING;

/// Floats per light in `RenderPlan::lights` (seven vec4s).
pub const LIGHT_STRIDE: i64 = 28;
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
    /// the id of the object's (first) material, and 2 for an instanced mesh: opaque draws
    /// are grouped by them so draws that share a program and its values follow each other
    pub material_id: i64,
    pub variant: i64,
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
    /// direction.xyz, decay | cos_outer, cos_inner, 0, 0 |
    /// half_width.xyz, 0 | half_height.xyz, 0 (the last two for rect area lights)
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

/// whether `a` is drawn after `b` (three.js's painterSortStable and
/// reversePainterSortStable): by group order, then render order; opaque then
/// by material and instancing, nearest first; transparent farthest first;
/// otherwise in scene order
fn after(a: &DrawItem, b: &DrawItem, transparent: bool) -> bool {
    if a.group_order != b.group_order {
        return a.group_order > b.group_order;
    }
    if a.render_order != b.render_order {
        return a.render_order > b.render_order;
    }
    if transparent {
        return a.depth > b.depth;
    }
    if a.material_id != b.material_id {
        return a.material_id > b.material_id;
    }
    if a.variant != b.variant {
        return a.variant > b.variant;
    }
    a.depth < b.depth
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
        lights.push(l.half_width.x);
        lights.push(l.half_width.y);
        lights.push(l.half_width.z);
        lights.push(0.0);
        lights.push(l.half_height.x);
        lights.push(l.half_height.y);
        lights.push(l.half_height.z);
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
        let mut material_id: i64 = 0;
        if let Some(d) = o.borrow().mesh_data() {
            if !d.materials.is_empty() {
                let m = d.materials[0].borrow();
                has_material = m.visible;
                is_transparent = m.transparent;
                material_id = m.id;
            }
        }
        if let ObjectKind::Sprite(sd) = &o.borrow().kind {
            let m = sd.material.borrow();
            has_material = m.visible;
            is_transparent = m.transparent;
            material_id = m.id;
        }
        if !has_material {
            continue;
        }
        let mut p = o.borrow().world_position();
        p.apply_matrix4(&view);
        let render_order = o.borrow().render_order;
        let variant: i64 = if kind == 2 { 2 } else { 0 };
        let item = DrawItem { object: o.clone(), kind, transparent: is_transparent, depth: p.z, render_order, group_order: group_order(o), material_id, variant };
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

fn side_value(s: Side) -> f64 {
    match s {
        Side::Front => 0.0,
        Side::Back => 1.0,
        Side::Double => 2.0,
    }
}

/// One shadow-casting light's shadow map: what to render it from and how
/// receivers read it (three.js's LightShadow.updateMatrices and the point
/// light's six faces).
#[derive(Clone)]
pub struct ShadowPlan {
    /// index of the light in `RenderPlan::lights`
    pub light: i64,
    /// the light's kind: 2 directional, 3 point, 4 spot
    pub kind: i64,
    pub map_width: f64,
    pub map_height: f64,
    pub bias: f64,
    pub normal_bias: f64,
    pub radius: f64,
    pub intensity: f64,
    pub blur_samples: i64,
    pub near: f64,
    pub far: f64,
    /// one view matrix, or six for a point light (+x, -x, +y, -y, +z, -z)
    pub views: Vec<Matrix4>,
    pub projection: Matrix4,
    /// world to shadow-map coordinates (0..1), or for a point light the
    /// translation to the light
    pub matrix: Matrix4,
}

/// The view matrix of a camera at `eye` looking at `target`.
fn view_towards(eye: &Vector3, target: &Vector3, up: &Vector3) -> Matrix4 {
    let mut world = Matrix4::identity();
    world.look_at(eye, target, up);
    world.set_position(eye.x, eye.y, eye.z);
    world.invert();
    world
}

/// The shadow maps the lights of `scene` need, in the order of the plan's
/// lights (call after `plan_frame`, which updates the world matrices).
pub fn shadow_plans(scene: &ObjectRef) -> Vec<ShadowPlan> {
    let objects = descendants(scene);
    let mut out: Vec<ShadowPlan> = Vec::new();
    let mut index: i64 = 0;
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
            ObjectKind::RectAreaLight(_) => 5,
            _ => -1,
        };
        if kind < 0 {
            continue;
        }
        let l = match ob.light() {
            Some(l) => l,
            None => continue,
        };
        let light_index = index;
        index += 1;
        if !ob.cast_shadow || kind < 2 || kind > 4 {
            continue;
        }
        let sh = match &l.shadow {
            Some(sh) => sh,
            None => continue,
        };
        let position = ob.world_position();
        let mut cam = match sh.camera.borrow().camera() {
            Some(c) => c.clone(),
            None => continue,
        };
        let mut views: Vec<Matrix4> = Vec::new();
        let mut matrix = Matrix4::identity();
        if kind == 3 {
            cam.fov = 90.0;
            cam.aspect = 1.0;
            if l.distance > 0.0 {
                cam.far = l.distance;
            }
            cam.update_perspective();
            let dirs: Vec<Vector3> = vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, -1.0),
            ];
            let ups: Vec<Vector3> = vec![
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, -1.0),
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
            ];
            for f in 0..6 {
                let mut target = position;
                target.add(&dirs[f]);
                views.push(view_towards(&position, &target, &ups[f]));
            }
            matrix.make_translation(-position.x, -position.y, -position.z);
        } else {
            let mut target = Vector3::zero();
            if let Some(t) = &l.target {
                target.set_from_matrix_position(&t.borrow().matrix_world);
            }
            if kind == 4 {
                cam.fov = RAD2DEG * 2.0 * l.angle;
                cam.aspect = sh.map_size.x / sh.map_size.y;
                if l.distance > 0.0 {
                    cam.far = l.distance;
                }
                cam.update_perspective();
            } else {
                cam.update_orthographic();
            }
            let view = view_towards(&position, &target, &Vector3::new(0.0, 1.0, 0.0));
            matrix.set(0.5, 0.0, 0.0, 0.5, 0.0, 0.5, 0.0, 0.5, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0, 1.0);
            let mut pv = Matrix4::identity();
            pv.multiply_matrices(&cam.projection_matrix, &view);
            matrix.multiply(&pv);
            views.push(view);
        }
        out.push(ShadowPlan {
            light: light_index,
            kind,
            map_width: sh.map_size.x,
            map_height: sh.map_size.y,
            bias: sh.bias,
            normal_bias: sh.normal_bias,
            radius: sh.radius,
            intensity: sh.intensity,
            blur_samples: sh.blur_samples,
            near: cam.near,
            far: cam.far,
            views,
            projection: cam.projection_matrix,
            matrix,
        });
    }
    out
}

/// A material's shading inputs as `MATERIAL_STRIDE` floats:
/// color.rgb, opacity | emissive.rgb, alpha_test |
/// specular.rgb, shininess | diffuse_scale, lit, specular_on, mode |
/// side (0 front, 1 back, 2 double), flat, vertex_colors, fog |
/// roughness, metalness (as set; the shader applies the maps and clamps),
/// physical (GGX), shadow side (-1 none, else as side).
/// mode: 0 plain, 1 toon, 2 normal, 3 depth, 4 matcap, 5 shadow.
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
    } else if m.kind == MaterialKind::Shadow {
        5.0
    } else {
        0.0
    };
    let side = side_value(m.side);
    let shadow_side = match m.shadow_side {
        Some(Side::Front) => 0.0,
        Some(Side::Back) => 1.0,
        Some(Side::Double) => 2.0,
        None => -1.0,
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
        shadow_side,
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

/// What decides a draw's program variant besides its material: the
/// resources bound for it, the object, and the frame. Both GPU backends fill
/// one and take the key from `program_key`, so they always agree.
#[derive(Clone)]
pub struct VariantInputs {
    pub has_map: bool,
    /// points sample the map across each point (no uv attribute)
    pub points_uv: bool,
    pub has_normal: bool,
    /// u_maps: bit per extra map bound (1 alpha, 2 roughness, 4 metalness, 8 normal, 16 ao, 32 matcap, 64 gradient, 128 bump)
    pub maps: i64,
    pub has_emap: bool,
    pub instanced: bool,
    pub has_disp: bool,
    pub sprite: bool,
    /// the object receives shadows and the frame has shadow maps
    pub receive: bool,
    /// the frame has a transmission texture and the material is transmissive
    pub transmission: bool,
    /// 0 none, 1 prefiltered (CubeUV), 2 equirect
    pub env_mode: i64,
    /// the material's alphaToCoverage (a JavaScript-side property)
    pub alpha_to_coverage: bool,
    pub shadow_pass: bool,
    /// the frame: tone mapping operator, linear output, fog kind (0 none, 1 linear, 2 exp2),
    /// bit per shadow slot it fills, light counts per kind
    pub tone: i64,
    pub linear_out: bool,
    pub fog_kind: i64,
    pub sh_slots: i64,
    pub light_counts: Vec<i64>,
}

impl VariantInputs {
    pub fn new() -> VariantInputs {
        VariantInputs {
            has_map: false,
            points_uv: false,
            has_normal: false,
            maps: 0,
            has_emap: false,
            instanced: false,
            has_disp: false,
            sprite: false,
            receive: false,
            transmission: false,
            env_mode: 0,
            alpha_to_coverage: false,
            shadow_pass: false,
            tone: 0,
            linear_out: false,
            fog_kind: 0,
            sh_slots: 0,
            light_counts: vec![0, 0, 0, 0, 0, 0],
        }
    }
}

fn flag(b: bool) -> i64 {
    if b {
        1
    } else {
        0
    }
}

/// The program variant of a draw, in `shaders::key_names` order (`mu`: the
/// material's `material_uniforms`). A shadow caster keeps only what decides
/// its discards.
pub fn program_key(m: &Material, mu: &Vec<f64>, v: &VariantInputs) -> Vec<i64> {
    let dashed = m.kind == MaterialKind::LineDashed;
    let opaque = !m.transparent && m.blending == Blending::Normal && !v.alpha_to_coverage;
    if v.shadow_pass {
        let mut k: Vec<i64> = vec![0; key_names().len()];
        k[0] = flag(v.has_map);
        k[1] = flag(v.points_uv);
        k[3] = flag(opaque);
        k[5] = v.maps & 1;
        k[9] = flag(dashed);
        k[11] = flag(v.instanced);
        k[12] = flag(v.has_disp);
        k[23] = 1;
        return k;
    }
    let physical = mu[22] > 0.5;
    let lit = mu[13] > 0.5;
    let mut k: Vec<i64> = vec![
        flag(v.has_map),
        flag(v.points_uv),
        flag(v.has_normal),
        flag(opaque),
        if m.tone_mapped { v.tone } else { 0 },
        v.maps,
        flag(v.linear_out),
        flag(m.premultiplied_alpha),
        flag(v.has_emap),
        flag(dashed),
        flag(v.receive),
        flag(v.instanced),
        flag(v.has_disp),
        flag(v.sprite),
        mu[15] as i64,
        flag(lit),
        flag(mu[14] > 0.5),
        flag(physical),
        flag(mu[17] > 0.5),
        flag(physical && m.sheen > 0.0),
        flag(physical && v.transmission),
        flag(v.light_counts[5] > 0),
        if mu[19] > 0.5 { v.fog_kind } else { 0 },
        0,
        v.env_mode,
        if v.receive { v.sh_slots } else { 0 },
    ];
    for i in 0..6 {
        k.push(if lit { v.light_counts[i] } else { 0 });
    }
    k
}
