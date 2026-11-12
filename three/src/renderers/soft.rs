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
use crate::maths::color::{linear_to_srgb, srgb_to_linear, Color, ColorSpace};
use crate::maths::matrix3::Matrix3;
use crate::maths::matrix4::Matrix4;
use crate::maths::vector3::Vector3;
use crate::renderers::dfg_table::dfg_table;
use crate::renderers::pbr::brdf_ggx_multiscatter;
use crate::scenes::fog::Fog;
use crate::textures::texture::{Image, Texture, CLAMP_TO_EDGE_WRAPPING, MIRRORED_REPEAT_WRAPPING, NEAREST_FILTER, REPEAT_WRAPPING, UV_MAPPING};

const RECIPROCAL_PI: f64 = 0.3183098861837907;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightIn {
    /// 0 ambient, 1 hemisphere, 2 directional, 3 point, 4 spot
    pub kind: i64,
    pub color: Color,
    pub ground: Color,
    pub intensity: f64,
    pub position: Vector3,
    pub direction: Vector3,
    pub distance: f64,
    pub decay: f64,
    pub cos_outer: f64,
    pub cos_inner: f64,
}

/// What shading needs from a material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub kind: MaterialKind,
    pub color: Color,
    pub emissive: Color,
    pub specular: Color,
    pub shininess: f64,
    pub diffuse_scale: f64,
    pub lit: bool,
    pub specular_on: bool,
    /// Standard/Physical: GGX specular from `roughness` and `metalness`
    pub physical: bool,
    pub roughness: f64,
    pub metalness: f64,
}

/// A vertex after lighting, in view space. Untextured: `color` is final.
/// Textured: `color` is the diffuse light times the albedo (the texel
/// multiplies it), `add` the specular and emissive light and `u`, `v`
/// the texture coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Vert {
    view: Vector3,
    color: Color,
    add: Color,
    u: f64,
    v: f64,
}

fn vert(view: Vector3, color: Color) -> Vert {
    Vert { view, color, add: Color::new(0.0, 0.0, 0.0), u: 0.0, v: 0.0 }
}

/// Light at a surface point, split so a texture can scale the diffuse part.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lit {
    diffuse: Color,
    add: Color,
}

/// A texture's pixels, linear RGBA 0..1, row 0 at the top of the image.
#[derive(Clone, Debug, PartialEq)]
pub struct TexImage {
    pub id: i64,
    pub version: i64,
    pub width: i64,
    pub height: i64,
    pub data: Vec<f64>,
}

/// How a draw samples its map.
#[derive(Clone, Copy, Debug)]
struct Sampler {
    /// index into `SoftRenderer::textures`, -1 when there is no map
    slot: i64,
    e: [f64; 9],
    transform: bool,
    wrap_s: i64,
    wrap_t: i64,
    flip_y: bool,
    nearest: bool,
    alpha_test: f64,
    fog_color: Color,
    /// fog per pixel from the depth (big triangles fog wrongly per vertex)
    fog: Option<Fog>,
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
    /// the DFG table for Standard/Physical materials (`DFG_SIZE` squared pairs)
    dfg: Vec<f64>,
}

const DFG_SIZE: i64 = 16;

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
    /// Uploaded texture pixels by texture id.
    pub textures: Vec<TexImage>,
    /// three.js's `toneMapping` constant and `toneMappingExposure`.
    pub tone_mapping: i64,
    pub tone_mapping_exposure: f64,
    /// the tone mapping of the material being drawn
    cur_tone: i64,
    /// the DFG table, made on first use
    dfg: Vec<f64>,
}

/// `m` (column-major 3x3, as GLSL's mat3 of column vectors) times `c`.
fn mul_cols(m: &[f64; 9], c: &Color) -> Color {
    Color::new(
        m[0] * c.r + m[3] * c.g + m[6] * c.b,
        m[1] * c.r + m[4] * c.g + m[7] * c.b,
        m[2] * c.r + m[5] * c.g + m[8] * c.b,
    )
}

