//! A frame as GPU commands (gpu.rs), worked out in the crate: the same
//! programs, uniforms, state and draw order as the WebGL2 backend
//! (js/three-webgl.js), so any executor (WebGL2, OpenGL ES 3 / GL 3.3,
//! later WebGPU) draws the same pixels without frame logic of its own.
//!
//! Covered so far: meshes, instanced meshes, lines and points with every
//! material kind the main program shades and its texture maps; all light
//! kinds except rect area lights; fog, tone mapping, output colour space,
//! blending, sides, wireframe, draw ranges and groups. A frame that needs
//! more (an environment, shadow maps, transmission, sprites, dashed lines,
//! morph targets, a texture background) is reported in `GpuFrame::missing`,
//! and the host draws it with the WebGL2 backend until those parts move
//! here too.
//!
//! Texture pixels come from the host, which decodes images its own way (the
//! browser's image and canvas decoders, a native PNG decoder, a canvas 2D
//! raster): `textures_to_upload` names the textures whose pixels are not
//! here at their current version, `set_texture_image` hands them over
//! (RGBA8, rows from the top), and `upload_images` does both for textures
//! whose `image` the crate holds (Rust hosts).

use ranger::prelude::*;

use crate::core::object3d::{Background, ObjectKind, ObjectRef};
use crate::materials::material::{Blending, Material, MaterialKind, MaterialRef};
use crate::maths::color::{linear_to_srgb, Color};
use crate::maths::matrix4::Matrix4;
use crate::renderers::dfg_table::dfg_table;
use crate::renderers::gpu::{GpuCommands, U_FLOAT, U_MAT3, U_MAT4, U_VEC2, U_VEC3, U_VEC4};
use crate::renderers::plan::{material_uniforms, map_transform, plan_frame, shadow_plans, DrawItem, LIGHT_STRIDE};
use crate::renderers::shaders::{key_names, main_fragment, main_vertex, uniform_names, MAX_LIGHTS};
use crate::core::object3d::visible_descendants;
use crate::maths::color::ColorSpace;
use crate::textures::texture::TextureRef;

// GL constants the stream carries
const GL_POINTS: i64 = 0;
const GL_LINES: i64 = 1;
const GL_LINE_LOOP: i64 = 2;
const GL_LINE_STRIP: i64 = 3;
const GL_TRIANGLES: i64 = 4;
const GL_ZERO: i64 = 0;
const GL_ONE: i64 = 1;
const GL_ONE_MINUS_SRC_COLOR: i64 = 769;
const GL_SRC_ALPHA: i64 = 770;
const GL_ONE_MINUS_SRC_ALPHA: i64 = 771;
const GL_DST_COLOR: i64 = 774;
const GL_FUNC_ADD: i64 = 32774;
const GL_LEQUAL: i64 = 515;
const GL_LINEAR: i64 = 9729;
const GL_CLAMP: i64 = 33071;

/// What the host's renderer is set to (three.js's WebGLRenderer fields).
#[derive(Clone)]
pub struct GpuSettings {
    pub width: i64,
    pub height: i64,
    pub pixel_ratio: f64,
    /// three.js tone mapping: 0 none, 1 linear, 2 Reinhard, 3 Cineon, 4 ACES, 6 AgX, 7 neutral
    pub tone_mapping: i64,
    pub exposure: f64,
    /// outputColorSpace "srgb-linear"
    pub output_linear: bool,
    pub clear_color: Color,
    pub clear_alpha: f64,
    /// shadowMap.enabled
    pub shadows: bool,
}

impl GpuSettings {
    pub fn new(width: i64, height: i64) -> GpuSettings {
        GpuSettings {
            width,
            height,
            pixel_ratio: 1.0,
            tone_mapping: 0,
            exposure: 1.0,
            output_linear: false,
            clear_color: Color::new(0.0, 0.0, 0.0),
            clear_alpha: 1.0,
            shadows: false,
        }
    }
}

/// One frame's result: the commands, what they could not draw, and three.js's render info.
#[derive(Clone)]
pub struct GpuFrame {
    pub commands: GpuCommands,
    /// features the frame needs that the command renderer does not draw yet (empty: complete)
    pub missing: Vec<String>,
    pub calls: i64,
    pub triangles: i64,
    pub lines: i64,
    pub points: i64,
}

#[derive(Clone)]
struct BufferEntry {
    id: i64,
    version: i64,
    length: i64,
}

/// A texture made on the GPU: its id there and the upload it holds.
#[derive(Clone)]
struct TextureEntry {
    id: i64,
    key: String,
}

/// Pixels the host handed over for a texture.
#[derive(Clone)]
struct HostImage {
    version: i64,
    width: i64,
    height: i64,
    data: Vec<u8>,
}

#[derive(Clone)]
struct ProgramEntry {
    id: i64,
    frame: i64,
}

/// Keeps the GPU objects the commands made (by id) between frames.
#[derive(Clone)]
pub struct GpuRenderer {
    next_id: i64,
    frame: i64,
    buffers: Map<String, BufferEntry>,
    programs: Map<String, ProgramEntry>,
    textures: Map<i64, TextureEntry>,
    /// the next id given to an object, geometry or texture that has none (0)
    auto_id: i64,
    images: Map<i64, HostImage>,
    /// the extra maps' uv transforms, 6 mat3 (u_t)
    map_t: Vec<f64>,
    names: Vec<String>,
    dfg: i64,
    shadow_flat: i64,
    shadow_cube: i64,
    /// blend factors last set (three.js keeps them for subtractive/multiply without premultiplied alpha)
    last_blend: Vec<i64>,
    // frame values
    lights: Vec<f64>,
    light_counts: Vec<i64>,
    fog_kind: i64,
    frame_floats: Vec<f64>,
    frame_ints: Vec<i64>,
}

