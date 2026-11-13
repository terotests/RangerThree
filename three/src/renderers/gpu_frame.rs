//! A frame as GPU commands (gpu.rs), worked out in the crate: the same
//! programs, uniforms, state and draw order as the WebGL2 backend
//! (js/three-webgl.js), so any executor (WebGL2, OpenGL ES 3 / GL 3.3,
//! later WebGPU) draws the same pixels without frame logic of its own.
//!
//! Covered: every playground example and the test scenes of
//! tests/gpu-compare.mjs, pixel for pixel: meshes, instanced meshes,
//! sprites, lines (dashed too) and points with every material kind the
//! main program shades and their texture maps; all light kinds with rect
//! area lights; shadow maps (PCF, basic, VSM, point light cubes); scene and
//! material environments (prefiltered as three.js's PMREM, or equirect);
//! texture and blurred backgrounds; transmission; fog, tone mapping, output
//! colour space, blending, sides, wireframe, draw ranges and groups; morph
//! targets (positions and normals, blended on the CPU when the influences
//! change); ShaderMaterial and RawShaderMaterial (gpu_shader.rs: three.js's
//! prefixes, GLSL 1 or 3, float/vec/mat/int/bool/sampler2D uniforms and
//! arrays, up to 6 own vertex attributes) against three.js r185. What a
//! frame needs beyond that (shader chunk `#include`s, samplerCube or integer
//! vector uniforms, instanced own attributes) is reported in
//! `GpuFrame::missing`, and the host draws such a frame another way.
//!
//! Texture pixels come from the host, which decodes images its own way (the
//! browser's image and canvas decoders, a native PNG decoder, a canvas 2D
//! raster): `textures_to_upload` names the textures whose pixels are not
//! here at their current version, `set_texture_image` hands them over
//! (RGBA8, rows from the top), and `upload_images` does both for textures
//! whose `image` the crate holds (Rust hosts).

use ranger::prelude::*;

use crate::core::object3d::{Background, MeshData, ObjectKind, ObjectRef};
use crate::materials::material::{Blending, Material, MaterialKind, MaterialRef, Side};
use crate::maths::color::{linear_to_srgb, Color};
use crate::maths::matrix4::Matrix4;
use crate::renderers::dfg_table::dfg_table;
use crate::renderers::ltc_table::{ltc_1, ltc_2};
use crate::renderers::gpu::{GpuCommands, U_FLOAT, U_MAT3, U_MAT4, U_VEC2, U_VEC3, U_VEC4};
use crate::renderers::gpu_shader::{includes, shader_sources, uniform_declarations, CUSTOM_ATTRIB_BASE, MAX_ATTRIBS};
use crate::renderers::plan::{material_uniforms, map_transform, plan_frame, shadow_plans, DrawItem, RenderPlan, LIGHT_STRIDE};
use crate::renderers::shaders::{background_fs, full_vs, key_names, main_fragment, main_vertex, pmrem_fs, uniform_names, vsm_blur_fs, MAX_LIGHTS};
use crate::core::object3d::visible_descendants;
use crate::maths::color::ColorSpace;
use crate::maths::euler::{Euler, EulerOrder};
use crate::textures::texture::{EQUIRECTANGULAR_REFLECTION_MAPPING, EQUIRECTANGULAR_REFRACTION_MAPPING};
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
    /// half-float textures can be drawn into (WebGL2: EXT_color_buffer_float);
    /// the prefiltered environment is kept in RGBA16F then, else RGBA8
    pub half_float: bool,
    /// shadowMap.type: 0 basic, 1 PCF, 2 PCF soft (drawn as PCF), 3 VSM
    pub shadow_type: i64,
    /// float textures filter (WebGL2: OES_texture_float_linear): the rect
    /// area light tables are RGBA32F then, else RGBA16F
    pub float_linear: bool,
    /// multisampling of the transmission target (three.js: min(4, MAX_SAMPLES))
    pub samples: i64,
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
            half_float: false,
            shadow_type: 1,
            float_linear: false,
            samples: 4,
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

/// A shader material's program: uniform names and GLSL types by location,
/// and its own attributes from CUSTOM_ATTRIB_BASE.
#[derive(Clone)]
struct ShaderProgram {
    id: i64,
    names: Vec<String>,
    types: Vec<String>,
    attribs: Vec<String>,
}

/// A morphed attribute's buffer and what it was blended from.
#[derive(Clone)]
struct MorphEntry {
    id: i64,
    key: String,
}

/// Texture units a shader material's samplers take: the per-draw map
/// units, never the ones a frame binds once (DFG 2, shadows, LTC 10).
const SHADER_UNITS: [i64; 9] = [0, 1, 3, 4, 5, 6, 7, 11, 12];

/// Column-major a * b.
fn mul4(a: &Vec<f64>, b: &Vec<f64>) -> Vec<f64> {
    let mut out: Vec<f64> = vec![0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut v = 0.0;
            for k in 0..4 {
                v += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = v;
        }
    }
    out
}