fn clamp_color(c: &Color) -> Color {
    Color::new(c.r.max(0.0).min(1.0), c.g.max(0.0).min(1.0), c.b.max(0.0).min(1.0))
}

fn rrt_odt(v: f64) -> f64 {
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    a / b
}

fn cineon(x: f64) -> f64 {
    ((x * (6.2 * x + 0.5)) / (x * (6.2 * x + 1.7) + 0.06)).powf(2.2)
}

fn agx_encode(x: f64) -> f64 {
    // 1e-10 keeps log2 finite (written as a division: Ranger's macro text
    // prints tiny literals in exponent form, which it cannot read back)
    let floor = 1.0 / 10000000000.0;
    agx_contrast(((x.max(floor).log2() + 12.47393) / (4.026069 + 12.47393)).max(0.0).min(1.0))
}

fn agx_contrast(x: f64) -> f64 {
    let x2 = x * x;
    let x4 = x2 * x2;
    15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232
}

/// three.js's tone mapping operators: 1 linear, 2 Reinhard, 3 Cineon,
/// 4 ACES filmic, 6 AgX, 7 neutral (others leave the colour).
pub fn tone_map(c_in: &Color, mode: i64, exposure: f64) -> Color {
    let c = Color::new(c_in.r * exposure, c_in.g * exposure, c_in.b * exposure);
    if mode == 1 {
        return clamp_color(&c);
    }
    if mode == 2 {
        return clamp_color(&Color::new(c.r / (1.0 + c.r), c.g / (1.0 + c.g), c.b / (1.0 + c.b)));
    }
    if mode == 3 {
        let r = (c.r - 0.004).max(0.0);
        let g = (c.g - 0.004).max(0.0);
        let b = (c.b - 0.004).max(0.0);
        return Color::new(cineon(r), cineon(g), cineon(b));
    }
    if mode == 4 {
        let input: [f64; 9] = [0.59719, 0.07600, 0.02840, 0.35458, 0.90834, 0.13383, 0.04823, 0.01566, 0.83777];
        let output: [f64; 9] = [1.60475, -0.10208, -0.00327, -0.53108, 1.10813, -0.07276, -0.07367, -0.00605, 1.07602];
        let s = Color::new(c.r / 0.6, c.g / 0.6, c.b / 0.6);
        let a = mul_cols(&input, &s);
        let t = Color::new(rrt_odt(a.r), rrt_odt(a.g), rrt_odt(a.b));
        return clamp_color(&mul_cols(&output, &t));
    }
    if mode == 6 {
        let to2020: [f64; 9] = [0.6274, 0.0691, 0.0164, 0.3293, 0.9195, 0.0880, 0.0433, 0.0113, 0.8956];
        let from2020: [f64; 9] = [1.6605, -0.1246, -0.0182, -0.5876, 1.1329, -0.1006, -0.0728, -0.0083, 1.1187];
        let inset: [f64; 9] = [0.856627153315983, 0.137318972929847, 0.11189821299995, 0.0951212405381588, 0.761241990602591, 0.0767994186031903, 0.0482516061458583, 0.101439036467562, 0.811302368396859];
        let outset: [f64; 9] = [1.1271005818144368, -0.1413297634984383, -0.14132976349843826, -0.11060664309660323, 1.157823702216272, -0.11060664309660294, -0.016493938717834573, -0.016493938717834257, 1.2519364065950405];
        let a = mul_cols(&inset, &mul_cols(&to2020, &c));
        let o = mul_cols(&outset, &Color::new(agx_encode(a.r), agx_encode(a.g), agx_encode(a.b)));
        let lin = Color::new(o.r.max(0.0).powf(2.2), o.g.max(0.0).powf(2.2), o.b.max(0.0).powf(2.2));
        return clamp_color(&mul_cols(&from2020, &lin));
    }
    if mode == 7 {
        let x = c.r.min(c.g).min(c.b);
        let off = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
        let d0 = Color::new(c.r - off, c.g - off, c.b - off);
        let peak = d0.r.max(d0.g).max(d0.b);
        if peak < 0.76 {
            return d0;
        }
        let d = 0.24;
        let np = 1.0 - d * d / (peak + d - 0.76);
        let k = np / peak;
        let g = 1.0 - 1.0 / (0.15 * (peak - np) + 1.0);
        return Color::new(d0.r * k + (np - d0.r * k) * g, d0.g * k + (np - d0.g * k) * g, d0.b * k + (np - d0.b * k) * g);
    }
    *c_in
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
    Vert {
        view: v,
        color: mix_color(&a.color, &b.color, t),
        add: mix_color(&a.add, &b.add, t),
        u: a.u + (b.u - a.u) * t,
        v: a.v + (b.v - a.v) * t,
    }
}

