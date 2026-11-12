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
    "uniform mat4 u_model, u_view, u_proj;\n" +
    "uniform mat3 u_uvt;\n" +
    "uniform int u_instanced;\n" +
    "uniform float u_pointSize, u_pointScale;\n" +
    "uniform int u_sizeAtt, u_sprite;\n" +
    "uniform vec2 u_center;\n" +
    "uniform float u_rotation;\n" +
    "out vec3 v_world; out vec3 v_normal; out vec2 v_uv; out vec2 v_uv0; out vec3 v_color; out float v_depth;\n" +
    "void main() {\n" +
    "  mat4 m = u_model;\n" +
    "  if (u_instanced == 1) m = u_model * mat4(a_im0, a_im1, a_im2, a_im3);\n" +
    "  vec4 w = m * vec4(a_position, 1.0);\n" +
    "  v_world = w.xyz;\n" +
    "  v_normal = mat3(transpose(inverse(m))) * a_normal;\n" +
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
    "    v_world = (inverse(u_view) * mv).xyz; v_normal = vec3(0.0, 0.0, 1.0); v_depth = -mv.z;\n" +
    "    gl_Position = u_proj * mv;\n" +
    "  }\n" +
    "  gl_PointSize = max(1.0, u_sizeAtt == 1 ? u_pointSize * u_pointScale / max(-vp.z, 1e-4) : u_pointSize);\n" +
    "}\n";
  var FS = "#version 300 es\n" +
    "precision highp float;\n" +
    "in vec3 v_world; in vec3 v_normal; in vec2 v_uv; in vec2 v_uv0; in vec3 v_color; in float v_depth;\n" +
    "uniform sampler2D u_emap;\n" +
    "uniform int u_hasEmap;\n" +
    // extra maps: bit 1 alpha, 2 roughness, 4 metalness, 8 normal, 16 ao, 32 matcap, 64 gradient; u_t: their uv transforms
    "uniform sampler2D u_alphaMap, u_roughMap, u_metalMap, u_normalMap, u_aoMap, u_matcap, u_gradMap;\n" +
    "uniform int u_maps, u_linearOut, u_premult;\n" +
    "uniform mat3 u_t[5];\n" +
    "uniform vec2 u_normalScale;\n" +
    "uniform float u_aoIntensity;\n" +
    "uniform mat3 u_euvt;\n" +
    "uniform vec4 u_mat[6];\n" +
    "uniform sampler2D u_dfg;\n" +
    "uniform vec4 u_lights[" + (MAX_LIGHTS * 5) + "];\n" +
    "uniform int u_nlights;\n" +
    "uniform vec4 u_fog[2];\n" +
    "uniform vec3 u_cam;\n" +
    "uniform mat4 u_view;\n" +
    "uniform mat3 u_uvt;\n" +
    "uniform sampler2D u_map;\n" +
    "uniform int u_hasMap, u_points, u_hasNormal, u_opaque, u_tone;\n" +
    "uniform float u_exposure;\n" +
    "out vec4 o_color;\n" +
    "vec3 toLinear(vec3 c) { return mix(c * 0.0773993808, pow(c * 0.9478672986 + 0.0521327014, vec3(2.4)), step(0.04045, c)); }\n" +
    "vec3 toSrgb(vec3 c) { return mix(c * 12.92, 1.055 * pow(c, vec3(0.41666)) - 0.055, step(0.0031308, c)); }\n" +
    // three.js's tone mapping operators (tonemapping_pars_fragment)
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
    "}\n" +
    "bool has(int b) { return (u_maps & b) != 0; }\n" +
    "vec2 tuv(int i) { return (u_t[i] * vec3(v_uv0, 1.0)).xy; }\n" +
    "void main() {\n" +
    "  vec4 c0 = u_mat[0], c1 = u_mat[1], c2 = u_mat[2], c3 = u_mat[3], c4 = u_mat[4], c5 = u_mat[5];\n" +
    "  int mode = int(c3.w + 0.5);\n" +
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
    "  vec3 n;\n" +
    "  if (c4.y > 0.5 || u_hasNormal == 0) n = normalize(cross(dFdx(v_world), dFdy(v_world)));\n" +
    "  else { n = normalize(v_normal); if (!gl_FrontFacing) n = -n; }\n" +
    "  vec3 n0 = n;\n" +
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
    "  }\n" +
    "  float ao = has(16) ? (texture(u_aoMap, tuv(4)).r - 1.0) * u_aoIntensity + 1.0 : 1.0;\n" +
    "  vec3 color;\n" +
    "  vec3 emissive = c1.rgb;\n" +
    "  if (u_hasEmap == 1) { vec3 e = texture(u_emap, (u_euvt * vec3(v_uv0, 1.0)).xy).rgb; emissive *= e; }\n" +
    "  if (mode == 3) {\n" +
    // MeshDepthMaterial, BasicDepthPacking: 1 - window depth, no colour space
    "    color = vec3(1.0 - gl_FragCoord.z);\n" +
    "  } else if (mode == 2) {\n" +
    "    vec3 vn = normalize((u_view * vec4(n, 0.0)).xyz);\n" +
    "    color = vn * 0.5 + 0.5;\n" +
    "  } else if (mode == 4) {\n" +
    // MeshMatcapMaterial: the view-space normal picks the matcap texel
    "    vec3 vn = normalize((u_view * vec4(n, 0.0)).xyz);\n" +
    "    vec3 vd = normalize(-(u_view * vec4(v_world, 1.0)).xyz);\n" +
    "    vec3 mx = normalize(vec3(vd.z, 0.0, -vd.x)); vec3 my = cross(vd, mx);\n" +
    "    vec2 muv = vec2(dot(mx, vn), dot(my, vn)) * 0.495 + 0.5;\n" +
    "    color = albedo * (has(32) ? texture(u_matcap, muv).rgb : vec3(mix(0.2, 0.8, muv.y)));\n" +
    "  } else if (c3.y < 0.5) {\n" +
    "    color = albedo * ao;\n" +
    "  } else {\n" +
    "    vec3 toEye = normalize(u_cam - v_world);\n" +
    "    vec3 dif = vec3(0.0), ind = vec3(0.0), spec = vec3(0.0), pspec = vec3(0.0);\n" +
    // Standard/Physical (three.js lights_physical_fragment): roughness widened by the normal's screen-space change
    "    bool physical = c5.z > 0.5;\n" +
    "    float rough = c5.x, metal = c5.y, dscale = c3.x;\n" +
    "    if (has(2)) rough *= texture(u_roughMap, tuv(1)).g;\n" +
    "    if (has(4)) metal *= texture(u_metalMap, tuv(2)).b;\n" +
    "    vec3 vn0 = (u_view * vec4(n0, 0.0)).xyz;\n" +
    "    vec3 dxy = max(abs(dFdx(vn0)), abs(dFdy(vn0)));\n" +
    "    if (physical) { rough = min(max(rough, 0.0525) + max(max(dxy.x, dxy.y), dxy.z), 1.0); dscale = 1.0 - metal; }\n" +
    "    vec3 f0 = mix(vec3(0.04), albedo, metal);\n" +
    "    float nv = clamp(dot(n, toEye), 0.0, 1.0);\n" +
    "    vec2 dfgV = texture(u_dfg, vec2(rough, nv)).rg;\n" +
    "    for (int i = 0; i < " + MAX_LIGHTS + "; i++) {\n" +
    "      if (i >= u_nlights) break;\n" +
    "      vec4 l0 = u_lights[i * 5], l1 = u_lights[i * 5 + 1], l2 = u_lights[i * 5 + 2], l3 = u_lights[i * 5 + 3], l4 = u_lights[i * 5 + 4];\n" +
    "      float kind = l0.w;\n" +
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
    "      vec3 diffuse = vec3(1.0);\n" +
    "      if (directional) {\n" +
    "        float nl = dot(n, dir);\n" +
    "        diffuse = vec3(max(nl, 0.0));\n" +
    // toon: the gradient map at n.l, or three.js's default two-tone ramp
    "        if (mode == 1) {\n" +
    "          vec2 coord = vec2(nl * 0.5 + 0.5, 0.0);\n" +
    "          if (has(64)) diffuse = vec3(texture(u_gradMap, coord).r);\n" +
    "          else { float fw = fwidth(coord.x) * 0.5; diffuse = vec3(mix(0.7, 1.0, smoothstep(0.7 - fw, 0.7 + fw, coord.x))); }\n" +
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
    "          pspec += c * (l1.w * k * nlc) * (F * (V * D) + fms * emsV * emsL);\n" +
    "        } else if (c3.z > 0.5 && nl > 0.0) {\n" +
    "          vec3 h = normalize(dir + toEye);\n" +
    "          float s = pow(max(dot(n, h), 0.0), c2.w) * (c2.w + 2.0) / 8.0 * 0.3183098862;\n" +
    "          spec += c * (l1.w * k * s * nl);\n" +
    "        }\n" +
    "      }\n" +
    "      if (kind < 1.5) ind += c * (l1.w * 0.3183098862);\n" +
    "      else dif += c * diffuse * (l1.w * k * 0.3183098862);\n" +
    "    }\n" +
    "    color = albedo * (dif + ind * ao) * dscale + c2.rgb * spec + pspec + emissive;\n" +
    "  }\n" +
    // MeshNormalMaterial and MeshDepthMaterial write their values as is (no colour space)
    "  bool raw = mode == 2 || mode == 3;\n" +
    "  if (!raw && u_tone > 0) color = toneMap(color);\n" +
    "  vec3 outc = max(color, 0.0);\n" +
    "  if (!raw && u_linearOut == 0) outc = toSrgb(outc);\n" +
    // three.js fogs after the colour-space conversion, with the fog colour in sRGB; linear fog is a smoothstep
    "  if (c4.w > 0.5 && u_fog[0].x > 0.5) {\n" +
    "    float f = u_fog[0].x < 1.5 ? smoothstep(u_fog[1].x, u_fog[1].y, v_depth)\n" +
    "      : 1.0 - exp(-u_fog[1].z * u_fog[1].z * v_depth * v_depth);\n" +
    "    vec3 fc = clamp(u_fog[0].yzw, 0.0, 1.0);\n" +
    "    outc = mix(outc, u_linearOut == 1 ? fc : toSrgb(fc), f);\n" +
    "  }\n" +
    "  if (u_premult == 1) outc *= alpha;\n" +
    "  o_color = vec4(outc, alpha);\n" +
    "}\n";

  // the extra material maps: property, sampler, texture unit, bit in u_maps, slot in u_t (-1 none)
  var MAPS = [
    { prop: "alphaMap", uniform: "u_alphaMap", unit: 3, bit: 1, t: 0 },
    { prop: "roughnessMap", uniform: "u_roughMap", unit: 4, bit: 2, t: 1 },
    { prop: "metalnessMap", uniform: "u_metalMap", unit: 5, bit: 4, t: 2 },
    { prop: "normalMap", uniform: "u_normalMap", unit: 6, bit: 8, t: 3 },
    { prop: "aoMap", uniform: "u_aoMap", unit: 7, bit: 16, t: 4 },
    { prop: "matcap", uniform: "u_matcap", unit: 8, bit: 32, t: -1 },
    { prop: "gradientMap", uniform: "u_gradMap", unit: 9, bit: 64, t: -1 }
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

  function Backend(gl, renderer) {
    this.gl = gl;
    this.renderer = renderer;
    var prog = gl.createProgram();
    gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, VS));
    gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error("WebGL2 program: " + gl.getProgramInfoLog(prog));
    this.prog = prog;
    var names = ["u_model", "u_view", "u_proj", "u_uvt", "u_instanced", "u_pointSize", "u_pointScale", "u_sizeAtt", "u_mat", "u_lights",
      "u_nlights", "u_fog", "u_cam", "u_map", "u_hasMap", "u_points", "u_hasNormal", "u_opaque", "u_tone", "u_exposure", "u_sprite", "u_center", "u_rotation", "u_emap", "u_hasEmap", "u_euvt", "u_dfg",
      "u_alphaMap", "u_roughMap", "u_metalMap", "u_normalMap", "u_aoMap", "u_matcap", "u_gradMap", "u_maps", "u_linearOut", "u_premult", "u_t", "u_normalScale", "u_aoIntensity"];
    this.u = {};
    for (var i = 0; i < names.length; i++) this.u[names[i]] = gl.getUniformLocation(prog, names[i]);
    this.buffers = new WeakMap();   // attribute -> { buf, version }
    this.wire = new WeakMap();      // geometry -> { buf, count, key }
    this.textures = new WeakMap();  // texture -> { tex, version }
    this.vao = gl.createVertexArray();
    this.lightData = new Float32Array(MAX_LIGHTS * 20);
    this.mapTransforms = new Float32Array(45);
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
  }
  var B = Backend.prototype;

  // a GPU buffer holding `attr`, re-uploaded when its version changes
  B.buffer = function (attr, target) {
    var gl = this.gl, e = this.buffers.get(attr);
    if (e && e.version === attr.version && e.length === attr.array.length) return e.buf;
    if (!e) { e = { buf: gl.createBuffer(), version: -1, length: 0 }; this.buffers.set(attr, e); }
    var data = target === gl.ELEMENT_ARRAY_BUFFER ? Uint32Array.from(attr.array) : Float32Array.from(attr.array);
    gl.bindBuffer(target, e.buf);
    gl.bufferData(target, data, attr.usage === 35048 ? gl.DYNAMIC_DRAW : gl.STATIC_DRAW);
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

  B.render = function (scene, camera) {
    var gl = this.gl, u = this.u, r = this.renderer, soft = r._soft;
    var plan = R.RenderersPlan.plan_frame(scene, camera, soft.clear_color, soft.clear_alpha);
    var w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
    gl.viewport(0, 0, w, h);
    var linearOut = r.outputColorSpace === "srgb-linear";
    var cl = plan.clear, s = linearOut ? function (x) { return x; } : R.MathsColor.linear_to_srgb;
    gl.clearColor(s(Math.min(1, Math.max(0, cl.r))), s(Math.min(1, Math.max(0, cl.g))), s(Math.min(1, Math.max(0, cl.b))), plan.clear_alpha);
    gl.depthMask(true);
    gl.colorMask(true, true, true, true);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.useProgram(this.prog);
    gl.bindVertexArray(this.vao);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.uniformMatrix4fv(u.u_view, false, plan.view.elements);
    gl.uniformMatrix4fv(u.u_proj, false, plan.projection.elements);
    gl.uniform3f(u.u_cam, plan.camera_position.x, plan.camera_position.y, plan.camera_position.z);
    var n = Math.min(MAX_LIGHTS, plan.light_count);
    this.lightData.fill(0);
    for (var i = 0; i < n * 20; i++) this.lightData[i] = plan.lights[i];
    gl.uniform4fv(u.u_lights, this.lightData);
    gl.uniform1i(u.u_nlights, n);
    gl.uniform4fv(u.u_fog, plan.fog);
    gl.uniform1f(u.u_pointScale, h * 0.5);
    gl.uniform1f(u.u_exposure, r.toneMappingExposure === undefined ? 1 : r.toneMappingExposure);
    // tone mapping (three.js constants: 0 none, 1 linear, 2 Reinhard, 3 Cineon, 4 ACES filmic, 6 AgX, 7 neutral)
    this.tone = r.toneMapping | 0;
    gl.uniform1i(u.u_map, 0);
    gl.uniform1i(u.u_emap, 1);
    gl.uniform1i(u.u_dfg, 2);
    gl.activeTexture(gl.TEXTURE2);
    gl.bindTexture(gl.TEXTURE_2D, this.dfg);
    gl.activeTexture(gl.TEXTURE0);
    for (var mi = 0; mi < MAPS.length; mi++) gl.uniform1i(u[MAPS[mi].uniform], MAPS[mi].unit);
    gl.uniform1i(u.u_linearOut, linearOut ? 1 : 0);
    var info = { triangles: 0, lines: 0, points: 0, calls: 0 };
    var override = scene.overrideMaterial || null;
    for (var k = 0; k < plan.items.length; k++) this.draw(plan.items[k], info, override);
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

  B.drawWith = function (item, material, info, group) {
    var gl = this.gl, u = this.u, o = item.object, kind = item.kind;
    var geometry = o.geometry, position = geometry.getAttribute("position");
    var sprite = kind === 7;
    var mu = R.RenderersPlan.material_uniforms(material);
    gl.uniform4fv(u.u_mat, mu);
    gl.uniformMatrix4fv(u.u_model, false, o.matrixWorld.elements);
    gl.uniformMatrix3fv(u.u_uvt, false, R.RenderersPlan.map_transform(material));
    var mesh = kind === 1 || kind === 2;
    this.attrib(0, position, 3);
    var hasNormal = mesh && !mu[17] && this.attrib(1, geometry.getAttribute("normal"), 3, [0, 0, 1, 0]);
    if (!hasNormal) this.attrib(1, null, 3, [0, 0, 1, 0]);
    this.attrib(2, geometry.getAttribute("uv"), 2, [0, 0, 0, 0]);
    this.attrib(3, mu[18] ? geometry.getAttribute("color") : null, 3, [1, 1, 1, 1]);
    gl.uniform1i(u.u_hasNormal, hasNormal ? 1 : 0);
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
    gl.uniform1i(u.u_instanced, instances > 0 ? 1 : 0);
    var map = material.map, glTex = map ? this.texture(map) : null;
    gl.uniform1i(u.u_hasMap, glTex ? 1 : 0);
    gl.uniform1i(u.u_points, kind === 6 ? 1 : 0);
    gl.uniform1i(u.u_sprite, sprite ? 1 : 0);
    if (sprite) {
      var ce = o.center || { x: 0.5, y: 0.5 };
      gl.uniform2f(u.u_center, ce.x, ce.y);
      gl.uniform1f(u.u_rotation, material.rotation || 0);
    }
    var emap = material.emissiveMap, glE = null;
    if (emap) {
      gl.activeTexture(gl.TEXTURE1);
      glE = this.texture(emap);
      if (glE) {
        if (emap.matrixAutoUpdate !== false && emap.updateMatrix) emap.updateMatrix();
        gl.uniformMatrix3fv(u.u_euvt, false, emap.matrix.elements);
      }
    }
    gl.uniform1i(u.u_hasEmap, glE ? 1 : 0);
    var bits = 0;
    for (var mi = 0; mi < MAPS.length; mi++) {
      var spec = MAPS[mi], t = material[spec.prop];
      if (!t) continue;
      gl.activeTexture(gl.TEXTURE0 + spec.unit);
      if (!this.texture(t)) continue;
      bits |= spec.bit;
      if (spec.t >= 0) {
        if (t.matrixAutoUpdate !== false && t.updateMatrix) t.updateMatrix();
        this.mapTransforms.set(t.matrix.elements, spec.t * 9);
      }
    }
    gl.activeTexture(gl.TEXTURE0);
    gl.uniform1i(u.u_maps, bits);
    if (bits) gl.uniformMatrix3fv(u.u_t, false, this.mapTransforms);
    var ns = material.normalScale, flip = mu[16] === 1 ? -1 : 1;
    gl.uniform2f(u.u_normalScale, (ns ? ns.x : 1) * flip, (ns ? ns.y : 1) * flip);
    gl.uniform1f(u.u_aoIntensity, material.aoMapIntensity == null ? 1 : material.aoMapIntensity);
    var blending = material.blending === undefined ? 1 : material.blending;
    var opaque = material.transparent !== true && blending === 1 && !material.alphaToCoverage;
    gl.uniform1i(u.u_opaque, opaque ? 1 : 0);
    gl.uniform1i(u.u_premult, material.premultipliedAlpha ? 1 : 0);
    gl.uniform1i(u.u_tone, material.toneMapped === false ? 0 : this.tone);
    gl.uniform1f(u.u_pointSize, (material.size || 1) * this.renderer._pixelRatio);
    gl.uniform1i(u.u_sizeAtt, material.sizeAttenuation === false ? 0 : 1);
    gl.frontFace(o.matrixWorld.determinant() < 0 ? gl.CW : gl.CCW);
    this.blend(material, opaque);
    gl.depthMask(material.depthWrite !== false);
    if (material.depthTest === false) gl.disable(gl.DEPTH_TEST);
    else { gl.enable(gl.DEPTH_TEST); gl.depthFunc(DEPTH_FUNCS[material.depthFunc == null ? 3 : material.depthFunc] || gl.LEQUAL); }
    var cw = material.colorWrite !== false;
    gl.colorMask(cw, cw, cw, cw);
    if (material.polygonOffset) { gl.enable(gl.POLYGON_OFFSET_FILL); gl.polygonOffset(material.polygonOffsetFactor || 0, material.polygonOffsetUnits || 0); }
    else gl.disable(gl.POLYGON_OFFSET_FILL);
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
    var side = mu[16], passes = [side];
    if (mesh && side === 2 && material.transparent === true && material.forceSinglePass !== true) passes = [1, 0];
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
