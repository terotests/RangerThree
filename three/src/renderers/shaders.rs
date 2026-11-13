//! The GLSL ES 3.00 shaders of the GPU backends, and how the main program's
//! variants are put together. One source for every backend: the WebGL2
//! backend (js/three-webgl.js) and the GPU command renderer (gpu_frame.rs)
//! take their programs from here. A variant is the main shader with the
//! features one draw uses as constants (`SPEC` uniforms and `K_` flags), so it
//! holds only the branches that draw takes, and one light loop per light kind.

use ranger::prelude::*;

/// Most lights one frame shades (`u_lights` holds seven vec4s for each).
pub const MAX_LIGHTS: i64 = 16;

/// Uniforms a variant turns into constants, in key order.
pub fn spec_names() -> Vec<String> {
    vec![
        String::from("u_hasMap"),
        String::from("u_points"),
        String::from("u_hasNormal"),
        String::from("u_opaque"),
        String::from("u_tone"),
        String::from("u_maps"),
        String::from("u_linearOut"),
        String::from("u_premult"),
        String::from("u_hasEmap"),
        String::from("u_dashed"),
        String::from("u_receive"),
        String::from("u_instanced"),
        String::from("u_hasDisp"),
        String::from("u_sprite"),
    ]
}

/// The main program's feature flags, in key order after `spec_names`
/// (`K_SHSLOTS`: bit per shadow map slot the frame fills, 1-4 the 2D slots 0-2, 8 the cube); then
/// the light counts per kind (ambient, hemisphere, directional, point, spot,
/// rect area), packed in that order.
pub fn flag_names() -> Vec<String> {
    vec![
        String::from("K_MODE"),
        String::from("K_LIT"),
        String::from("K_PHONG"),
        String::from("K_PHYS"),
        String::from("K_FLAT"),
        String::from("K_SHEEN"),
        String::from("K_TRANS"),
        String::from("K_RECT"),
        String::from("K_FOG"),
        String::from("K_SHPASS"),
        String::from("K_ENV"),
        String::from("K_SHSLOTS"),
        String::from("K_NAMB"),
        String::from("K_NHEMI"),
        String::from("K_NDIR"),
        String::from("K_NPOINT"),
        String::from("K_NSPOT"),
        String::from("K_NRECT"),
    ]
}

/// The main program's uniforms, in the order the GPU backends look them up
/// (GPU command UNIFORM locations are positions in this list).
pub fn uniform_names() -> Vec<String> {
    vec![
        String::from("u_model"),
        String::from("u_view"),
        String::from("u_proj"),
        String::from("u_uvt"),
        String::from("u_instanced"),
        String::from("u_pointSize"),
        String::from("u_pointScale"),
        String::from("u_sizeAtt"),
        String::from("u_mat"),
        String::from("u_lights"),
        String::from("u_nlights"),
        String::from("u_fog"),
        String::from("u_cam"),
        String::from("u_map"),
        String::from("u_hasMap"),
        String::from("u_points"),
        String::from("u_hasNormal"),
        String::from("u_opaque"),
        String::from("u_tone"),
        String::from("u_exposure"),
        String::from("u_sprite"),
        String::from("u_center"),
        String::from("u_rotation"),
        String::from("u_emap"),
        String::from("u_hasEmap"),
        String::from("u_euvt"),
        String::from("u_dfg"),
        String::from("u_alphaMap"),
        String::from("u_roughMap"),
        String::from("u_metalMap"),
        String::from("u_normalMap"),
        String::from("u_aoMap"),
        String::from("u_matcap"),
        String::from("u_gradMap"),
        String::from("u_maps"),
        String::from("u_linearOut"),
        String::from("u_premult"),
        String::from("u_t"),
        String::from("u_normalScale"),
        String::from("u_aoIntensity"),
        String::from("u_dispMap"),
        String::from("u_hasDisp"),
        String::from("u_dispT"),
        String::from("u_dispScale"),
        String::from("u_dispBias"),
        String::from("u_dashScale"),
        String::from("u_dashed"),
        String::from("u_dashSize"),
        String::from("u_dashTotal"),
        String::from("u_bumpScale"),
        String::from("u_bumpMap"),
        String::from("u_sheen"),
        String::from("u_trans"),
        String::from("u_attColor"),
        String::from("u_transMap"),
        String::from("u_transSize"),
        String::from("u_ltc"),
        String::from("u_envMap"),
        String::from("u_env"),
        String::from("u_envRot"),
        String::from("u_envUV"),
        String::from("u_sh0"),
        String::from("u_sh1"),
        String::from("u_sh2"),
        String::from("u_shc0"),
        String::from("u_shMat"),
        String::from("u_shParam"),
        String::from("u_shSize"),
        String::from("u_receive"),
        String::from("u_normalMat"),
        String::from("u_viewInv"),
    ]
}

/// `spec_names` then `flag_names`: the order of a variant's values.
pub fn key_names() -> Vec<String> {
    let mut v = spec_names();
    for n in flag_names() {
        v.push(n);
    }
    v
}

/// `line` without its newline is `uniform int <name>;` for a name in `spec`.
fn is_spec_uniform(line: &str, spec: &Vec<String>) -> bool {
    if !line.starts_with("uniform int ") || !line.ends_with(";") {
        return false;
    }
    let n = line.as_bytes().len();
    let name = line[12..n - 1].to_string();
    for s in spec.iter() {
        if *s == name {
            return true;
        }
    }
    false
}

/// `src` without the `uniform int` lines of the uniforms in `spec` (they are #defines in a variant).
fn strip_spec_uniforms(src: &str, spec: &Vec<String>) -> String {
    let mut out = String::new();
    let mut start: usize = 0;
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut i: usize = 0;
    while i < n {
        if bytes[i] == 10 {
            let line = src[start..i].to_string();
            if !is_spec_uniform(&line, spec) {
                out.push_str(&line);
                out.push_str("\n");
            }
            start = i + 1;
        }
        i += 1;
    }
    if start < n {
        out.push_str(&src[start..n]);
    }
    out
}

/// The main fragment shader of one variant: `shadow_type` (1 PCF, also without
/// shadows; 2 basic; 3 VSM) and `vals` in `key_names` order.
pub fn main_fragment(shadow_type: i64, vals: &Vec<i64>) -> String {
    let keys = key_names();
    let mut head = String::from("#version 300 es\n#define SHADOW_TYPE ");
    head.push_str(&shadow_type.to_string());
    head.push_str("\n");
    for i in 0..keys.len() {
        head.push_str("#define ");
        head.push_str(&keys[i]);
        head.push_str(" ");
        head.push_str(&vals[i].to_string());
        head.push_str("\n");
    }
    let body = strip_spec_uniforms(&main_fragment_template(), &spec_names());
    // one loop per light kind the frame has, with the kind a constant
    let mut loops = String::new();
    let mut first: i64 = 0;
    let at = keys.len() - 6;
    let light = light_body();
    for kind in 0..6 {
        let count = vals[at + kind];
        if count == 0 {
            continue;
        }
        loops.push_str("    for (int i = ");
        loops.push_str(&first.to_string());
        loops.push_str("; i < ");
        loops.push_str(&(first + count).to_string());
        loops.push_str("; i++) {\n");
        loops.push_str("      vec4 l0 = u_lights[i * 7], l1 = u_lights[i * 7 + 1], l2 = u_lights[i * 7 + 2], l3 = u_lights[i * 7 + 3], l4 = u_lights[i * 7 + 4];\n");
        loops.push_str("      const float kind = ");
        loops.push_str(&(kind as i64).to_string());
        loops.push_str(".0;\n");
        loops.push_str(&light);
        loops.push_str("    }\n");
        first += count;
    }
    head.push_str(&body.replace("//@LIGHTS\n", &loops));
    head
}