/// A texel coordinate wrapped into 0..n-1.
fn wrap_texel(i: i64, n: i64, mode: i64) -> i64 {
    if i >= 0 && i < n {
        return i;
    }
    if mode == REPEAT_WRAPPING || mode == MIRRORED_REPEAT_WRAPPING {
        let m = i % n;
        return if m < 0 { m + n } else { m };
    }
    if i < 0 {
        0
    } else {
        n - 1
    }
}

fn wrap_coord(v: f64, mode: i64) -> f64 {
    if v >= 0.0 && v <= 1.0 {
        return v;
    }
    if mode == REPEAT_WRAPPING {
        return v - v.floor();
    }
    if mode == MIRRORED_REPEAT_WRAPPING {
        let f = v.floor();
        let r = v - f;
        if (f.abs() as i64) % 2 == 1 {
            return 1.0 - r;
        }
        return r;
    }
    if v < 0.0 {
        0.0
    } else {
        1.0
    }
}

pub fn surface_of(m: &Material) -> Surface {
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
    let physical = m.kind == MaterialKind::MeshStandard || m.kind == MaterialKind::MeshPhysical;
    let roughness = m.roughness.max(0.0525).min(1.0);
    Surface { kind: m.kind, color: m.color, emissive, specular, shininess, diffuse_scale, lit, specular_on, physical, roughness, metalness: m.metalness }
}

/// The light at a surface point `at` (world) with normal `n` (world),
/// split into the diffuse light (times `albedo`) and the rest.
fn shade_parts(s: &Surface, albedo: &Color, n: &Vector3, at: &Vector3, frame: &Frame) -> Lit {
    let none = Color::new(0.0, 0.0, 0.0);
    if s.kind == MaterialKind::MeshNormal {
        let mut vn = *n;
        vn.transform_direction(&frame.view);
        // three.js writes the packed normal without a colour space, so
        // undo the sRGB encoding the output applies
        let c = Color::new(srgb_to_linear(vn.x * 0.5 + 0.5), srgb_to_linear(vn.y * 0.5 + 0.5), srgb_to_linear(vn.z * 0.5 + 0.5));
        return Lit { diffuse: c, add: none };
    }
    if !s.lit {
        return Lit { diffuse: *albedo, add: none };
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
    let mut pr = 0.0;
    let mut pg = 0.0;
    let mut pb = 0.0;
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
            if s.physical && nl > 0.0 {
                // three.js's RE_Direct_Physical: irradiance * BRDF_GGX_Multiscatter
                let f0 = mix_color(&Color::new(0.04, 0.04, 0.04), albedo, s.metalness);
                let brdf = brdf_ggx_multiscatter(&dir, &to_eye, n, &f0, s.roughness, &frame.dfg, DFG_SIZE);
                let e = l.intensity * k * nl;
                pr += c.r * e * brdf.r;
                pg += c.g * e * brdf.g;
                pb += c.b * e * brdf.b;
            } else if s.specular_on && nl > 0.0 {
                let mut h = dir;
                h.add(&to_eye);
                h.normalize();
                // three.js's BRDF_BlinnPhong: (s / 2 + 1) / pi * 0.25
                let spec = n.dot(&h).max(0.0).powf(s.shininess) * (s.shininess + 2.0) / 8.0 * RECIPROCAL_PI;
                let e = l.intensity * k * spec * nl;
                sr += c.r * e;
                sg += c.g * e;
                sb += c.b * e;
            }
        }
        // BRDF_Lambert: albedo / pi (three.js's physical light units)
        let e = l.intensity * k * diffuse * RECIPROCAL_PI;
        r += c.r * e;
        g += c.g * e;
        b += c.b * e;
    }
    let d = s.diffuse_scale;
    Lit {
        diffuse: Color::new(albedo.r * r * d, albedo.g * g * d, albedo.b * b * d),
        add: Color::new(
            s.specular.r * sr + pr + s.emissive.r,
            s.specular.g * sg + pg + s.emissive.g,
            s.specular.b * sb + pb + s.emissive.b,
        ),
    }
}