impl Default for GpuRenderer {
    fn default() -> GpuRenderer {
        GpuRenderer::new()
    }
}

/// The inverse transpose of a 4x4 matrix's upper 3x3 (column-major).
fn normal_matrix(e: &[f64; 16]) -> Vec<f64> {
    let a = e[0];
    let b = e[1];
    let c = e[2];
    let d = e[4];
    let f = e[5];
    let g = e[6];
    let h = e[8];
    let i = e[9];
    let j = e[10];
    let aa = f * j - g * i;
    let bb = g * h - d * j;
    let cc = d * i - f * h;
    let det = a * aa + b * bb + c * cc;
    let k = if det == 0.0 { 0.0 } else { 1.0 / det };
    vec![
        aa * k,
        bb * k,
        cc * k,
        (c * i - b * j) * k,
        (a * j - c * h) * k,
        (b * h - a * i) * k,
        (b * g - c * f) * k,
        (c * d - a * g) * k,
        (a * f - b * d) * k,
    ]
}

fn elements(m: &Matrix4) -> Vec<f64> {
    let mut v: Vec<f64> = Vec::new();
    for i in 0..16 {
        v.push(m.elements[i]);
    }
    v
}

fn clamp01(x: f64) -> f64 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// The textures `m` samples (the command renderer draws none yet).
/// The maps the main program samples (`material_map` k), in this order: map, emissive map,
/// then the extra maps of `EXTRA_MAPS`, then the displacement map. Light
/// and specular maps are not drawn by the WebGL2 backend either.
const MAP_COUNT: i64 = 11;

/// Ids this renderer gives start here, far above the JS facade's counters.
const AUTO_ID_BASE: i64 = 1000000000000;

fn material_map(m: &Material, k: i64) -> Option<TextureRef> {
    match k {
        0 => m.map.clone(),
        1 => m.emissive_map.clone(),
        2 => m.alpha_map.clone(),
        3 => m.roughness_map.clone(),
        4 => m.metalness_map.clone(),
        5 => m.normal_map.clone(),
        6 => m.ao_map.clone(),
        7 => m.matcap.clone(),
        8 => m.gradient_map.clone(),
        9 => m.bump_map.clone(),
        10 => m.displacement_map.clone(),
        _ => None,
    }
}