/// The textures `m` reads: its maps and its shader uniforms' samplers.
fn material_textures(m: &Material) -> Vec<TextureRef> {
    let mut out: Vec<TextureRef> = Vec::new();
    for k in 0..MAP_COUNT {
        if let Some(t) = material_map(m, k) {
            out.push(t);
        }
    }
    for u in m.shader_uniforms.iter() {
        if let Some(t) = &u.texture {
            out.push(t.clone());
        }
    }
    out
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

/// A prefiltered environment (three.js's PMREM) of an equirectangular
/// texture: the levels (each a 3 x 2 atlas of cube faces) and all of them
/// in one CubeUV atlas.
#[derive(Clone)]
struct EnvEntry {
    version: i64,
    levels: Vec<i64>,
    sizes: Vec<i64>,
    lod_max: i64,
    atlas: i64,
    aw: i64,
    ah: i64,
}

/// A shadow slot's GPU objects (2D slots 0-2, the point light's cube 3):
/// the depth texture and its targets (six faces for a cube), and for VSM
/// the blur's two colour textures and targets.
#[derive(Clone)]
struct ShadowRes {
    w: i64,
    h: i64,
    kind: i64,
    point: bool,
    depth: i64,
    targets: Vec<i64>,
    pass: i64,
    pass_t: i64,
    map: i64,
    map_t: i64,
}

fn no_shadow_res() -> ShadowRes {
    ShadowRes { w: 0, h: 0, kind: 0, point: false, depth: 0, targets: Vec::new(), pass: 0, pass_t: 0, map: 0, map_t: 0 }
}

/// A frame's texture background, for passes that draw it again.
#[derive(Clone)]
struct BackgroundUse {
    tex: Option<TextureRef>,
    blur: f64,
    intensity: f64,
    rotation: Euler,
}

/// The environment a draw uses: its texture, whether it is prefiltered
/// (standard and physical materials) and whether it is the scene's.
#[derive(Clone)]
struct EnvUse {
    tex: TextureRef,
    pbr: bool,
    scene: bool,
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
    shader_progs: Map<String, ShaderProgram>,
    morphs: Map<String, MorphEntry>,
    textures: Map<i64, TextureEntry>,
    /// the next id given to an object, geometry or texture that has none (0)
    auto_id: i64,
    images: Map<i64, HostImage>,
    /// the extra maps' uv transforms, 6 mat3 (u_t)
    map_t: Vec<f64>,
    envs: Map<i64, EnvEntry>,
    pm_prog: i64,
    bg_prog: i64,
    /// the scene being drawn's environment, its intensity and rotation, the tone mapping
    scene_env: Option<TextureRef>,
    scene_env_intensity: f64,
    scene_env_rotation: Euler,
    /// u_envUV, u_env and u_envRot of the draw's environment (env_bind)
    env_vals: Vec<f64>,
    /// the frame uniforms' version: programs that saw an older one get them again
    fver: i64,
    /// shadow map type of the frame's programs (1 PCF, 2 basic, 3 VSM) and the pass being drawn (0: the screen)
    stype: i64,
    shadow_pass: i64,
    has_shadows: bool,
    /// per slot; depth 0: not made yet
    shadow_res: Vec<ShadowRes>,
    /// u_shMat, u_shParam, u_shSize (empty until a frame had shadows)
    shadow_mat: Vec<f64>,
    shadow_param: Vec<f64>,
    shadow_size: Vec<f64>,
    vsm_prog: i64,
    /// the rect area light tables (renderers/ltc_table.rs) side by side, made on first use
    ltc: i64,
    /// transmission: the multisampled target, its resolve target and
    /// mipmapped texture, their size; the texture draws sample (0 none),
    /// u_transSize once set, and the side the back-face pass forces (-1 none)
    trans_ms: i64,
    trans_res: i64,
    trans_store: i64,
    trans_w: i64,
    trans_h: i64,
    trans_tex: i64,
    trans_size: Vec<f64>,
    draw_side: i64,
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
/// then the extra maps (`EXTRA_UNITS`), the displacement map and the
/// environment map. Light and specular maps are not drawn by the WebGL2
/// backend either.
const MAP_COUNT: i64 = 12;

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
        11 => m.env_map.clone(),
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

/// three.js's roughnessToMip: roughness -> level of the prefiltered environment.
fn roughness_to_mip(r: f64) -> f64 {
    if r >= 0.8 {
        (1.0 - r) * (-1.0 + 2.0) / 0.2 - 2.0
    } else if r >= 0.4 {
        (0.8 - r) * (2.0 + 1.0) / 0.4 - 1.0
    } else if r >= 0.305 {
        (0.4 - r) * (3.0 - 2.0) / (0.4 - 0.305) + 2.0
    } else if r >= 0.21 {
        (0.305 - r) * (4.0 - 3.0) / (0.305 - 0.21) + 3.0
    } else {
        -2.0 * (1.16 * r).log2()
    }
}

fn is_equirect(t: &TextureRef) -> bool {
    let m = t.borrow().mapping;
    m == EQUIRECTANGULAR_REFLECTION_MAPPING || m == EQUIRECTANGULAR_REFRACTION_MAPPING
}

/// The rotation matrix of an Euler as its 3x3 (column-major), or transposed.
fn euler_mat3(e: &Euler, transposed: bool) -> Vec<f64> {
    let mut m = Matrix4::identity();
    m.make_rotation_from_euler(e);
    let x = m.elements;
    if transposed {
        vec![x[0], x[4], x[8], x[1], x[5], x[9], x[2], x[6], x[10]]
    } else {
        vec![x[0], x[1], x[2], x[4], x[5], x[6], x[8], x[9], x[10]]
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
            shader_progs: Map::new(),
            morphs: Map::new(),
            textures: Map::new(),
            auto_id: AUTO_ID_BASE,
            images: Map::new(),
            map_t: vec![0.0; 54],
            envs: Map::new(),
            pm_prog: 0,
            bg_prog: 0,
            scene_env: None,
            scene_env_intensity: 1.0,
            scene_env_rotation: Euler::new(0.0, 0.0, 0.0, EulerOrder::Xyz),
            env_vals: Vec::new(),
            fver: 0,
            stype: 1,
            shadow_pass: 0,
            has_shadows: false,
            shadow_res: vec![no_shadow_res(), no_shadow_res(), no_shadow_res(), no_shadow_res()],
            shadow_mat: Vec::new(),
            shadow_param: Vec::new(),
            shadow_size: Vec::new(),
            vsm_prog: 0,
            ltc: 0,
            trans_ms: 0,
            trans_res: 0,
            trans_store: 0,
            trans_w: 0,
            trans_h: 0,
            trans_tex: 0,
            trans_size: Vec::new(),
            draw_side: -1,
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
        let mut key = self.stype.to_string();
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
            c.program(id, &main_vertex(vals), &main_fragment(self.stype, vals), &self.names);
            c.use_program(id);
            self.set_units(c);
        } else {
            c.use_program(id);
        }
        if seen != self.fver {
            self.frame_uniforms(c);
        }
        self.programs.insert(key, ProgramEntry { id, frame: self.fver });
    }

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

    /// The scene's background and environment textures.
    fn scene_textures(&self, scene: &ObjectRef) -> Vec<TextureRef> {
        let mut out: Vec<TextureRef> = Vec::new();
        if let ObjectKind::Scene(sd) = &scene.borrow().kind {
            if let Background::Texture(t) = &sd.background {
                out.push(t.clone());
            }
            if let Some(t) = &sd.environment {
                out.push(t.clone());
            }
        }
        out
    }

    /// Gives an id to every object, geometry and map texture under `scene`
    /// that has none (0), since the caches key on them. The JS facade
    /// numbers its own as three.js does; objects made in Rust start at 0,
    /// so Rust hosts call this (through `upload_images`) before `render`.
    pub fn assign_ids(&mut self, scene: &ObjectRef) {
        for t in self.scene_textures(scene).iter() {
            if t.borrow().id == 0 {
                t.borrow_mut().id = self.auto_id;
                self.auto_id += 1;
            }
        }
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
                ObjectKind::Sprite(sd) => MeshData::new(sd.geometry.clone(), sd.material.clone()),
                _ => continue,
            };
            if data.geometry.borrow().id == 0 {
                data.geometry.borrow_mut().id = self.auto_id;
                self.auto_id += 1;
            }
            for m in data.materials.iter() {
                for t in material_textures(&m.borrow()).iter() {
                    if t.borrow().id == 0 {
                        t.borrow_mut().id = self.auto_id;
                        self.auto_id += 1;
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
        for t in self.scene_textures(scene).iter() {
            let id = t.borrow().id;
            seen.push(id);
            if self.texture_image_version(id) != t.borrow().version {
                out.push(t.clone());
            }
        }
        for o in visible_descendants(scene).iter() {
            let ob = o.borrow();
            let data = match &ob.kind {
                ObjectKind::Mesh(d) => d.clone(),
                ObjectKind::InstancedMesh(d) => d.clone(),
                ObjectKind::Points(d) => d.clone(),
                ObjectKind::Line(d) => d.clone(),
                ObjectKind::LineSegments(d) => d.clone(),
                ObjectKind::LineLoop(d) => d.clone(),
                ObjectKind::Sprite(sd) => MeshData::new(sd.geometry.clone(), sd.material.clone()),
                _ => continue,
            };
            for m in data.materials.iter() {
                for t in material_textures(&m.borrow()).iter() {
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

    /// Full-screen passes: no attributes (the programs use gl_VertexID), no
    /// blending, depth or culling.
    fn fullscreen_state(&self, c: &mut GpuCommands) {
        for loc in 0..10 {
            c.attrib(loc, 0, 4, 0, 0, 0, &vec![0.0, 0.0, 0.0, 1.0]);
        }
        c.state(&vec![0, GL_FUNC_ADD, GL_FUNC_ADD, GL_ONE, GL_ZERO, GL_ONE, GL_ZERO, 0, GL_LEQUAL, 0, 0, 1, 1, 0], &vec![0.0; 6]);
    }

    /// The environment material `m` uses (three-webgl.js envOf).
    fn env_of(&self, m: &Material) -> Option<EnvUse> {
        let pbr = m.kind == MaterialKind::MeshStandard || m.kind == MaterialKind::MeshPhysical;
        let lp = m.kind == MaterialKind::MeshLambert || m.kind == MaterialKind::MeshPhong;
        if !pbr && !lp && m.kind != MaterialKind::MeshBasic {
            return None;
        }
        let mut tex: Option<TextureRef> = None;
        let mut scene = false;
        if let Some(t) = &m.env_map {
            tex = Some(t.clone());
        } else if pbr || lp {
            if let Some(t) = &self.scene_env {
                tex = Some(t.clone());
                scene = true;
            }
        }
        match tex {
            Some(t) => {
                if !is_equirect(&t) || self.texture_image_version(t.borrow().id) < 0 {
                    return None;
                }
                Some(EnvUse { tex: t, pbr, scene })
            }
            None => None,
        }
    }

    /// The prefiltered levels of an equirectangular texture, made when its
    /// version changes (three-webgl.js pmrem); false when its pixels are not here.
    fn pmrem(&mut self, c: &mut GpuCommands, t: &TextureRef, half_float: bool) -> bool {
        let tid = t.borrow().id;
        let version = t.borrow().version;
        let mut width: i64 = 0;
        if let Some(im) = self.images.get(&tid) {
            width = im.width;
        }
        if width <= 0 {
            return false;
        }
        let mut stale: Vec<i64> = Vec::new();
        if let Some(e) = self.envs.get(&tid) {
            if e.version == version {
                return true;
            }
            for l in e.levels.iter() {
                stale.push(*l);
            }
            stale.push(e.atlas);
        }
        for id in stale.iter() {
            c.release(*id);
        }
        if self.pm_prog == 0 {
            self.pm_prog = self.id();
            let names: Vec<String> = vec![
                String::from("u_src"),
                String::from("u_pass"),
                String::from("u_size"),
                String::from("u_sizeIn"),
                String::from("u_rough"),
            ];
            c.program(self.pm_prog, &full_vs(), &pmrem_fs(), &names);
        }
        let lod_max = ((width as f64) / 4.0).log2().floor() as i64;
        let cube: i64 = 1 << lod_max;
        let n = lod_max - 4 + 1 + 6;
        let mut sizes: Vec<i64> = Vec::new();
        for i in 0..n {
            sizes.push(if i > lod_max - 4 { 16 } else { 1 << (lod_max - i) });
        }
        let internal = if half_float { 34842 } else { 32856 };
        let ty = if half_float { 5131 } else { 5121 };
        c.use_program(self.pm_prog);
        self.fullscreen_state(c);
        c.uniform_int(0, 0);
        // level 0: the equirect texture as cube faces
        let mut levels: Vec<i64> = Vec::new();
        let mut level_targets: Vec<i64> = Vec::new();
        let l0 = self.id();
        c.texture(l0, &vec![0, internal, 6408, ty, 3 * cube, 2 * cube, GL_LINEAR, GL_LINEAR, GL_CLAMP, GL_CLAMP, 0], &Vec::new());
        let t0 = self.id();
        c.target(t0, 3 * cube, 2 * cube, 0, 1, l0, 0, 0);
        levels.push(l0);
        level_targets.push(t0);
        c.pass(t0, 3 * cube, 2 * cube, 0, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
        self.texture(c, t, 0);
        self.pm_pass(c, 0, cube, 0, 0.0);
        // each level filters the one before it with the extra roughness between them (three.js's _applyGGXFilter)
        for k in 1..n {
            let target = (k as f64) / ((n - 1) as f64);
            let source = ((k - 1) as f64) / ((n - 1) as f64);
            let rough = (target * target - source * source).sqrt() * (target * 1.25);
            let sz = sizes[k as usize];
            let tmp = self.id();
            c.texture(tmp, &vec![0, internal, 6408, ty, 3 * sz, 2 * sz, GL_LINEAR, GL_LINEAR, GL_CLAMP, GL_CLAMP, 0], &Vec::new());
            let tmp_t = self.id();
            c.target(tmp_t, 3 * sz, 2 * sz, 0, 1, tmp, 0, 0);
            c.pass(tmp_t, 3 * sz, 2 * sz, 0, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
            c.bind(0, levels[(k - 1) as usize], false);
            self.pm_pass(c, 1, sz, sizes[(k - 1) as usize], rough);
            let lv = self.id();
            c.texture(lv, &vec![0, internal, 6408, ty, 3 * sz, 2 * sz, GL_LINEAR, GL_LINEAR, GL_CLAMP, GL_CLAMP, 0], &Vec::new());
            let lv_t = self.id();
            c.target(lv_t, 3 * sz, 2 * sz, 0, 1, lv, 0, 0);
            c.pass(lv_t, 3 * sz, 2 * sz, 0, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
            c.bind(0, tmp, false);
            self.pm_pass(c, 1, sz, sz, 0.0);
            c.release(tmp_t);
            c.release(tmp);
            levels.push(lv);
            level_targets.push(lv_t);
        }
        // every level in one texture laid out as three.js's CubeUV map: the levels down to 16 texels in a
        // column, the rougher 16-texel levels in a row beside the last one
        let aw = 3 * (if cube > 112 { cube } else { 112 });
        let ah = 4 * cube;
        let atlas = self.id();
        c.texture(atlas, &vec![0, internal, 6408, ty, aw, ah, GL_LINEAR, GL_LINEAR, GL_CLAMP, GL_CLAMP, 0], &Vec::new());
        let atlas_t = self.id();
        c.target(atlas_t, aw, ah, 0, 1, atlas, 0, 0);
        for q in 0..n {
            let sz = sizes[q as usize];
            let m = lod_max - q;
            let ax = if m >= 4 { 0 } else { (4 - m) * 48 };
            let ay = 4 * (cube - (if sz > 16 { sz } else { 16 }));
            c.copy(level_targets[q as usize], atlas_t, &vec![0, 0, 3 * sz, 2 * sz, ax, ay, ax + 3 * sz, ay + 2 * sz], 16384, 9728);
        }
        for lt in level_targets.iter() {
            c.release(*lt);
        }
        c.release(atlas_t);
        self.envs.insert(tid, EnvEntry { version, levels, sizes, lod_max, atlas, aw, ah });
        true
    }

    fn pm_pass(&self, c: &mut GpuCommands, kind: i64, size: i64, size_in: i64, rough: f64) {
        c.uniform_int(1, kind);
        c.uniform(2, U_FLOAT, &vec![size as f64]);
        c.uniform(3, U_FLOAT, &vec![size_in as f64]);
        c.uniform(4, U_FLOAT, &vec![rough]);
        c.draw(GL_TRIANGLES, 0, 3, 0, 0);
    }

    /// The prefiltered environments the frame's materials use, made before the screen pass.
    fn prepare_envs(&mut self, c: &mut GpuCommands, items: &Vec<DrawItem>, half_float: bool) {
        let mut seen: Vec<i64> = Vec::new();
        for it in items.iter() {
            let ob = it.object.borrow();
            let mut mats: Vec<MaterialRef> = Vec::new();
            if let Some(d) = ob.mesh_data() {
                for m in d.materials.iter() {
                    mats.push(m.clone());
                }
            }
            for m in mats.iter() {
                if let Some(e) = self.env_of(&m.borrow()) {
                    let id = e.tex.borrow().id;
                    if e.pbr && !seen.contains(&id) {
                        seen.push(id);
                        self.pmrem(c, &e.tex, half_float);
                    }
                }
            }
        }
    }

    /// scene.background when it is a texture (three-webgl.js background): a
    /// screen-filling texture, or the world seen through an equirectangular
    /// map, blurred through the prefiltered levels when backgroundBlurriness > 0.
    #[allow(clippy::too_many_arguments)]
    fn background(&mut self, c: &mut GpuCommands, tex: &TextureRef, blur: f64, intensity: f64, rotation: &Euler, view: &Matrix4, projection: &Matrix4, s: &GpuSettings) {
        let tid = tex.borrow().id;
        let mut width: i64 = 0;
        if let Some(im) = self.images.get(&tid) {
            width = im.width;
        }
        if width <= 0 {
            return;
        }
        let equirect = is_equirect(tex);
        let mut blurred = false;
        if equirect && blur > 0.0 {
            blurred = self.pmrem(c, tex, s.half_float);
        }
        if self.bg_prog == 0 {
            self.bg_prog = self.id();
            let names: Vec<String> = vec![
                String::from("u_mode"),
                String::from("u_tone"),
                String::from("u_linearOut"),
                String::from("u_exposure"),
                String::from("u_intensity"),
                String::from("u_lod"),
                String::from("u_mipF"),
                String::from("u_sizeA"),
                String::from("u_sizeB"),
                String::from("u_tex"),
                String::from("u_lvA"),
                String::from("u_lvB"),
                String::from("u_uvt"),
                String::from("u_rot"),
                String::from("u_view"),
                String::from("u_proj"),
                String::from("u_vp"),
            ];
            c.program(self.bg_prog, &full_vs(), &background_fs(), &names);
        }
        c.use_program(self.bg_prog);
        self.fullscreen_state(c);
        c.uniform_int(9, 0);
        c.uniform_int(10, 15);
        c.uniform_int(11, 3);
        c.uniform(16, U_VEC2, &vec![s.width as f64, s.height as f64]);
        c.uniform(14, U_MAT4, &elements(view));
        c.uniform(15, U_MAT4, &elements(projection));
        c.uniform(13, U_MAT3, &euler_mat3(rotation, false));
        c.uniform(4, U_FLOAT, &vec![intensity]);
        // three.js tone-maps a background unless its texture is sRGB (the prefiltered levels are linear)
        let srgb = tex.borrow().color_space == ColorSpace::Srgb;
        c.uniform_int(1, if srgb && !blurred { 0 } else { s.tone_mapping });
        c.uniform(3, U_FLOAT, &vec![s.exposure]);
        c.uniform_int(2, if s.output_linear { 1 } else { 0 });
        if blurred {
            let mut lod_max: i64 = 0;
            let mut levels: Vec<i64> = Vec::new();
            let mut sizes: Vec<i64> = Vec::new();
            if let Some(e) = self.envs.get(&tid) {
                lod_max = e.lod_max;
                levels = e.levels.clone();
                sizes = e.sizes.clone();
            }
            let mut mip = roughness_to_mip(blur);
            if mip < -2.0 {
                mip = -2.0;
            }
            if mip > lod_max as f64 {
                mip = lod_max as f64;
            }
            let mi = mip.floor();
            let mf = mip - mi;
            let a = lod_max - (mi as i64);
            let b = if a - 1 > 0 { a - 1 } else { 0 };
            c.bind(15, levels[a as usize], false);
            c.bind(3, levels[b as usize], false);
            c.uniform(7, U_FLOAT, &vec![sizes[a as usize] as f64]);
            c.uniform(8, U_FLOAT, &vec![sizes[b as usize] as f64]);
            c.uniform(6, U_FLOAT, &vec![mf]);
            c.uniform_int(0, 3);
        } else {
            if !self.texture(c, tex, 0) {
                return;
            }
            if equirect {
                // the mip level whose texel is one screen pixel wide (no derivative jump at the seam)
                let pixel = 2.0 * (1.0 / projection.elements[5].abs()).atan() / (s.height as f64);
                let mut lod = (pixel / (2.0 * std::f64::consts::PI / (width as f64))).log2();
                if lod < 0.0 {
                    lod = 0.0;
                }
                c.uniform(5, U_FLOAT, &vec![lod]);
                c.uniform_int(0, 2);
            } else {
                c.uniform(12, U_MAT3, &texture_matrix(tex));
                c.uniform_int(0, 1);
            }
        }
        c.draw(GL_TRIANGLES, 0, 3, 0, 0);
    }

    /// Binds a draw's environment on unit 11 and sets u_envUV, u_env and
    /// u_envRot (three-webgl.js envBind / envUniforms); its mode (1
    /// prefiltered, 2 equirect) or 0 for none.
    #[allow(unused_assignments)]
    fn env_bind(&mut self, c: &mut GpuCommands, m: &Material) -> i64 {
        let e = match self.env_of(m) {
            Some(e) => e,
            None => return 0,
        };
        let tid = e.tex.borrow().id;
        let mut uv: Vec<f64> = Vec::new();
        let mut env: Vec<f64> = Vec::new();
        let mut mode: i64 = 0;
        if e.pbr {
            let mut found = false;
            let mut atlas: i64 = 0;
            if let Some(pm) = self.envs.get(&tid) {
                found = true;
                atlas = pm.atlas;
                uv = vec![pm.lod_max as f64, 1.0 / (pm.aw as f64), 1.0 / (pm.ah as f64), 0.0];
            }
            if !found {
                return 0;
            }
            c.bind(11, atlas, false);
            let intensity = if e.scene { self.scene_env_intensity } else { m.env_map_intensity };
            env = vec![1.0, intensity, 0.0, 0.0];
            mode = 1;
        } else {
            if !self.texture(c, &e.tex, 11) {
                return 0;
            }
            let refract = if e.tex.borrow().mapping == EQUIRECTANGULAR_REFRACTION_MAPPING { m.refraction_ratio } else { 0.0 };
            uv = vec![0.0, 0.0, 0.0, refract];
            env = vec![2.0, 1.0, m.combine as f64, m.reflectivity];
            mode = 2;
        }
        // three.js passes the inverse (transpose) of the rotation
        let rot = if e.scene { self.scene_env_rotation } else { m.env_map_rotation };
        let mut vals: Vec<f64> = uv;
        for v in env.iter() {
            vals.push(*v);
        }
        for v in euler_mat3(&rot, true).iter() {
            vals.push(*v);
        }
        self.env_vals = vals;
        mode
    }

    /// The uniforms of the environment `env_bind` bound (after the program is in use).
    fn env_uniforms(&self, c: &mut GpuCommands, mode: i64) {
        if mode == 0 {
            c.uniform(self.loc("u_env"), U_VEC4, &vec![0.0, 0.0, 0.0, 0.0]);
            return;
        }
        let v = &self.env_vals;
        c.uniform(self.loc("u_envUV"), U_VEC4, &vec![v[0], v[1], v[2], v[3]]);
        c.uniform(self.loc("u_env"), U_VEC4, &vec![v[4], v[5], v[6], v[7]]);
        let mut r: Vec<f64> = Vec::new();
        for i in 8..17 {
            r.push(v[i]);
        }
        c.uniform(self.loc("u_envRot"), U_MAT3, &r);
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

    /// The samplers' texture units (as three-webgl.js setUnits).
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
        if !self.trans_size.is_empty() {
            c.uniform(self.loc("u_transSize"), U_VEC2, &self.trans_size);
        }
        if !self.shadow_mat.is_empty() {
            c.uniform(self.loc("u_shMat"), U_MAT4, &self.shadow_mat);
            c.uniform(self.loc("u_shParam"), U_VEC4, &self.shadow_param);
            c.uniform(self.loc("u_shSize"), U_VEC4, &self.shadow_size);
        }
    }

    /// The opaque scene (and background) drawn linear and untoned into a
    /// multisampled target, resolved to a mipmapped texture that
    /// transmissive materials sample (three.js's renderTransmissionPass);
    /// double-sided transmissive objects add their back faces.
    #[allow(clippy::too_many_arguments)]
    fn transmission_pass(&mut self, c: &mut GpuCommands, frame: &mut GpuFrame, plan: &RenderPlan, bg: &BackgroundUse, opaque: &Vec<DrawItem>, transmissive: &Vec<DrawItem>, s: &GpuSettings) {
        let w = s.width;
        let h = s.height;
        if self.trans_ms == 0 || self.trans_w != w || self.trans_h != h {
            if self.trans_ms != 0 {
                c.release(self.trans_ms);
                c.release(self.trans_res);
                c.release(self.trans_store);
            }
            let fmt = if s.half_float { 34842 } else { 32856 };
            self.trans_ms = self.id();
            c.target(self.trans_ms, w, h, s.samples, 2, fmt, 2, 33190);
            self.trans_store = self.id();
            c.texture(self.trans_store, &vec![0, fmt, 6408, if s.half_float { 5131 } else { 5121 }, w, h, 9987, GL_LINEAR, GL_CLAMP, GL_CLAMP, 0], &Vec::new());
            self.trans_res = self.id();
            c.target(self.trans_res, w, h, 0, 1, self.trans_store, 0, 0);
            self.trans_w = w;
            self.trans_h = h;
        }
        // the clear colour in linear; a see-through clear becomes white at half alpha
        let cl = plan.clear;
        let rgba = if plan.clear_alpha < 1.0 { vec![1.0, 1.0, 1.0, 0.5] } else { vec![cl.r, cl.g, cl.b, plan.clear_alpha] };
        c.pass(self.trans_ms, w, h, 3, &rgba, 1.0);
        let mut ls = s.clone();
        ls.tone_mapping = 0;
        ls.output_linear = true;
        if let Some(t) = &bg.tex {
            self.background(c, t, bg.blur, bg.intensity, &bg.rotation, &plan.view, &plan.projection, &ls);
        }
        for it in opaque.iter() {
            self.draw(c, frame, it, &None, &ls);
        }
        self.trans_resolve(c, w, h);
        let mut back = false;
        for it in transmissive.iter() {
            let mut double = false;
            if let Some(m) = first_material(&it.object) {
                double = m.borrow().side == Side::Double;
            }
            if !double {
                continue;
            }
            if !back {
                self.trans_tex = self.trans_store;
                back = true;
            }
            self.draw_side = 1;
            self.draw(c, frame, it, &None, &ls);
            self.draw_side = -1;
        }
        if back {
            self.trans_resolve(c, w, h);
        }
        c.pass(0, w, h, 0, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
        self.trans_tex = self.trans_store;
        c.bind(14, self.trans_store, false);
        self.trans_size = vec![w as f64, h as f64];
        self.fver += 1;
    }

    fn trans_resolve(&self, c: &mut GpuCommands, w: i64, h: i64) {
        c.copy(self.trans_ms, self.trans_res, &vec![0, 0, w, h, 0, 0, w, h], 16384, 9728);
        c.mipmaps(self.trans_store);
        c.bind(14, self.trans_store, false);
        // drawing goes on into the multisampled target
        c.pass(self.trans_ms, w, h, 0, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
    }

    /// Every shadow sampler needs a texture of its kind bound even when no
    /// light uses it, and none may hold a map being rendered: 1x1 compared
    /// depth textures for PCF and basic, nothing for VSM's plain samplers.
    fn bind_shadow_dummies(&self, c: &mut GpuCommands, kind: i64) {
        let units: Vec<i64> = vec![8, 9, 13, 15];
        for i in 0..4 {
            c.bind(units[i], 0, false);
            c.bind(units[i], 0, true);
            if kind != 3 {
                if i == 3 {
                    c.bind(15, self.shadow_cube, true);
                } else {
                    c.bind(units[i], self.shadow_flat, false);
                }
            }
        }
    }

    /// The frame's shadow maps (three.js's WebGLShadowMap): depth from each
    /// shadow camera of the casters, point lights into six cube faces, VSM
    /// blurred. Returns each light's slot (2D 0-2, the point light 3; -1 none).
    #[allow(clippy::too_many_arguments)]
    fn render_shadows(&mut self, c: &mut GpuCommands, scene: &ObjectRef, items: &Vec<DrawItem>, kind: i64, s: &GpuSettings, missing: &mut Vec<String>) -> Vec<i64> {
        let mut slots: Vec<i64> = vec![-1; MAX_LIGHTS as usize];
        let plans = shadow_plans(scene);
        if plans.is_empty() {
            return slots;
        }
        let mut casters: Vec<DrawItem> = Vec::new();
        for it in items.iter() {
            let ob = it.object.borrow();
            if it.kind >= 1 && it.kind <= 6 && (ob.cast_shadow || (kind == 3 && ob.receive_shadow)) {
                casters.push(it.clone());
            }
        }
        if self.shadow_mat.is_empty() {
            self.shadow_mat = vec![0.0; 64];
            self.shadow_param = vec![0.0; 16];
            self.shadow_size = vec![0.0; 16];
        }
        let base = self.frame_floats.clone();
        let saved_ints = self.frame_ints.clone();
        self.frame_ints = vec![0];
        self.shadow_pass = kind;
        let mut next2d: i64 = 0;
        let mut used_point = false;
        let mut frame = GpuFrame { commands: GpuCommands::new(), missing: Vec::new(), calls: 0, triangles: 0, lines: 0, points: 0 };
        for sp in plans.iter() {
            let point = sp.kind == 3;
            let mut slot: i64 = 0;
            if point {
                if used_point || kind == 3 {
                    continue;
                }
                slot = 3;
                used_point = true;
            } else {
                if next2d > 2 {
                    continue;
                }
                slot = next2d;
                next2d += 1;
            }
            let w = sp.map_width as i64;
            let h = sp.map_height as i64;
            let res = self.shadow_target(c, slot, w, h, kind, point, s.half_float);
            let faces = if point { 6 } else { 1 };
            let fw = w;
            let fh = if point { w } else { h };
            for f in 0..faces {
                c.pass(res.targets[f as usize], fw, fh, 2, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
                // this face's view and the shadow camera's projection
                let mut ff: Vec<f64> = elements(&sp.views[f as usize]);
                for i in 16..32 {
                    ff.push(base[i]);
                }
                for x in elements(&sp.projection) {
                    ff.push(x);
                }
                for i in 48..base.len() {
                    ff.push(base[i]);
                }
                self.frame_floats = ff;
                self.fver += 1;
                for it in casters.iter() {
                    self.draw(c, &mut frame, it, &None, s);
                }
            }
            if kind == 3 {
                self.vsm_blur(c, &res, sp.radius, sp.blur_samples);
            }
            // the receivers' side of it
            let m = elements(&sp.matrix);
            for i in 0..16 {
                self.shadow_mat[(slot as usize) * 16 + i] = m[i];
            }
            let o = (slot as usize) * 4;
            self.shadow_param[o] = sp.intensity;
            self.shadow_param[o + 1] = sp.bias;
            self.shadow_param[o + 2] = sp.normal_bias;
            self.shadow_param[o + 3] = sp.radius;
            self.shadow_size[o] = fw as f64;
            self.shadow_size[o + 1] = fh as f64;
            self.shadow_size[o + 2] = sp.near;
            self.shadow_size[o + 3] = sp.far;
            if (sp.light as usize) < slots.len() {
                slots[sp.light as usize] = slot;
            }
            let unit = if slot == 0 {
                8
            } else if slot == 1 {
                9
            } else if slot == 2 {
                13
            } else {
                15
            };
            c.bind(unit, if kind == 3 { res.map } else { res.depth }, point);
        }
        for m in frame.missing.iter() {
            missing.push(m.clone());
        }
        self.shadow_pass = 0;
        self.frame_floats = base;
        self.frame_ints = saved_ints;
        slots
    }

    /// A shadow slot's depth texture and targets, remade when its size, type or kind changes.
    #[allow(clippy::too_many_arguments)]
    fn shadow_target(&mut self, c: &mut GpuCommands, slot: i64, w: i64, h: i64, kind: i64, point: bool, half_float: bool) -> ShadowRes {
        let r = self.shadow_res[slot as usize].clone();
        if r.depth != 0 && r.w == w && r.h == h && r.kind == kind && r.point == point {
            return r;
        }
        let mut old: Vec<i64> = Vec::new();
        if r.depth != 0 {
            for t in r.targets.iter() {
                old.push(*t);
            }
            old.push(r.depth);
            if r.pass != 0 {
                old.push(r.pass_t);
                old.push(r.pass);
                old.push(r.map_t);
                old.push(r.map);
            }
        }
        for id in old.iter() {
            c.release(*id);
        }
        let vsm = kind == 3;
        let filter = if kind == 1 { GL_LINEAR } else { 9728 };
        let depth = self.id();
        // DEPTH_COMPONENT32F / FLOAT for VSM, else DEPTH_COMPONENT24 / UNSIGNED_INT, compared
        let size_h = if point { w } else { h };
        c.texture(
            depth,
            &vec![
                if point { 1 } else { 0 },
                if vsm { 36012 } else { 33190 },
                6402,
                if vsm { 5126 } else { 5125 },
                w,
                size_h,
                filter,
                filter,
                GL_CLAMP,
                GL_CLAMP,
                if vsm { 0 } else { 1 },
            ],
            &Vec::new(),
        );
        let mut targets: Vec<i64> = Vec::new();
        if point {
            for f in 0..6 {
                let t = self.id();
                c.target(t, w, w, 0, 0, 0, 3 + f, depth);
                targets.push(t);
            }
        } else {
            let t = self.id();
            c.target(t, w, h, 0, 0, 0, 1, depth);
            targets.push(t);
        }
        let mut res = ShadowRes { w, h, kind, point, depth, targets, pass: 0, pass_t: 0, map: 0, map_t: 0 };
        if vsm {
            let params: Vec<i64> = vec![
                0,
                if half_float { 33327 } else { 32856 },
                if half_float { 33319 } else { 6408 },
                if half_float { 5131 } else { 5121 },
                w,
                h,
                GL_LINEAR,
                GL_LINEAR,
                GL_CLAMP,
                GL_CLAMP,
                0,
            ];
            res.pass = self.id();
            c.texture(res.pass, &params, &Vec::new());
            res.pass_t = self.id();
            c.target(res.pass_t, w, h, 0, 1, res.pass, 0, 0);
            res.map = self.id();
            c.texture(res.map, &params, &Vec::new());
            res.map_t = self.id();
            c.target(res.map_t, w, h, 0, 1, res.map, 0, 0);
        }
        self.shadow_res[slot as usize] = res.clone();
        res
    }

    /// VSM: the depth map blurred into moments, horizontally then vertically.
    fn vsm_blur(&mut self, c: &mut GpuCommands, res: &ShadowRes, radius: f64, samples: i64) {
        if self.vsm_prog == 0 {
            self.vsm_prog = self.id();
            let names: Vec<String> = vec![
                String::from("u_src"),
                String::from("u_res"),
                String::from("u_radius"),
                String::from("u_samples"),
                String::from("u_horizontal"),
            ];
            c.program(self.vsm_prog, &full_vs(), &vsm_blur_fs(), &names);
        }
        c.use_program(self.vsm_prog);
        self.fullscreen_state(c);
        c.uniform_int(0, 0);
        c.uniform(1, U_VEC2, &vec![res.w as f64, res.h as f64]);
        c.uniform(2, U_FLOAT, &vec![radius]);
        c.uniform(3, U_FLOAT, &vec![samples as f64]);
        c.pass(res.pass_t, res.w, res.h, 0, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
        c.bind(0, res.depth, false);
        c.uniform_int(4, 0);
        c.draw(GL_TRIANGLES, 0, 3, 0, 0);
        c.pass(res.map_t, res.w, res.h, 0, &vec![0.0, 0.0, 0.0, 0.0], 1.0);
        c.bind(0, res.pass, false);
        c.uniform_int(4, 1);
        c.draw(GL_TRIANGLES, 0, 3, 0, 0);
    }

    /// `scene` from `camera` as commands.
    pub fn render(&mut self, scene: &ObjectRef, camera: &ObjectRef, s: &GpuSettings) -> GpuFrame {
        let mut c = GpuCommands::new();
        let mut missing: Vec<String> = Vec::new();
        self.frame += 1;
        let plan = plan_frame(scene, camera, &s.clear_color, s.clear_alpha);
        let mut override_mat: Option<MaterialRef> = None;
        let mut bg_tex: Option<TextureRef> = None;
        let mut bg_blur: f64 = 0.0;
        let mut bg_intensity: f64 = 1.0;
        let mut bg_rotation = Euler::new(0.0, 0.0, 0.0, EulerOrder::Xyz);
        self.scene_env = None;
        if let ObjectKind::Scene(sd) = &scene.borrow().kind {
            if let Background::Texture(t) = &sd.background {
                bg_tex = Some(t.clone());
            }
            bg_blur = sd.background_blurriness;
            bg_intensity = sd.background_intensity;
            bg_rotation = sd.background_rotation;
            if let Some(t) = &sd.environment {
                self.scene_env = Some(t.clone());
            }
            self.scene_env_intensity = sd.environment_intensity;
            self.scene_env_rotation = sd.environment_rotation;
            if let Some(m) = &sd.override_material {
                override_mat = Some(m.clone());
            }
        }
        self.ensure_fixed(&mut c);
        let w = s.width;
        let h = s.height;
        // the frame's uniforms (the shadow passes replace view and projection)
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
        let camera_floats = ff.clone();
        self.frame_floats = ff;
        // shadow type (three.js: 0 basic, 1 PCF, 2 PCF soft = PCF, 3 VSM) -> 2, 1, 3; 0 none
        let stype = if !s.shadows {
            0
        } else if s.shadow_type == 0 {
            2
        } else if s.shadow_type == 3 {
            3
        } else {
            1
        };
        self.stype = if stype == 0 { 1 } else { stype };
        self.bind_shadow_dummies(&mut c, self.stype);
        let mut slots: Vec<i64> = vec![-1; MAX_LIGHTS as usize];
        if stype != 0 {
            slots = self.render_shadows(&mut c, scene, &plan.items, stype, s, &mut missing);
        }
        self.prepare_envs(&mut c, &plan.items, s.half_float);
        let cl = plan.clear;
        let rgba = if s.output_linear {
            vec![clamp01(cl.r), clamp01(cl.g), clamp01(cl.b), plan.clear_alpha]
        } else {
            vec![linear_to_srgb(clamp01(cl.r)), linear_to_srgb(clamp01(cl.g)), linear_to_srgb(clamp01(cl.b)), plan.clear_alpha]
        };
        c.pass(0, w, h, 3, &rgba, 1.0);
        c.bind(2, self.dfg, false);
        if let Some(t) = &bg_tex {
            self.background(&mut c, t, bg_blur, bg_intensity, &bg_rotation, &plan.view, &plan.projection, s);
        }
        self.frame_floats = camera_floats;
        self.fver += 1;
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
                    if slots[i] >= 0 {
                        lights[at * stride + 18] = (slots[i] + 1) as f64;
                    }
                    at += 1;
                    counts[kind as usize] += 1;
                }
            }
        }
        if counts[5] > 0 && self.ltc == 0 {
            // both tables side by side (64 x 64 each); lookups stay half a texel inside each, so they never mix
            let l1 = ltc_1();
            let l2 = ltc_2();
            let mut both: Vec<f64> = vec![0.0; 128 * 64 * 4];
            for ly in 0..64 {
                for lx in 0..64 {
                    for lc in 0..4 {
                        both[(ly * 128 + lx) * 4 + lc] = l1[(ly * 64 + lx) * 4 + lc];
                        both[(ly * 128 + 64 + lx) * 4 + lc] = l2[(ly * 64 + lx) * 4 + lc];
                    }
                }
            }
            self.ltc = self.id();
            c.texture(self.ltc, &vec![0, if s.float_linear { 34836 } else { 34842 }, 6408, 5126, 128, 64, 9728, GL_LINEAR, GL_CLAMP, GL_CLAMP, 0], &both);
        }
        if self.ltc != 0 {
            c.bind(10, self.ltc, false);
        }
        self.lights = lights;
        self.light_counts = counts;
        self.has_shadows = false;
        for i in 0..(n as usize) {
            if slots[i] >= 0 {
                self.has_shadows = true;
            }
        }
        self.frame_ints = vec![n];
        self.fog_kind = if plan.fog[0] > 1.5 {
            2
        } else if plan.fog[0] > 0.5 {
            1
        } else {
            0
        };
        // three.js's lists: opaque, transmissive (by depth), transparent
        let mut opaque: Vec<DrawItem> = Vec::new();
        let mut transmissive: Vec<DrawItem> = Vec::new();
        let mut transparent: Vec<DrawItem> = Vec::new();
        for it in plan.items.iter() {
            let mut is_trans = false;
            if let Some(m) = first_material(&it.object) {
                is_trans = m.borrow().transmission > 0.0;
            }
            if is_trans {
                // insertion keeps equal depths in list order (a stable sort)
                let mut at = transmissive.len();
                while at > 0 && transmissive[at - 1].depth > it.depth {
                    at -= 1;
                }
                transmissive.insert(at, it.clone());
            } else if it.transparent {
                transparent.push(it.clone());
            } else {
                opaque.push(it.clone());
            }
        }
        let mut frame = GpuFrame { commands: GpuCommands::new(), missing: Vec::new(), calls: 0, triangles: 0, lines: 0, points: 0 };
        self.trans_tex = 0;
        if !transmissive.is_empty() && override_mat.is_none() {
            let bg = BackgroundUse { tex: bg_tex.clone(), blur: bg_blur, intensity: bg_intensity, rotation: bg_rotation };
            self.transmission_pass(&mut c, &mut frame, &plan, &bg, &opaque, &transmissive, s);
        }
        for it in opaque.iter() {
            self.draw(&mut c, &mut frame, it, &override_mat, s);
        }
        for it in transmissive.iter() {
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
            ObjectKind::Sprite(sd) => MeshData::new(sd.geometry.clone(), sd.material.clone()),
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

    /// The buffer of attribute `name` (3 components) with the object's morph
    /// targets blended in (three.js: base + sum of influence * target, the
    /// target less the base unless `morphTargetsRelative`); 0 when the
    /// geometry has no morph targets for it.
    fn morph_buffer(&mut self, c: &mut GpuCommands, oid: &str, name: &str, data: &MeshData) -> i64 {
        let g = data.geometry.borrow();
        let base = match g.get_attribute(name) {
            Some(a) => a,
            None => return 0,
        };
        let b = base.borrow();
        let influences = &data.morph_target_influences;
        let mut key = b.version.to_string();
        let mut used: Vec<i64> = Vec::new();
        if let Some(targets) = g.morph_attributes.get(name) {
            for (i, t) in targets.iter().enumerate() {
                if i < influences.len() && influences[i] != 0.0 {
                    used.push(i as i64);
                    key.push_str(":");
                    key.push_str(&t.borrow().version.to_string());
                    key.push_str("*");
                    key.push_str(&influences[i].to_string());
                }
            }
        } else {
            return 0;
        }
        let bkey = oid.to_string() + ":morph:" + name;
        let mut id: i64 = 0;
        if let Some(e) = self.morphs.get(&bkey) {
            if e.key == key {
                return e.id;
            }
            id = e.id;
        }
        let n = b.count;
        let mut out: Vec<f64> = vec![0.0; n * 3];
        for i in 0..n {
            let v = b.get_vector3(i);
            out[i * 3] = v.x;
            out[i * 3 + 1] = v.y;
            out[i * 3 + 2] = v.z;
        }
        if let Some(targets) = g.morph_attributes.get(name) {
            for u in used.iter() {
                let k = *u as usize;
                let w = influences[k];
                let t = targets[k].borrow();
                let m = if t.count < n { t.count } else { n };
                for i in 0..m {
                    let v = t.get_vector3(i);
                    let mut dx = v.x;
                    let mut dy = v.y;
                    let mut dz = v.z;
                    if !g.morph_targets_relative {
                        let o = b.get_vector3(i);
                        dx -= o.x;
                        dy -= o.y;
                        dz -= o.z;
                    }
                    out[i * 3] += w * dx;
                    out[i * 3 + 1] += w * dy;
                    out[i * 3 + 2] += w * dz;
                }
            }
        }
        if name == "normal" {
            // three.js normalizes each vertex normal before it is interpolated; a blend of
            // targets is not unit length, and interpolating it unnormalized shades differently
            for i in 0..n {
                let l = (out[i * 3] * out[i * 3] + out[i * 3 + 1] * out[i * 3 + 1] + out[i * 3 + 2] * out[i * 3 + 2]).sqrt();
                if l > 0.0 {
                    out[i * 3] /= l;
                    out[i * 3 + 1] /= l;
                    out[i * 3 + 2] /= l;
                }
            }
        }
        if id == 0 {
            id = self.id();
        }
        c.buffer(id, false, true, &out);
        self.morphs.insert(bkey, MorphEntry { id, key });
        id
    }

    /// Uses the program of a ShaderMaterial / RawShaderMaterial and sets its
    /// uniforms and own attributes; false, with the reason in
    /// `frame.missing`, when this path cannot draw it.
    #[allow(clippy::too_many_arguments)]
    fn use_shader(&mut self, c: &mut GpuCommands, frame: &mut GpuFrame, m: &Material, model: &Matrix4, instanced: bool, colors: bool, data: &MeshData) -> bool {
        let raw = m.kind == MaterialKind::RawShader;
        let mut ok = true;
        for inc in includes(&m.vertex_shader).iter() {
            frame.missing.push(String::from("shader chunk ") + inc);
            ok = false;
        }
        for inc in includes(&m.fragment_shader).iter() {
            frame.missing.push(String::from("shader chunk ") + inc);
            ok = false;
        }
        if !ok {
            return false;
        }
        let mut key = String::from(if raw { "raw" } else { "sh" });
        key.push_str(if m.glsl3 { ":3" } else { ":1" });
        key.push_str(if instanced { ":i" } else { ":-" });
        key.push_str(if colors { ":c" } else { ":-" });
        for d in m.shader_defines.iter() {
            key.push_str("\n");
            key.push_str(d);
        }
        key.push_str("\n#vs\n");
        key.push_str(&m.vertex_shader);
        key.push_str("\n#fs\n");
        key.push_str(&m.fragment_shader);
        if !self.shader_progs.contains_key(&key) {
            let src = shader_sources(m, raw, instanced, colors);
            if CUSTOM_ATTRIB_BASE + (src.len() as i64 - 2) > MAX_ATTRIBS {
                frame.missing.push(String::from("more than 6 shader attributes"));
                return false;
            }
            let decl = uniform_declarations(&src[0], &src[1]);
            let mut names: Vec<String> = Vec::new();
            let mut types: Vec<String> = Vec::new();
            let mut i = 0;
            while i + 1 < decl.len() {
                types.push(decl[i].clone());
                names.push(decl[i + 1].clone());
                i += 2;
            }
            let mut attribs: Vec<String> = Vec::new();
            for k in 2..src.len() {
                attribs.push(src[k].clone());
            }
            let id = self.id();
            c.program(id, &src[0], &src[1], &names);
            self.shader_progs.insert(key.clone(), ShaderProgram { id, names, types, attribs });
        }
        let p = match self.shader_progs.get(&key) {
            Some(e) => e.clone(),
            None => return false,
        };
        c.use_program(p.id);
        let f = self.frame_floats.clone();
        let mut view: Vec<f64> = Vec::new();
        let mut proj: Vec<f64> = Vec::new();
        for i in 0..16 {
            view.push(f[i]);
            proj.push(f[32 + i]);
        }
        let model_e = elements(model);
        let mv = mul4(&view, &model_e);
        let mut mva: [f64; 16] = [0.0; 16];
        for i in 0..16 {
            mva[i] = mv[i];
        }
        let mut unit: usize = 0;
        for i in 0..p.names.len() {
            let n = p.names[i].clone();
            let t = p.types[i].clone();
            let loc = i as i64;
            if n == "modelMatrix" {
                c.uniform(loc, U_MAT4, &model_e);
                continue;
            }
            if n == "viewMatrix" {
                c.uniform(loc, U_MAT4, &view);
                continue;
            }
            if n == "projectionMatrix" {
                c.uniform(loc, U_MAT4, &proj);
                continue;
            }
            if n == "modelViewMatrix" {
                c.uniform(loc, U_MAT4, &mv);
                continue;
            }
            if n == "normalMatrix" {
                c.uniform(loc, U_MAT3, &normal_matrix(&mva));
                continue;
            }
            if n == "cameraPosition" {
                c.uniform(loc, U_VEC3, &vec![f[48], f[49], f[50]]);
                continue;
            }
            if n == "isOrthographic" {
                c.uniform_int(loc, if proj[15] == 1.0 { 1 } else { 0 });
                continue;
            }
            let mut found = false;
            let mut values: Vec<f64> = Vec::new();
            let mut tex: Option<TextureRef> = None;
            for u in m.shader_uniforms.iter() {
                if u.name == n {
                    found = true;
                    values = u.values.clone();
                    tex = u.texture.clone();
                }
            }
            if !found {
                continue;
            }
            if t == "sampler2D" {
                if let Some(tx) = &tex {
                    if unit >= SHADER_UNITS.len() {
                        frame.missing.push(String::from("more than 9 shader textures"));
                        return false;
                    }
                    let u = SHADER_UNITS[unit];
                    unit += 1;
                    self.texture(c, tx, u);
                    c.uniform_int(loc, u);
                }
                continue;
            }
            if t == "int" || t == "bool" {
                if !values.is_empty() {
                    c.uniform_int(loc, values[0] as i64);
                }
                continue;
            }
            let kind = if t == "float" {
                U_FLOAT
            } else if t == "vec2" {
                U_VEC2
            } else if t == "vec3" {
                U_VEC3
            } else if t == "vec4" {
                U_VEC4
            } else if t == "mat3" {
                U_MAT3
            } else if t == "mat4" {
                U_MAT4
            } else {
                0
            };
            if kind == 0 {
                frame.missing.push(String::from("shader uniform type ") + &t);
                return false;
            }
            let size: usize = if kind == U_VEC2 {
                2
            } else if kind == U_VEC3 {
                3
            } else if kind == U_VEC4 {
                4
            } else if kind == U_MAT3 {
                9
            } else if kind == U_MAT4 {
                16
            } else {
                1
            };
            let whole = values.len() / size * size;
            if whole == 0 {
                continue;
            }
            let mut v: Vec<f64> = Vec::new();
            for k in 0..whole {
                v.push(values[k]);
            }
            c.uniform(loc, kind, &v);
        }
        let g = data.geometry.borrow();
        let gid = g.id.to_string();
        for k in 0..p.attribs.len() {
            let loc = CUSTOM_ATTRIB_BASE + k as i64;
            match g.get_attribute(&p.attribs[k]) {
                Some(a) => {
                    let aa = a.borrow();
                    let bkey = gid.clone() + ":" + &p.attribs[k];
                    let b = self.buffer(c, &bkey, aa.version, &aa.array, false, aa.usage == 35048);
                    let size = if aa.item_size < 4 { aa.item_size as i64 } else { 4 };
                    c.attrib(loc, b, size, (aa.item_size as i64) * 4, 0, 0, &Vec::new());
                }
                None => c.attrib(loc, 0, 4, 0, 0, 0, &vec![0.0, 0.0, 0.0, 0.0]),
            }
        }
        // three.js feeds a shader every attribute it declares, vertexColors or not
        if !colors {
            if let Some(a) = g.get_attribute("color") {
                let aa = a.borrow();
                let bkey = gid.clone() + ":color";
                let b = self.buffer(c, &bkey, aa.version, &aa.array, false, aa.usage == 35048);
                let size = if aa.item_size < 4 { aa.item_size as i64 } else { 4 };
                c.attrib(3, b, size, (aa.item_size as i64) * 4, 0, 0, &Vec::new());
            }
        }
        true
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
        let dashed = m.kind == MaterialKind::LineDashed;
        let sprite = kind == 7;
        let shader = self.shadow_pass == 0 && (m.kind == MaterialKind::Shader || m.kind == MaterialKind::RawShader);
        let morphed = !data.morph_target_influences.is_empty();
        let oid_s = o.id.to_string();
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
        let mut pbuf: i64 = if morphed { self.morph_buffer(c, &oid_s, "position", data) } else { 0 };
        if pbuf != 0 {
            c.attrib(0, pbuf, 3, 12, 0, 0, &Vec::new());
        } else {
            pbuf = self.buffer(c, &pkey, pos.version, &pos.array, false, pos.usage == 35048);
            let psize = if pos.item_size < 3 { pos.item_size as i64 } else { 3 };
            c.attrib(0, pbuf, psize, (pos.item_size as i64) * 4, 0, 0, &Vec::new());
        }
        let mut has_normal = false;
        if mesh && mu[17] < 0.5 {
            if let Some(nattr) = geometry.get_attribute("normal") {
                let na = nattr.borrow();
                let mb = if morphed { self.morph_buffer(c, &oid_s, "normal", data) } else { 0 };
                if mb != 0 {
                    c.attrib(1, mb, 3, 12, 0, 0, &Vec::new());
                } else {
                    let key = gid.clone() + ":normal";
                    let b = self.buffer(c, &key, na.version, &na.array, false, na.usage == 35048);
                    let size = if na.item_size < 3 { na.item_size as i64 } else { 3 };
                    c.attrib(1, b, size, (na.item_size as i64) * 4, 0, 0, &Vec::new());
                }
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
        let mut has_distance = false;
        if dashed {
            if let Some(ld) = geometry.get_attribute("lineDistance") {
                let la = ld.borrow();
                let key = gid.clone() + ":lineDistance";
                let b = self.buffer(c, &key, la.version, &la.array, false, la.usage == 35048);
                c.attrib(9, b, 1, (la.item_size as i64) * 4, 0, 0, &Vec::new());
                has_distance = true;
            }
        }
        if !has_distance {
            c.attrib(9, 0, 1, 0, 0, 0, &vec![0.0, 0.0, 0.0, 0.0]);
        }
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
        let sp = self.shadow_pass;
        let trans = if self.trans_tex != 0 { m.transmission } else { 0.0 };
        let env_mode = if sp != 0 || shader { 0 } else { self.env_bind(c, &m) };
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
            if dashed { 1 } else { 0 },
            0,
            if instances > 0 { 1 } else { 0 },
            if has_disp { 1 } else { 0 },
            if sprite { 1 } else { 0 },
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
            env_mode,
        ];
        vals[21] = if self.light_counts[5] > 0 { 1 } else { 0 };
        vals[20] = if physical == 1 && trans > 0.0 { 1 } else { 0 };
        for k in 0..6 {
            vals.push(if lit { self.light_counts[k] } else { 0 });
        }
        if sp != 0 {
            // a shadow caster keeps only what decides its discards
            let keep: Vec<i64> = vec![0, 1, 3, 9, 11, 12];
            let mut v2: Vec<i64> = vec![0; vals.len()];
            for k in keep.iter() {
                v2[*k as usize] = vals[*k as usize];
            }
            v2[5] = bits & 1;
            v2[23] = 1;
            vals = v2;
        } else {
            vals[10] = if o.receive_shadow && self.has_shadows { 1 } else { 0 };
        }
        if shader {
            if !self.use_shader(c, frame, &m, &o.matrix_world, instances > 0, colored, data) {
                return;
            }
        } else {
        if vals.len() != key_names().len() {
            frame.missing.push(String::from("program key"));
            return;
        }
        self.use_main(c, &vals);
        c.uniform(self.loc("u_mat"), U_VEC4, &mu);
        self.env_uniforms(c, env_mode);
        c.uniform(self.loc("u_model"), U_MAT4, &elements(&o.matrix_world));
        c.uniform(self.loc("u_normalMat"), U_MAT3, &normal_matrix(&o.matrix_world.elements));
        c.uniform(self.loc("u_uvt"), U_MAT3, &map_transform(&m));
        if dashed {
            c.uniform(self.loc("u_dashScale"), U_FLOAT, &vec![m.scale]);
            c.uniform(self.loc("u_dashSize"), U_FLOAT, &vec![m.dash_size]);
            c.uniform(self.loc("u_dashTotal"), U_FLOAT, &vec![m.dash_size + m.gap_size]);
        } else {
            c.uniform(self.loc("u_dashScale"), U_FLOAT, &vec![0.0]);
        }
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
        if trans > 0.0 {
            let ad = m.attenuation_distance;
            c.uniform(self.loc("u_trans"), U_VEC4, &vec![trans, m.thickness, m.ior, if ad > 0.0 && ad < 1.0e300 { ad } else { 0.0 }]);
            c.uniform(self.loc("u_attColor"), U_VEC3, &vec![m.attenuation_color.r, m.attenuation_color.g, m.attenuation_color.b]);
        } else {
            c.uniform(self.loc("u_trans"), U_VEC4, &vec![0.0, 0.0, 0.0, 0.0]);
        }
        c.uniform_int(self.loc("u_instanced"), if instances > 0 { 1 } else { 0 });
        c.uniform_int(self.loc("u_sprite"), if sprite { 1 } else { 0 });
        if sprite {
            let mut center: Vec<f64> = vec![0.5, 0.5];
            if let ObjectKind::Sprite(sd) = &o.kind {
                center = vec![sd.center.x, sd.center.y];
            }
            c.uniform(self.loc("u_center"), U_VEC2, &center);
            c.uniform(self.loc("u_rotation"), U_FLOAT, &vec![m.rotation]);
        }
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
        }
        // state
        let ccw = o.matrix_world.determinant() >= 0.0;
        let blend = if sp != 0 { vec![0, GL_FUNC_ADD, GL_FUNC_ADD, GL_ONE, GL_ZERO, GL_ONE, GL_ZERO] } else { self.blend_state(&m, opaque) };
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
        let mut side = if self.draw_side >= 0 { self.draw_side } else { mu[16] as i64 };
        let mut passes: Vec<i64> = vec![side];
        if sp != 0 {
            // shadow side: the material's shadowSide, else front <-> back (VSM keeps the side)
            side = if mu[23] >= 0.0 {
                mu[23] as i64
            } else if sp == 3 {
                side
            } else if side == 0 {
                1
            } else if side == 1 {
                0
            } else {
                2
            };
            passes = vec![side];
        } else if mesh && side == 2 && m.transparent && !m.force_single_pass {
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
                    if m.depth_test || sp != 0 { 1 } else { 0 },
                    GL_LEQUAL,
                    if m.depth_write || sp != 0 { 1 } else { 0 },
                    cull,
                    if ccw { 1 } else { 0 },
                    if m.color_write || sp != 0 { 1 } else { 0 },
                    if m.polygon_offset && sp == 0 { 1 } else { 0 },
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