pub fn collect_lights(objects: &Vec<ObjectRef>) -> Vec<LightIn> {
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
            textures: Vec::new(),
            tone_mapping: 0,
            tone_mapping_exposure: 1.0,
            cur_tone: 0,
            dfg: Vec::new(),
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

    /// Stores a texture's pixels: `data` is RGBA, 4 bytes a pixel, row 0
    /// at the top; `srgb` converts them to linear. Same id replaces.
    pub fn set_texture_image(&mut self, texture_id: i64, version: i64, width: i64, height: i64, data: Vec<u8>, srgb: bool) {
        let mut lut: Vec<f64> = Vec::new();
        for i in 0..256 {
            let c = i as f64 / 255.0;
            lut.push(if srgb { srgb_to_linear(c) } else { c });
        }
        let n = (width * height) as usize;
        let mut px: Vec<f64> = vec![0.0; n * 4];
        for i in 0..n {
            if i * 4 + 3 >= data.len() {
                break;
            }
            px[i * 4] = lut[data[i * 4] as usize];
            px[i * 4 + 1] = lut[data[i * 4 + 1] as usize];
            px[i * 4 + 2] = lut[data[i * 4 + 2] as usize];
            px[i * 4 + 3] = data[i * 4 + 3] as f64 / 255.0;
        }
        let img = TexImage { id: texture_id, version, width: width.max(1), height: height.max(1), data: px };
        let slot = self.texture_slot(texture_id);
        if slot >= 0 {
            self.textures[slot as usize] = img;
        } else {
            self.textures.push(img);
        }
    }

    /// Uploads a texture's own image (native callers; the JS facade calls
    /// `set_texture_image` with decoded canvas or image pixels).
    pub fn upload_texture(&mut self, texture: &Texture) {
        if let Some(im) = &texture.image {
            let img: Image = im.borrow().clone();
            let srgb = texture.color_space == ColorSpace::Srgb;
            self.set_texture_image(texture.id, texture.version, img.width, img.height, img.data, srgb);
        }
    }

    /// The version of the stored pixels for a texture id, -1 when none.
    pub fn texture_version(&self, texture_id: i64) -> i64 {
        let slot = self.texture_slot(texture_id);
        if slot < 0 {
            return -1;
        }
        self.textures[slot as usize].version
    }

    pub fn remove_texture(&mut self, texture_id: i64) {
        let slot = self.texture_slot(texture_id);
        if slot >= 0 {
            self.textures.remove(slot as usize);
        }
    }

    fn texture_slot(&self, texture_id: i64) -> i64 {
        for i in 0..self.textures.len() {
            if self.textures[i].id == texture_id {
                return i as i64;
            }
        }
        -1
    }

    fn sampler_of(&self, material: &Material, frame: &Frame) -> Sampler {
        let mut fog_color = Color::new(0.0, 0.0, 0.0);
        if let Some(f) = &frame.fog {
            fog_color = f.color;
        }
        let mut s = Sampler {
            slot: -1,
            e: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            transform: false,
            wrap_s: CLAMP_TO_EDGE_WRAPPING,
            wrap_t: CLAMP_TO_EDGE_WRAPPING,
            flip_y: false,
            nearest: false,
            alpha_test: material.alpha_test,
            fog_color,
            fog: None,
        };
        if material.fog {
            s.fog = frame.fog.clone();
        }
        if let Some(t) = &material.map {
            let tb = t.borrow();
            s.slot = self.texture_slot(tb.id);
            let mut m = tb.matrix;
            if tb.matrix_auto_update {
                m.set_uv_transform(tb.offset.x, tb.offset.y, tb.repeat.x, tb.repeat.y, tb.rotation, tb.center.x, tb.center.y);
            }
            s.e = m.elements;
            s.transform = tb.mapping == UV_MAPPING;
            s.wrap_s = tb.wrap_s;
            s.wrap_t = tb.wrap_t;
            s.flip_y = tb.flip_y;
            s.nearest = tb.mag_filter == NEAREST_FILTER;
        }
        s
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
        let mut alpha = self.clear_alpha;
        let mut fog: Option<Fog> = None;
        if let ObjectKind::Scene(s) = &scene.borrow().kind {
            if let Background::Color(c) = &s.background {
                // three.js draws a colour background opaque
                clear = *c;
                alpha = 1.0;
            }
            fog = s.fog.clone();
        }
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
        if self.dfg.is_empty() {
            self.dfg = dfg_table();
        }
        let dfg = self.dfg.clone();
        let frame = Frame { view, projection, near, camera_position, lights, fog, dfg };
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
        self.cur_tone = if material.tone_mapped && material.kind != MaterialKind::MeshNormal && material.kind != MaterialKind::MeshDepth { self.tone_mapping } else { 0 };
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

    fn tone(&self, c: &Color) -> Color {
        if self.cur_tone == 0 {
            return *c;
        }
        tone_map(c, self.cur_tone, self.tone_mapping_exposure)
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
        let sampler = self.sampler_of(material, frame);
        let textured = sampler.slot >= 0;
        let uvs = if textured { geometry.get_attribute("uv") } else { None };
        let blend = material.transparent && (opacity < 1.0 || textured);
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
                    vs.push(vert(v, self.fogged(&base, v.z, frame, material.fog)));
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
            let mut flat = Lit { diffuse: Color::new(0.0, 0.0, 0.0), add: Color::new(0.0, 0.0, 0.0) };
            if normals.is_none() {
                let mut n = face_n;
                if flip {
                    n.negate();
                }
                flat = shade_parts(&surface, &base, &n, &centre, frame);
            }
            for k in 0..3 {
                let mut albedo = base;
                if let Some(ca) = &colors {
                    let c = ca.borrow().get_vector3(ids[k]);
                    albedo = mul_color(&albedo, &Color::new(c.x, c.y, c.z));
                }
                let mut lit = flat;
                if let Some(na) = &normals {
                    let mut n = na.borrow().get_vector3(ids[k]);
                    n.apply_normal_matrix(&normal_matrix);
                    if flip {
                        n.negate();
                    }
                    lit = shade_parts(&surface, &albedo, &n, &wp[k], frame);
                } else if colors.is_some() {
                    let mut n = face_n;
                    if flip {
                        n.negate();
                    }
                    lit = shade_parts(&surface, &albedo, &n, &wp[k], frame);
                }
                let mut v = wp[k];
                v.apply_matrix4(&frame.view);
                if textured {
                    let mut vx = vert(v, lit.diffuse);
                    vx.add = lit.add;
                    if let Some(ua) = &uvs {
                        let ub = ua.borrow();
                        vx.u = ub.get_x(ids[k]);
                        vx.v = ub.get_y(ids[k]);
                    }
                    verts.push(vx);
                } else if material.kind == MaterialKind::MeshDepth {
                    // BasicDepthPacking: 1 - window depth, written without a
                    // colour space (undo the output's sRGB encoding)
                    let ndc = self.to_screen(&v, frame).z;
                    let d = srgb_to_linear((1.0 - (ndc * 0.5 + 0.5)).max(0.0).min(1.0));
                    verts.push(vert(v, Color::new(d, d, d)));
                } else {
                    let sum = Color::new(lit.diffuse.r + lit.add.r, lit.diffuse.g + lit.add.g, lit.diffuse.b + lit.add.b);
                    verts.push(vert(v, self.fogged(&sum, v.z, frame, material.fog)));
                }
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
                if textured {
                    self.fill_textured(&screen[0], &screen[k], &screen[k + 1], &clipped[0], &clipped[k], &clipped[k + 1], &sampler, opacity, blend);
                } else {
                    self.fill_triangle(&screen[0], &screen[k], &screen[k + 1], &clipped[0].color, &clipped[k].color, &clipped[k + 1].color, opacity, blend);
                }
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
            verts.push(vert(p, self.fogged(&c, p.z, frame, material.fog)));
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
        // three.js: size * (drawing buffer height / 2) / depth
        let scale = self.height as f64 * 0.5;
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
            // as GL: the pixels whose centres fall in the point's square,
            // at least one pixel
            let half = (size * 0.5).max(0.5);
            let x0 = (s.x - half - 0.5).ceil() as i64;
            let x1 = (s.x + half - 0.5).ceil() as i64;
            let y0 = (s.y - half - 0.5).ceil() as i64;
            let y1 = (s.y + half - 0.5).ceil() as i64;
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
        let o = self.tone(c);
        self.pixels[p] = to_byte(o.r);
        self.pixels[p + 1] = to_byte(o.g);
        self.pixels[p + 2] = to_byte(o.b);
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
                        let o = self.tone(&Color::new(w0 * ca.r + w1 * cb.r + w2 * cc.r, w0 * ca.g + w1 * cb.g + w2 * cc.g, w0 * ca.b + w1 * cb.b + w2 * cc.b));
                        let r = to_byte(o.r);
                        let g = to_byte(o.g);
                        let bl = to_byte(o.b);
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

    /// A textured triangle: perspective-correct UVs, the texel times the
    /// diffuse light plus the specular and emissive light, then fog.
    fn fill_textured(&mut self, a: &Vector3, b: &Vector3, c: &Vector3, va: &Vert, vb: &Vert, vc: &Vert, s: &Sampler, opacity: f64, blend: bool) {
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i64;
        let max_x = a.x.max(b.x).max(c.x).ceil().min(self.width as f64 - 1.0) as i64;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i64;
        let max_y = a.y.max(b.y).max(c.y).ceil().min(self.height as f64 - 1.0) as i64;
        let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        if area == 0.0 {
            return;
        }
        let slot = s.slot as usize;
        let tw = self.textures[slot].width;
        let th = self.textures[slot].height;
        // 1/w and attributes over w, w = -view z
        let ia = 1.0 / (-va.view.z).max(1e-6);
        let ib = 1.0 / (-vb.view.z).max(1e-6);
        let ic = 1.0 / (-vc.view.z).max(1e-6);
        let e = s.e;
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
                        let q0 = w0 * ia;
                        let q1 = w1 * ib;
                        let q2 = w2 * ic;
                        let qs = 1.0 / (q0 + q1 + q2);
                        let p0 = q0 * qs;
                        let p1 = q1 * qs;
                        let p2 = q2 * qs;
                        let mut u = p0 * va.u + p1 * vb.u + p2 * vc.u;
                        let mut v = p0 * va.v + p1 * vb.v + p2 * vc.v;
                        if s.transform {
                            let tu = e[0] * u + e[3] * v + e[6];
                            let tv = e[1] * u + e[4] * v + e[7];
                            u = tu;
                            v = tv;
                        }
                        // repeat wraps per texel below, so u = 1 stays seamless
                        if s.wrap_s != REPEAT_WRAPPING {
                            u = wrap_coord(u, s.wrap_s);
                        }
                        if s.wrap_t != REPEAT_WRAPPING {
                            v = wrap_coord(v, s.wrap_t);
                        }
                        if s.flip_y {
                            v = 1.0 - v;
                        }
                        let mut tr = 0.0;
                        let mut tg = 0.0;
                        let mut tb = 0.0;
                        let mut ta = 0.0;
                        if s.nearest {
                            let ix = wrap_texel((u * tw as f64).floor() as i64, tw, s.wrap_s);
                            let iy = wrap_texel((v * th as f64).floor() as i64, th, s.wrap_t);
                            let k = ((iy * tw + ix) * 4) as usize;
                            tr = self.textures[slot].data[k];
                            tg = self.textures[slot].data[k + 1];
                            tb = self.textures[slot].data[k + 2];
                            ta = self.textures[slot].data[k + 3];
                        } else {
                            let fx = u * tw as f64 - 0.5;
                            let fy = v * th as f64 - 0.5;
                            let x0 = fx.floor();
                            let y0 = fy.floor();
                            let ax = fx - x0;
                            let ay = fy - y0;
                            let xa = wrap_texel(x0 as i64, tw, s.wrap_s);
                            let xb = wrap_texel(x0 as i64 + 1, tw, s.wrap_s);
                            let ya = wrap_texel(y0 as i64, th, s.wrap_t);
                            let yb = wrap_texel(y0 as i64 + 1, th, s.wrap_t);
                            let k00 = ((ya * tw + xa) * 4) as usize;
                            let k10 = ((ya * tw + xb) * 4) as usize;
                            let k01 = ((yb * tw + xa) * 4) as usize;
                            let k11 = ((yb * tw + xb) * 4) as usize;
                            let f00 = (1.0 - ax) * (1.0 - ay);
                            let f10 = ax * (1.0 - ay);
                            let f01 = (1.0 - ax) * ay;
                            let f11 = ax * ay;
                            for ch in 0..4 {
                                let t = &self.textures[slot].data;
                                let val = t[k00 + ch] * f00 + t[k10 + ch] * f10 + t[k01 + ch] * f01 + t[k11 + ch] * f11;
                                if ch == 0 {
                                    tr = val;
                                } else if ch == 1 {
                                    tg = val;
                                } else if ch == 2 {
                                    tb = val;
                                } else {
                                    ta = val;
                                }
                            }
                        }
                        let alpha = ta * opacity;
                        if alpha < s.alpha_test || alpha <= 0.0 {
                            x += 1;
                            continue;
                        }
                        let mut fog = 0.0;
                        if let Some(f) = &s.fog {
                            fog = f.factor(qs);
                        }
                        let mut r = (w0 * va.color.r + w1 * vb.color.r + w2 * vc.color.r) * tr + w0 * va.add.r + w1 * vb.add.r + w2 * vc.add.r;
                        let mut g = (w0 * va.color.g + w1 * vb.color.g + w2 * vc.color.g) * tg + w0 * va.add.g + w1 * vb.add.g + w2 * vc.add.g;
                        let mut bl = (w0 * va.color.b + w1 * vb.color.b + w2 * vc.color.b) * tb + w0 * va.add.b + w1 * vb.add.b + w2 * vc.add.b;
                        if fog > 0.0 {
                            r += (s.fog_color.r - r) * fog;
                            g += (s.fog_color.g - g) * fog;
                            bl += (s.fog_color.b - bl) * fog;
                        }
                        let o = self.tone(&Color::new(r, g, bl));
                        r = o.r;
                        g = o.g;
                        bl = o.b;
                        let p = at * 4;
                        if blend {
                            self.pixels[p] = to_byte(r) * alpha + self.pixels[p] * (1.0 - alpha);
                            self.pixels[p + 1] = to_byte(g) * alpha + self.pixels[p + 1] * (1.0 - alpha);
                            self.pixels[p + 2] = to_byte(bl) * alpha + self.pixels[p + 2] * (1.0 - alpha);
                        } else {
                            self.depth[at] = z;
                            self.pixels[p] = to_byte(r);
                            self.pixels[p + 1] = to_byte(g);
                            self.pixels[p + 2] = to_byte(bl);
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
