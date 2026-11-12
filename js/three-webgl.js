// SPDX-License-Identifier: MIT
//
// The GPU backend of THREE.WebGLRenderer: WebGL2. The crate plans each
// frame (three/src/renderers/plan.rs: matrices, packed lights and fog,
// material inputs, the sorted draw list); this file only keeps GPU buffers
// and textures in step with the scene and issues the draws. Shading
// follows the software renderer (soft.rs), per pixel instead of per
// vertex. Load after three-renderer.js; WebGLRenderer uses it whenever the
// canvas gives a WebGL2 context, unless { software: true } is passed.
(function (T, R) {
  "use strict";
  var MAX_LIGHTS = 16;
  // colour-space helpers and three.js's tone mapping operators (tonemapping_pars_fragment)
  var COMMON = "" +
    "vec3 toLinear(vec3 c) { return mix(c * 0.0773993808, pow(c * 0.9478672986 + 0.0521327014, vec3(2.4)), step(0.04045, c)); }\n" +
    "vec3 toSrgb(vec3 c) { return mix(c * 12.92, 1.055 * pow(c, vec3(0.41666)) - 0.055, step(0.0031308, c)); }\n" +
    "vec3 rrtOdt(vec3 v) { vec3 a = v * (v + 0.0245786) - 0.000090537; vec3 b = v * (0.983729 * v + 0.4329510) + 0.238081; return a / b; }\n" +
    "vec3 agxContrast(vec3 x) { vec3 x2 = x * x; vec3 x4 = x2 * x2; return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232; }\n" +
    "vec3 toneMap(vec3 c) {\n" +
    "  if (u_tone == 1) return clamp(u_exposure * c, 0.0, 1.0);\n" +
    "  if (u_tone == 2) { c *= u_exposure; return clamp(c / (vec3(1.0) + c), 0.0, 1.0); }\n" +
    "  if (u_tone == 3) { c *= u_exposure; c = max(vec3(0.0), c - 0.004); return pow((c * (6.2 * c + 0.5)) / (c * (6.2 * c + 1.7) + 0.06), vec3(2.2)); }\n" +
    "  if (u_tone == 4) {\n" +
    "    const mat3 inM = mat3(vec3(0.59719, 0.07600, 0.02840), vec3(0.35458, 0.90834, 0.13383), vec3(0.04823, 0.01566, 0.83777));\n" +
    "    const mat3 outM = mat3(vec3(1.60475, -0.10208, -0.00327), vec3(-0.53108, 1.10813, -0.07276), vec3(-0.07367, -0.00605, 1.07602));\n" +
    "    c *= u_exposure / 0.6; c = outM * rrtOdt(inM * c); return clamp(c, 0.0, 1.0);\n" +
    "  }\n" +
    "  if (u_tone == 6) {\n" +
    "    const mat3 toRec2020 = mat3(vec3(0.6274, 0.0691, 0.0164), vec3(0.3293, 0.9195, 0.0880), vec3(0.0433, 0.0113, 0.8956));\n" +
    "    const mat3 fromRec2020 = mat3(vec3(1.6605, -0.1246, -0.0182), vec3(-0.5876, 1.1329, -0.1006), vec3(-0.0728, -0.0083, 1.1187));\n" +
    "    const mat3 inset = mat3(vec3(0.856627153315983, 0.137318972929847, 0.11189821299995), vec3(0.0951212405381588, 0.761241990602591, 0.0767994186031903), vec3(0.0482516061458583, 0.101439036467562, 0.811302368396859));\n" +
    "    const mat3 outset = mat3(vec3(1.1271005818144368, -0.1413297634984383, -0.14132976349843826), vec3(-0.11060664309660323, 1.157823702216272, -0.11060664309660294), vec3(-0.016493938717834573, -0.016493938717834257, 1.2519364065950405));\n" +
    "    c *= u_exposure; c = inset * (toRec2020 * c);\n" +
    "    c = clamp((log2(max(c, 1e-10)) + 12.47393) / (4.026069 + 12.47393), 0.0, 1.0);\n" +
    "    c = outset * agxContrast(c); c = pow(max(vec3(0.0), c), vec3(2.2));\n" +
    "    return clamp(fromRec2020 * c, 0.0, 1.0);\n" +
    "  }\n" +
    "  if (u_tone == 7) {\n" +
    "    c *= u_exposure; float x = min(c.r, min(c.g, c.b)); float off = x < 0.08 ? x - 6.25 * x * x : 0.04; c -= off;\n" +
    "    float peak = max(c.r, max(c.g, c.b)); if (peak < 0.76) return c;\n" +
    "    float d = 0.24; float np = 1.0 - d * d / (peak + d - 0.76); c *= np / peak;\n" +
    "    float g = 1.0 - 1.0 / (0.15 * (peak - np) + 1.0); return mix(c, vec3(np), g);\n" +
    "  }\n" +
    "  return c;\n" +
    "}\n";
  var VS = "#version 300 es\n" +
    "layout(location=0) in vec3 a_position;\n" +
    "layout(location=1) in vec3 a_normal;\n" +
    "layout(location=2) in vec2 a_uv;\n" +
    "layout(location=3) in vec3 a_color;\n" +
    "layout(location=4) in vec4 a_im0;\n" +
    "layout(location=5) in vec4 a_im1;\n" +
    "layout(location=6) in vec4 a_im2;\n" +
    "layout(location=7) in vec4 a_im3;\n" +
    "layout(location=8) in vec3 a_icolor;\n" +
    "layout(location=9) in float a_lineDistance;\n" +
    "uniform sampler2D u_dispMap;\n" +
    "uniform int u_hasDisp;\n" +
    "uniform mat3 u_dispT;\n" +
    "uniform float u_dispScale, u_dispBias, u_dashScale;\n" +
    "uniform mat4 u_model, u_view, u_proj;\n" +
    "uniform mat3 u_uvt;\n" +
    "uniform int u_instanced;\n" +
    "uniform float u_pointSize, u_pointScale;\n" +
    "uniform int u_sizeAtt;\n" +
    "uniform int u_sprite;\n" +
    "uniform mat3 u_normalMat;\n" +
    "uniform mat4 u_viewInv;\n" +
    "uniform vec2 u_center;\n" +
    "uniform float u_rotation;\n" +
    "out vec3 v_world; out vec3 v_normal; out vec2 v_uv; out vec2 v_uv0; out vec3 v_color; out float v_depth; out float v_lineDist;\n" +
    "void main() {\n" +
    "  mat4 m = u_model;\n" +
    "  if (u_instanced == 1) m = u_model * mat4(a_im0, a_im1, a_im2, a_im3);\n" +
    // displacement along the normal; the shading normal stays as it was (three.js)
    "  vec3 pos = a_position;\n" +
    "  if (u_hasDisp == 1) pos += normalize(a_normal) * (texture(u_dispMap, (u_dispT * vec3(a_uv, 1.0)).xy).x * u_dispScale + u_dispBias);\n" +
    "  v_lineDist = u_dashScale * a_lineDistance;\n" +
    "  vec4 w = m * vec4(pos, 1.0);\n" +
    "  v_world = w.xyz;\n" +
        // the normal matrix comes from the CPU; an instance's own matrix is applied with its scale divided out (three.js's defaultnormal_vertex)
    "  vec3 nrm = a_normal;\n" +
    "  if (u_instanced == 1) { mat3 im = mat3(a_im0.xyz, a_im1.xyz, a_im2.xyz); nrm /= vec3(dot(im[0], im[0]), dot(im[1], im[1]), dot(im[2], im[2])); nrm = im * nrm; }\n" +
    "  v_normal = u_normalMat * nrm;\n" +
    "  v_uv = (u_uvt * vec3(a_uv, 1.0)).xy;\n" +
    "  v_color = a_color * a_icolor;\n" +
    "  vec4 vp = u_view * w;\n" +
    "  v_depth = -vp.z;\n" +
    "  gl_Position = u_proj * vp;\n" +
    "  v_uv0 = a_uv;\n" +
    // a sprite: a camera-facing quad scaled by the model's scale (three.js's sprite_vert)
    "  if (u_sprite == 1) {\n" +
    "    vec4 mv = u_view * u_model * vec4(0.0, 0.0, 0.0, 1.0);\n" +
    "    vec2 sc = vec2(length(u_model[0].xyz), length(u_model[1].xyz));\n" +
    "    if (u_sizeAtt == 0 && u_proj[2][3] == -1.0) sc *= -mv.z;\n" +
    "    vec2 ap = (a_position.xy - (u_center - vec2(0.5))) * sc;\n" +
    "    mv.xy += vec2(cos(u_rotation) * ap.x - sin(u_rotation) * ap.y, sin(u_rotation) * ap.x + cos(u_rotation) * ap.y);\n" +
    "    v_world = (u_viewInv * mv).xyz; v_normal = vec3(0.0, 0.0, 1.0); v_depth = -mv.z;\n" +
    "    gl_Position = u_proj * mv;\n" +
    "  }\n" +
    "  gl_PointSize = max(1.0, u_sizeAtt == 1 ? u_pointSize * u_pointScale / max(-vp.z, 1e-4) : u_pointSize);\n" +
    "}\n";
  // the fragment shader after its #version line and SHADOW_TYPE (1 PCF, 2 basic, 3 VSM)
  // the faces of the prefiltered environment: 3 x 2 cube faces per level, one-texel border (three.js's cube_uv_reflection)
  var CUBE_UV = "" +
    "float faceOf(vec3 d) { vec3 a = abs(d); if (a.x > a.z) return a.x > a.y ? (d.x > 0.0 ? 0.0 : 3.0) : (d.y > 0.0 ? 1.0 : 4.0); return a.z > a.y ? (d.z > 0.0 ? 2.0 : 5.0) : (d.y > 0.0 ? 1.0 : 4.0); }\n" +
    "vec2 faceUv(vec3 d, float f) {\n" +
    "  vec2 uv;\n" +
    "  if (f == 0.0) uv = vec2(d.z, d.y) / abs(d.x); else if (f == 1.0) uv = vec2(-d.x, -d.z) / abs(d.y);\n" +
    "  else if (f == 2.0) uv = vec2(-d.x, d.y) / abs(d.z); else if (f == 3.0) uv = vec2(-d.z, d.y) / abs(d.x);\n" +
    "  else if (f == 4.0) uv = vec2(-d.x, d.z) / abs(d.y); else uv = vec2(d.x, d.y) / abs(d.z);\n" +
    "  return 0.5 * (uv + 1.0);\n" +
    "}\n" +
    "vec3 faceDir(vec2 uv, float f) {\n" +
    "  uv = 2.0 * uv - 1.0; vec3 d = vec3(uv, 1.0);\n" +
    "  if (f == 0.0) d = d.zyx; else if (f == 1.0) { d = d.xzy; d.xz *= -1.0; } else if (f == 2.0) d.x *= -1.0;\n" +
    "  else if (f == 3.0) { d = d.zyx; d.xz *= -1.0; } else if (f == 4.0) { d = d.xzy; d.xy *= -1.0; } else d.z *= -1.0;\n" +
    "  return d;\n" +
    "}\n" +
    "vec3 cubeUv(sampler2D t, vec3 d, float s) {\n" +
    "  float f = faceOf(d); vec2 uv = faceUv(d, f) * (s - 2.0) + 1.0;\n" +
    "  if (f > 2.0) { uv.y += s; f -= 3.0; }\n" +
    "  uv.x += f * s;\n" +
    "  return textureLod(t, uv / vec2(3.0 * s, 2.0 * s), 0.0).rgb;\n" +
    "}\n" +
    "vec2 equirectUv(vec3 d) { return vec2(atan(d.z, d.x) * 0.1591549431 + 0.5, asin(clamp(d.y, -1.0, 1.0)) * 0.3183098862 + 0.5); }\n";
  var FS_BODY = "" +
    "precision highp float;\n" +
    "in vec3 v_world; in vec3 v_normal; in vec2 v_uv; in vec2 v_uv0; in vec3 v_color; in float v_depth; in float v_lineDist;\n" +
    "uniform int u_dashed;\n" +
    "uniform float u_dashSize, u_dashTotal, u_bumpScale;\n" +
    "uniform sampler2D u_bumpMap;\n" +
    "uniform vec4 u_sheen;\n" +
    // transmission: amount, thickness, ior, attenuation distance (0 = none); the opaque scene behind
    "uniform vec4 u_trans;\n" +
    "uniform vec3 u_attColor;\n" +
    "uniform sampler2D u_transMap;\n" +
    "uniform vec2 u_transSize;\n" +
    "uniform mat4 u_model, u_proj;\n" +
    "uniform sampler2D u_emap;\n" +
    "uniform int u_hasEmap;\n" +
    // extra maps: bit 1 alpha, 2 roughness, 4 metalness, 8 normal, 16 ao, 32 matcap, 64 gradient; u_t: their uv transforms
    "uniform sampler2D u_alphaMap, u_roughMap, u_metalMap, u_normalMap, u_aoMap, u_matcap, u_gradMap;\n" +
    "uniform int u_maps;\n" +
    "uniform int u_linearOut;\n" +
    "uniform int u_premult;\n" +
    "uniform mat3 u_t[6];\n" +
    "uniform vec2 u_normalScale;\n" +
    "uniform float u_aoIntensity;\n" +
    "uniform mat3 u_euvt;\n" +
    "uniform vec4 u_mat[6];\n" +
    "uniform sampler2D u_dfg;\n" +
    "uniform vec4 u_lights[" + (MAX_LIGHTS * 7) + "];\n" +
    "uniform sampler2D u_ltc;\n" +
    // shadow maps: slots 0-2 directional/spot (2D), slot 3 point (cube)
    "#if SHADOW_TYPE == 3\n" +
    "uniform sampler2D u_sh0, u_sh1, u_sh2;\n" +
    "uniform samplerCube u_shc0;\n" +
    "#else\n" +
    "uniform highp sampler2DShadow u_sh0, u_sh1, u_sh2;\n" +
    "uniform highp samplerCubeShadow u_shc0;\n" +
    "#endif\n" +
    "uniform mat4 u_shMat[4];\n" +
    "uniform vec4 u_shParam[4];\n" +
    "uniform vec4 u_shSize[4];\n" +
    "uniform int u_receive;\n" +
    // environment: u_env = mode (1 prefiltered for Standard/Physical, 2 reflected equirect for Basic/Lambert/Phong),
    // intensity, combine (multiply, mix, add), reflectivity; u_envUV = max mip, texel width, height, refraction ratio (0 reflects)
    "uniform sampler2D u_envMap;\n" +
    "uniform vec4 u_env, u_envUV;\n" +
    "uniform mat3 u_envRot;\n" +
    "uniform int u_nlights;\n" +
    "uniform vec4 u_fog[2];\n" +
    "uniform vec3 u_cam;\n" +
    "uniform mat4 u_view;\n" +
    "uniform mat3 u_uvt;\n" +
    "uniform sampler2D u_map;\n" +
    "uniform int u_hasMap;\n" +
    "uniform int u_points;\n" +
    "uniform int u_hasNormal;\n" +
    "uniform int u_opaque;\n" +
    "uniform int u_tone;\n" +
    "uniform float u_exposure;\n" +
    "out vec4 o_color;\n" +
    COMMON + CUBE_UV +
    "bool has(int b) { return (u_maps & b) != 0; }\n" +
    // cubic B-spline filtered sample of a mip level pair (three.js's textureBicubic)
    "float bw0(float a) { return (1.0 / 6.0) * (a * (a * (-a + 3.0) - 3.0) + 1.0); }\n" +
    "float bw1(float a) { return (1.0 / 6.0) * (a * a * (3.0 * a - 6.0) + 4.0); }\n" +
    "float bw2(float a) { return (1.0 / 6.0) * (a * (a * (-3.0 * a + 3.0) + 3.0) + 1.0); }\n" +
    "float bw3(float a) { return (1.0 / 6.0) * (a * a * a); }\n" +
    "vec4 bicubic(sampler2D t, vec2 uv, vec2 size, float lod) {\n" +
    "  uv = uv * size + 0.5; vec2 iuv = floor(uv), f = fract(uv);\n" +
    "  float g0x = bw0(f.x) + bw1(f.x), g1x = bw2(f.x) + bw3(f.x), g0y = bw0(f.y) + bw1(f.y), g1y = bw2(f.y) + bw3(f.y);\n" +
    "  float h0x = -1.0 + bw1(f.x) / g0x, h1x = 1.0 + bw3(f.x) / g1x, h0y = -1.0 + bw1(f.y) / g0y, h1y = 1.0 + bw3(f.y) / g1y;\n" +
    "  vec2 px = 1.0 / size;\n" +
    "  return g0y * (g0x * textureLod(t, (vec2(iuv.x + h0x, iuv.y + h0y) - 0.5) * px, lod) + g1x * textureLod(t, (vec2(iuv.x + h1x, iuv.y + h0y) - 0.5) * px, lod)) +\n" +
    "    g1y * (g0x * textureLod(t, (vec2(iuv.x + h0x, iuv.y + h1y) - 0.5) * px, lod) + g1x * textureLod(t, (vec2(iuv.x + h1x, iuv.y + h1y) - 0.5) * px, lod));\n" +
    "}\n" +
    "vec4 textureBicubic(sampler2D t, vec2 uv, float lod) {\n" +
    "  vec4 a = bicubic(t, uv, vec2(textureSize(t, int(lod))), floor(lod));\n" +
    "  vec4 b = bicubic(t, uv, vec2(textureSize(t, int(lod + 1.0))), ceil(lod));\n" +
    "  return mix(a, b, fract(lod));\n" +
    "}\n" +
    // the rectangle's form factor seen through the transform minv (three.js's LTC_Evaluate)
    "vec3 ltcEdge(vec3 a, vec3 b) { float x = dot(a, b), y = abs(x); float va = 0.8543985 + (0.4965155 + 0.0145206 * y) * y, vb = 3.4175940 + (4.1616724 + y) * y, v = va / vb;\n" +
    "  float ts = x > 0.0 ? v : 0.5 * inversesqrt(max(1.0 - x * x, 1e-7)) - v; return cross(a, b) * ts; }\n" +
    "float ltcEvaluate(vec3 N, vec3 V, vec3 P, mat3 minv, vec3 rc[4]) {\n" +
    "  vec3 e1 = rc[1] - rc[0], e2 = rc[3] - rc[0];\n" +
    "  if (dot(cross(e1, e2), P - rc[0]) < 0.0) return 0.0;\n" +
    "  vec3 t1 = normalize(V - N * dot(V, N)), t2 = -cross(N, t1);\n" +
    "  mat3 m = minv * transpose(mat3(t1, t2, N));\n" +
    "  vec3 c0 = normalize(m * (rc[0] - P)), c1 = normalize(m * (rc[1] - P)), c2 = normalize(m * (rc[2] - P)), c3 = normalize(m * (rc[3] - P));\n" +
    "  vec3 f = ltcEdge(c0, c1) + ltcEdge(c1, c2) + ltcEdge(c2, c3) + ltcEdge(c3, c0);\n" +
    "  float l = length(f); return max((l * l + f.z) / (l + 1.0), 0.0);\n" +
    "}\n" +
    // shadows as three.js reads them (shadowmap_pars_fragment): PCF with five Vogel-disk taps turned by
    // interleaved gradient noise, basic, or VSM; u_shParam = intensity, bias, normal bias, radius;
    // u_shSize = map width, height, near, far
    "float ign(vec2 p) { return fract(52.9829189 * fract(dot(p, vec2(0.06711056, 0.00583715)))); }\n" +
    "vec2 vogel(int i, float phi) { float r = sqrt((float(i) + 0.5) / 5.0); float t = float(i) * 2.399963229728653 + phi; return vec2(cos(t), sin(t)) * r; }\n" +
    "#if SHADOW_TYPE == 3\n" +
    "vec2 shRG(int i, vec2 p) { if (i == 0) return texture(u_sh0, p).rg; if (i == 1) return texture(u_sh1, p).rg; return texture(u_sh2, p).rg; }\n" +
    "#else\n" +
    "float shTap(int i, vec3 p) { if (i == 0) return texture(u_sh0, p); if (i == 1) return texture(u_sh1, p); return texture(u_sh2, p); }\n" +
    "#endif\n" +
    "float shadowAt(int slot, vec3 nrm) {\n" +
    "  vec4 pr = u_shParam[slot], sz = u_shSize[slot];\n" +
    "  vec4 sc = u_shMat[slot] * vec4(v_world + nrm * pr.z, 1.0);\n" +
    "  float shadow = 1.0;\n" +
    "  if (slot == 3) {\n" +
    "#if SHADOW_TYPE != 3\n" +
    "    vec3 av = abs(sc.xyz); float vz = max(max(av.x, av.y), av.z);\n" +
    "    if (vz - sz.w <= 0.0 && vz - sz.z >= 0.0) {\n" +
    "      float dp = (sz.w * (vz - sz.z)) / (vz * (sz.w - sz.z)) + pr.y;\n" +
    "      vec3 bd = normalize(sc.xyz);\n" +
    "#if SHADOW_TYPE == 1\n" +
    "      float texel = pr.w / sz.x; vec3 ad = abs(bd);\n" +
    "      vec3 tg = normalize(cross(bd, ad.x > ad.z ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0))); vec3 bt = cross(bd, tg);\n" +
    "      float phi = ign(gl_FragCoord.xy) * 6.283185307179586;\n" +
    "      shadow = 0.0;\n" +
    "      for (int k = 0; k < 5; k++) { vec2 o = vogel(k, phi); shadow += texture(u_shc0, vec4(bd + (tg * o.x + bt * o.y) * texel, dp)); }\n" +
    "      shadow *= 0.2;\n" +
    "#else\n" +
    "      shadow = texture(u_shc0, vec4(bd, dp));\n" +
    "#endif\n" +
    "    }\n" +
    "#endif\n" +
    "  } else {\n" +
    "    sc.xyz /= sc.w; sc.z += pr.y;\n" +
    "    if (sc.x >= 0.0 && sc.x <= 1.0 && sc.y >= 0.0 && sc.y <= 1.0 && sc.z <= 1.0) {\n" +
    "#if SHADOW_TYPE == 1\n" +
    "      float rad = pr.w / sz.x, phi = ign(gl_FragCoord.xy) * 6.283185307179586;\n" +
    "      shadow = 0.0;\n" +
    "      for (int k = 0; k < 5; k++) shadow += shTap(slot, vec3(sc.xy + vogel(k, phi) * rad, sc.z));\n" +
    "      shadow *= 0.2;\n" +
    "#elif SHADOW_TYPE == 2\n" +
    "      shadow = shTap(slot, sc.xyz);\n" +
    "#else\n" +
    "      vec2 ds = shRG(slot, sc.xy); float mean = ds.x, variance = ds.y * ds.y;\n" +
    "      float hard = step(sc.z, mean);\n" +
    "      if (hard == 1.0) shadow = 1.0;\n" +
    "      else { variance = max(variance, 0.0000001); float d = sc.z - mean; float pm = clamp((variance / (variance + d * d) - 0.3) / 0.65, 0.0, 1.0); shadow = max(hard, pm); }\n" +
    "#endif\n" +
    "    }\n" +
    "  }\n" +
    "  return mix(1.0, shadow, pr.x);\n" +
    "}\n" +
    "float sheenAlbedo(float nv, float r) { float r2 = r * r, ri = 1.0 / (r + 0.1); float a = -1.9362 + 1.0678 * r + 0.4573 * r2 - 0.8469 * ri; float b = -0.6014 + 0.5538 * r - 0.4670 * r2 - 0.1255 * ri; return clamp(exp(a * clamp(nv, 0.0, 1.0) + b), 0.0, 1.0); }\n" +
    "float envMip(float r) {\n" +
    "  if (r >= 0.8) return (1.0 - r) * 5.0 - 2.0; if (r >= 0.4) return (0.8 - r) * 7.5 - 1.0;\n" +
    "  if (r >= 0.305) return (0.4 - r) / 0.095 + 2.0; if (r >= 0.21) return (0.305 - r) / 0.095 + 3.0; return -2.0 * log2(1.16 * r);\n" +
    "}\n" +
    "vec3 envLevel(vec3 d, float mipInt) {\n" +
    "  float f = faceOf(d), fi = max(4.0 - mipInt, 0.0); mipInt = max(mipInt, 4.0); float fs = exp2(mipInt);\n" +
    "  vec2 uv = faceUv(d, f) * (fs - 2.0) + 1.0;\n" +
    "  if (f > 2.0) { uv.y += fs; f -= 3.0; }\n" +
    "  uv.x += f * fs + fi * 48.0; uv.y += 4.0 * (exp2(u_envUV.x) - fs);\n" +
    "  return textureLod(u_envMap, uv * u_envUV.yz, 0.0).rgb;\n" +
    "}\n" +
    "vec3 envCubeUV(vec3 d, float r) { float m = clamp(envMip(r), -2.0, u_envUV.x), mf = fract(m), mi = floor(m); vec3 c = envLevel(d, mi); return mf == 0.0 ? c : mix(c, envLevel(d, mi + 1.0), mf); }\n" +
    "vec3 envEquirect(vec3 d) {\n" +
    "  vec2 uv = equirectUv(d), dx = dFdx(uv), dy = dFdy(uv);\n" +
    "  dx.x -= floor(dx.x + 0.5); dy.x -= floor(dy.x + 0.5);\n" +
    "  return textureGrad(u_envMap, uv, dx, dy).rgb;\n" +
    "}\n" +
    "vec2 tuv(int i) { return (u_t[i] * vec3(v_uv0, 1.0)).xy; }\n" +
    "void main() {\n" +
    "  if (u_dashed == 1 && mod(v_lineDist, u_dashTotal) > u_dashSize) discard;\n" +
    "  vec4 c0 = u_mat[0], c1 = u_mat[1], c2 = u_mat[2], c3 = u_mat[3], c4 = u_mat[4], c5 = u_mat[5];\n" +
    "  const int mode = K_MODE;\n" +
    "  vec3 albedo = c0.rgb * v_color;\n" +
    "  float alpha = c0.a;\n" +
    "  if (u_hasMap == 1) {\n" +
    "    vec2 uv = v_uv;\n" +
    "    if (u_points == 1) uv = (u_uvt * vec3(gl_PointCoord.x, 1.0 - gl_PointCoord.y, 1.0)).xy;\n" +
    "    vec4 t = texture(u_map, uv);\n" +
    "    albedo *= t.rgb; alpha *= t.a;\n" +
    "  }\n" +
    "  if (has(1)) alpha *= texture(u_alphaMap, tuv(0)).g;\n" +
    "  if (alpha < c1.a) discard;\n" +
    "  if (u_opaque == 1) alpha = 1.0;\n" +
    // a shadow caster: depth only, after the same discards
    "#if K_SHPASS == 1\n" +
    "  o_color = vec4(albedo, alpha); return;\n" +
    "#endif\n" +
    "  vec3 n;\n" +
    "  if (K_FLAT == 1 || u_hasNormal == 0) n = normalize(cross(dFdx(v_world), dFdy(v_world)));\n" +
    "  else { n = normalize(v_normal); if (!gl_FrontFacing) n = -n; }\n" +
    "  vec3 n0 = n;\n" +
    "  vec3 shN = u_hasNormal == 1 ? normalize(v_normal) : vec3(0.0, 0.0, 1.0);\n" +
    // tangent-space normal map without tangents (three.js's getTangentFrame)
    "  if (has(8)) {\n" +
    "    vec2 nuv = tuv(3);\n" +
    "    vec3 mapN = texture(u_normalMap, nuv).xyz * 2.0 - 1.0; mapN.xy *= u_normalScale;\n" +
    "    vec3 q0 = dFdx(v_world), q1 = dFdy(v_world); vec2 st0 = dFdx(nuv), st1 = dFdy(nuv);\n" +
    "    vec3 q1p = cross(q1, n), q0p = cross(n, q0);\n" +
    "    vec3 tt = q1p * st0.x + q0p * st1.x, bb = q1p * st0.y + q0p * st1.y;\n" +
    "    float det = max(dot(tt, tt), dot(bb, bb)); float sc = det == 0.0 ? 0.0 : inversesqrt(det);\n" +
    "    tt *= sc; bb *= sc;\n" +
    "    if (c4.x > 1.5 && c4.y < 0.5 && u_hasNormal == 1 && !gl_FrontFacing) { tt = -tt; bb = -bb; }\n" +
    "    n = normalize(mat3(tt, bb, n) * mapN);\n" +
    "  } else if (has(128)) {\n" +
    // bump map: the height's screen-space slope tilts the normal (three.js's perturbNormalArb)
    "    vec2 buv = tuv(5), sx = dFdx(buv), sy = dFdy(buv);\n" +
    "    float hll = u_bumpScale * texture(u_bumpMap, buv).x;\n" +
    "    vec2 dh = vec2(u_bumpScale * texture(u_bumpMap, buv + sx).x - hll, u_bumpScale * texture(u_bumpMap, buv + sy).x - hll);\n" +
    "    vec3 sgx = normalize(dFdx(v_world)), sgy = normalize(dFdy(v_world));\n" +
    "    vec3 r1 = cross(sgy, n), r2 = cross(n, sgx);\n" +
    "    float det = dot(sgx, r1) * (gl_FrontFacing ? 1.0 : -1.0);\n" +
    "    n = normalize(abs(det) * n - sign(det) * (dh.x * r1 + dh.y * r2));\n" +
    "  }\n" +
    "  float ao = has(16) ? (texture(u_aoMap, tuv(4)).r - 1.0) * u_aoIntensity + 1.0 : 1.0;\n" +
    "  vec3 color;\n" +
    "  float transAlpha = 1.0;\n" +
    "  vec3 emissive = c1.rgb;\n" +
    "  if (u_hasEmap == 1) { vec3 e = texture(u_emap, (u_euvt * vec3(v_uv0, 1.0)).xy).rgb; emissive *= e; }\n" +
    "  if (mode == 3) {\n" +
    // MeshDepthMaterial, BasicDepthPacking: 1 - window depth, no colour space
    "    color = vec3(1.0 - gl_FragCoord.z);\n" +
    "  } else if (mode == 2) {\n" +
    "    vec3 vn = normalize((u_view * vec4(n, 0.0)).xyz);\n" +
    "    color = vn * 0.5 + 0.5;\n" +
    "  } else if (mode == 5) {\n" +
    // ShadowMaterial: its colour where the lights' shadows fall
    "    float mask = 1.0;\n" +
    "    for (int i = 0; i < " + MAX_LIGHTS + "; i++) {\n" +
    "      if (i >= u_nlights) break;\n" +
    "      int slot = int(u_lights[i * 7 + 4].z + 0.5) - 1;\n" +
    "      if (slot >= 0 && u_receive == 1) mask *= shadowAt(slot, shN);\n" +
    "    }\n" +
    "    color = c0.rgb; alpha = c0.a * (1.0 - mask);\n" +
    "  } else if (mode == 4) {\n" +
    // MeshMatcapMaterial: the view-space normal picks the matcap texel
    "    vec3 vn = normalize((u_view * vec4(n, 0.0)).xyz);\n" +
    "    vec3 vd = normalize(-(u_view * vec4(v_world, 1.0)).xyz);\n" +
    "    vec3 mx = normalize(vec3(vd.z, 0.0, -vd.x)); vec3 my = cross(vd, mx);\n" +
    "    vec2 muv = vec2(dot(mx, vn), dot(my, vn)) * 0.495 + 0.5;\n" +
    "    color = albedo * (has(32) ? texture(u_matcap, muv).rgb : vec3(mix(0.2, 0.8, muv.y)));\n" +
    "  } else if (K_LIT == 0) {\n" +
    "    color = albedo * ao;\n" +
    "  } else {\n" +
    "    vec3 toEye = normalize(u_cam - v_world);\n" +
    "    vec3 dif = vec3(0.0), ind = vec3(0.0), spec = vec3(0.0), pspec = vec3(0.0), rectDif = vec3(0.0);\n" +
    // Standard/Physical (three.js lights_physical_fragment): roughness widened by the normal's screen-space change
    "    const bool physical = K_PHYS == 1;\n" +
    "    float rough = c5.x, metal = c5.y, dscale = c3.x;\n" +
    "    if (has(2)) rough *= texture(u_roughMap, tuv(1)).g;\n" +
    "    if (has(4)) metal *= texture(u_metalMap, tuv(2)).b;\n" +
    "    vec3 vn0 = (u_view * vec4(n0, 0.0)).xyz;\n" +
    "    vec3 dxy = max(abs(dFdx(vn0)), abs(dFdy(vn0)));\n" +
    "    if (physical) { rough = min(max(rough, 0.0525) + max(max(dxy.x, dxy.y), dxy.z), 1.0); dscale = 1.0 - metal; }\n" +
    "    vec3 f0 = mix(vec3(0.04), albedo, metal);\n" +
    "    float nv = clamp(dot(n, toEye), 0.0, 1.0);\n" +
    "    vec2 dfgV = texture(u_dfg, vec2(rough, nv)).rg;\n" +
    // sheen (Charlie distribution, Neubelt visibility) and its energy loss on the rest
    "    const bool sheenOn = K_SHEEN == 1;\n" +
    "    float shR = max(u_sheen.a, 0.0001), shMax = max(u_sheen.r, max(u_sheen.g, u_sheen.b));\n" +
    "    vec3 sheenSpec = vec3(0.0);\n" +
    "//@LIGHTS\n" +
    "    if (sheenOn) ind *= 1.0 - shMax * sheenAlbedo(nv, shR);\n" +
    "    vec3 totalDiffuse = albedo * (dif + ind * ao) * dscale + albedo * dscale * rectDif;\n" +
    // image-based light from the prefiltered environment (three.js's getIBLIrradiance/Radiance, RE_IndirectSpecular_Physical)
    "    vec3 envSpec = vec3(0.0);\n" +
    "    if (physical && K_ENV == 1) {\n" +
    "      vec3 irr = 3.141592653589793 * envCubeUV(u_envRot * n, 1.0) * u_env.y;\n" +
    "      vec3 rv = reflect(-toEye, n); float r2 = rough * rough; rv = normalize(mix(rv, n, r2 * r2));\n" +
    "      vec3 rad = envCubeUV(u_envRot * rv, rough) * u_env.y;\n" +
    "      float ess = dfgV.x + dfgV.y, ems = 1.0 - ess;\n" +
    "      vec3 fssD = vec3(0.04) * dfgV.x + dfgV.y, fssM = albedo * dfgV.x + dfgV.y;\n" +
    "      vec3 favgD = vec3(0.04 + 0.96 * 0.047619), favgM = albedo + (1.0 - albedo) * 0.047619;\n" +
    "      vec3 msD = fssD * favgD / (1.0 - ems * favgD) * ems, msM = fssM * favgM / (1.0 - ems * favgM) * ems;\n" +
    "      vec3 cwi = irr * 0.3183098862;\n" +
    "      vec3 ed = albedo * (1.0 - metal) * (1.0 - (fssD + msD)) * cwi;\n" +
    "      envSpec = rad * mix(fssD, fssM, metal) + mix(msD, msM, metal) * cwi;\n" +
    "      if (sheenOn) { float sc = 1.0 - shMax * sheenAlbedo(nv, shR); ed *= sc; envSpec *= sc; }\n" +
    "      if (has(16)) envSpec *= clamp(pow(nv + ao, exp2(-16.0 * rough - 1.0)) - 1.0 + ao, 0.0, 1.0);\n" +
    "      totalDiffuse += ed * ao;\n" +
    "    }\n" +
    // transmission: the opaque scene seen through the refracted exit point (three.js's getIBLVolumeRefraction)
    "    if (K_TRANS == 1) {\n" +
    "      float ior = u_trans.z;\n" +
    "      vec3 scale = vec3(length(u_model[0].xyz), length(u_model[1].xyz), length(u_model[2].xyz));\n" +
    "      vec3 ray = normalize(refract(-toEye, n, 1.0 / ior)) * u_trans.y * scale;\n" +
    "      vec4 ndc = u_proj * u_view * vec4(v_world + ray, 1.0);\n" +
    "      vec2 rc = (ndc.xy / ndc.w + 1.0) / 2.0;\n" +
    "      vec4 tl = textureBicubic(u_transMap, rc, log2(u_transSize.x) * rough * clamp(ior * 2.0 - 2.0, 0.0, 1.0));\n" +
    "      vec3 tm = albedo * dscale;\n" +
    "      if (u_trans.w > 0.0) tm *= exp(log(u_attColor) / u_trans.w * length(ray));\n" +
    "      vec3 F = f0 * dfgV.x + dfgV.y;\n" +
    "      float tf = (tm.r + tm.g + tm.b) / 3.0;\n" +
    "      transAlpha = mix(1.0, 1.0 - (1.0 - tl.a) * tf, u_trans.x);\n" +
    "      totalDiffuse = mix(totalDiffuse, (1.0 - F) * tm * tl.rgb, u_trans.x);\n" +
    "    }\n" +
    "    color = totalDiffuse + c2.rgb * spec + pspec + emissive + sheenSpec + envSpec;\n" +
    "  }\n" +
    // Basic, Lambert and Phong: the environment reflected (or refracted) and combined (three.js's envmap_fragment)
    "  if (K_ENV == 2 && mode == 0 && K_PHYS == 0) {\n" +
    "    vec3 ctf = u_proj[3][3] > 0.5 ? normalize(vec3(-u_view[0][2], -u_view[1][2], -u_view[2][2])) : normalize(v_world - u_cam);\n" +
    "    vec3 rv = u_envUV.w > 0.0 ? refract(ctf, n, u_envUV.w) : reflect(ctf, n);\n" +
    "    vec3 ec = envEquirect(u_envRot * rv);\n" +
    "    float st = u_env.w;\n" +
    "    if (u_env.z < 0.5) color = mix(color, color * ec, st); else if (u_env.z < 1.5) color = mix(color, ec, st); else color += ec * st;\n" +
    "  }\n" +
    // MeshNormalMaterial and MeshDepthMaterial write their values as is (no colour space)
    "  bool raw = mode == 2 || mode == 3;\n" +
    "  if (!raw && u_tone > 0) color = toneMap(color);\n" +
    "  vec3 outc = max(color, 0.0);\n" +
    "  if (!raw && u_linearOut == 0) outc = toSrgb(outc);\n" +
    // three.js fogs after the colour-space conversion, with the fog colour in sRGB; linear fog is a smoothstep
    "  if (K_FOG > 0) {\n" +
    "    float f = K_FOG == 1 ? smoothstep(u_fog[1].x, u_fog[1].y, v_depth)\n" +
    "      : 1.0 - exp(-u_fog[1].z * u_fog[1].z * v_depth * v_depth);\n" +
    "    vec3 fc = clamp(u_fog[0].yzw, 0.0, 1.0);\n" +
    "    outc = mix(outc, u_linearOut == 1 ? fc : toSrgb(fc), f);\n" +
    "  }\n" +
    "  alpha *= transAlpha;\n" +
    "  if (u_premult == 1) outc *= alpha;\n" +
    "  o_color = vec4(outc, alpha);\n" +
    "}\n";

  // one light of a known kind (the loops are written per kind into each variant, so the
  // kind tests fold away): l0..l4 and `kind` are set by the loop
  var LIGHT_BODY = "" +
    // rect area light (Standard/Physical only): linearly transformed cosines (three.js's RE_Direct_RectArea_Physical)
    "      if (K_RECT == 1 && kind > 4.5) {\n" +
    "        if (physical) {\n" +
    "          vec3 hw = u_lights[i * 7 + 5].xyz, hh = u_lights[i * 7 + 6].xyz, lc = l0.rgb * l1.w;\n" +
    "          vec3 rc[4]; rc[0] = l2.xyz + hw - hh; rc[1] = l2.xyz - hw - hh; rc[2] = l2.xyz - hw + hh; rc[3] = l2.xyz + hw + hh;\n" +
    "          vec2 luv = vec2(rough, sqrt(1.0 - nv)) * (63.0 / 64.0) + 0.5 / 64.0;\n" +
    "          vec4 t1 = texture(u_ltc, vec2(luv.x * 0.5, luv.y)), t2 = texture(u_ltc, vec2(luv.x * 0.5 + 0.5, luv.y));\n" +
    "          mat3 minv = mat3(vec3(t1.x, 0.0, t1.y), vec3(0.0, 1.0, 0.0), vec3(t1.z, 0.0, t1.w));\n" +
    "          pspec += lc * (f0 * t2.x + (1.0 - f0) * t2.y) * ltcEvaluate(n, toEye, v_world, minv, rc);\n" +
    "          rectDif += lc * ltcEvaluate(n, toEye, v_world, mat3(1.0), rc);\n" +
    "        }\n" +
    "        continue;\n" +
    "      }\n" +
    "      vec3 c = l0.rgb; float k = 1.0; vec3 dir = l3.xyz; bool directional = true;\n" +
    "      if (kind < 0.5) { directional = false; }\n" +
    "      else if (kind < 1.5) { c = mix(l1.rgb, l0.rgb, dot(n, l3.xyz) * 0.5 + 0.5); directional = false; }\n" +
    "      else if (kind > 2.5) {\n" +
    "        dir = l2.xyz - v_world; float d = length(dir); dir /= max(d, 1e-6);\n" +
    "        float fall = 1.0 / max(pow(d, l3.w), 0.01);\n" +
    "        if (l2.w > 0.0) { float r = d / l2.w; float w = clamp(1.0 - r * r * r * r, 0.0, 1.0); fall *= w * w; }\n" +
    "        k = fall;\n" +
    "        if (kind > 3.5) {\n" +
    "          float cs = dot(dir, -l3.xyz);\n" +
    "          float e = l4.y > l4.x ? clamp((cs - l4.x) / (l4.y - l4.x), 0.0, 1.0) : (cs >= l4.x ? 1.0 : 0.0);\n" +
    "          k *= e * e * (3.0 - 2.0 * e);\n" +
    "        }\n" +
    "      }\n" +
    "      if (directional) { int slot = int(l4.z + 0.5) - 1; if (slot >= 0 && u_receive == 1) c *= shadowAt(slot, shN); }\n" +
    "      vec3 diffuse = vec3(1.0); float shComp = 1.0;\n" +
    "      if (directional) {\n" +
    "        float nl = dot(n, dir);\n" +
    "        diffuse = vec3(max(nl, 0.0));\n" +
    // toon: the gradient map at n.l, or three.js's default two-tone ramp
    "        if (mode == 1) {\n" +
    "          vec2 coord = vec2(nl * 0.5 + 0.5, 0.0);\n" +
    "          if (has(64)) diffuse = vec3(texture(u_gradMap, coord).r);\n" +
    "          else { float fw = fwidth(coord.x) * 0.5; diffuse = vec3(mix(0.7, 1.0, smoothstep(0.7 - fw, 0.7 + fw, coord.x))); }\n" +
    "        }\n" +
    "        if (sheenOn) {\n" +
    "          float nlc0 = clamp(nl, 0.0, 1.0); vec3 hs = normalize(dir + toEye); float nhs = clamp(dot(n, hs), 0.0, 1.0);\n" +
    "          float sa = shR * shR, s2 = max(1.0 - nhs * nhs, 0.0078125);\n" +
    "          float Dc = (2.0 + 1.0 / sa) * pow(s2, 0.5 / sa) / 6.283185307;\n" +
    "          float Vn = clamp(1.0 / (4.0 * (nlc0 + nv - nlc0 * nv)), 0.0, 1.0);\n" +
    "          sheenSpec += c * (l1.w * k * nlc0) * u_sheen.rgb * (Dc * Vn);\n" +
    "          shComp = 1.0 - shMax * max(sheenAlbedo(nv, shR), sheenAlbedo(nlc0, shR));\n" +
    "        }\n" +
    "        if (physical && nl > 0.0) {\n" +
    "          vec3 h = normalize(dir + toEye);\n" +
    "          float a = rough * rough, a2 = a * a;\n" +
    "          float nlc = clamp(nl, 0.0, 1.0), nh = clamp(dot(n, h), 0.0, 1.0), vh = clamp(dot(toEye, h), 0.0, 1.0);\n" +
    "          float fr = exp2((-5.55473 * vh - 6.98316) * vh);\n" +
    "          vec3 F = f0 * (1.0 - fr) + fr;\n" +
    "          float gv = nlc * sqrt(a2 + (1.0 - a2) * nv * nv), gl = nv * sqrt(a2 + (1.0 - a2) * nlc * nlc);\n" +
    "          float V = 0.5 / max(gv + gl, 1e-6);\n" +
    "          float dd = nh * nh * (a2 - 1.0) + 1.0; float D = 0.3183098862 * a2 / (dd * dd);\n" +
    "          vec2 dfgL = texture(u_dfg, vec2(rough, nlc)).rg;\n" +
    "          vec3 fssV = f0 * dfgV.x + dfgV.y, fssL = f0 * dfgL.x + dfgL.y;\n" +
    "          float emsV = 1.0 - (dfgV.x + dfgV.y), emsL = 1.0 - (dfgL.x + dfgL.y);\n" +
    "          vec3 favg = f0 + (1.0 - f0) * 0.047619;\n" +
    "          vec3 fms = fssV * fssL * favg / (1.0 - emsV * emsL * favg + 1e-6);\n" +
    "          pspec += c * (l1.w * k * nlc * shComp) * (F * (V * D) + fms * emsV * emsL);\n" +
    "        } else if (K_PHONG == 1 && nl > 0.0) {\n" +
    "          vec3 h = normalize(dir + toEye);\n" +
    "          float s = pow(max(dot(n, h), 0.0), c2.w) * (c2.w + 2.0) / 8.0 * 0.3183098862;\n" +
    "          spec += c * (l1.w * k * s * nl);\n" +
    "        }\n" +
    "      }\n" +
    "      if (kind < 1.5) ind += c * (l1.w * 0.3183098862);\n" +
    "      else dif += c * diffuse * (l1.w * k * 0.3183098862 * (directional ? shComp : 1.0));\n" +
    "";

  // the extra material maps: property, sampler, texture unit, bit in u_maps, slot in u_t (-1 none)
  var MAPS = [
    { prop: "alphaMap", uniform: "u_alphaMap", unit: 3, bit: 1, t: 0 },
    { prop: "roughnessMap", uniform: "u_roughMap", unit: 4, bit: 2, t: 1 },
    { prop: "metalnessMap", uniform: "u_metalMap", unit: 5, bit: 4, t: 2 },
    { prop: "normalMap", uniform: "u_normalMap", unit: 6, bit: 8, t: 3 },
    { prop: "aoMap", uniform: "u_aoMap", unit: 7, bit: 16, t: 4 },
    // matcap, gradient and bump maps share units with maps their materials cannot have at once
    { prop: "matcap", uniform: "u_matcap", unit: 4, bit: 32, t: -1 },
    { prop: "gradientMap", uniform: "u_gradMap", unit: 5, bit: 64, t: -1 },
    { prop: "bumpMap", uniform: "u_bumpMap", unit: 6, bit: 128, t: 5 }
  ];
  // three.js blend constants -> GL
  var EQUATIONS = { 100: 32774, 101: 32778, 102: 32779, 103: 32775, 104: 32776 };
  var FACTORS = { 200: 0, 201: 1, 202: 768, 203: 769, 204: 770, 205: 771, 206: 772, 207: 773, 208: 774, 209: 775, 210: 776, 211: 32769, 212: 32770, 213: 32771, 214: 32772 };

  var FILTERS = { 1003: 9728, 1004: 9984, 1005: 9986, 1006: 9729, 1007: 9985, 1008: 9987 };
  var WRAPS = { 1000: 10497, 1001: 33071, 1002: 33648 };

  function compile(gl, type, src) {
    var s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error("WebGL2 shader: " + gl.getShaderInfoLog(s));
    return s;
  }

  var NAMES = ["u_model", "u_view", "u_proj", "u_uvt", "u_instanced", "u_pointSize", "u_pointScale", "u_sizeAtt", "u_mat", "u_lights",
    "u_nlights", "u_fog", "u_cam", "u_map", "u_hasMap", "u_points", "u_hasNormal", "u_opaque", "u_tone", "u_exposure", "u_sprite", "u_center", "u_rotation", "u_emap", "u_hasEmap", "u_euvt", "u_dfg",
    "u_alphaMap", "u_roughMap", "u_metalMap", "u_normalMap", "u_aoMap", "u_matcap", "u_gradMap", "u_maps", "u_linearOut", "u_premult", "u_t", "u_normalScale", "u_aoIntensity",
    "u_dispMap", "u_hasDisp", "u_dispT", "u_dispScale", "u_dispBias", "u_dashScale", "u_dashed", "u_dashSize", "u_dashTotal", "u_bumpScale", "u_bumpMap", "u_sheen",
    "u_trans", "u_attColor", "u_transMap", "u_transSize", "u_ltc", "u_envMap", "u_env", "u_envRot", "u_envUV",
    "u_sh0", "u_sh1", "u_sh2", "u_shc0", "u_shMat", "u_shParam", "u_shSize", "u_receive", "u_normalMat", "u_viewInv"];

  // the inverse transpose of a 4x4 matrix's upper 3x3 (column-major), into out
  function normalMatrix(e, out) {
    var a = e[0], b = e[1], c = e[2], d = e[4], f = e[5], g = e[6], h = e[8], i = e[9], j = e[10];
    var A = f * j - g * i, B = g * h - d * j, C = d * i - f * h;
    var det = a * A + b * B + c * C, k = det === 0 ? 0 : 1 / det;
    out[0] = A * k; out[1] = B * k; out[2] = C * k;
    out[3] = (c * i - b * j) * k; out[4] = (a * j - c * h) * k; out[5] = (b * h - a * i) * k;
    out[6] = (b * g - c * f) * k; out[7] = (c * d - a * g) * k; out[8] = (a * f - b * d) * k;
    return out;
  }

  function Backend(gl, renderer) {
    this.gl = gl;
    this.renderer = renderer;
    this.mains = {};
    this.frame = {};
    this.fver = 0;
    this.cur = null;
    this.buffers = new WeakMap();   // attribute -> { buf, version }
    this.wire = new WeakMap();      // geometry -> { buf, count, key }
    this.textures = new WeakMap();  // texture -> { tex, version }
    this.vao = gl.createVertexArray();
    this.emptyVao = gl.createVertexArray();
    this.envs = new WeakMap();      // equirect texture -> prefiltered levels
    this.lightData = new Float32Array(MAX_LIGHTS * 28);
    this.mapTransforms = new Float32Array(54);
    this.normalMat = new Float32Array(9);
    this.textureCount = 0;
    // the DFG table for Standard/Physical lighting (renderers/dfg_table.rs)
    this.dfg = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.dfg);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RG16F, 16, 16, 0, gl.RG, gl.FLOAT, Float32Array.from(R.RenderersDfgTable.dfg_table()));
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    // the rect area light tables (renderers/ltc_table.rs), float when float textures filter, else half float
    var floatLinear = !!gl.getExtension("OES_texture_float_linear");
    // both tables side by side (64 x 64 each); lookups stay half a texel inside each, so they never mix
    var l1 = R.RenderersLtcTable.ltc_1(), l2 = R.RenderersLtcTable.ltc_2(), both = new Float32Array(128 * 64 * 4);
    for (var ly = 0; ly < 64; ly++) for (var lx = 0; lx < 64; lx++) for (var lc = 0; lc < 4; lc++) {
      both[(ly * 128 + lx) * 4 + lc] = l1[(ly * 64 + lx) * 4 + lc];
      both[(ly * 128 + 64 + lx) * 4 + lc] = l2[(ly * 64 + lx) * 4 + lc];
    }
    this.ltc = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.ltc);
    gl.texImage2D(gl.TEXTURE_2D, 0, floatLinear ? gl.RGBA32F : gl.RGBA16F, 128, 64, 0, gl.RGBA, gl.FLOAT, both);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);
  }
  var B = Backend.prototype;

  // The main program is compiled per variant: the features a draw uses
  // (SPEC uniforms and the K_ flags) become constants, so the shader holds
  // only the branches that draw takes. `key` lists the values in SPEC_KEYS
  // order after the shadow type (1 PCF, also without shadows; 2 basic; 3 VSM).
  var SPEC = ["u_hasMap", "u_points", "u_hasNormal", "u_opaque", "u_tone", "u_maps", "u_linearOut", "u_premult", "u_hasEmap", "u_dashed", "u_receive",
    "u_instanced", "u_hasDisp", "u_sprite"];
  var K_FLAGS = ["K_MODE", "K_LIT", "K_PHONG", "K_PHYS", "K_FLAT", "K_SHEEN", "K_TRANS", "K_RECT", "K_FOG", "K_SHPASS", "K_ENV",
    // lights per kind (ambient, hemisphere, directional, point, spot, rect area), packed in that order
    "K_NAMB", "K_NHEMI", "K_NDIR", "K_NPOINT", "K_NSPOT", "K_NRECT"];
  var SPEC_KEYS = SPEC.concat(K_FLAGS);
  function fragmentSource(type, vals) {
    var head = "#version 300 es\n#define SHADOW_TYPE " + type + "\n";
    for (var i = 0; i < SPEC_KEYS.length; i++) head += "#define " + SPEC_KEYS[i] + " " + vals[i] + "\n";
    var body = FS_BODY.replace(/^uniform int (u_\w+);\n/gm, function (line, name) { return SPEC.indexOf(name) >= 0 ? "" : line; });
    // one loop per light kind the frame has, with the kind a constant
    var loops = "", first = 0, at = SPEC_KEYS.indexOf("K_NAMB");
    for (var kind = 0; kind < 6; kind++) {
      var count = vals[at + kind];
      if (!count) continue;
      loops += "    for (int i = " + first + "; i < " + (first + count) + "; i++) {\n" +
        "      vec4 l0 = u_lights[i * 7], l1 = u_lights[i * 7 + 1], l2 = u_lights[i * 7 + 2], l3 = u_lights[i * 7 + 3], l4 = u_lights[i * 7 + 4];\n" +
        "      const float kind = " + kind + ".0;\n" + LIGHT_BODY + "    }\n";
      first += count;
    }
    return head + body.replace("//@LIGHTS\n", loops);
  }
  function vertexSource(type, vals) {
    var head = "#version 300 es\n";
    for (var i = 0; i < SPEC.length; i++) head += "#define " + SPEC[i] + " " + vals[i] + "\n";
    return VS.replace("#version 300 es\n", head).replace(/^uniform int (u_\w+);\n/gm, function (line, name) { return SPEC.indexOf(name) >= 0 ? "" : line; });
  }
  B.useMain = function (type, vals) {
    if (!vals) { vals = []; for (var i = 0; i < SPEC_KEYS.length; i++) vals.push(0); }
    var key = type + ":" + vals.join(","), m = this.mains[key], gl = this.gl;
    if (!m) {
      m = this.mains[key] = program(gl, vertexSource(type, vals), fragmentSource(type, vals), NAMES);
      m.seen = 0;
      this.programCount = (this.programCount || 0) + 1;
      gl.useProgram(m.prog);
      this.setUnits(m.u);
      this.cur = null;
    }
    if (this.cur !== m) { gl.useProgram(m.prog); this.cur = m; }
    this.prog = m.prog;
    this.u = m.u;
    // the frame's uniforms this program has not seen yet
    if (m.seen < this.fver) {
      for (var name in this.frame) {
        var e = this.frame[name];
        if (e.ver > m.seen) setUniform(gl, m.u[name], e.kind, e.v);
      }
      m.seen = this.fver;
    }
  };

  // a uniform every variant shares for the whole frame (or pass): recorded,
  // and set on each program when it is next used
  B.frameUniform = function (name, kind, v) {
    if (kind !== "i" && kind !== "f") v = Float32Array.from(v);
    this.frame[name] = { kind: kind, v: v, ver: ++this.fver };
    if (this.cur) { setUniform(this.gl, this.cur.u[name], kind, v); this.cur.seen = this.fver; }
  };
  function setUniform(gl, loc, kind, v) {
    if (loc == null) return;
    if (kind === "m4") gl.uniformMatrix4fv(loc, false, v);
    else if (kind === "v4") gl.uniform4fv(loc, v);
    else if (kind === "v3") gl.uniform3fv(loc, v);
    else if (kind === "v2") gl.uniform2fv(loc, v);
    else if (kind === "i") gl.uniform1i(loc, v);
    else gl.uniform1f(loc, v);
  }

  // a GPU buffer holding `attr`, re-uploaded when its version changes
  B.buffer = function (attr, target) {
    var gl = this.gl, e = this.buffers.get(attr);
    if (e && e.version === attr.version && e.length === attr.array.length) return e.buf;
    if (!e) { e = { buf: gl.createBuffer(), version: -1, length: 0 }; this.buffers.set(attr, e); }
    // typed arrays of the right kind go as they are; a same-sized update rewrites the buffer in place
    var arr = attr.array, index = target === gl.ELEMENT_ARRAY_BUFFER;
    var data = index ? (arr instanceof Uint32Array ? arr : Uint32Array.from(arr)) : (arr instanceof Float32Array ? arr : Float32Array.from(arr));
    gl.bindBuffer(target, e.buf);
    if (e.length === arr.length) gl.bufferSubData(target, 0, data);
    else gl.bufferData(target, data, attr.usage === 35048 ? gl.DYNAMIC_DRAW : gl.STATIC_DRAW);
    e.version = attr.version; e.length = attr.array.length;
    return e.buf;
  };

  B.attrib = function (loc, attr, size, fallback) {
    var gl = this.gl;
    if (!attr) {
      gl.disableVertexAttribArray(loc);
      if (fallback) gl.vertexAttrib4f(loc, fallback[0], fallback[1], fallback[2], fallback[3]);
      return false;
    }
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer(attr, gl.ARRAY_BUFFER));
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, Math.min(size, attr.itemSize), gl.FLOAT, false, attr.itemSize * 4, 0);
    gl.vertexAttribDivisor(loc, 0);
    return true;
  };

  // triangle edges as a line index (for wireframe)
  B.wireIndex = function (geometry, index, count) {
    var gl = this.gl, key = (index ? index.version : -1) + ":" + count;
    var e = this.wire.get(geometry);
    if (e && e.key === key) return e;
    if (!e) { e = { buf: gl.createBuffer(), count: 0, key: "" }; this.wire.set(geometry, e); }
    var out = new Uint32Array(Math.floor(count / 3) * 6), j = 0;
    for (var i = 0; i + 2 < count; i += 3) {
      var a = index ? index.array[i] : i, b = index ? index.array[i + 1] : i + 1, c = index ? index.array[i + 2] : i + 2;
      out[j++] = a; out[j++] = b; out[j++] = b; out[j++] = c; out[j++] = c; out[j++] = a;
    }
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, e.buf);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, out, gl.STATIC_DRAW);
    e.count = j; e.key = key;
    return e;
  };

  // the GL texture for `tex`, uploaded when its version changes; null when
  // its image is not ready
  B.texture = function (tex) {
    var gl = this.gl, img = tex.image;
    if (!img) return null;
    var e = this.textures.get(tex);
    var srgb = tex.colorSpace === "srgb";
    if (!e || e.version !== tex.version || e.srgb !== srgb) {
      var dom = !(img.data && img.width && img.height);
      if (dom && (!(img.naturalWidth || img.videoWidth || img.width) || img.complete === false)) return e ? e.tex : null;
      if (!e) { e = { tex: gl.createTexture(), version: -1 }; this.textures.set(tex, e); this.textureCount++; }
      gl.bindTexture(gl.TEXTURE_2D, e.tex);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, !!tex.flip_y);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, !!tex.premultiply_alpha);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      if (dom) {
        gl.texImage2D(gl.TEXTURE_2D, 0, srgb ? gl.SRGB8_ALPHA8 : gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
      } else {
        var px = T.WebGLRenderer.decodeImage(img, tex);
        var bytes = new Uint8Array(px.data.buffer, px.data.byteOffset, px.data.byteLength);
        gl.texImage2D(gl.TEXTURE_2D, 0, srgb ? gl.SRGB8_ALPHA8 : gl.RGBA, px.width, px.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, bytes);
      }
      var mip = tex.min_filter !== 1003 && tex.min_filter !== 1006;
      if (mip && tex.generate_mipmaps) gl.generateMipmap(gl.TEXTURE_2D);
      e.mip = mip && tex.generate_mipmaps;
      e.version = tex.version; e.srgb = srgb;
    }
    gl.bindTexture(gl.TEXTURE_2D, e.tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, WRAPS[tex.wrap_s] || gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, WRAPS[tex.wrap_t] || gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, tex.mag_filter === 1003 ? gl.NEAREST : gl.LINEAR);
    var min = FILTERS[tex.min_filter] || gl.LINEAR;
    if (!e.mip) min = (min === 9728 || min === 9984 || min === 9986) ? gl.NEAREST : gl.LINEAR;
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, min);
    return e.tex;
  };


  // Background textures and the prefiltered environment ("PMREM": the
  // same face layout, level sizes and GGX filtering as three.js, so blurred
  // backgrounds and, later, environment lighting match it). A level is a
  // 3 x 2 atlas of cube faces, each `size` texels with a one-texel border.
  var FULL_VS = "#version 300 es\n" +
    "void main() { vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0); }\n";
  var PMREM_FS = "#version 300 es\n" +
    "precision highp float;\n" +
    "uniform sampler2D u_src;\n" +
    "uniform int u_pass;\n" +
    "uniform float u_size, u_sizeIn, u_rough;\n" +
    "out vec4 o_color;\n" +
    CUBE_UV +
    "float vdc(uint b) { b = (b << 16u) | (b >> 16u); b = ((b & 0x55555555u) << 1u) | ((b & 0xAAAAAAAAu) >> 1u); b = ((b & 0x33333333u) << 2u) | ((b & 0xCCCCCCCCu) >> 2u);\n" +
    "  b = ((b & 0x0F0F0F0Fu) << 4u) | ((b & 0xF0F0F0F0u) >> 4u); b = ((b & 0x00FF00FFu) << 8u) | ((b & 0xFF00FF00u) >> 8u); return float(b) * 2.3283064365386963e-10; }\n" +
    "void main() {\n" +
    "  vec2 px = gl_FragCoord.xy; float col = floor(px.x / u_size), row = floor(px.y / u_size);\n" +
    "  vec3 n = normalize(faceDir((px - vec2(col, row) * u_size - 1.0) / (u_size - 2.0), col + 3.0 * row));\n" +
    "  vec3 c;\n" +
    "  if (u_pass == 0) c = texture(u_src, equirectUv(n)).rgb;\n" +
    "  else if (u_rough < 0.001) c = cubeUv(u_src, n, u_sizeIn);\n" +
    "  else {\n" +
    // GGX visible-normal sampling around n with the view along n (Heitz 2018), weighted by n.l
    "    vec3 up = abs(n.z) < 0.999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);\n" +
    "    vec3 tx = normalize(cross(up, n)), ty = cross(n, tx);\n" +
    "    float a = u_rough * u_rough; vec3 sum = vec3(0.0); float wsum = 0.0;\n" +
    "    for (uint i = 0u; i < 256u; i++) {\n" +
    "      float r = sqrt(float(i) / 256.0), phi = 6.283185307 * vdc(i);\n" +
    "      float t1 = r * cos(phi), t2 = r * sin(phi);\n" +
    "      vec3 nh = vec3(t1, t2, sqrt(max(0.0, 1.0 - t1 * t1 - t2 * t2)));\n" +
    "      vec3 ht = normalize(vec3(a * nh.x, a * nh.y, max(0.0, nh.z)));\n" +
    "      vec3 h = normalize(tx * ht.x + ty * ht.y + n * ht.z);\n" +
    "      vec3 l = normalize(2.0 * dot(n, h) * h - n);\n" +
    "      float nl = max(dot(n, l), 0.0);\n" +
    "      if (nl > 0.0) { sum += cubeUv(u_src, l, u_sizeIn) * nl; wsum += nl; }\n" +
    "    }\n" +
    "    c = wsum > 0.0 ? sum / wsum : vec3(0.0);\n" +
    "  }\n" +
    "  o_color = vec4(c, 1.0);\n" +
    "}\n";
  var BG_FS = "#version 300 es\n" +
    "precision highp float;\n" +
    "uniform int u_mode, u_tone, u_linearOut;\n" +
    "uniform float u_exposure, u_intensity, u_lod, u_mipF, u_sizeA, u_sizeB;\n" +
    "uniform sampler2D u_tex, u_lvA, u_lvB;\n" +
    "uniform mat3 u_uvt, u_rot;\n" +
    "uniform mat4 u_view, u_proj;\n" +
    "uniform vec2 u_vp;\n" +
    "out vec4 o_color;\n" +
    COMMON + CUBE_UV +
    "void main() {\n" +
    "  vec2 q = gl_FragCoord.xy / u_vp;\n" +
    "  vec4 c = vec4(0.0, 0.0, 0.0, 1.0);\n" +
    "  if (u_mode == 1) c = texture(u_tex, (u_uvt * vec3(q, 1.0)).xy);\n" +
    "  else {\n" +
    "    mat4 ip = inverse(u_proj); vec4 a = ip * vec4(q * 2.0 - 1.0, -1.0, 1.0), b = ip * vec4(q * 2.0 - 1.0, 1.0, 1.0);\n" +
    "    vec3 d = normalize(u_rot * (inverse(mat3(u_view)) * (b.xyz / b.w - a.xyz / a.w)));\n" +
    "    if (u_mode == 2) c.rgb = textureLod(u_tex, equirectUv(d), u_lod).rgb;\n" +
    "    else { c.rgb = cubeUv(u_lvA, d, u_sizeA); if (u_mipF > 0.0) c.rgb = mix(c.rgb, cubeUv(u_lvB, d, u_sizeB), u_mipF); }\n" +
    "  }\n" +
    "  c.rgb *= u_intensity;\n" +
    "  if (u_tone > 0) c.rgb = toneMap(c.rgb);\n" +
    "  c.rgb = max(c.rgb, 0.0);\n" +
    "  if (u_linearOut == 0) c.rgb = toSrgb(c.rgb);\n" +
    "  o_color = c;\n" +
    "}\n";

  // VSM: the depth map blurred into mean and deviation, vertically then horizontally (three.js's vsm.glsl)
  var VSM_FS = "#version 300 es\n" +
    "precision highp float;\n" +
    "uniform sampler2D u_src;\n" +
    "uniform vec2 u_res;\n" +
    "uniform float u_radius, u_samples;\n" +
    "uniform int u_horizontal;\n" +
    "out vec4 o_color;\n" +
    "void main() {\n" +
    "  float mean = 0.0, sq = 0.0;\n" +
    "  float stride = u_samples <= 1.0 ? 0.0 : 2.0 / (u_samples - 1.0), start = u_samples <= 1.0 ? 0.0 : -1.0;\n" +
    "  for (float i = 0.0; i < 64.0; i++) {\n" +
    "    if (i >= u_samples) break;\n" +
    "    float off = start + i * stride;\n" +
    "    if (u_horizontal == 1) { vec2 d = texture(u_src, (gl_FragCoord.xy + vec2(off, 0.0) * u_radius) / u_res).rg; mean += d.x; sq += d.y * d.y + d.x * d.x; }\n" +
    "    else { float z = texture(u_src, (gl_FragCoord.xy + vec2(0.0, off) * u_radius) / u_res).r; mean += z; sq += z * z; }\n" +
    "  }\n" +
    "  mean /= u_samples; sq /= u_samples;\n" +
    "  o_color = vec4(mean, sqrt(max(0.0, sq - mean * mean)), 0.0, 1.0);\n" +
    "}\n";

  function program(gl, vs, fs, names) {
    var prog = gl.createProgram();
    gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, vs));
    gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error("WebGL2 program: " + gl.getProgramInfoLog(prog));
    var u = {};
    for (var i = 0; i < names.length; i++) u[names[i]] = gl.getUniformLocation(prog, names[i]);
    return { prog: prog, u: u };
  }

  // the roughness -> level mapping of the prefiltered environment (three.js's roughnessToMip)
  function roughnessToMip(r) {
    if (r >= 0.8) return (1.0 - r) * (-1.0 + 2.0) / 0.2 - 2.0;
    if (r >= 0.4) return (0.8 - r) * (2.0 + 1.0) / 0.4 - 1.0;
    if (r >= 0.305) return (0.4 - r) * (3.0 - 2.0) / (0.4 - 0.305) + 2.0;
    if (r >= 0.21) return (0.305 - r) * (4.0 - 3.0) / (0.305 - 0.21) + 3.0;
    return -2.0 * Math.log2(1.16 * r);
  }

  // the prefiltered levels of an equirectangular texture, made once per texture version
  B.pmrem = function (tex) {
    var gl = this.gl, e = this.envs.get(tex);
    if (e && e.version === tex.version) return e;
    var img = tex.image;
    var w = img && (img.width || img.naturalWidth || img.videoWidth);
    if (!w) return null;
    if (!this.pm) {
      this.pm = program(gl, FULL_VS, PMREM_FS, ["u_src", "u_pass", "u_size", "u_sizeIn", "u_rough"]);
      this.fbo = gl.createFramebuffer();
      this.halfFloat = !!gl.getExtension("EXT_color_buffer_float");
    }
    if (e) { for (var d = 0; d < e.levels.length; d++) gl.deleteTexture(e.levels[d]); gl.deleteTexture(e.atlas); }
    var lodMax = Math.floor(Math.log2(w / 4)), cube = Math.pow(2, lodMax);
    var n = lodMax - 4 + 1 + 6, sizes = [], levels = [];
    for (var i = 0; i < n; i++) sizes.push(i > lodMax - 4 ? 16 : Math.pow(2, lodMax - i));
    var self = this, pm = this.pm, u = pm.u;
    var make = function (s) {
      var t = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texImage2D(gl.TEXTURE_2D, 0, self.halfFloat ? gl.RGBA16F : gl.RGBA8, 3 * s, 2 * s, 0, gl.RGBA, self.halfFloat ? gl.HALF_FLOAT : gl.UNSIGNED_BYTE, null);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      return t;
    };
    var pass = function (target, s, src, kind, sIn, rough) {
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, target, 0);
      gl.viewport(0, 0, 3 * s, 2 * s);
      gl.activeTexture(gl.TEXTURE0);
      if (kind === 0) self.texture(tex); else gl.bindTexture(gl.TEXTURE_2D, src);
      gl.uniform1i(u.u_pass, kind); gl.uniform1f(u.u_size, s); gl.uniform1f(u.u_sizeIn, sIn); gl.uniform1f(u.u_rough, rough);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };
    gl.useProgram(pm.prog);
    gl.bindVertexArray(this.emptyVao);
    gl.disable(gl.BLEND); gl.disable(gl.DEPTH_TEST); gl.disable(gl.CULL_FACE); gl.disable(gl.POLYGON_OFFSET_FILL);
    gl.colorMask(true, true, true, true);
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.fbo);
    gl.uniform1i(u.u_src, 0);
    levels.push(make(cube));
    pass(levels[0], cube, null, 0, 0, 0);
    // each level filters the one before it with the extra roughness between them (three.js's _applyGGXFilter)
    for (var k = 1; k < n; k++) {
      var target = k / (n - 1), source = (k - 1) / (n - 1);
      var rough = Math.sqrt(target * target - source * source) * (target * 1.25);
      var tmp = make(sizes[k]);
      pass(tmp, sizes[k], levels[k - 1], 1, sizes[k - 1], rough);
      levels.push(make(sizes[k]));
      pass(levels[k], sizes[k], tmp, 1, sizes[k], 0);
      gl.deleteTexture(tmp);
    }
    // every level in one texture laid out as three.js's CubeUV map: the levels down to 16 texels in a
    // column, the rougher 16-texel levels in a row beside the last one
    var aw = 3 * Math.max(cube, 112), ah = 4 * cube;
    var atlas = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, atlas);
    gl.texImage2D(gl.TEXTURE_2D, 0, self.halfFloat ? gl.RGBA16F : gl.RGBA8, aw, ah, 0, gl.RGBA, self.halfFloat ? gl.HALF_FLOAT : gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);
    if (!this.fbo2) this.fbo2 = gl.createFramebuffer();
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this.fbo2);
    gl.framebufferTexture2D(gl.DRAW_FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, atlas, 0);
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.fbo);
    for (var q = 0; q < n; q++) {
      var s = sizes[q], m = lodMax - q, ax = m >= 4 ? 0 : (4 - m) * 48, ay = 4 * (cube - Math.max(s, 16));
      gl.framebufferTexture2D(gl.READ_FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, levels[q], 0);
      gl.blitFramebuffer(0, 0, 3 * s, 2 * s, ax, ay, ax + 3 * s, ay + 2 * s, gl.COLOR_BUFFER_BIT, gl.NEAREST);
    }
    gl.framebufferTexture2D(gl.READ_FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, null, 0);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this.fbo2);
    gl.framebufferTexture2D(gl.DRAW_FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, null, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    e = { version: tex.version, levels: levels, sizes: sizes, lodMax: lodMax, atlas: atlas, aw: aw, ah: ah };
    this.envs.set(tex, e);
    return e;
  };

  // scene.background when it is a texture: a screen-filling texture (uv
  // mapping) or the world seen through an equirectangular map, blurred
  // through the prefiltered levels when backgroundBlurriness > 0
  B.background = function (scene, plan, linearOut, w, h) {
    var gl = this.gl, tex = scene.background;
    if (!tex || tex.isColor || !tex.image) return;
    var equirect = tex.mapping === 303 || tex.mapping === 304;
    var blur = scene.backgroundBlurriness || 0;
    var env = equirect && blur > 0 ? this.pmrem(tex) : null;
    if (!this.bg) this.bg = program(gl, FULL_VS, BG_FS, ["u_mode", "u_tone", "u_linearOut", "u_exposure", "u_intensity", "u_lod", "u_mipF", "u_sizeA", "u_sizeB",
      "u_tex", "u_lvA", "u_lvB", "u_uvt", "u_rot", "u_view", "u_proj", "u_vp"]);
    var bg = this.bg, u = bg.u;
    gl.useProgram(bg.prog);
    gl.bindVertexArray(this.emptyVao);
    gl.viewport(0, 0, w, h);
    gl.disable(gl.BLEND); gl.disable(gl.DEPTH_TEST); gl.disable(gl.CULL_FACE); gl.depthMask(false);
    gl.uniform1i(u.u_tex, 0); gl.uniform1i(u.u_lvA, 15); gl.uniform1i(u.u_lvB, 3);
    gl.uniform2f(u.u_vp, w, h);
    gl.uniformMatrix4fv(u.u_view, false, plan.view.elements);
    gl.uniformMatrix4fv(u.u_proj, false, plan.projection.elements);
    var rot = new T.Matrix4();
    if (scene.backgroundRotation) rot.makeRotationFromEuler(scene.backgroundRotation);
    var re = rot.elements;
    gl.uniformMatrix3fv(u.u_rot, false, [re[0], re[1], re[2], re[4], re[5], re[6], re[8], re[9], re[10]]);
    gl.uniform1f(u.u_intensity, scene.backgroundIntensity == null ? 1 : scene.backgroundIntensity);
    // three.js tone-maps a background unless its texture is sRGB (the prefiltered levels are linear)
    gl.uniform1i(u.u_tone, tex.colorSpace === "srgb" && !env ? 0 : this.tone);
    gl.uniform1f(u.u_exposure, this.renderer.toneMappingExposure === undefined ? 1 : this.renderer.toneMappingExposure);
    gl.uniform1i(u.u_linearOut, linearOut ? 1 : 0);
    if (env) {
      var mip = Math.min(Math.max(roughnessToMip(blur), -2), env.lodMax);
      var mi = Math.floor(mip), mf = mip - mi;
      var a = env.lodMax - mi, b = Math.max(0, a - 1);
      gl.activeTexture(gl.TEXTURE15); gl.bindTexture(gl.TEXTURE_2D, env.levels[a]);
      gl.activeTexture(gl.TEXTURE3); gl.bindTexture(gl.TEXTURE_2D, env.levels[b]);
      gl.uniform1f(u.u_sizeA, env.sizes[a]); gl.uniform1f(u.u_sizeB, env.sizes[b]);
      gl.uniform1f(u.u_mipF, mf);
      gl.uniform1i(u.u_mode, 3);
    } else {
      gl.activeTexture(gl.TEXTURE0);
      if (!this.texture(tex)) return;
      if (equirect) {
        // the mip level whose texel is one screen pixel wide (no derivative jump at the seam)
        var pixel = 2 * Math.atan(1 / Math.abs(plan.projection.elements[5])) / h;
        gl.uniform1f(u.u_lod, Math.max(0, Math.log2(pixel / (2 * Math.PI / (tex.image.width || 1)))));
        gl.uniform1i(u.u_mode, 2);
      } else {
        if (tex.matrixAutoUpdate !== false && tex.updateMatrix) tex.updateMatrix();
        gl.uniformMatrix3fv(u.u_uvt, false, tex.matrix.elements);
        gl.uniform1i(u.u_mode, 1);
      }
    }
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.activeTexture(gl.TEXTURE0);
    this.cur = null;
  };


  // the opaque scene (and background) drawn linear and untoned into a
  // multisampled target, resolved to a mipmapped texture that transmissive
  // materials sample (three.js's renderTransmissionPass); double-sided
  // transmissive objects add their back faces
  B.transmissionPass = function (scene, plan, opaque, transmissive, w, h) {
    var gl = this.gl, t = this.trans;
    if (!t || t.w !== w || t.h !== h) {
      if (t) { gl.deleteFramebuffer(t.ms); gl.deleteFramebuffer(t.res); gl.deleteRenderbuffer(t.color); gl.deleteRenderbuffer(t.depth); gl.deleteTexture(t.tex); }
      var half = !!gl.getExtension("EXT_color_buffer_float");
      t = this.trans = { w: w, h: h, ms: gl.createFramebuffer(), res: gl.createFramebuffer(), color: gl.createRenderbuffer(), depth: gl.createRenderbuffer(), tex: gl.createTexture() };
      var samples = Math.min(4, gl.getParameter(gl.MAX_SAMPLES));
      gl.bindRenderbuffer(gl.RENDERBUFFER, t.color);
      gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, half ? gl.RGBA16F : gl.RGBA8, w, h);
      gl.bindRenderbuffer(gl.RENDERBUFFER, t.depth);
      gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, gl.DEPTH_COMPONENT24, w, h);
      gl.bindFramebuffer(gl.FRAMEBUFFER, t.ms);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, t.color);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, t.depth);
      gl.bindTexture(gl.TEXTURE_2D, t.tex);
      var levels = Math.floor(Math.log2(Math.max(w, h))) + 1;
      gl.texStorage2D(gl.TEXTURE_2D, levels, half ? gl.RGBA16F : gl.RGBA8, w, h);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.bindFramebuffer(gl.FRAMEBUFFER, t.res);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, t.tex, 0);
    }
    var resolve = function () {
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, t.ms);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, t.res);
      gl.blitFramebuffer(0, 0, w, h, 0, 0, w, h, gl.COLOR_BUFFER_BIT, gl.NEAREST);
      gl.bindFramebuffer(gl.FRAMEBUFFER, t.ms);
      gl.activeTexture(gl.TEXTURE14);
      gl.bindTexture(gl.TEXTURE_2D, t.tex);
      gl.generateMipmap(gl.TEXTURE_2D);
      gl.activeTexture(gl.TEXTURE0);
    };
    gl.bindFramebuffer(gl.FRAMEBUFFER, t.ms);
    gl.viewport(0, 0, w, h);
    // the clear colour in linear; a see-through clear becomes white at half alpha
    var cl = plan.clear, ca = plan.clear_alpha;
    if (ca < 1) gl.clearColor(1, 1, 1, 0.5); else gl.clearColor(cl.r, cl.g, cl.b, ca);
    gl.colorMask(true, true, true, true);
    gl.depthMask(true);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    var tone = this.tone, info = { triangles: 0, lines: 0, points: 0, calls: 0 };
    this.tone = 0;
    this.background(scene, plan, true, w, h);
    gl.depthMask(true);
    this.cur = null;
    gl.bindVertexArray(this.vao);
    this.outLinear = 1;
    for (var i = 0; i < opaque.length; i++) this.draw(opaque[i], info, null);
    resolve();
    var back = false;
    for (var j = 0; j < transmissive.length; j++) {
      var m = transmissive[j].object.material;
      if (m.side !== 2) continue;
      if (!back) { this.transTex = t.tex; back = true; }
      this.drawSide = 1;
      this.draw(transmissive[j], info, null);
      this.drawSide = null;
    }
    if (back) resolve();
    this.tone = tone;
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, w, h);
    this.outLinear = this.linearOut ? 1 : 0;
    this.transTex = t.tex;
    gl.activeTexture(gl.TEXTURE14);
    gl.bindTexture(gl.TEXTURE_2D, t.tex);
    gl.activeTexture(gl.TEXTURE0);
    this.frameUniform("u_transSize", "v2", [w, h]);
  };

  // a depth texture for a shadow map: 2D or cube, compared (PCF, basic) or plain (VSM)
  function depthTexture(gl, cube, size, w, h, type) {
    var t = gl.createTexture(), target = cube ? gl.TEXTURE_CUBE_MAP : gl.TEXTURE_2D;
    gl.bindTexture(target, t);
    var fmt = type === 3 ? gl.DEPTH_COMPONENT32F : gl.DEPTH_COMPONENT24, ty = type === 3 ? gl.FLOAT : gl.UNSIGNED_INT;
    if (cube) for (var f = 0; f < 6; f++) gl.texImage2D(gl.TEXTURE_CUBE_MAP_POSITIVE_X + f, 0, fmt, size, size, 0, gl.DEPTH_COMPONENT, ty, null);
    else gl.texImage2D(gl.TEXTURE_2D, 0, fmt, w, h, 0, gl.DEPTH_COMPONENT, ty, null);
    var filter = type === 1 ? gl.LINEAR : gl.NEAREST;
    gl.texParameteri(target, gl.TEXTURE_MIN_FILTER, filter);
    gl.texParameteri(target, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(target, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(target, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    if (type !== 3) {
      gl.texParameteri(target, gl.TEXTURE_COMPARE_MODE, gl.COMPARE_REF_TO_TEXTURE);
      gl.texParameteri(target, gl.TEXTURE_COMPARE_FUNC, gl.LEQUAL);
    }
    gl.bindTexture(target, null);
    return t;
  }

  function colorTarget(gl, w, h, half, filter) {
    var t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texImage2D(gl.TEXTURE_2D, 0, half ? gl.RG16F : gl.RGBA8, w, h, 0, half ? gl.RG : gl.RGBA, half ? gl.HALF_FLOAT : gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);
    return t;
  }

  var SHADOW_UNITS = [8, 9, 13, 15];

  // every shadow sampler needs a texture of its kind bound even when no light
  // uses it (an empty unit reads as a colour texture, which a shadow sampler
  // rejects), and none may hold a map that is being rendered: 1x1 compared
  // depth textures for PCF and basic, nothing for VSM's plain samplers
  B.bindShadowDummies = function (type) {
    var gl = this.gl;
    if (!this.shDummy) this.shDummy = { flat: depthTexture(gl, false, 1, 1, 1, 1), cube: depthTexture(gl, true, 1, 1, 1, 1) };
    for (var i = 0; i < 4; i++) {
      gl.activeTexture(gl.TEXTURE0 + SHADOW_UNITS[i]);
      gl.bindTexture(gl.TEXTURE_2D, null);
      gl.bindTexture(gl.TEXTURE_CUBE_MAP, null);
      if (type !== 3) gl.bindTexture(i === 3 ? gl.TEXTURE_CUBE_MAP : gl.TEXTURE_2D, i === 3 ? this.shDummy.cube : this.shDummy.flat);
    }
    gl.activeTexture(gl.TEXTURE0);
  };

  // the shadow maps of this frame (three.js's WebGLShadowMap): depth from each
  // shadow camera of the casters (back faces of front-sided materials, as
  // three.js draws them), point lights into six cube faces, VSM blurred.
  // Returns the slot of each light index (2D slots 0-2, the point light 3).
  B.renderShadows = function (scene, plan, type) {
    var gl = this.gl, slots = {};
    var plans = R.RenderersPlan.shadow_plans(scene);
    if (!plans.length) return slots;
    var casters = [];
    for (var i = 0; i < plan.items.length; i++) {
      var it = plan.items[i], o = it.object;
      if (it.kind >= 1 && it.kind <= 6 && (o.castShadow || (type === 3 && o.receiveShadow))) casters.push(it);
    }
    this.shadowRes = this.shadowRes || [];
    this.shadowData = this.shadowData || { mat: new Float32Array(64), param: new Float32Array(16), size: new Float32Array(16) };
    var next2d = 0, usedPoint = false, half = !!gl.getExtension("EXT_color_buffer_float");
    var info = { triangles: 0, lines: 0, points: 0, calls: 0 };
    this.cur = null;
    gl.bindVertexArray(this.vao);
    this.frameUniform("u_nlights", "i", 0);
    this.shadowPass = type;
    for (var p = 0; p < plans.length; p++) {
      var sp = plans[p], point = sp.kind === 3, slot;
      if (point) { if (usedPoint || type === 3) continue; slot = 3; usedPoint = true; }
      else { if (next2d > 2) continue; slot = next2d++; }
      var w = sp.map_width | 0, h = sp.map_height | 0, size = w;
      var res = this.shadowRes[slot];
      if (!res || res.w !== w || res.h !== h || res.type !== type || res.point !== point) {
        if (res) { gl.deleteTexture(res.depth); if (res.map) gl.deleteTexture(res.map); if (res.pass) gl.deleteTexture(res.pass); }
        res = this.shadowRes[slot] = { w: w, h: h, type: type, point: point, fbo: res ? res.fbo : gl.createFramebuffer(), depth: depthTexture(gl, point, size, w, h, type) };
        if (type === 3) { res.pass = colorTarget(gl, w, h, half, gl.LINEAR); res.map = colorTarget(gl, w, h, half, gl.LINEAR); }
      }
      gl.bindFramebuffer(gl.FRAMEBUFFER, res.fbo);
      this.frameUniform("u_proj", "m4", sp.projection.elements);
      var faces = point ? 6 : 1;
      for (var f = 0; f < faces; f++) {
        if (point) gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_CUBE_MAP_POSITIVE_X + f, res.depth, 0);
        else gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_2D, res.depth, 0);
        gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, null, 0);
        gl.drawBuffers([gl.NONE]);
        gl.viewport(0, 0, point ? size : w, point ? size : h);
        gl.depthMask(true);
        gl.clear(gl.DEPTH_BUFFER_BIT);
        this.frameUniform("u_view", "m4", sp.views[f].elements);
        for (var c = 0; c < casters.length; c++) this.draw(casters[c], info, null);
      }
      gl.drawBuffers([gl.COLOR_ATTACHMENT0]);
      if (type === 3) this.vsmBlur(res, sp);
      // the receivers' side of it
      var d = this.shadowData;
      d.mat.set(sp.matrix.elements, slot * 16);
      d.param[slot * 4] = sp.intensity; d.param[slot * 4 + 1] = sp.bias; d.param[slot * 4 + 2] = sp.normal_bias; d.param[slot * 4 + 3] = sp.radius;
      d.size[slot * 4] = point ? size : w; d.size[slot * 4 + 1] = point ? size : h; d.size[slot * 4 + 2] = sp.near; d.size[slot * 4 + 3] = sp.far;
      slots[sp.light] = slot;
      gl.activeTexture(gl.TEXTURE0 + SHADOW_UNITS[slot]);
      gl.bindTexture(point ? gl.TEXTURE_CUBE_MAP : gl.TEXTURE_2D, type === 3 ? res.map : res.depth);
      gl.activeTexture(gl.TEXTURE0);
    }
    this.shadowPass = 0;
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return slots;
  };

  B.vsmBlur = function (res, sp) {
    var gl = this.gl;
    if (!this.vsm) this.vsm = program(gl, FULL_VS, VSM_FS, ["u_src", "u_res", "u_radius", "u_samples", "u_horizontal"]);
    var v = this.vsm, u = v.u;
    gl.useProgram(v.prog);
    gl.bindVertexArray(this.emptyVao);
    gl.disable(gl.DEPTH_TEST); gl.disable(gl.BLEND); gl.disable(gl.CULL_FACE);
    gl.uniform1i(u.u_src, 0);
    gl.uniform2f(u.u_res, res.w, res.h);
    gl.uniform1f(u.u_radius, sp.radius);
    gl.uniform1f(u.u_samples, sp.blur_samples);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_2D, null, 0);
    var steps = [[res.depth, res.pass, 0], [res.pass, res.map, 1]];
    for (var i = 0; i < 2; i++) {
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, steps[i][1], 0);
      gl.viewport(0, 0, res.w, res.h);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, steps[i][0]);
      gl.uniform1i(u.u_horizontal, steps[i][2]);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, null, 0);
    this.cur = null;
    gl.bindVertexArray(this.vao);
    gl.enable(gl.DEPTH_TEST);
  };

  // the texture unit of every sampler of the main program
  B.setUnits = function (u) {
    var gl = this.gl;
    gl.uniform1i(u.u_map, 0);
    gl.uniform1i(u.u_emap, 1);
    gl.uniform1i(u.u_dfg, 2);
    for (var mi = 0; mi < MAPS.length; mi++) gl.uniform1i(u[MAPS[mi].uniform], MAPS[mi].unit);
    gl.uniform1i(u.u_dispMap, 12);
    gl.uniform1i(u.u_ltc, 10);
    gl.uniform1i(u.u_envMap, 11);
    gl.uniform1i(u.u_transMap, 14);
    gl.uniform1i(u.u_sh0, SHADOW_UNITS[0]);
    gl.uniform1i(u.u_sh1, SHADOW_UNITS[1]);
    gl.uniform1i(u.u_sh2, SHADOW_UNITS[2]);
    gl.uniform1i(u.u_shc0, SHADOW_UNITS[3]);
  };

  // three.js's choice of environment for a material: its own envMap, else scene.environment for
  // Standard/Physical, Lambert and Phong; Standard/Physical read it prefiltered
  function envOf(material, scene) {
    var pbr = !!material.isMeshStandardMaterial, lp = !!(material.isMeshLambertMaterial || material.isMeshPhongMaterial);
    if (!pbr && !lp && !material.isMeshBasicMaterial) return null;
    var own = material.envMap || null, tex = own || ((pbr || lp) && scene && scene.environment) || null;
    if (!tex || tex.isColor || !tex.image || !(tex.mapping === 303 || tex.mapping === 304)) return null;
    return { tex: tex, pbr: pbr, scene: !own };
  }

  // the prefiltered environments this frame needs, made before any drawing (they use their own targets)
  B.prepareEnvs = function (scene, plan) {
    var seen = [];
    for (var i = 0; i < plan.items.length; i++) {
      var ms = plan.items[i].object.material;
      ms = Array.isArray(ms) ? ms : ms ? [ms] : [];
      for (var j = 0; j < ms.length; j++) {
        var e = envOf(ms[j], scene);
        if (e && e.pbr && seen.indexOf(e.tex) < 0) { seen.push(e.tex); this.pmrem(e.tex); }
      }
    }
  };

  B.render = function (scene, camera) {
    var gl = this.gl, r = this.renderer, soft = r._soft;
    var plan = R.RenderersPlan.plan_frame(scene, camera, soft.clear_color, soft.clear_alpha);
    this.scene = scene;
    var w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
    // shadow type (three.js: 0 basic, 1 PCF, 2 PCF soft = PCF, 3 VSM) -> 2, 1, 3; 0 none
    var sm = r.shadowMap, stype = sm && sm.enabled ? (sm.type === 0 ? 2 : sm.type === 3 ? 3 : 1) : 0;
    this.stype = stype || 1;
    this.cur = null;
    this.outLinear = r.outputColorSpace === "srgb-linear" ? 1 : 0;
    this.bindShadowDummies(stype || 1);
    var slots = stype ? this.renderShadows(scene, plan, stype) : {};
    this.prepareEnvs(scene, plan);
    this.cur = null;
    gl.viewport(0, 0, w, h);
    var linearOut = r.outputColorSpace === "srgb-linear";
    var cl = plan.clear, s = linearOut ? function (x) { return x; } : R.MathsColor.linear_to_srgb;
    gl.clearColor(s(Math.min(1, Math.max(0, cl.r))), s(Math.min(1, Math.max(0, cl.g))), s(Math.min(1, Math.max(0, cl.b))), plan.clear_alpha);
    gl.depthMask(true);
    gl.colorMask(true, true, true, true);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    // tone mapping (three.js constants: 0 none, 1 linear, 2 Reinhard, 3 Cineon, 4 ACES filmic, 6 AgX, 7 neutral)
    this.tone = r.toneMapping | 0;
    this.background(scene, plan, linearOut, w, h);
    gl.depthMask(true);
    this.cur = null;
    gl.bindVertexArray(this.vao);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    this.frameUniform("u_view", "m4", plan.view.elements);
    this.frameUniform("u_viewInv", "m4", (this._viewInv || (this._viewInv = new T.Matrix4())).fromArray(plan.view.elements).invert().elements);
    this.frameUniform("u_proj", "m4", plan.projection.elements);
    this.frameUniform("u_cam", "v3", [plan.camera_position.x, plan.camera_position.y, plan.camera_position.z]);
    var n = Math.min(MAX_LIGHTS, plan.light_count);
    this.lightData.fill(0);
    // packed by kind (stable), so each kind's loop reads a contiguous range
    var order = [];
    this.lightCounts = [0, 0, 0, 0, 0, 0];
    for (var i = 0; i < n; i++) { var lk = Math.round(plan.lights[i * 28 + 3]); order.push(i); this.lightCounts[lk]++; }
    order.sort(function (a, b) { return (plan.lights[a * 28 + 3] - plan.lights[b * 28 + 3]) || a - b; });
    for (var oi = 0; oi < n; oi++) {
      var src = order[oi];
      for (var j = 0; j < 28; j++) this.lightData[oi * 28 + j] = plan.lights[src * 28 + j];
      if (slots[src] !== undefined) this.lightData[oi * 28 + 18] = slots[src] + 1;
    }
    if (this.shadowData) {
      this.frameUniform("u_shMat", "m4", this.shadowData.mat);
      this.frameUniform("u_shParam", "v4", this.shadowData.param);
      this.frameUniform("u_shSize", "v4", this.shadowData.size);
    }
    this.frameUniform("u_lights", "v4", this.lightData);
    this.frameUniform("u_nlights", "i", n);
    this.frameUniform("u_fog", "v4", plan.fog);
    this.frameUniform("u_pointScale", "f", h * 0.5);
    this.frameUniform("u_exposure", "f", r.toneMappingExposure === undefined ? 1 : r.toneMappingExposure);
    // the frame's variant flags: rect area lights, shadow slots, fog kind
    this.hasRect = this.lightCounts[5] ? 1 : 0;
    this.hasShadows = 0;
    for (var si in slots) if (si < n) this.hasShadows = 1;
    this.fogKind = plan.fog[0] > 1.5 ? 2 : plan.fog[0] > 0.5 ? 1 : 0;
    gl.activeTexture(gl.TEXTURE2);
    gl.bindTexture(gl.TEXTURE_2D, this.dfg);
    gl.activeTexture(gl.TEXTURE10); gl.bindTexture(gl.TEXTURE_2D, this.ltc);
    gl.activeTexture(gl.TEXTURE0);
    this.linearOut = linearOut;
    var info = { triangles: 0, lines: 0, points: 0, calls: 0 };
    var override = scene.overrideMaterial || null;
    // three.js's three lists: opaque, transmissive (back to front), transparent
    var opaque = [], transmissive = [], transparent = [];
    for (var k = 0; k < plan.items.length; k++) {
      var it = plan.items[k], m = it.object.material;
      if (m && !Array.isArray(m) && m.transmission > 0) transmissive.push(it);
      else if (it.transparent) transparent.push(it);
      else opaque.push(it);
    }
    transmissive.sort(function (a, b) { return a.depth - b.depth; });
    this.transTex = null;
    if (transmissive.length && !override) this.transmissionPass(scene, plan, opaque, transmissive, w, h);
    var lists = [opaque, transmissive, transparent];
    for (var li = 0; li < 3; li++) for (var k2 = 0; k2 < lists[li].length; k2++) this.draw(lists[li][k2], info, override);
    gl.disable(gl.POLYGON_OFFSET_FILL);
    gl.colorMask(true, true, true, true);
    gl.bindVertexArray(null);
    r.info.render.calls = info.calls;
    r.info.render.triangles = info.triangles;
    r.info.render.lines = info.lines;
    r.info.render.points = info.points;
    r.info.memory.textures = this.textureCount;
  };

  var DEPTH_FUNCS = [512, 519, 513, 515, 514, 518, 516, 517];

  // one draw-list item: each geometry group with its own material when the
  // object has a material array, the scene's override material when set
  B.draw = function (item, info, override) {
    var o = item.object, geometry = o.geometry, mats = o.material;
    if (!geometry || !mats || !geometry.getAttribute("position")) return;
    var pick = function (m) { return override && m.allowOverride !== false ? override : m; };
    if (Array.isArray(mats)) {
      var groups = geometry.groups || [];
      for (var i = 0; i < groups.length; i++) {
        var m = mats[groups[i].materialIndex];
        if (m && m.visible !== false) this.drawWith(item, pick(m), info, groups[i]);
      }
    } else {
      this.drawWith(item, pick(mats), info, null);
    }
  };

  // three.js's setBlending
  B.blend = function (material, opaque) {
    var gl = this.gl, blending = material.blending === undefined ? 1 : material.blending;
    if (blending === 0 || opaque) { gl.disable(gl.BLEND); return; }
    gl.enable(gl.BLEND);
    if (blending === 5) {
      var eq = EQUATIONS[material.blendEquation == null ? 100 : material.blendEquation];
      var eqA = material.blendEquationAlpha == null ? eq : EQUATIONS[material.blendEquationAlpha];
      var src = FACTORS[material.blendSrc == null ? 204 : material.blendSrc];
      var dst = FACTORS[material.blendDst == null ? 205 : material.blendDst];
      var srcA = material.blendSrcAlpha == null ? src : FACTORS[material.blendSrcAlpha];
      var dstA = material.blendDstAlpha == null ? dst : FACTORS[material.blendDstAlpha];
      gl.blendEquationSeparate(eq, eqA);
      gl.blendFuncSeparate(src, dst, srcA, dstA);
      var bc = material.blendColor;
      gl.blendColor(bc ? bc.r : 0, bc ? bc.g : 0, bc ? bc.b : 0, material.blendAlpha || 0);
      return;
    }
    gl.blendEquation(gl.FUNC_ADD);
    if (material.premultipliedAlpha) {
      if (blending === 1) gl.blendFuncSeparate(gl.ONE, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      else if (blending === 2) gl.blendFunc(gl.ONE, gl.ONE);
      else if (blending === 3) gl.blendFuncSeparate(gl.ZERO, gl.ONE_MINUS_SRC_COLOR, gl.ZERO, gl.ONE);
      else if (blending === 4) gl.blendFuncSeparate(gl.DST_COLOR, gl.ONE_MINUS_SRC_ALPHA, gl.ZERO, gl.ONE);
    } else {
      // three.js leaves the previous function for subtractive and multiply here (and logs an error)
      if (blending === 1) gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      else if (blending === 2) gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE, gl.ONE, gl.ONE);
    }
  };

  // the draw's environment: binds its texture on unit 11 and returns the
  // uniform values (mode 1 prefiltered, 2 equirect), or null
  B.envBind = function (material) {
    var gl = this.gl, scene = this.scene, e = this.shadowPass ? null : envOf(material, scene);
    var pm = e && e.pbr ? this.envs.get(e.tex) : null;
    if (!e || (e.pbr && !pm)) return null;
    var out;
    gl.activeTexture(gl.TEXTURE11);
    if (e.pbr) {
      gl.bindTexture(gl.TEXTURE_2D, pm.atlas);
      var intensity = e.scene ? (scene.environmentIntensity == null ? 1 : scene.environmentIntensity) : (material.envMapIntensity == null ? 1 : material.envMapIntensity);
      out = { mode: 1, uv: [pm.lodMax, 1 / pm.aw, 1 / pm.ah, 0], env: [1, intensity, 0, 0] };
    } else {
      if (!this.texture(e.tex)) { gl.activeTexture(gl.TEXTURE0); return null; }
      var refract = e.tex.mapping === 304 ? (material.refractionRatio == null ? 0.98 : material.refractionRatio) : 0;
      out = { mode: 2, uv: [0, 0, 0, refract], env: [2, 1, material.combine | 0, material.reflectivity == null ? 1 : material.reflectivity] };
    }
    gl.activeTexture(gl.TEXTURE0);
    // three.js passes the inverse (transpose) of the rotation
    var rot = e.scene ? scene.environmentRotation : material.envMapRotation;
    var m4 = this._envM || (this._envM = new T.Matrix4());
    m4.identity();
    if (rot) m4.makeRotationFromEuler(rot);
    var me = m4.elements;
    out.rot = [me[0], me[4], me[8], me[1], me[5], me[9], me[2], me[6], me[10]];
    return out;
  };

  B.envUniforms = function (env) {
    var gl = this.gl, u = this.u;
    if (!env) { gl.uniform4f(u.u_env, 0, 0, 0, 0); return; }
    gl.uniform4fv(u.u_envUV, env.uv);
    gl.uniform4fv(u.u_env, env.env);
    gl.uniformMatrix3fv(u.u_envRot, false, env.rot);
  };

  B.drawWith = function (item, material, info, group) {
    var gl = this.gl, o = item.object, kind = item.kind;
    var geometry = o.geometry, position = geometry.getAttribute("position");
    var sprite = kind === 7;
    var mu = R.RenderersPlan.material_uniforms(material);
    var mesh = kind === 1 || kind === 2;
    // buffers and textures first: what is bound decides the program variant
    this.attrib(0, position, 3);
    var hasNormal = mesh && !mu[17] && this.attrib(1, geometry.getAttribute("normal"), 3, [0, 0, 1, 0]);
    if (!hasNormal) this.attrib(1, null, 3, [0, 0, 1, 0]);
    this.attrib(2, geometry.getAttribute("uv"), 2, [0, 0, 0, 0]);
    this.attrib(3, mu[18] ? geometry.getAttribute("color") : null, 3, [1, 1, 1, 1]);
    var dashed = !!material.isLineDashedMaterial || material.type === "LineDashedMaterial";
    this.attrib(9, dashed ? geometry.getAttribute("lineDistance") : null, 1, [0, 0, 0, 0]);
    var disp = mesh ? material.displacementMap : null, glD = null;
    if (disp) {
      gl.activeTexture(gl.TEXTURE12);
      glD = this.texture(disp);
      gl.activeTexture(gl.TEXTURE0);
    }
    var sheen = material.sheen || 0, shc = material.sheenColor;
    var tr = this.transTex ? material.transmission || 0 : 0;
    var instances = 0;
    if (kind === 2 && o.instanceMatrix) {
      var im = o.instanceMatrix;
      gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer(im, gl.ARRAY_BUFFER));
      for (var c = 0; c < 4; c++) {
        gl.enableVertexAttribArray(4 + c);
        gl.vertexAttribPointer(4 + c, 4, gl.FLOAT, false, 64, c * 16);
        gl.vertexAttribDivisor(4 + c, 1);
      }
      instances = o.count;
      if (o.instanceColor) {
        gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer(o.instanceColor, gl.ARRAY_BUFFER));
        gl.enableVertexAttribArray(8);
        gl.vertexAttribPointer(8, 3, gl.FLOAT, false, 12, 0);
        gl.vertexAttribDivisor(8, 1);
      } else { gl.disableVertexAttribArray(8); gl.vertexAttrib4f(8, 1, 1, 1, 1); }
    } else {
      for (var c2 = 4; c2 < 8; c2++) gl.disableVertexAttribArray(c2);
      gl.disableVertexAttribArray(8); gl.vertexAttrib4f(8, 1, 1, 1, 1);
    }
    var map = material.map, glTex = map ? this.texture(map) : null;
    // points sample the map across each point, unless the geometry has uvs (then one texel per point, as three.js)
    var pointsUv = kind === 6 && !geometry.getAttribute("uv") ? 1 : 0;
    var emap = material.emissiveMap, glE = null;
    if (emap) {
      gl.activeTexture(gl.TEXTURE1);
      glE = this.texture(emap);
    }
    var bits = 0;
    for (var mi = 0; mi < MAPS.length; mi++) {
      var spec = MAPS[mi], t = material[spec.prop];
      if (!t || (spec.bit === 128 && material.normalMap)) continue;
      gl.activeTexture(gl.TEXTURE0 + spec.unit);
      if (!this.texture(t)) continue;
      bits |= spec.bit;
      if (spec.t >= 0) {
        if (t.matrixAutoUpdate !== false && t.updateMatrix) t.updateMatrix();
        this.mapTransforms.set(t.matrix.elements, spec.t * 9);
      }
    }
    gl.activeTexture(gl.TEXTURE0);
    var env = this.envBind(material);
    var blending = material.blending === undefined ? 1 : material.blending;
    var opaque = material.transparent !== true && blending === 1 && !material.alphaToCoverage;
    var physical = mu[22] > 0.5 ? 1 : 0;
    var receive = o.receiveShadow && this.hasShadows ? 1 : 0;
    // the program variant, in SPEC_KEYS order; a shadow caster keeps only what decides its discards
    var sp = this.shadowPass ? 1 : 0;
    this.useMain(this.stype, sp ? [glTex ? 1 : 0, pointsUv, 0, opaque ? 1 : 0, 0, bits & 1, 0, 0, 0, dashed ? 1 : 0, 0, instances > 0 ? 1 : 0, glD ? 1 : 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0] : [
      glTex ? 1 : 0, pointsUv, hasNormal ? 1 : 0, opaque ? 1 : 0, material.toneMapped === false ? 0 : this.tone, bits, this.outLinear,
      material.premultipliedAlpha ? 1 : 0, glE ? 1 : 0, dashed ? 1 : 0, receive, instances > 0 ? 1 : 0, glD ? 1 : 0, sprite ? 1 : 0,
      mu[15] | 0, mu[13] > 0.5 ? 1 : 0, mu[14] > 0.5 ? 1 : 0, physical, mu[17] > 0.5 ? 1 : 0,
      physical && sheen > 0 && shc ? 1 : 0, physical && tr > 0 ? 1 : 0, this.hasRect, mu[19] > 0.5 ? this.fogKind : 0, 0, env ? env.mode : 0].concat(mu[13] > 0.5 ? this.lightCounts : [0, 0, 0, 0, 0, 0]));
    var u = this.u;
    gl.uniform4fv(u.u_mat, mu);
    this.envUniforms(env);
    gl.uniformMatrix4fv(u.u_model, false, o.matrixWorld.elements);
    gl.uniformMatrix3fv(u.u_normalMat, false, normalMatrix(o.matrixWorld.elements, this.normalMat));
    gl.uniformMatrix3fv(u.u_uvt, false, R.RenderersPlan.map_transform(material));
    if (dashed) {
      gl.uniform1f(u.u_dashScale, material.scale == null ? 1 : material.scale);
      gl.uniform1f(u.u_dashSize, material.dashSize == null ? 3 : material.dashSize);
      gl.uniform1f(u.u_dashTotal, (material.dashSize == null ? 3 : material.dashSize) + (material.gapSize == null ? 1 : material.gapSize));
    } else gl.uniform1f(u.u_dashScale, 0);
    if (glD) {
      if (disp.matrixAutoUpdate !== false && disp.updateMatrix) disp.updateMatrix();
      gl.uniformMatrix3fv(u.u_dispT, false, disp.matrix.elements);
      gl.uniform1f(u.u_dispScale, material.displacementScale == null ? 1 : material.displacementScale);
      gl.uniform1f(u.u_dispBias, material.displacementBias || 0);
    }
    gl.uniform1i(u.u_hasDisp, glD ? 1 : 0);
    gl.uniform1f(u.u_bumpScale, material.bumpScale == null ? 1 : material.bumpScale);
    if (sheen > 0 && shc) gl.uniform4f(u.u_sheen, shc.r * sheen, shc.g * sheen, shc.b * sheen, Math.min(1, Math.max(0.0001, material.sheenRoughness == null ? 1 : material.sheenRoughness)));
    else gl.uniform4f(u.u_sheen, 0, 0, 0, 0);
    if (tr > 0) {
      var ad = material.attenuationDistance, ac = material.attenuationColor;
      gl.uniform4f(u.u_trans, tr, material.thickness || 0, material.ior == null ? 1.5 : material.ior, isFinite(ad) && ad > 0 ? ad : 0);
      gl.uniform3f(u.u_attColor, ac ? ac.r : 1, ac ? ac.g : 1, ac ? ac.b : 1);
    } else gl.uniform4f(u.u_trans, 0, 0, 0, 0);
    gl.uniform1i(u.u_instanced, instances > 0 ? 1 : 0);
    gl.uniform1i(u.u_sprite, sprite ? 1 : 0);
    if (sprite) {
      var ce = o.center || { x: 0.5, y: 0.5 };
      gl.uniform2f(u.u_center, ce.x, ce.y);
      gl.uniform1f(u.u_rotation, material.rotation || 0);
    }
    if (glE) {
      if (emap.matrixAutoUpdate !== false && emap.updateMatrix) emap.updateMatrix();
      gl.uniformMatrix3fv(u.u_euvt, false, emap.matrix.elements);
    }
    if (bits) gl.uniformMatrix3fv(u.u_t, false, this.mapTransforms);
    var ns = material.normalScale, flip = mu[16] === 1 ? -1 : 1;
    gl.uniform2f(u.u_normalScale, (ns ? ns.x : 1) * flip, (ns ? ns.y : 1) * flip);
    gl.uniform1f(u.u_aoIntensity, material.aoMapIntensity == null ? 1 : material.aoMapIntensity);
    gl.uniform1f(u.u_pointSize, (material.size || 1) * this.renderer._pixelRatio);
    gl.uniform1i(u.u_sizeAtt, material.sizeAttenuation === false ? 0 : 1);
    gl.frontFace(o.matrixWorld.determinant() < 0 ? gl.CW : gl.CCW);
    if (this.shadowPass) {
      // the depth material three.js draws shadow casters with: depth only, no blending or offset
      gl.disable(gl.BLEND);
      gl.depthMask(true);
      gl.enable(gl.DEPTH_TEST);
      gl.depthFunc(gl.LEQUAL);
      gl.disable(gl.POLYGON_OFFSET_FILL);
    } else {
      this.blend(material, opaque);
      gl.depthMask(material.depthWrite !== false);
      if (material.depthTest === false) gl.disable(gl.DEPTH_TEST);
      else { gl.enable(gl.DEPTH_TEST); gl.depthFunc(DEPTH_FUNCS[material.depthFunc == null ? 3 : material.depthFunc] || gl.LEQUAL); }
      var cw = material.colorWrite !== false;
      gl.colorMask(cw, cw, cw, cw);
      if (material.polygonOffset) { gl.enable(gl.POLYGON_OFFSET_FILL); gl.polygonOffset(material.polygonOffsetFactor || 0, material.polygonOffsetUnits || 0); }
      else gl.disable(gl.POLYGON_OFFSET_FILL);
    }
    // the range: draw range, intersected with the group (three.js's renderBufferDirect)
    var index = geometry.index, total = index ? index.count : position.count;
    var range = geometry.drawRange || { start: 0, count: Infinity };
    var start = range.start, end = range.start + range.count;
    if (group) { start = Math.max(start, group.start); end = Math.min(end, group.start + group.count); }
    start = Math.max(start, 0); end = Math.min(end, total);
    if (end <= start) return;
    var mode = gl.TRIANGLES;
    if (kind === 3) mode = gl.LINE_STRIP;
    else if (kind === 4) mode = gl.LINES;
    else if (kind === 5) mode = gl.LINE_LOOP;
    else if (kind === 6) mode = gl.POINTS;
    // side; a transparent double-sided mesh is drawn back faces first, then front (three.js's two passes)
    var side = this.drawSide != null ? this.drawSide : mu[16], passes = [side];
    if (this.shadowPass) {
      // shadow side: the material's shadowSide, else front <-> back (VSM keeps the side)
      side = mu[23] >= 0 ? mu[23] : (this.shadowPass === 3 ? mu[16] : (mu[16] === 0 ? 1 : mu[16] === 1 ? 0 : 2));
      passes = [side];
    } else if (mesh && side === 2 && material.transparent === true && material.forceSinglePass !== true) passes = [1, 0];
    for (var p = 0; p < passes.length; p++) {
      if (!mesh || passes[p] === 2) gl.disable(gl.CULL_FACE);
      else { gl.enable(gl.CULL_FACE); gl.cullFace(passes[p] === 1 ? gl.FRONT : gl.BACK); }
      this.issue(mesh && material.wireframe, mode, geometry, index, total, start, end, instances, info);
    }
  };

  B.issue = function (wire, mode, geometry, index, total, start, end, instances, info) {
    var gl = this.gl, each = Math.max(1, instances), drawn = end - start;
    if (wire) {
      // two line indices per triangle index
      var wi = this.wireIndex(geometry, index, total);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, wi.buf);
      var n = Math.min(drawn * 2, wi.count - start * 2);
      if (instances > 0) gl.drawElementsInstanced(gl.LINES, n, gl.UNSIGNED_INT, start * 8, instances);
      else gl.drawElements(gl.LINES, n, gl.UNSIGNED_INT, start * 8);
      info.lines += (n / 2) * each;
    } else {
      if (index) {
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.buffer(index, gl.ELEMENT_ARRAY_BUFFER));
        if (instances > 0) gl.drawElementsInstanced(mode, drawn, gl.UNSIGNED_INT, start * 4, instances);
        else gl.drawElements(mode, drawn, gl.UNSIGNED_INT, start * 4);
      } else {
        if (instances > 0) gl.drawArraysInstanced(mode, start, drawn, instances);
        else gl.drawArrays(mode, start, drawn);
      }
      if (mode === gl.TRIANGLES) info.triangles += Math.floor(drawn / 3) * each;
      else if (mode === gl.POINTS) info.points += drawn;
      else if (mode === gl.LINES) info.lines += Math.floor(drawn / 2);
      else info.lines += Math.max(0, drawn - (mode === gl.LINE_LOOP ? 0 : 1));
    }
    info.calls++;
  };

  B.readPixels = function () {
    var gl = this.gl, w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
    var raw = new Uint8Array(w * h * 4), out = new Uint8ClampedArray(w * h * 4);
    gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, raw);
    for (var y = 0; y < h; y++) out.set(raw.subarray((h - 1 - y) * w * 4, (h - y) * w * 4), y * w * 4);
    return out;
  };

  B.dispose = function () {
    var ext = this.gl.getExtension("WEBGL_lose_context");
    if (ext) ext.loseContext();
  };

  T.__WebGL2Backend = Backend;
})(THREE, RangerThreeCore);