/// The main vertex shader of one variant (`vals` as for `main_fragment`; the
/// vertex shader reads only the `spec_names` part).
pub fn main_vertex(vals: &Vec<i64>) -> String {
    let spec = spec_names();
    let mut head = String::from("#version 300 es\n");
    for i in 0..spec.len() {
        head.push_str("#define ");
        head.push_str(&spec[i]);
        head.push_str(" ");
        head.push_str(&vals[i].to_string());
        head.push_str("\n");
    }
    let body = strip_spec_uniforms(&main_vertex_template(), &spec);
    head.push_str(&body.replace("#version 300 es\n", ""));
    head
}

/// Colour-space helpers and three.js's tone mapping operators (tonemapping_pars_fragment).
pub fn common() -> String {
    String::from(r#"vec3 toLinear(vec3 c) { return mix(c * 0.0773993808, pow(c * 0.9478672986 + 0.0521327014, vec3(2.4)), step(0.04045, c)); }
vec3 toSrgb(vec3 c) { return mix(c * 12.92, 1.055 * pow(c, vec3(0.41666)) - 0.055, step(0.0031308, c)); }
vec3 rrtOdt(vec3 v) { vec3 a = v * (v + 0.0245786) - 0.000090537; vec3 b = v * (0.983729 * v + 0.4329510) + 0.238081; return a / b; }
vec3 agxContrast(vec3 x) { vec3 x2 = x * x; vec3 x4 = x2 * x2; return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232; }
vec3 toneMap(vec3 c) {
  if (u_tone == 1) return clamp(u_exposure * c, 0.0, 1.0);
  if (u_tone == 2) { c *= u_exposure; return clamp(c / (vec3(1.0) + c), 0.0, 1.0); }
  if (u_tone == 3) { c *= u_exposure; c = max(vec3(0.0), c - 0.004); return pow((c * (6.2 * c + 0.5)) / (c * (6.2 * c + 1.7) + 0.06), vec3(2.2)); }
  if (u_tone == 4) {
    const mat3 inM = mat3(vec3(0.59719, 0.07600, 0.02840), vec3(0.35458, 0.90834, 0.13383), vec3(0.04823, 0.01566, 0.83777));
    const mat3 outM = mat3(vec3(1.60475, -0.10208, -0.00327), vec3(-0.53108, 1.10813, -0.07276), vec3(-0.07367, -0.00605, 1.07602));
    c *= u_exposure / 0.6; c = outM * rrtOdt(inM * c); return clamp(c, 0.0, 1.0);
  }
  if (u_tone == 6) {
    const mat3 toRec2020 = mat3(vec3(0.6274, 0.0691, 0.0164), vec3(0.3293, 0.9195, 0.0880), vec3(0.0433, 0.0113, 0.8956));
    const mat3 fromRec2020 = mat3(vec3(1.6605, -0.1246, -0.0182), vec3(-0.5876, 1.1329, -0.1006), vec3(-0.0728, -0.0083, 1.1187));
    const mat3 inset = mat3(vec3(0.856627153315983, 0.137318972929847, 0.11189821299995), vec3(0.0951212405381588, 0.761241990602591, 0.0767994186031903), vec3(0.0482516061458583, 0.101439036467562, 0.811302368396859));
    const mat3 outset = mat3(vec3(1.1271005818144368, -0.1413297634984383, -0.14132976349843826), vec3(-0.11060664309660323, 1.157823702216272, -0.11060664309660294), vec3(-0.016493938717834573, -0.016493938717834257, 1.2519364065950405));
    c *= u_exposure; c = inset * (toRec2020 * c);
    c = clamp((log2(max(c, 1e-10)) + 12.47393) / (4.026069 + 12.47393), 0.0, 1.0);
    c = outset * agxContrast(c); c = pow(max(vec3(0.0), c), vec3(2.2));
    return clamp(fromRec2020 * c, 0.0, 1.0);
  }
  if (u_tone == 7) {
    c *= u_exposure; float x = min(c.r, min(c.g, c.b)); float off = x < 0.08 ? x - 6.25 * x * x : 0.04; c -= off;
    float peak = max(c.r, max(c.g, c.b)); if (peak < 0.76) return c;
    float d = 0.24; float np = 1.0 - d * d / (peak + d - 0.76); c *= np / peak;
    float g = 1.0 - 1.0 / (0.15 * (peak - np) + 1.0); return mix(c, vec3(np), g);
  }
  return c;
}
"#)
}

/// The main vertex shader before its variant `#define`s (`main_vertex`).
pub fn main_vertex_template() -> String {
    String::from(r#"#version 300 es
layout(location=0) in vec3 a_position;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec2 a_uv;
layout(location=3) in vec3 a_color;
layout(location=4) in vec4 a_im0;
layout(location=5) in vec4 a_im1;
layout(location=6) in vec4 a_im2;
layout(location=7) in vec4 a_im3;
layout(location=8) in vec3 a_icolor;
layout(location=9) in float a_lineDistance;
uniform sampler2D u_dispMap;
#if u_hasDisp == 1
#define T_DISP(uv) texture(u_dispMap, uv)
#else
#define T_DISP(uv) vec4(0.0)
#endif
uniform int u_hasDisp;
uniform mat3 u_dispT;
uniform float u_dispScale, u_dispBias, u_dashScale;
uniform mat4 u_model, u_view, u_proj;
uniform mat3 u_uvt;
uniform int u_instanced;
uniform float u_pointSize, u_pointScale;
uniform int u_sizeAtt;
uniform int u_sprite;
uniform mat3 u_normalMat;
uniform mat4 u_viewInv;
uniform vec2 u_center;
uniform float u_rotation;
out vec3 v_world; out vec3 v_normal; out vec2 v_uv; out vec2 v_uv0; out vec3 v_color; out float v_depth; out float v_lineDist;
void main() {
  mat4 m = u_model;
  if (u_instanced == 1) m = u_model * mat4(a_im0, a_im1, a_im2, a_im3);
  vec3 pos = a_position;
  if (u_hasDisp == 1) pos += normalize(a_normal) * (T_DISP((u_dispT * vec3(a_uv, 1.0)).xy).x * u_dispScale + u_dispBias);
  v_lineDist = u_dashScale * a_lineDistance;
  vec4 w = m * vec4(pos, 1.0);
  v_world = w.xyz;
  vec3 nrm = a_normal;
  if (u_instanced == 1) { mat3 im = mat3(a_im0.xyz, a_im1.xyz, a_im2.xyz); nrm /= vec3(dot(im[0], im[0]), dot(im[1], im[1]), dot(im[2], im[2])); nrm = im * nrm; }
  v_normal = u_normalMat * nrm;
  v_uv = (u_uvt * vec3(a_uv, 1.0)).xy;
  v_color = a_color * a_icolor;
  vec4 vp = u_view * w;
  v_depth = -vp.z;
  gl_Position = u_proj * vp;
  v_uv0 = a_uv;
  if (u_sprite == 1) {
    vec4 mv = u_view * u_model * vec4(0.0, 0.0, 0.0, 1.0);
    vec2 sc = vec2(length(u_model[0].xyz), length(u_model[1].xyz));
    if (u_sizeAtt == 0 && u_proj[2][3] == -1.0) sc *= -mv.z;
    vec2 ap = (a_position.xy - (u_center - vec2(0.5))) * sc;
    mv.xy += vec2(cos(u_rotation) * ap.x - sin(u_rotation) * ap.y, sin(u_rotation) * ap.x + cos(u_rotation) * ap.y);
    v_world = (u_viewInv * mv).xyz; v_normal = vec3(0.0, 0.0, 1.0); v_depth = -mv.z;
    gl_Position = u_proj * mv;
  }
  gl_PointSize = max(1.0, u_sizeAtt == 1 ? u_pointSize * u_pointScale / max(-vp.z, 1e-4) : u_pointSize);
}
"#)
}

/// The faces of the prefiltered environment: 3 x 2 cube faces per level, one-texel border (three.js's cube_uv_reflection).
pub fn cube_uv() -> String {
    String::from(r#"float faceOf(vec3 d) { vec3 a = abs(d); if (a.x > a.z) return a.x > a.y ? (d.x > 0.0 ? 0.0 : 3.0) : (d.y > 0.0 ? 1.0 : 4.0); return a.z > a.y ? (d.z > 0.0 ? 2.0 : 5.0) : (d.y > 0.0 ? 1.0 : 4.0); }
vec2 faceUv(vec3 d, float f) {
  vec2 uv;
  if (f == 0.0) uv = vec2(d.z, d.y) / abs(d.x); else if (f == 1.0) uv = vec2(-d.x, -d.z) / abs(d.y);
  else if (f == 2.0) uv = vec2(-d.x, d.y) / abs(d.z); else if (f == 3.0) uv = vec2(-d.z, d.y) / abs(d.x);
  else if (f == 4.0) uv = vec2(-d.x, d.z) / abs(d.y); else uv = vec2(d.x, d.y) / abs(d.z);
  return 0.5 * (uv + 1.0);
}
vec3 faceDir(vec2 uv, float f) {
  uv = 2.0 * uv - 1.0; vec3 d = vec3(uv, 1.0);
  if (f == 0.0) d = d.zyx; else if (f == 1.0) { d = d.xzy; d.xz *= -1.0; } else if (f == 2.0) d.x *= -1.0;
  else if (f == 3.0) { d = d.zyx; d.xz *= -1.0; } else if (f == 4.0) { d = d.xzy; d.xy *= -1.0; } else d.z *= -1.0;
  return d;
}
vec3 cubeUv(sampler2D t, vec3 d, float s) {
  float f = faceOf(d); vec2 uv = faceUv(d, f) * (s - 2.0) + 1.0;
  if (f > 2.0) { uv.y += s; f -= 3.0; }
  uv.x += f * s;
  return textureLod(t, uv / vec2(3.0 * s, 2.0 * s), 0.0).rgb;
}
vec2 equirectUv(vec3 d) { return vec2(atan(d.z, d.x) * 0.1591549431 + 0.5, asin(clamp(d.y, -1.0, 1.0)) * 0.3183098862 + 0.5); }
"#)
}

/// The main fragment shader after its `#version` line and `SHADOW_TYPE` (1 PCF, 2 basic, 3 VSM), before
/// its variant `#define`s and light loops (`main_fragment`).
pub fn main_fragment_template() -> String {
    let mut s = String::new();
    s.push_str(r#"precision highp float;
// every texture read goes through a macro that is a constant when its feature is off, so a
// variant's disabled samplers are not statically used: WebGL2 counts statically used samplers
// against MAX_TEXTURE_IMAGE_UNITS (16 on Apple's Metal), even in branches that fold away
#if u_hasMap == 1
#define T_MAP(uv) texture(u_map, uv)
#else
#define T_MAP(uv) vec4(1.0)
#endif
#if (u_maps & 1) != 0
#define T_ALPHA(uv) texture(u_alphaMap, uv)
#else
#define T_ALPHA(uv) vec4(1.0)
#endif
#if (u_maps & 2) != 0
#define T_ROUGH(uv) texture(u_roughMap, uv)
#else
#define T_ROUGH(uv) vec4(1.0)
#endif
#if (u_maps & 4) != 0
#define T_METAL(uv) texture(u_metalMap, uv)
#else
#define T_METAL(uv) vec4(1.0)
#endif
#if (u_maps & 8) != 0
#define T_NORMAL(uv) texture(u_normalMap, uv)
#else
#define T_NORMAL(uv) vec4(0.5, 0.5, 1.0, 1.0)
#endif
#if (u_maps & 16) != 0
#define T_AO(uv) texture(u_aoMap, uv)
#else
#define T_AO(uv) vec4(1.0)
#endif
#if (u_maps & 32) != 0
#define T_MATCAP(uv) texture(u_matcap, uv)
#else
#define T_MATCAP(uv) vec4(1.0)
#endif
#if (u_maps & 64) != 0
#define T_GRAD(uv) texture(u_gradMap, uv)
#else
#define T_GRAD(uv) vec4(1.0)
#endif
#if (u_maps & 128) != 0
#define T_BUMP(uv) texture(u_bumpMap, uv)
#else
#define T_BUMP(uv) vec4(0.0)
#endif
#if u_hasEmap == 1
#define T_EMAP(uv) texture(u_emap, uv)
#else
#define T_EMAP(uv) vec4(1.0)
#endif
#if K_PHYS == 1
#define T_DFG(uv) texture(u_dfg, uv)
#else
#define T_DFG(uv) vec4(0.0)
#endif
#if K_RECT == 1
#define T_LTC(uv) texture(u_ltc, uv)
#else
#define T_LTC(uv) vec4(0.0)
#endif
#if K_ENV != 0
#define T_ENVLOD(uv) textureLod(u_envMap, uv, 0.0)
#define T_ENVGRAD(uv, dx, dy) textureGrad(u_envMap, uv, dx, dy)
#else
#define T_ENVLOD(uv) vec4(0.0)
#define T_ENVGRAD(uv, dx, dy) vec4(0.0)
#endif
#if K_TRANS == 1
#define T_TRANS(uv, lod) textureBicubic(u_transMap, uv, lod)
#else
#define T_TRANS(uv, lod) vec4(0.0)
#endif
in vec3 v_world; in vec3 v_normal; in vec2 v_uv; in vec2 v_uv0; in vec3 v_color; in float v_depth; in float v_lineDist;
uniform int u_dashed;
uniform float u_dashSize, u_dashTotal, u_bumpScale;
uniform sampler2D u_bumpMap;
uniform vec4 u_sheen;
uniform vec4 u_trans;
uniform vec3 u_attColor;
uniform sampler2D u_transMap;
uniform vec2 u_transSize;
uniform mat4 u_model, u_proj;
uniform sampler2D u_emap;
uniform int u_hasEmap;
uniform sampler2D u_alphaMap, u_roughMap, u_metalMap, u_normalMap, u_aoMap, u_matcap, u_gradMap;
uniform int u_maps;
uniform int u_linearOut;
uniform int u_premult;
uniform mat3 u_t[6];
uniform vec2 u_normalScale;
uniform float u_aoIntensity;
uniform mat3 u_euvt;
uniform vec4 u_mat[6];
uniform sampler2D u_dfg;
uniform vec4 u_lights["#);
    s.push_str(&(MAX_LIGHTS * 7).to_string());
    s.push_str(r#"];
uniform sampler2D u_ltc;
#if SHADOW_TYPE == 3
uniform sampler2D u_sh0, u_sh1, u_sh2;
uniform samplerCube u_shc0;
#else
uniform highp sampler2DShadow u_sh0, u_sh1, u_sh2;
uniform highp samplerCubeShadow u_shc0;
#endif
uniform mat4 u_shMat[4];
uniform vec4 u_shParam[4];
uniform vec4 u_shSize[4];
uniform int u_receive;
uniform sampler2D u_envMap;
uniform vec4 u_env, u_envUV;
uniform mat3 u_envRot;
uniform int u_nlights;
uniform vec4 u_fog[2];
uniform vec3 u_cam;
uniform mat4 u_view;
uniform mat3 u_uvt;
uniform sampler2D u_map;
uniform int u_hasMap;
uniform int u_points;
uniform int u_hasNormal;
uniform int u_opaque;
uniform int u_tone;
uniform float u_exposure;
out vec4 o_color;
"#);
    s.push_str(&common());
    s.push_str(&cube_uv());
    s.push_str(r#"bool has(int b) { return (u_maps & b) != 0; }
float bw0(float a) { return (1.0 / 6.0) * (a * (a * (-a + 3.0) - 3.0) + 1.0); }
float bw1(float a) { return (1.0 / 6.0) * (a * a * (3.0 * a - 6.0) + 4.0); }
float bw2(float a) { return (1.0 / 6.0) * (a * (a * (-3.0 * a + 3.0) + 3.0) + 1.0); }
float bw3(float a) { return (1.0 / 6.0) * (a * a * a); }
vec4 bicubic(sampler2D t, vec2 uv, vec2 size, float lod) {
  uv = uv * size + 0.5; vec2 iuv = floor(uv), f = fract(uv);
  float g0x = bw0(f.x) + bw1(f.x), g1x = bw2(f.x) + bw3(f.x), g0y = bw0(f.y) + bw1(f.y), g1y = bw2(f.y) + bw3(f.y);
  float h0x = -1.0 + bw1(f.x) / g0x, h1x = 1.0 + bw3(f.x) / g1x, h0y = -1.0 + bw1(f.y) / g0y, h1y = 1.0 + bw3(f.y) / g1y;
  vec2 px = 1.0 / size;
  return g0y * (g0x * textureLod(t, (vec2(iuv.x + h0x, iuv.y + h0y) - 0.5) * px, lod) + g1x * textureLod(t, (vec2(iuv.x + h1x, iuv.y + h0y) - 0.5) * px, lod)) +
    g1y * (g0x * textureLod(t, (vec2(iuv.x + h0x, iuv.y + h1y) - 0.5) * px, lod) + g1x * textureLod(t, (vec2(iuv.x + h1x, iuv.y + h1y) - 0.5) * px, lod));
}
vec4 textureBicubic(sampler2D t, vec2 uv, float lod) {
  vec4 a = bicubic(t, uv, vec2(textureSize(t, int(lod))), floor(lod));
  vec4 b = bicubic(t, uv, vec2(textureSize(t, int(lod + 1.0))), ceil(lod));
  return mix(a, b, fract(lod));
}
vec3 ltcEdge(vec3 a, vec3 b) { float x = dot(a, b), y = abs(x); float va = 0.8543985 + (0.4965155 + 0.0145206 * y) * y, vb = 3.4175940 + (4.1616724 + y) * y, v = va / vb;
  float ts = x > 0.0 ? v : 0.5 * inversesqrt(max(1.0 - x * x, 1e-7)) - v; return cross(a, b) * ts; }
float ltcEvaluate(vec3 N, vec3 V, vec3 P, mat3 minv, vec3 rc[4]) {
  vec3 e1 = rc[1] - rc[0], e2 = rc[3] - rc[0];
  if (dot(cross(e1, e2), P - rc[0]) < 0.0) return 0.0;
  vec3 t1 = normalize(V - N * dot(V, N)), t2 = -cross(N, t1);
  mat3 m = minv * transpose(mat3(t1, t2, N));
  vec3 c0 = normalize(m * (rc[0] - P)), c1 = normalize(m * (rc[1] - P)), c2 = normalize(m * (rc[2] - P)), c3 = normalize(m * (rc[3] - P));
  vec3 f = ltcEdge(c0, c1) + ltcEdge(c1, c2) + ltcEdge(c2, c3) + ltcEdge(c3, c0);
  float l = length(f); return max((l * l + f.z) / (l + 1.0), 0.0);
}
float ign(vec2 p) { return fract(52.9829189 * fract(dot(p, vec2(0.06711056, 0.00583715)))); }
vec2 vogel(int i, float phi) { float r = sqrt((float(i) + 0.5) / 5.0); float t = float(i) * 2.399963229728653 + phi; return vec2(cos(t), sin(t)) * r; }
#if u_receive == 1
#if SHADOW_TYPE == 3
vec2 shRG(int i, vec2 p) {
#if (K_SHSLOTS & 1) != 0
  if (i == 0) return texture(u_sh0, p).rg;
#endif
#if (K_SHSLOTS & 2) != 0
  if (i == 1) return texture(u_sh1, p).rg;
#endif
#if (K_SHSLOTS & 4) != 0
  if (i == 2) return texture(u_sh2, p).rg;
#endif
  return vec2(1.0);
}
#else
float shTap(int i, vec3 p) {
#if (K_SHSLOTS & 1) != 0
  if (i == 0) return texture(u_sh0, p);
#endif
#if (K_SHSLOTS & 2) != 0
  if (i == 1) return texture(u_sh1, p);
#endif
#if (K_SHSLOTS & 4) != 0
  if (i == 2) return texture(u_sh2, p);
#endif
  return 1.0;
}
#if (K_SHSLOTS & 8) != 0
#define T_SHC(p) texture(u_shc0, p)
#else
#define T_SHC(p) 1.0
#endif
#endif
float shadowAt(int slot, vec3 nrm) {
  vec4 pr = u_shParam[slot], sz = u_shSize[slot];
  vec4 sc = u_shMat[slot] * vec4(v_world + nrm * pr.z, 1.0);
  float shadow = 1.0;
  if (slot == 3) {
#if SHADOW_TYPE != 3
    vec3 av = abs(sc.xyz); float vz = max(max(av.x, av.y), av.z);
    if (vz - sz.w <= 0.0 && vz - sz.z >= 0.0) {
      float dp = (sz.w * (vz - sz.z)) / (vz * (sz.w - sz.z)) + pr.y;
      vec3 bd = normalize(sc.xyz);
#if SHADOW_TYPE == 1
      float texel = pr.w / sz.x; vec3 ad = abs(bd);
      vec3 tg = normalize(cross(bd, ad.x > ad.z ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0))); vec3 bt = cross(bd, tg);
      float phi = ign(gl_FragCoord.xy) * 6.283185307179586;
      shadow = 0.0;
      for (int k = 0; k < 5; k++) { vec2 o = vogel(k, phi); shadow += T_SHC(vec4(bd + (tg * o.x + bt * o.y) * texel, dp)); }
      shadow *= 0.2;
#else
      shadow = T_SHC(vec4(bd, dp));
#endif
    }
#endif
  } else {
    sc.xyz /= sc.w; sc.z += pr.y;
    if (sc.x >= 0.0 && sc.x <= 1.0 && sc.y >= 0.0 && sc.y <= 1.0 && sc.z <= 1.0) {
#if SHADOW_TYPE == 1
      float rad = pr.w / sz.x, phi = ign(gl_FragCoord.xy) * 6.283185307179586;
      shadow = 0.0;
      for (int k = 0; k < 5; k++) shadow += shTap(slot, vec3(sc.xy + vogel(k, phi) * rad, sc.z));
      shadow *= 0.2;
#elif SHADOW_TYPE == 2
      shadow = shTap(slot, sc.xyz);
#else
      vec2 ds = shRG(slot, sc.xy); float mean = ds.x, variance = ds.y * ds.y;
      float hard = step(sc.z, mean);
      if (hard == 1.0) shadow = 1.0;
      else { variance = max(variance, 0.0000001); float d = sc.z - mean; float pm = clamp((variance / (variance + d * d) - 0.3) / 0.65, 0.0, 1.0); shadow = max(hard, pm); }
#endif
    }
  }
  return mix(1.0, shadow, pr.x);
}
#else
float shadowAt(int slot, vec3 nrm) { return 1.0; }
#endif
float sheenAlbedo(float nv, float r) { float r2 = r * r, ri = 1.0 / (r + 0.1); float a = -1.9362 + 1.0678 * r + 0.4573 * r2 - 0.8469 * ri; float b = -0.6014 + 0.5538 * r - 0.4670 * r2 - 0.1255 * ri; return clamp(exp(a * clamp(nv, 0.0, 1.0) + b), 0.0, 1.0); }
float envMip(float r) {
  if (r >= 0.8) return (1.0 - r) * 5.0 - 2.0; if (r >= 0.4) return (0.8 - r) * 7.5 - 1.0;
  if (r >= 0.305) return (0.4 - r) / 0.095 + 2.0; if (r >= 0.21) return (0.305 - r) / 0.095 + 3.0; return -2.0 * log2(1.16 * r);
}
vec3 envLevel(vec3 d, float mipInt) {
  float f = faceOf(d), fi = max(4.0 - mipInt, 0.0); mipInt = max(mipInt, 4.0); float fs = exp2(mipInt);
  vec2 uv = faceUv(d, f) * (fs - 2.0) + 1.0;
  if (f > 2.0) { uv.y += fs; f -= 3.0; }
  uv.x += f * fs + fi * 48.0; uv.y += 4.0 * (exp2(u_envUV.x) - fs);
  return T_ENVLOD(uv * u_envUV.yz).rgb;
}
vec3 envCubeUV(vec3 d, float r) { float m = clamp(envMip(r), -2.0, u_envUV.x), mf = fract(m), mi = floor(m); vec3 c = envLevel(d, mi); return mf == 0.0 ? c : mix(c, envLevel(d, mi + 1.0), mf); }
vec3 envEquirect(vec3 d) {
  vec2 uv = equirectUv(d), dx = dFdx(uv), dy = dFdy(uv);
  dx.x -= floor(dx.x + 0.5); dy.x -= floor(dy.x + 0.5);
  return T_ENVGRAD(uv, dx, dy).rgb;
}
vec2 tuv(int i) { return (u_t[i] * vec3(v_uv0, 1.0)).xy; }
void main() {
  if (u_dashed == 1 && mod(v_lineDist, u_dashTotal) > u_dashSize) discard;
  vec4 c0 = u_mat[0], c1 = u_mat[1], c2 = u_mat[2], c3 = u_mat[3], c4 = u_mat[4], c5 = u_mat[5];
  const int mode = K_MODE;
  vec3 albedo = c0.rgb * v_color;
  float alpha = c0.a;
  if (u_hasMap == 1) {
    vec2 uv = v_uv;
    if (u_points == 1) uv = (u_uvt * vec3(gl_PointCoord.x, 1.0 - gl_PointCoord.y, 1.0)).xy;
    vec4 t = T_MAP(uv);
    albedo *= t.rgb; alpha *= t.a;
  }
  if (has(1)) alpha *= T_ALPHA(tuv(0)).g;
  if (alpha < c1.a) discard;
  if (u_opaque == 1) alpha = 1.0;
#if K_SHPASS == 1
  o_color = vec4(albedo, alpha); return;
#endif
  vec3 n;
  if (K_FLAT == 1 || u_hasNormal == 0) n = normalize(cross(dFdx(v_world), dFdy(v_world)));
  else { n = normalize(v_normal); if (!gl_FrontFacing) n = -n; }
  vec3 n0 = n;
  vec3 shN = u_hasNormal == 1 ? normalize(v_normal) : vec3(0.0, 0.0, 1.0);
  if (has(8)) {
    vec2 nuv = tuv(3);
    vec3 mapN = T_NORMAL(nuv).xyz * 2.0 - 1.0; mapN.xy *= u_normalScale;
    vec3 q0 = dFdx(v_world), q1 = dFdy(v_world); vec2 st0 = dFdx(nuv), st1 = dFdy(nuv);
    vec3 q1p = cross(q1, n), q0p = cross(n, q0);
    vec3 tt = q1p * st0.x + q0p * st1.x, bb = q1p * st0.y + q0p * st1.y;
    float det = max(dot(tt, tt), dot(bb, bb)); float sc = det == 0.0 ? 0.0 : inversesqrt(det);
    tt *= sc; bb *= sc;
    if (c4.x > 1.5 && c4.y < 0.5 && u_hasNormal == 1 && !gl_FrontFacing) { tt = -tt; bb = -bb; }
    n = normalize(mat3(tt, bb, n) * mapN);
  } else if (has(128)) {
    vec2 buv = tuv(5), sx = dFdx(buv), sy = dFdy(buv);
    float hll = u_bumpScale * T_BUMP(buv).x;
    vec2 dh = vec2(u_bumpScale * T_BUMP(buv + sx).x - hll, u_bumpScale * T_BUMP(buv + sy).x - hll);
    vec3 sgx = normalize(dFdx(v_world)), sgy = normalize(dFdy(v_world));
    vec3 r1 = cross(sgy, n), r2 = cross(n, sgx);
    float det = dot(sgx, r1) * (gl_FrontFacing ? 1.0 : -1.0);
    n = normalize(abs(det) * n - sign(det) * (dh.x * r1 + dh.y * r2));
  }
  float ao = has(16) ? (T_AO(tuv(4)).r - 1.0) * u_aoIntensity + 1.0 : 1.0;
  vec3 color;
  float transAlpha = 1.0;
  vec3 emissive = c1.rgb;
  if (u_hasEmap == 1) { vec3 e = T_EMAP((u_euvt * vec3(v_uv0, 1.0)).xy).rgb; emissive *= e; }
  if (mode == 3) {
    color = vec3(1.0 - gl_FragCoord.z);
  } else if (mode == 2) {
    vec3 vn = normalize((u_view * vec4(n, 0.0)).xyz);
    color = vn * 0.5 + 0.5;
  } else if (mode == 5) {
    float mask = 1.0;
    for (int i = 0; i < "#);
    s.push_str(&MAX_LIGHTS.to_string());
    s.push_str(r#"; i++) {
      if (i >= u_nlights) break;
      int slot = int(u_lights[i * 7 + 4].z + 0.5) - 1;
      if (slot >= 0 && u_receive == 1) mask *= shadowAt(slot, shN);
    }
    color = c0.rgb; alpha = c0.a * (1.0 - mask);
  } else if (mode == 4) {
    vec3 vn = normalize((u_view * vec4(n, 0.0)).xyz);
    vec3 vd = normalize(-(u_view * vec4(v_world, 1.0)).xyz);
    vec3 mx = normalize(vec3(vd.z, 0.0, -vd.x)); vec3 my = cross(vd, mx);
    vec2 muv = vec2(dot(mx, vn), dot(my, vn)) * 0.495 + 0.5;
    color = albedo * (has(32) ? T_MATCAP(muv).rgb : vec3(mix(0.2, 0.8, muv.y)));
  } else if (K_LIT == 0) {
    color = albedo * ao;
  } else {
    vec3 toEye = normalize(u_cam - v_world);
    vec3 dif = vec3(0.0), ind = vec3(0.0), spec = vec3(0.0), pspec = vec3(0.0), rectDif = vec3(0.0);
    const bool physical = K_PHYS == 1;
    float rough = c5.x, metal = c5.y, dscale = c3.x;
    if (has(2)) rough *= T_ROUGH(tuv(1)).g;
    if (has(4)) metal *= T_METAL(tuv(2)).b;
    vec3 vn0 = (u_view * vec4(n0, 0.0)).xyz;
    vec3 dxy = max(abs(dFdx(vn0)), abs(dFdy(vn0)));
    if (physical) { rough = min(max(rough, 0.0525) + max(max(dxy.x, dxy.y), dxy.z), 1.0); dscale = 1.0 - metal; }
    vec3 f0 = mix(vec3(0.04), albedo, metal);
    float nv = clamp(dot(n, toEye), 0.0, 1.0);
    vec2 dfgV = T_DFG(vec2(rough, nv)).rg;
    const bool sheenOn = K_SHEEN == 1;
    float shR = max(u_sheen.a, 0.0001), shMax = max(u_sheen.r, max(u_sheen.g, u_sheen.b));
    vec3 sheenSpec = vec3(0.0);
//@LIGHTS
    if (sheenOn) ind *= 1.0 - shMax * sheenAlbedo(nv, shR);
    vec3 totalDiffuse = albedo * (dif + ind * ao) * dscale + albedo * dscale * rectDif;
    vec3 envSpec = vec3(0.0);
    if (physical && K_ENV == 1) {
      vec3 irr = 3.141592653589793 * envCubeUV(u_envRot * n, 1.0) * u_env.y;
      vec3 rv = reflect(-toEye, n); float r2 = rough * rough; rv = normalize(mix(rv, n, r2 * r2));
      vec3 rad = envCubeUV(u_envRot * rv, rough) * u_env.y;
      float ess = dfgV.x + dfgV.y, ems = 1.0 - ess;
      vec3 fssD = vec3(0.04) * dfgV.x + dfgV.y, fssM = albedo * dfgV.x + dfgV.y;
      vec3 favgD = vec3(0.04 + 0.96 * 0.047619), favgM = albedo + (1.0 - albedo) * 0.047619;
      vec3 msD = fssD * favgD / (1.0 - ems * favgD) * ems, msM = fssM * favgM / (1.0 - ems * favgM) * ems;
      vec3 cwi = irr * 0.3183098862;
      vec3 ed = albedo * (1.0 - metal) * (1.0 - (fssD + msD)) * cwi;
      envSpec = rad * mix(fssD, fssM, metal) + mix(msD, msM, metal) * cwi;
      if (sheenOn) { float sc = 1.0 - shMax * sheenAlbedo(nv, shR); ed *= sc; envSpec *= sc; }
      if (has(16)) envSpec *= clamp(pow(nv + ao, exp2(-16.0 * rough - 1.0)) - 1.0 + ao, 0.0, 1.0);
      totalDiffuse += ed * ao;
    }
    if (K_TRANS == 1) {
      float ior = u_trans.z;
      vec3 scale = vec3(length(u_model[0].xyz), length(u_model[1].xyz), length(u_model[2].xyz));
      vec3 ray = normalize(refract(-toEye, n, 1.0 / ior)) * u_trans.y * scale;
      vec4 ndc = u_proj * u_view * vec4(v_world + ray, 1.0);
      vec2 rc = (ndc.xy / ndc.w + 1.0) / 2.0;
      vec4 tl = T_TRANS(rc, log2(u_transSize.x) * rough * clamp(ior * 2.0 - 2.0, 0.0, 1.0));
      vec3 tm = albedo * dscale;
      if (u_trans.w > 0.0) tm *= exp(log(u_attColor) / u_trans.w * length(ray));
      vec3 F = f0 * dfgV.x + dfgV.y;
      float tf = (tm.r + tm.g + tm.b) / 3.0;
      transAlpha = mix(1.0, 1.0 - (1.0 - tl.a) * tf, u_trans.x);
      totalDiffuse = mix(totalDiffuse, (1.0 - F) * tm * tl.rgb, u_trans.x);
    }
    color = totalDiffuse + c2.rgb * spec + pspec + emissive + sheenSpec + envSpec;
  }
  if (K_ENV == 2 && mode == 0 && K_PHYS == 0) {
    vec3 ctf = u_proj[3][3] > 0.5 ? normalize(vec3(-u_view[0][2], -u_view[1][2], -u_view[2][2])) : normalize(v_world - u_cam);
    vec3 rv = u_envUV.w > 0.0 ? refract(ctf, n, u_envUV.w) : reflect(ctf, n);
    vec3 ec = envEquirect(u_envRot * rv);
    float st = u_env.w;
    if (u_env.z < 0.5) color = mix(color, color * ec, st); else if (u_env.z < 1.5) color = mix(color, ec, st); else color += ec * st;
  }
  bool raw = mode == 2 || mode == 3;
  if (!raw && u_tone > 0) color = toneMap(color);
  vec3 outc = max(color, 0.0);
  if (!raw && u_linearOut == 0) outc = toSrgb(outc);
  if (K_FOG > 0) {
    float f = K_FOG == 1 ? smoothstep(u_fog[1].x, u_fog[1].y, v_depth)
      : 1.0 - exp(-u_fog[1].z * u_fog[1].z * v_depth * v_depth);
    vec3 fc = clamp(u_fog[0].yzw, 0.0, 1.0);
    outc = mix(outc, u_linearOut == 1 ? fc : toSrgb(fc), f);
  }
  alpha *= transAlpha;
  if (u_premult == 1) outc *= alpha;
  o_color = vec4(outc, alpha);
}
"#);
    s
}

/// One light of a known kind: the loops are written per kind into each variant, so the
/// kind tests fold away; `l0`..`l4` and `kind` are set by the loop.
pub fn light_body() -> String {
    String::from(r#"      if (K_RECT == 1 && kind > 4.5) {
        if (physical) {
          vec3 hw = u_lights[i * 7 + 5].xyz, hh = u_lights[i * 7 + 6].xyz, lc = l0.rgb * l1.w;
          vec3 rc[4]; rc[0] = l2.xyz + hw - hh; rc[1] = l2.xyz - hw - hh; rc[2] = l2.xyz - hw + hh; rc[3] = l2.xyz + hw + hh;
          vec2 luv = vec2(rough, sqrt(1.0 - nv)) * (63.0 / 64.0) + 0.5 / 64.0;
          vec4 t1 = T_LTC(vec2(luv.x * 0.5, luv.y)), t2 = T_LTC(vec2(luv.x * 0.5 + 0.5, luv.y));
          mat3 minv = mat3(vec3(t1.x, 0.0, t1.y), vec3(0.0, 1.0, 0.0), vec3(t1.z, 0.0, t1.w));
          pspec += lc * (f0 * t2.x + (1.0 - f0) * t2.y) * ltcEvaluate(n, toEye, v_world, minv, rc);
          rectDif += lc * ltcEvaluate(n, toEye, v_world, mat3(1.0), rc);
        }
        continue;
      }
      vec3 c = l0.rgb; float k = 1.0; vec3 dir = l3.xyz; bool directional = true;
      if (kind < 0.5) { directional = false; }
      else if (kind < 1.5) { c = mix(l1.rgb, l0.rgb, dot(n, l3.xyz) * 0.5 + 0.5); directional = false; }
      else if (kind > 2.5) {
        dir = l2.xyz - v_world; float d = length(dir); dir /= max(d, 1e-6);
        float fall = 1.0 / max(pow(d, l3.w), 0.01);
        if (l2.w > 0.0) { float r = d / l2.w; float w = clamp(1.0 - r * r * r * r, 0.0, 1.0); fall *= w * w; }
        k = fall;
        if (kind > 3.5) {
          float cs = dot(dir, -l3.xyz);
          float e = l4.y > l4.x ? clamp((cs - l4.x) / (l4.y - l4.x), 0.0, 1.0) : (cs >= l4.x ? 1.0 : 0.0);
          k *= e * e * (3.0 - 2.0 * e);
        }
      }
      if (directional) { int slot = int(l4.z + 0.5) - 1; if (slot >= 0 && u_receive == 1) c *= shadowAt(slot, shN); }
      vec3 diffuse = vec3(1.0); float shComp = 1.0;
      if (directional) {
        float nl = dot(n, dir);
        diffuse = vec3(max(nl, 0.0));
        if (mode == 1) {
          vec2 coord = vec2(nl * 0.5 + 0.5, 0.0);
          if (has(64)) diffuse = vec3(T_GRAD(coord).r);
          else { float fw = fwidth(coord.x) * 0.5; diffuse = vec3(mix(0.7, 1.0, smoothstep(0.7 - fw, 0.7 + fw, coord.x))); }
        }
        if (sheenOn) {
          float nlc0 = clamp(nl, 0.0, 1.0); vec3 hs = normalize(dir + toEye); float nhs = clamp(dot(n, hs), 0.0, 1.0);
          float sa = shR * shR, s2 = max(1.0 - nhs * nhs, 0.0078125);
          float Dc = (2.0 + 1.0 / sa) * pow(s2, 0.5 / sa) / 6.283185307;
          float Vn = clamp(1.0 / (4.0 * (nlc0 + nv - nlc0 * nv)), 0.0, 1.0);
          sheenSpec += c * (l1.w * k * nlc0) * u_sheen.rgb * (Dc * Vn);
          shComp = 1.0 - shMax * max(sheenAlbedo(nv, shR), sheenAlbedo(nlc0, shR));
        }
        if (physical && nl > 0.0) {
          vec3 h = normalize(dir + toEye);
          float a = rough * rough, a2 = a * a;
          float nlc = clamp(nl, 0.0, 1.0), nh = clamp(dot(n, h), 0.0, 1.0), vh = clamp(dot(toEye, h), 0.0, 1.0);
          float fr = exp2((-5.55473 * vh - 6.98316) * vh);
          vec3 F = f0 * (1.0 - fr) + fr;
          float gv = nlc * sqrt(a2 + (1.0 - a2) * nv * nv), gl = nv * sqrt(a2 + (1.0 - a2) * nlc * nlc);
          float V = 0.5 / max(gv + gl, 1e-6);
          float dd = nh * nh * (a2 - 1.0) + 1.0; float D = 0.3183098862 * a2 / (dd * dd);
          vec2 dfgL = T_DFG(vec2(rough, nlc)).rg;
          vec3 fssV = f0 * dfgV.x + dfgV.y, fssL = f0 * dfgL.x + dfgL.y;
          float emsV = 1.0 - (dfgV.x + dfgV.y), emsL = 1.0 - (dfgL.x + dfgL.y);
          vec3 favg = f0 + (1.0 - f0) * 0.047619;
          vec3 fms = fssV * fssL * favg / (1.0 - emsV * emsL * favg + 1e-6);
          pspec += c * (l1.w * k * nlc * shComp) * (F * (V * D) + fms * emsV * emsL);
        } else if (K_PHONG == 1 && nl > 0.0) {
          vec3 h = normalize(dir + toEye);
          float s = pow(max(dot(n, h), 0.0), c2.w) * (c2.w + 2.0) / 8.0 * 0.3183098862;
          spec += c * (l1.w * k * s * nl);
        }
      }
      if (kind < 1.5) ind += c * (l1.w * 0.3183098862);
      else dif += c * diffuse * (l1.w * k * 0.3183098862 * (directional ? shComp : 1.0));
"#)
}

/// A full-screen triangle for the passes below.
pub fn full_vs() -> String {
    String::from(r#"#version 300 es
void main() { vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0); }
"#)
}

/// Prefiltering an environment into the CubeUV atlas (three.js's PMREMGenerator).
pub fn pmrem_fs() -> String {
    let mut s = String::new();
    s.push_str(r#"#version 300 es
precision highp float;
uniform sampler2D u_src;
uniform int u_pass;
uniform float u_size, u_sizeIn, u_rough;
out vec4 o_color;
"#);
    s.push_str(&cube_uv());
    s.push_str(r#"float vdc(uint b) { b = (b << 16u) | (b >> 16u); b = ((b & 0x55555555u) << 1u) | ((b & 0xAAAAAAAAu) >> 1u); b = ((b & 0x33333333u) << 2u) | ((b & 0xCCCCCCCCu) >> 2u);
  b = ((b & 0x0F0F0F0Fu) << 4u) | ((b & 0xF0F0F0F0u) >> 4u); b = ((b & 0x00FF00FFu) << 8u) | ((b & 0xFF00FF00u) >> 8u); return float(b) * 2.3283064365386963e-10; }
void main() {
  vec2 px = gl_FragCoord.xy; float col = floor(px.x / u_size), row = floor(px.y / u_size);
  vec3 n = normalize(faceDir((px - vec2(col, row) * u_size - 1.0) / (u_size - 2.0), col + 3.0 * row));
  vec3 c;
  if (u_pass == 0) c = texture(u_src, equirectUv(n)).rgb;
  else if (u_rough < 0.001) c = cubeUv(u_src, n, u_sizeIn);
  else {
    vec3 up = abs(n.z) < 0.999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
    vec3 tx = normalize(cross(up, n)), ty = cross(n, tx);
    float a = u_rough * u_rough; vec3 sum = vec3(0.0); float wsum = 0.0;
    for (uint i = 0u; i < 256u; i++) {
      float r = sqrt(float(i) / 256.0), phi = 6.283185307 * vdc(i);
      float t1 = r * cos(phi), t2 = r * sin(phi);
      vec3 nh = vec3(t1, t2, sqrt(max(0.0, 1.0 - t1 * t1 - t2 * t2)));
      vec3 ht = normalize(vec3(a * nh.x, a * nh.y, max(0.0, nh.z)));
      vec3 h = normalize(tx * ht.x + ty * ht.y + n * ht.z);
      vec3 l = normalize(2.0 * dot(n, h) * h - n);
      float nl = max(dot(n, l), 0.0);
      if (nl > 0.0) { sum += cubeUv(u_src, l, u_sizeIn) * nl; wsum += nl; }
    }
    c = wsum > 0.0 ? sum / wsum : vec3(0.0);
  }
  o_color = vec4(c, 1.0);
}
"#);
    s
}

/// Scene backgrounds: colour, equirect, cube and CubeUV textures.
pub fn background_fs() -> String {
    let mut s = String::new();
    s.push_str(r#"#version 300 es
precision highp float;
uniform int u_mode, u_tone, u_linearOut;
uniform float u_exposure, u_intensity, u_lod, u_mipF, u_sizeA, u_sizeB;
uniform sampler2D u_tex, u_lvA, u_lvB;
uniform mat3 u_uvt, u_rot;
uniform mat4 u_view, u_proj;
uniform vec2 u_vp;
out vec4 o_color;
"#);
    s.push_str(&common());
    s.push_str(&cube_uv());
    s.push_str(r#"void main() {
  vec2 q = gl_FragCoord.xy / u_vp;
  vec4 c = vec4(0.0, 0.0, 0.0, 1.0);
  if (u_mode == 1) c = texture(u_tex, (u_uvt * vec3(q, 1.0)).xy);
  else {
    mat4 ip = inverse(u_proj); vec4 a = ip * vec4(q * 2.0 - 1.0, -1.0, 1.0), b = ip * vec4(q * 2.0 - 1.0, 1.0, 1.0);
    vec3 d = normalize(u_rot * (inverse(mat3(u_view)) * (b.xyz / b.w - a.xyz / a.w)));
    if (u_mode == 2) c.rgb = textureLod(u_tex, equirectUv(d), u_lod).rgb;
    else { c.rgb = cubeUv(u_lvA, d, u_sizeA); if (u_mipF > 0.0) c.rgb = mix(c.rgb, cubeUv(u_lvB, d, u_sizeB), u_mipF); }
  }
  c.rgb *= u_intensity;
  if (u_tone > 0) c.rgb = toneMap(c.rgb);
  c.rgb = max(c.rgb, 0.0);
  if (u_linearOut == 0) c.rgb = toSrgb(c.rgb);
  o_color = c;
}
"#);
    s
}

/// The separable blur of VSM shadow maps.
pub fn vsm_blur_fs() -> String {
    String::from(r#"#version 300 es
precision highp float;
uniform sampler2D u_src;
uniform vec2 u_res;
uniform float u_radius, u_samples;
uniform int u_horizontal;
out vec4 o_color;
void main() {
  float mean = 0.0, sq = 0.0;
  float stride = u_samples <= 1.0 ? 0.0 : 2.0 / (u_samples - 1.0), start = u_samples <= 1.0 ? 0.0 : -1.0;
  for (float i = 0.0; i < 64.0; i++) {
    if (i >= u_samples) break;
    float off = start + i * stride;
    if (u_horizontal == 1) { vec2 d = texture(u_src, (gl_FragCoord.xy + vec2(off, 0.0) * u_radius) / u_res).rg; mean += d.x; sq += d.y * d.y + d.x * d.x; }
    else { float z = texture(u_src, (gl_FragCoord.xy + vec2(0.0, off) * u_radius) / u_res).r; mean += z; sq += z * z; }
  }
  mean /= u_samples; sq /= u_samples;
  o_color = vec4(mean, sqrt(max(0.0, sq - mean * mean)), 0.0, 1.0);
}
"#)
}