/// The extra maps (material_maps 2..9): texture unit, bit in u_maps, slot
/// in u_t (-1 none). Matcap, gradient and bump maps share units with maps
/// their materials cannot have at once.
const EXTRA_UNITS: [i64; 8] = [3, 4, 5, 6, 7, 4, 5, 6];
const EXTRA_BITS: [i64; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
const EXTRA_SLOTS: [i64; 8] = [0, 1, 2, 3, 4, -1, -1, 5];

/// A texture's uv transform (its matrix, recomputed when it updates itself).
fn texture_matrix(t: &TextureRef) -> Vec<f64> {
    let tb = t.borrow();
    let mut mat = tb.matrix;
    if tb.matrix_auto_update {
        mat.set_uv_transform(tb.offset.x, tb.offset.y, tb.repeat.x, tb.repeat.y, tb.rotation, tb.center.x, tb.center.y);
    }
    let mut out: Vec<f64> = Vec::new();
    for i in 0..9 {
        out.push(mat.elements[i]);
    }
    out
}

/// three.js filter and wrap constants as GL's.
fn gl_filter(f: i64) -> i64 {
    if f == 1003 {
        9728
    } else if f == 1004 {
        9984
    } else if f == 1005 {
        9986
    } else if f == 1007 {
        9985
    } else if f == 1008 {
        9987
    } else {
        9729
    }
}

fn gl_wrap(w: i64) -> i64 {
    if w == 1000 {
        10497
    } else if w == 1002 {
        33648
    } else {
        33071
    }
}

impl GpuRenderer {
    pub fn new() -> GpuRenderer {
        GpuRenderer {
            next_id: 1,
            frame: 0,
            buffers: Map::new(),
            programs: Map::new(),
            textures: Map::new(),
            auto_id: AUTO_ID_BASE,
            images: Map::new(),
            map_t: vec![0.0; 54],
            names: uniform_names(),
            dfg: 0,
            shadow_flat: 0,
            shadow_cube: 0,
            last_blend: vec![GL_FUNC_ADD, GL_FUNC_ADD, GL_ONE, GL_ZERO, GL_ONE, GL_ZERO],
            lights: Vec::new(),
            light_counts: vec![0, 0, 0, 0, 0, 0],
            fog_kind: 0,
            frame_floats: Vec::new(),
            frame_ints: Vec::new(),
        }
    }

    fn id(&mut self) -> i64 {
        let v = self.next_id;
        self.next_id += 1;
        v
    }

    /// The position of uniform `name` in the main program's names.
    fn loc(&self, name: &str) -> i64 {
        for i in 0..self.names.len() {
            if self.names[i] == name {
                return i as i64;
            }
        }
        -1
    }

    /// The textures every frame binds: the DFG table (unit 2) and the
    /// depth textures that stand in for shadow maps (units 8, 9, 13, 15).
    fn ensure_fixed(&mut self, c: &mut GpuCommands) {
        if self.dfg != 0 {
            return;
        }
        self.dfg = self.id();
        c.texture(self.dfg, &vec![0, 33327, 33319, 5126, 16, 16, GL_LINEAR, GL_LINEAR, GL_CLAMP, GL_CLAMP, 0], &dfg_table());
        self.shadow_flat = self.id();
        c.texture(self.shadow_flat, &vec![0, 33190, 6402, 5125, 1, 1, GL_LINEAR, GL_LINEAR, GL_CLAMP, GL_CLAMP, 1], &Vec::new());
        self.shadow_cube = self.id();
        c.texture(self.shadow_cube, &vec![1, 33190, 6402, 5125, 1, 1, GL_LINEAR, GL_LINEAR, GL_CLAMP, GL_CLAMP, 1], &Vec::new());
    }

    /// The buffer for `key`, (re)uploaded when its version or length changed.
    fn buffer(&mut self, c: &mut GpuCommands, key: &str, version: i64, data: &Vec<f64>, index: bool, dynamic: bool) -> i64 {
        let length = data.len() as i64;
        let mut id: i64 = 0;
        if let Some(e) = self.buffers.get(key) {
            if e.version == version && e.length == length {
                return e.id;
            }
            id = e.id;
        }
        if id == 0 {
            id = self.id();
        }
        c.buffer(id, index, dynamic, data);
        self.buffers.insert(key.to_string(), BufferEntry { id, version, length });
        id
    }

    /// The main program of a variant, made on first use; sets the frame's
    /// uniforms when the program has not had them this frame.
    fn use_main(&mut self, c: &mut GpuCommands, vals: &Vec<i64>) {
        let mut key = String::from("1");
        for v in vals.iter() {
            key.push_str(",");
            key.push_str(&v.to_string());
        }
        let mut id: i64 = 0;
        let mut seen: i64 = -1;
        if let Some(e) = self.programs.get(&key) {
            id = e.id;
            seen = e.frame;
        }
        if id == 0 {
            id = self.id();
            c.program(id, &main_vertex(vals), &main_fragment(1, vals), &self.names);
            c.use_program(id);
            self.set_units(c);
        } else {
            c.use_program(id);
        }
        if seen != self.frame {
            self.frame_uniforms(c);
        }
        self.programs.insert(key, ProgramEntry { id, frame: self.frame });
    }

    /// The samplers' texture units (as three-webgl.js setUnits).
    /// Hands over a texture's pixels: RGBA8, `width` x `height`, rows from the top.
    pub fn set_texture_image(&mut self, texture_id: i64, version: i64, width: i64, height: i64, data: Vec<u8>) {
        self.images.insert(texture_id, HostImage { version, width, height, data });
    }

    /// The version of the pixels held for a texture id, -1 when none.
    pub fn texture_image_version(&self, texture_id: i64) -> i64 {
        match self.images.get(&texture_id) {
            Some(im) => im.version,
            None => -1,
        }
    }

    /// Forgets a texture's pixels and its GPU texture (the host disposed it).
    pub fn remove_texture(&mut self, c: &mut GpuCommands, texture_id: i64) {
        self.images.remove(&texture_id);
        let mut gpu_id: i64 = 0;
        if let Some(e) = self.textures.get(&texture_id) {
            gpu_id = e.id;
        }
        if gpu_id != 0 {
            self.textures.remove(&texture_id);
            c.release(gpu_id);
        }
    }

    /// Gives an id to every object, geometry and map texture under `scene`
    /// that has none (0), since the caches key on them. The JS facade
    /// numbers its own as three.js does; objects made in Rust start at 0,
    /// so Rust hosts call this (through `upload_images`) before `render`.
    pub fn assign_ids(&mut self, scene: &ObjectRef) {
        for o in visible_descendants(scene).iter() {
            if o.borrow().id == 0 {
                o.borrow_mut().id = self.auto_id;
                self.auto_id += 1;
            }
            let ob = o.borrow();
            let data = match &ob.kind {
                ObjectKind::Mesh(d) => d.clone(),
                ObjectKind::InstancedMesh(d) => d.clone(),
                ObjectKind::Points(d) => d.clone(),
                ObjectKind::Line(d) => d.clone(),
                ObjectKind::LineSegments(d) => d.clone(),
                ObjectKind::LineLoop(d) => d.clone(),
                _ => continue,
            };
            if data.geometry.borrow().id == 0 {
                data.geometry.borrow_mut().id = self.auto_id;
                self.auto_id += 1;
            }
            for m in data.materials.iter() {
                for k in 0..MAP_COUNT {
                    if let Some(t) = material_map(&m.borrow(), k) {
                        if t.borrow().id == 0 {
                            t.borrow_mut().id = self.auto_id;
                            self.auto_id += 1;
                        }
                    }
                }
            }
        }
    }

    /// The textures of the scene's visible materials whose pixels are not
    /// held at their current version; the host decodes each and calls
    /// `set_texture_image` before `render`.
    pub fn textures_to_upload(&self, scene: &ObjectRef) -> Vec<TextureRef> {
        let mut out: Vec<TextureRef> = Vec::new();
        let mut seen: Vec<i64> = Vec::new();
        for o in visible_descendants(scene).iter() {
            let ob = o.borrow();
            let data = match &ob.kind {
                ObjectKind::Mesh(d) => d.clone(),
                ObjectKind::InstancedMesh(d) => d.clone(),
                ObjectKind::Points(d) => d.clone(),
                ObjectKind::Line(d) => d.clone(),
                ObjectKind::LineSegments(d) => d.clone(),
                ObjectKind::LineLoop(d) => d.clone(),
                _ => continue,
            };
            for m in data.materials.iter() {
                for k in 0..MAP_COUNT {
                    if let Some(t) = material_map(&m.borrow(), k) {
                        let id = t.borrow().id;
                        let version = t.borrow().version;
                        if !seen.contains(&id) {
                            seen.push(id);
                            if self.texture_image_version(id) != version {
                                out.push(t.clone());
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// For Rust hosts, before each `render`: numbers what has no id
    /// (`assign_ids`) and hands over the pixels of every texture
    /// `textures_to_upload` names from the texture's own `image`.
    pub fn upload_images(&mut self, scene: &ObjectRef) {
        self.assign_ids(scene);
        for t in self.textures_to_upload(scene).iter() {
            let tb = t.borrow();
            if let Some(im) = &tb.image {
                let img = im.borrow();
                self.set_texture_image(tb.id, tb.version, img.width, img.height, img.data.clone());
            }
        }
    }

    /// Binds texture `t` to `unit`, uploading it when its pixels or
    /// parameters changed; false when its pixels are not here (the map is
    /// then left out, as the WebGL2 backend does for an image still loading).
    fn texture(&mut self, c: &mut GpuCommands, t: &TextureRef, unit: i64) -> bool {
        let tb = t.borrow();
        let mut width: i64 = 0;
        let mut height: i64 = 0;
        let mut version: i64 = 0;
        let mut found = false;
        if let Some(im) = self.images.get(&tb.id) {
            width = im.width;
            height = im.height;
            version = im.version;
            found = true;
        }
        if !found {
            return false;
        }
        let srgb = tb.color_space == ColorSpace::Srgb;
        let mip = tb.min_filter != 1003 && tb.min_filter != 1006 && tb.generate_mipmaps;
        let mut min = gl_filter(tb.min_filter);
        if !mip {
            min = if min == 9728 || min == 9984 || min == 9986 { 9728 } else { 9729 };
        }
        let mag = if tb.mag_filter == 1003 { 9728 } else { 9729 };
        let params: Vec<i64> = vec![
            0,
            if srgb { 35907 } else { 6408 },
            6408,
            5121,
            width,
            height,
            min,
            mag,
            gl_wrap(tb.wrap_s),
            gl_wrap(tb.wrap_t),
            if mip { 2 } else { 0 },
        ];
        let mut key = version.to_string() + if tb.flip_y { ":f" } else { ":-" } + if tb.premultiply_alpha { "p" } else { "-" };
        for p in params.iter() {
            key = key + ":" + &p.to_string();
        }
        let mut id: i64 = 0;
        let mut same = false;
        if let Some(e) = self.textures.get(&tb.id) {
            id = e.id;
            same = e.key == key;
        }
        if id == 0 {
            id = self.id();
        }
        if !same {
            let mut rows: Vec<u8> = Vec::new();
            if let Some(im) = self.images.get(&tb.id) {
                rows = texture_rows(im, tb.flip_y, tb.premultiply_alpha);
            }
            c.texture_bytes(id, &params, &rows);
            self.textures.insert(tb.id, TextureEntry { id, key });
        }
        c.bind(unit, id, false);
        true
    }

    fn set_units(&self, c: &mut GpuCommands) {
        let units: Vec<String> = vec![
            String::from("u_map"),
            String::from("u_emap"),
            String::from("u_dfg"),
            String::from("u_alphaMap"),
            String::from("u_roughMap"),
            String::from("u_metalMap"),
            String::from("u_normalMap"),
            String::from("u_aoMap"),
            String::from("u_matcap"),
            String::from("u_gradMap"),
            String::from("u_bumpMap"),
            String::from("u_dispMap"),
            String::from("u_ltc"),
            String::from("u_envMap"),
            String::from("u_transMap"),
            String::from("u_sh0"),
            String::from("u_sh1"),
            String::from("u_sh2"),
            String::from("u_shc0"),
        ];
        let at: Vec<i64> = vec![0, 1, 2, 3, 4, 5, 6, 7, 4, 5, 6, 12, 10, 11, 14, 8, 9, 13, 15];
        for i in 0..units.len() {
            c.uniform_int(self.loc(&units[i]), at[i]);
        }
    }

    /// The frame's uniforms, recorded in `frame_floats` / `frame_ints` by `render`.
    fn frame_uniforms(&self, c: &mut GpuCommands) {
        let f = &self.frame_floats;
        let mut v: Vec<f64> = Vec::new();
        for i in 0..16 {
            v.push(f[i]);
        }
        c.uniform(self.loc("u_view"), U_MAT4, &v);
        v = Vec::new();
        for i in 16..32 {
            v.push(f[i]);
        }
        c.uniform(self.loc("u_viewInv"), U_MAT4, &v);
        v = Vec::new();
        for i in 32..48 {
            v.push(f[i]);
        }
        c.uniform(self.loc("u_proj"), U_MAT4, &v);
        c.uniform(self.loc("u_cam"), U_VEC3, &vec![f[48], f[49], f[50]]);
        c.uniform(self.loc("u_lights"), U_VEC4, &self.lights);
        c.uniform_int(self.loc("u_nlights"), self.frame_ints[0]);
        c.uniform(self.loc("u_fog"), U_VEC4, &vec![f[51], f[52], f[53], f[54], f[55], f[56], f[57], f[58]]);
        c.uniform(self.loc("u_pointScale"), U_FLOAT, &vec![f[59]]);
        c.uniform(self.loc("u_exposure"), U_FLOAT, &vec![f[60]]);
    }

    /// `scene` from `camera` as commands.
    pub fn render(&mut self, scene: &ObjectRef, camera: &ObjectRef, s: &GpuSettings) -> GpuFrame {
        let mut c = GpuCommands::new();
        let mut missing: Vec<String> = Vec::new();
        self.frame += 1;
        let plan = plan_frame(scene, camera, &s.clear_color, s.clear_alpha);
        let mut override_mat: Option<MaterialRef> = None;
        if let ObjectKind::Scene(sd) = &scene.borrow().kind {
            if let Background::Texture(_) = &sd.background {
                missing.push(String::from("background texture"));
            }
            if sd.environment.is_some() {
                missing.push(String::from("scene environment"));
            }
            if let Some(m) = &sd.override_material {
                override_mat = Some(m.clone());
            }
        }
        if s.shadows && !shadow_plans(scene).is_empty() {
            missing.push(String::from("shadow maps"));
        }
        self.ensure_fixed(&mut c);
        let w = s.width;
        let h = s.height;
        let cl = plan.clear;
        let rgba = if s.output_linear {
            vec![clamp01(cl.r), clamp01(cl.g), clamp01(cl.b), plan.clear_alpha]
        } else {
            vec![linear_to_srgb(clamp01(cl.r)), linear_to_srgb(clamp01(cl.g)), linear_to_srgb(clamp01(cl.b)), plan.clear_alpha]
        };
        c.pass(0, w, h, 3, &rgba, 1.0);
        // shadow-map stand-ins (shadow type 1) and the DFG table
        c.bind(8, self.shadow_flat, false);
        c.bind(9, self.shadow_flat, false);
        c.bind(13, self.shadow_flat, false);
        c.bind(15, self.shadow_cube, true);
        c.bind(2, self.dfg, false);
        // the frame's uniforms
        let mut ff: Vec<f64> = elements(&plan.view);
        let mut inv = plan.view;
        inv.invert();
        for x in elements(&inv) {
            ff.push(x);
        }
        for x in elements(&plan.projection) {
            ff.push(x);
        }
        ff.push(plan.camera_position.x);
        ff.push(plan.camera_position.y);
        ff.push(plan.camera_position.z);
        for i in 0..8 {
            ff.push(plan.fog[i]);
        }
        ff.push((h as f64) * 0.5);
        ff.push(s.exposure);
        self.frame_floats = ff;
        // lights packed by kind (stable), so each kind's loop reads a contiguous range
        let n = if plan.light_count < MAX_LIGHTS { plan.light_count } else { MAX_LIGHTS };
        let stride = LIGHT_STRIDE as usize;
        let mut lights: Vec<f64> = vec![0.0; (MAX_LIGHTS as usize) * stride];
        let mut counts: Vec<i64> = vec![0, 0, 0, 0, 0, 0];
        let mut at: usize = 0;
        for kind in 0..6 {
            for i in 0..(n as usize) {
                let lk = plan.lights[i * stride + 3].round() as i64;
                if lk == kind {
                    for j in 0..stride {
                        lights[at * stride + j] = plan.lights[i * stride + j];
                    }
                    at += 1;
                    counts[kind as usize] += 1;
                }
            }
        }
        if counts[5] > 0 {
            missing.push(String::from("rect area lights"));
        }
        self.lights = lights;
        self.light_counts = counts;
        self.frame_ints = vec![n];
        self.fog_kind = if plan.fog[0] > 1.5 {
            2
        } else if plan.fog[0] > 0.5 {
            1
        } else {
            0
        };
        // three.js's lists: opaque, transmissive (back to front), transparent
        let mut opaque: Vec<DrawItem> = Vec::new();
        let mut transparent: Vec<DrawItem> = Vec::new();
        for it in plan.items.iter() {
            let mut transmissive = false;
            if let Some(m) = first_material(&it.object) {
                transmissive = m.borrow().transmission > 0.0;
            }
            if transmissive {
                missing.push(String::from("transmission"));
            } else if it.transparent {
                transparent.push(it.clone());
            } else {
                opaque.push(it.clone());
            }
        }
        let mut frame = GpuFrame { commands: GpuCommands::new(), missing: Vec::new(), calls: 0, triangles: 0, lines: 0, points: 0 };
        for it in opaque.iter() {
            self.draw(&mut c, &mut frame, it, &override_mat, s);
        }
        for it in transparent.iter() {
            self.draw(&mut c, &mut frame, it, &override_mat, s);
        }
        for m in frame.missing.iter() {
            missing.push(m.clone());
        }
        frame.missing = dedup(missing);
        frame.commands = c;
        frame
    }

    /// One draw-list item: each geometry group with its own material when the
    /// object has a material array, the scene's override material when set.
    fn draw(&mut self, c: &mut GpuCommands, frame: &mut GpuFrame, item: &DrawItem, override_mat: &Option<MaterialRef>, s: &GpuSettings) {
        let o = item.object.borrow();
        let data = match &o.kind {
            ObjectKind::Mesh(d) => d.clone(),
            ObjectKind::InstancedMesh(d) => d.clone(),
            ObjectKind::Points(d) => d.clone(),
            ObjectKind::Line(d) => d.clone(),
            ObjectKind::LineSegments(d) => d.clone(),
            ObjectKind::LineLoop(d) => d.clone(),
            ObjectKind::Sprite(_) => {
                frame.missing.push(String::from("sprites"));
                return;
            }
            _ => return,
        };
        let groups = data.geometry.borrow().groups.clone();
        if data.material_is_array {
            for g in groups.iter() {
                let mi = g.material_index as usize;
                if mi < data.materials.len() {
                    let m = data.materials[mi].clone();
                    if m.borrow().visible {
                        let use_m = match override_mat {
                            Some(om) => om.clone(),
                            None => m,
                        };
                        self.draw_with(c, frame, item, &data, &use_m, g.start, g.count, true, s);
                    }
                }
            }
        } else if !data.materials.is_empty() {
            let m = data.materials[0].clone();
            let use_m = match override_mat {
                Some(om) => om.clone(),
                None => m,
            };
            self.draw_with(c, frame, item, &data, &use_m, 0, 0, false, s);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_with(
        &mut self,
        c: &mut GpuCommands,
        frame: &mut GpuFrame,
        item: &DrawItem,
        data: &crate::core::object3d::MeshData,
        material: &MaterialRef,
        group_start: i64,
        group_count: i64,
        grouped: bool,
        s: &GpuSettings,
    ) {
        let m = material.borrow().clone();
        let o = item.object.borrow();
        let kind = item.kind;
        let mesh = kind == 1 || kind == 2;
        if m.env_map.is_some() {
            frame.missing.push(String::from("environment map"));
            return;
        }
        if m.kind == MaterialKind::LineDashed {
            frame.missing.push(String::from("dashed lines"));
            return;
        }
        if !data.morph_target_influences.is_empty() {
            frame.missing.push(String::from("morph targets"));
            return;
        }
        if m.kind == MaterialKind::Shader || m.kind == MaterialKind::RawShader {
            frame.missing.push(String::from("shader materials"));
            return;
        }
        let geometry = data.geometry.borrow();
        let gid = geometry.id.to_string();
        let position = match geometry.get_attribute("position") {
            Some(p) => p,
            None => return,
        };
        let mu = material_uniforms(&m);
        // attributes
        let pos = position.borrow();
        let pkey = gid.clone() + ":position";
        let pbuf = self.buffer(c, &pkey, pos.version, &pos.array, false, pos.usage == 35048);
        let psize = if pos.item_size < 3 { pos.item_size as i64 } else { 3 };
        c.attrib(0, pbuf, psize, (pos.item_size as i64) * 4, 0, 0, &Vec::new());
        let mut has_normal = false;
        if mesh && mu[17] < 0.5 {
            if let Some(nattr) = geometry.get_attribute("normal") {
                let na = nattr.borrow();
                let key = gid.clone() + ":normal";
                let b = self.buffer(c, &key, na.version, &na.array, false, na.usage == 35048);
                let size = if na.item_size < 3 { na.item_size as i64 } else { 3 };
                c.attrib(1, b, size, (na.item_size as i64) * 4, 0, 0, &Vec::new());
                has_normal = true;
            }
        }
        if !has_normal {
            c.attrib(1, 0, 3, 0, 0, 0, &vec![0.0, 0.0, 1.0, 0.0]);
        }
        let has_uv = geometry.get_attribute("uv").is_some();
        match geometry.get_attribute("uv") {
            Some(u) => {
                let ua = u.borrow();
                let key = gid.clone() + ":uv";
                let b = self.buffer(c, &key, ua.version, &ua.array, false, ua.usage == 35048);
                let size = if ua.item_size < 2 { ua.item_size as i64 } else { 2 };
                c.attrib(2, b, size, (ua.item_size as i64) * 4, 0, 0, &Vec::new());
            }
            None => c.attrib(2, 0, 2, 0, 0, 0, &vec![0.0, 0.0, 0.0, 0.0]),
        }
        let mut colored = false;
        if mu[18] > 0.5 {
            if let Some(col) = geometry.get_attribute("color") {
                let ca = col.borrow();
                let key = gid.clone() + ":color";
                let b = self.buffer(c, &key, ca.version, &ca.array, false, ca.usage == 35048);
                let size = if ca.item_size < 3 { ca.item_size as i64 } else { 3 };
                c.attrib(3, b, size, (ca.item_size as i64) * 4, 0, 0, &Vec::new());
                colored = true;
            }
        }
        if !colored {
            c.attrib(3, 0, 3, 0, 0, 0, &vec![1.0, 1.0, 1.0, 1.0]);
        }
        c.attrib(9, 0, 1, 0, 0, 0, &vec![0.0, 0.0, 0.0, 0.0]);
        let mut instances: i64 = 0;
        let oid = o.id.to_string();
        if kind == 2 {
            if let Some(im) = &data.instance_matrix {
                let ia = im.borrow();
                let key = oid.clone() + ":instanceMatrix";
                let b = self.buffer(c, &key, ia.version, &ia.array, false, ia.usage == 35048);
                for col in 0..4 {
                    c.attrib(4 + col, b, 4, 64, col * 16, 1, &Vec::new());
                }
                instances = data.count;
                match &data.instance_color {
                    Some(ic) => {
                        let ca = ic.borrow();
                        let key2 = oid.clone() + ":instanceColor";
                        let b2 = self.buffer(c, &key2, ca.version, &ca.array, false, ca.usage == 35048);
                        c.attrib(8, b2, 3, 12, 0, 1, &Vec::new());
                    }
                    None => c.attrib(8, 0, 3, 0, 0, 0, &vec![1.0, 1.0, 1.0, 1.0]),
                }
            }
        }
        if instances == 0 {
            for col in 4..8 {
                c.attrib(col, 0, 4, 0, 0, 0, &vec![0.0, 0.0, 0.0, 1.0]);
            }
            c.attrib(8, 0, 3, 0, 0, 0, &vec![1.0, 1.0, 1.0, 1.0]);
        }
        // texture maps
        let mut disp_t: Vec<f64> = Vec::new();
        let mut has_disp = false;
        if mesh {
            if let Some(t) = &material_map(&m, 10) {
                has_disp = self.texture(c, t, 12);
                if has_disp {
                    disp_t = texture_matrix(t);
                }
            }
        }
        let mut has_map = false;
        if let Some(t) = &material_map(&m, 0) {
            has_map = self.texture(c, t, 0);
        }
        let mut has_emap = false;
        let mut emap_t: Vec<f64> = Vec::new();
        if let Some(t) = &material_map(&m, 1) {
            has_emap = self.texture(c, t, 1);
            if has_emap {
                emap_t = texture_matrix(t);
            }
        }
        let mut bits: i64 = 0;
        for k in 0..8 {
            if let Some(t) = &material_map(&m, (k + 2) as i64) {
                if EXTRA_BITS[k] == 128 && m.normal_map.is_some() {
                    continue;
                }
                if !self.texture(c, t, EXTRA_UNITS[k]) {
                    continue;
                }
                bits |= EXTRA_BITS[k];
                if EXTRA_SLOTS[k] >= 0 {
                    let tm = texture_matrix(t);
                    for j in 0..9 {
                        self.map_t[(EXTRA_SLOTS[k] as usize) * 9 + j] = tm[j];
                    }
                }
            }
        }
        let points_uv = if kind == 6 && !has_uv { 1 } else { 0 };
        let opaque = !m.transparent && m.blending == Blending::Normal;
        let physical = if mu[22] > 0.5 { 1 } else { 0 };
        let sheen_on = if physical == 1 && m.sheen > 0.0 { 1 } else { 0 };
        let lit = mu[13] > 0.5;
        // the program variant, in key_names order
        let mut vals: Vec<i64> = vec![
            if has_map { 1 } else { 0 },
            points_uv,
            if has_normal { 1 } else { 0 },
            if opaque { 1 } else { 0 },
            if m.tone_mapped { s.tone_mapping } else { 0 },
            bits,
            if s.output_linear { 1 } else { 0 },
            if m.premultiplied_alpha { 1 } else { 0 },
            if has_emap { 1 } else { 0 },
            0,
            0,
            if instances > 0 { 1 } else { 0 },
            if has_disp { 1 } else { 0 },
            0,
            mu[15] as i64,
            if lit { 1 } else { 0 },
            if mu[14] > 0.5 { 1 } else { 0 },
            physical,
            if mu[17] > 0.5 { 1 } else { 0 },
            sheen_on,
            0,
            0,
            if mu[19] > 0.5 { self.fog_kind } else { 0 },
            0,
            0,
        ];
        for k in 0..6 {
            vals.push(if lit { self.light_counts[k] } else { 0 });
        }
        if vals.len() != key_names().len() {
            frame.missing.push(String::from("program key"));
            return;
        }
        self.use_main(c, &vals);
        c.uniform(self.loc("u_mat"), U_VEC4, &mu);
        c.uniform(self.loc("u_env"), U_VEC4, &vec![0.0, 0.0, 0.0, 0.0]);
        c.uniform(self.loc("u_model"), U_MAT4, &elements(&o.matrix_world));
        c.uniform(self.loc("u_normalMat"), U_MAT3, &normal_matrix(&o.matrix_world.elements));
        c.uniform(self.loc("u_uvt"), U_MAT3, &map_transform(&m));
        c.uniform(self.loc("u_dashScale"), U_FLOAT, &vec![0.0]);
        if has_disp {
            c.uniform(self.loc("u_dispT"), U_MAT3, &disp_t);
            c.uniform(self.loc("u_dispScale"), U_FLOAT, &vec![m.displacement_scale]);
            c.uniform(self.loc("u_dispBias"), U_FLOAT, &vec![m.displacement_bias]);
        }
        c.uniform_int(self.loc("u_hasDisp"), if has_disp { 1 } else { 0 });
        c.uniform(self.loc("u_bumpScale"), U_FLOAT, &vec![m.bump_scale]);
        if sheen_on == 1 {
            let sr = if m.sheen_roughness < 0.0001 {
                0.0001
            } else if m.sheen_roughness > 1.0 {
                1.0
            } else {
                m.sheen_roughness
            };
            c.uniform(self.loc("u_sheen"), U_VEC4, &vec![m.sheen_color.r * m.sheen, m.sheen_color.g * m.sheen, m.sheen_color.b * m.sheen, sr]);
        } else {
            c.uniform(self.loc("u_sheen"), U_VEC4, &vec![0.0, 0.0, 0.0, 0.0]);
        }
        c.uniform(self.loc("u_trans"), U_VEC4, &vec![0.0, 0.0, 0.0, 0.0]);
        c.uniform_int(self.loc("u_instanced"), if instances > 0 { 1 } else { 0 });
        c.uniform_int(self.loc("u_sprite"), 0);
        if has_emap {
            c.uniform(self.loc("u_euvt"), U_MAT3, &emap_t);
        }
        if bits != 0 {
            let mt = self.map_t.clone();
            c.uniform(self.loc("u_t"), U_MAT3, &mt);
        }
        let flip = if mu[16] == 1.0 { -1.0 } else { 1.0 };
        c.uniform(self.loc("u_normalScale"), U_VEC2, &vec![m.normal_scale.x * flip, m.normal_scale.y * flip]);
        c.uniform(self.loc("u_aoIntensity"), U_FLOAT, &vec![m.ao_map_intensity]);
        c.uniform(self.loc("u_pointSize"), U_FLOAT, &vec![m.size * s.pixel_ratio]);
        c.uniform_int(self.loc("u_sizeAtt"), if m.size_attenuation { 1 } else { 0 });
        // state
        let ccw = o.matrix_world.determinant() >= 0.0;
        let blend = self.blend_state(&m, opaque);
        // the range: draw range, intersected with the group
        let index = geometry.index.clone();
        let total: i64 = match &index {
            Some(ix) => ix.borrow().count as i64,
            None => pos.count as i64,
        };
        let mut start = geometry.draw_range.start as i64;
        let mut end: i64 = if geometry.draw_range.count >= 1.0e15 { total } else { start + geometry.draw_range.count as i64 };
        if grouped {
            if group_start > start {
                start = group_start;
            }
            if group_start + group_count < end {
                end = group_start + group_count;
            }
        }
        if start < 0 {
            start = 0;
        }
        if end > total {
            end = total;
        }
        if end <= start {
            return;
        }
        let mode = if kind == 3 {
            GL_LINE_STRIP
        } else if kind == 4 {
            GL_LINES
        } else if kind == 5 {
            GL_LINE_LOOP
        } else if kind == 6 {
            GL_POINTS
        } else {
            GL_TRIANGLES
        };
        let side = mu[16] as i64;
        let mut passes: Vec<i64> = vec![side];
        if mesh && side == 2 && m.transparent {
            passes = vec![1, 0];
        }
        let wire = mesh && m.wireframe;
        let mut index_buf: i64 = 0;
        let mut wire_count: i64 = 0;
        if wire {
            let mut lines: Vec<f64> = Vec::new();
            let mut ver: i64 = -1;
            let mut i: i64 = 0;
            while i + 2 < total {
                let (a, b, cc) = match &index {
                    Some(ix) => {
                        let arr = &ix.borrow().array;
                        (arr[i as usize], arr[(i + 1) as usize], arr[(i + 2) as usize])
                    }
                    None => (i as f64, (i + 1) as f64, (i + 2) as f64),
                };
                lines.push(a);
                lines.push(b);
                lines.push(b);
                lines.push(cc);
                lines.push(cc);
                lines.push(a);
                i += 3;
            }
            if let Some(ix) = &index {
                ver = ix.borrow().version;
            }
            wire_count = lines.len() as i64;
            let key = gid.clone() + ":wire:" + &total.to_string();
            index_buf = self.buffer(c, &key, ver, &lines, true, false);
        } else if let Some(ix) = &index {
            let ia = ix.borrow();
            let key = gid.clone() + ":index";
            index_buf = self.buffer(c, &key, ia.version, &ia.array, true, false);
        }
        let each = if instances > 1 { instances } else { 1 };
        let drawn = end - start;
        for p in passes.iter() {
            let cull = if !mesh || *p == 2 {
                0
            } else if *p == 1 {
                2
            } else {
                1
            };
            c.state(
                &vec![
                    blend[0],
                    blend[1],
                    blend[2],
                    blend[3],
                    blend[4],
                    blend[5],
                    blend[6],
                    if m.depth_test { 1 } else { 0 },
                    GL_LEQUAL,
                    if m.depth_write { 1 } else { 0 },
                    cull,
                    if ccw { 1 } else { 0 },
                    if m.color_write { 1 } else { 0 },
                    if m.polygon_offset { 1 } else { 0 },
                ],
                &vec![0.0, 0.0, 0.0, 0.0, m.polygon_offset_factor, m.polygon_offset_units],
            );
            if wire {
                let mut nw = drawn * 2;
                if wire_count - start * 2 < nw {
                    nw = wire_count - start * 2;
                }
                c.draw(GL_LINES, start * 2, nw, instances, index_buf);
                frame.lines += (nw / 2) * each;
            } else {
                c.draw(mode, start, drawn, instances, index_buf);
                if mode == GL_TRIANGLES {
                    frame.triangles += (drawn / 3) * each;
                } else if mode == GL_POINTS {
                    frame.points += drawn;
                } else if mode == GL_LINES {
                    frame.lines += drawn / 2;
                } else {
                    let l = if mode == GL_LINE_LOOP { drawn } else { drawn - 1 };
                    frame.lines += if l > 0 { l } else { 0 };
                }
            }
            frame.calls += 1;
        }
    }

    /// three.js's setBlending as the STATE block's first seven values.
    fn blend_state(&mut self, m: &Material, opaque: bool) -> Vec<i64> {
        if m.blending == Blending::None || opaque {
            let mut v = vec![0];
            for x in self.last_blend.iter() {
                v.push(*x);
            }
            return v;
        }
        let mut f: Vec<i64> = self.last_blend.clone();
        f[0] = GL_FUNC_ADD;
        f[1] = GL_FUNC_ADD;
        if m.premultiplied_alpha {
            if m.blending == Blending::Normal {
                f = vec![GL_FUNC_ADD, GL_FUNC_ADD, GL_ONE, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA];
            } else if m.blending == Blending::Additive {
                f = vec![GL_FUNC_ADD, GL_FUNC_ADD, GL_ONE, GL_ONE, GL_ONE, GL_ONE];
            } else if m.blending == Blending::Subtractive {
                f = vec![GL_FUNC_ADD, GL_FUNC_ADD, GL_ZERO, GL_ONE_MINUS_SRC_COLOR, GL_ZERO, GL_ONE];
            } else if m.blending == Blending::Multiply {
                f = vec![GL_FUNC_ADD, GL_FUNC_ADD, GL_DST_COLOR, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE];
            }
        } else if m.blending == Blending::Normal {
            f = vec![GL_FUNC_ADD, GL_FUNC_ADD, GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA];
        } else if m.blending == Blending::Additive {
            f = vec![GL_FUNC_ADD, GL_FUNC_ADD, GL_SRC_ALPHA, GL_ONE, GL_ONE, GL_ONE];
        }
        self.last_blend = f.clone();
        let mut v = vec![1];
        for x in f.iter() {
            v.push(*x);
        }
        v
    }
}

fn first_material(o: &ObjectRef) -> Option<MaterialRef> {
    match &o.borrow().kind {
        ObjectKind::Mesh(d) => {
            if !d.material_is_array && !d.materials.is_empty() {
                Some(d.materials[0].clone())
            } else {
                None
            }
        }
        ObjectKind::InstancedMesh(d) => {
            if !d.material_is_array && !d.materials.is_empty() {
                Some(d.materials[0].clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The host's rows (top first) as GL takes them (bottom first) when the
/// texture flips, as WebGL's UNPACK_FLIP_Y; alpha premultiplied when asked.
fn texture_rows(img: &HostImage, flip_y: bool, premultiply: bool) -> Vec<u8> {
    let w = img.width as usize;
    let h = img.height as usize;
    let row = w * 4;
    let mut out: Vec<u8> = vec![0; row * h];
    for y in 0..h {
        let src = if flip_y { (h - 1 - y) * row } else { y * row };
        for x in 0..row {
            if src + x < img.data.len() {
                out[y * row + x] = img.data[src + x];
            }
        }
    }
    if premultiply {
        let mut i = 0;
        while i + 3 < out.len() {
            let a = out[i + 3] as f64 / 255.0;
            for k in 0..3 {
                out[i + k] = (out[i + k] as f64 * a).round() as u8;
            }
            i += 4;
        }
    }
    out
}

fn dedup(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v.iter() {
        let mut found = false;
        for t in out.iter() {
            if t == s {
                found = true;
            }
        }
        if !found {
            out.push(s.clone());
        }
    }
    out
}
