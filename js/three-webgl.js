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
    "uniform int u_hasEmap, u_emapSrgb;\n" +
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
    "uniform int u_hasMap, u_mapSrgb, u_points, u_hasNormal, u_opaque, u_tone;\n" +
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
    "void main() {\n" +
    "  vec4 c0 = u_mat[0], c1 = u_mat[1], c2 = u_mat[2], c3 = u_mat[3], c4 = u_mat[4], c5 = u_mat[5];\n" +
    "  int mode = int(c3.w + 0.5);\n" +
    "  vec3 albedo = c0.rgb * v_color;\n" +
    "  float alpha = c0.a;\n" +
    "  if (u_hasMap == 1) {\n" +
    "    vec2 uv = v_uv;\n" +
    "    if (u_points == 1) uv = (u_uvt * vec3(gl_PointCoord.x, 1.0 - gl_PointCoord.y, 1.0)).xy;\n" +
    "    vec4 t = texture(u_map, uv);\n" +
    "    if (u_mapSrgb == 1) t.rgb = toLinear(t.rgb);\n" +
    "    albedo *= t.rgb; alpha *= t.a;\n" +
    "  }\n" +
    "  if (alpha < c1.a || alpha <= 0.0) discard;\n" +
    "  if (u_opaque == 1) alpha = 1.0;\n" +
    "  vec3 n;\n" +
    "  if (c4.y > 0.5 || u_hasNormal == 0) n = normalize(cross(dFdx(v_world), dFdy(v_world)));\n" +
    "  else { n = normalize(v_normal); if (!gl_FrontFacing) n = -n; }\n" +
    "  vec3 color;\n" +
    "  vec3 emissive = c1.rgb;\n" +
    "  if (u_hasEmap == 1) { vec3 e = texture(u_emap, (u_euvt * vec3(v_uv0, 1.0)).xy).rgb; if (u_emapSrgb == 1) e = toLinear(e); emissive *= e; }\n" +
    "  if (mode == 3) {\n" +
    // MeshDepthMaterial, BasicDepthPacking: 1 - window depth, no colour space
    "    color = vec3(1.0 - gl_FragCoord.z);\n" +
    "  } else if (mode == 2) {\n" +
    "    vec3 vn = normalize((u_view * vec4(n, 0.0)).xyz);\n" +
    "    color = vn * 0.5 + 0.5;\n" +
    "  } else if (c3.y < 0.5) {\n" +
    "    color = albedo;\n" +
    "  } else {\n" +
    "    vec3 toEye = normalize(u_cam - v_world);\n" +
    "    vec3 dif = vec3(0.0), spec = vec3(0.0), pspec = vec3(0.0);\n" +
    // Standard/Physical (three.js lights_physical_fragment): roughness widened by the normal's screen-space change
    "    bool physical = c5.z > 0.5;\n" +
    "    float rough = c5.x;\n" +
    "    if (physical) { vec3 dxy = max(abs(dFdx(v_normal)), abs(dFdy(v_normal))); rough = min(rough + max(max(dxy.x, dxy.y), dxy.z), 1.0); }\n" +
    "    vec3 f0 = mix(vec3(0.04), albedo, c5.y);\n" +
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
    "      float diffuse = 1.0;\n" +
    "      if (directional) {\n" +
    "        float nl = dot(n, dir);\n" +
    "        diffuse = max(nl, 0.0);\n" +
    "        if (mode == 1) diffuse = nl > 0.5 ? 1.0 : (nl > 0.0 ? 0.5 : 0.0);\n" +
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
    "      dif += c * (l1.w * k * diffuse * 0.3183098862);\n" +
    "    }\n" +
    "    color = albedo * dif * c3.x + c2.rgb * spec + pspec + emissive;\n" +
    "  }\n" +
    "  if (c4.w > 0.5 && u_fog[0].x > 0.5) {\n" +
    "    float f = u_fog[0].x < 1.5 ? (u_fog[1].y == u_fog[1].x ? step(u_fog[1].y, v_depth) : clamp((v_depth - u_fog[1].x) / (u_fog[1].y - u_fog[1].x), 0.0, 1.0))\n" +
    "      : clamp(1.0 - exp(-(u_fog[1].z * v_depth) * (u_fog[1].z * v_depth)), 0.0, 1.0);\n" +
    "    color = mix(color, u_fog[0].yzw, f);\n" +
    "  }\n" +
    "  // MeshNormalMaterial writes its packed normal as is (no colour space)\n" +
    "  if (mode < 2 && u_tone > 0) color = toneMap(color);\n" +
    "  o_color = vec4(mode >= 2 ? clamp(color, 0.0, 1.0) : toSrgb(clamp(color, 0.0, 1.0)), alpha);\n" +
    "}\n";

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
      "u_nlights", "u_fog", "u_cam", "u_map", "u_hasMap", "u_mapSrgb", "u_points", "u_hasNormal", "u_opaque", "u_tone", "u_exposure", "u_sprite", "u_center", "u_rotation", "u_emap", "u_hasEmap", "u_emapSrgb", "u_euvt"];
    this.u = {};
    for (var i = 0; i < names.length; i++) this.u[names[i]] = gl.getUniformLocation(prog, names[i]);
    this.buffers = new WeakMap();   // attribute -> { buf, version }
    this.wire = new WeakMap();      // geometry -> { buf, count, key }
    this.textures = new WeakMap();  // texture -> { tex, version }
    this.vao = gl.createVertexArray();
    this.lightData = new Float32Array(MAX_LIGHTS * 20);
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
    if (!e || e.version !== tex.version) {
      var dom = !(img.data && img.width && img.height);
      if (dom && (!(img.naturalWidth || img.videoWidth || img.width) || img.complete === false)) return e ? e.tex : null;
      if (!e) { e = { tex: gl.createTexture(), version: -1 }; this.textures.set(tex, e); this.textureCount++; }
      gl.bindTexture(gl.TEXTURE_2D, e.tex);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, !!tex.flip_y);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, !!tex.premultiply_alpha);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      if (dom) {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
      } else {
        var px = T.WebGLRenderer.decodeImage(img, tex);
        var bytes = new Uint8Array(px.data.buffer, px.data.byteOffset, px.data.byteLength);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, px.width, px.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, bytes);
      }
      var mip = tex.min_filter !== 1003 && tex.min_filter !== 1006;
      if (mip && tex.generate_mipmaps) gl.generateMipmap(gl.TEXTURE_2D);
      e.mip = mip && tex.generate_mipmaps;
      e.version = tex.version;
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
    var cl = plan.clear, s = R.MathsColor.linear_to_srgb;
    gl.clearColor(s(Math.min(1, Math.max(0, cl.r))), s(Math.min(1, Math.max(0, cl.g))), s(Math.min(1, Math.max(0, cl.b))), plan.clear_alpha);
    gl.depthMask(true);
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
    gl.activeTexture(gl.TEXTURE0);
    var info = { triangles: 0, lines: 0, points: 0, calls: 0 };
    for (var k = 0; k < plan.items.length; k++) this.draw(plan.items[k], info);
    gl.bindVertexArray(null);
    r.info.render.calls = info.calls;
    r.info.render.triangles = info.triangles;
    r.info.render.lines = info.lines;
    r.info.render.points = info.points;
    r.info.memory.textures = this.textureCount;
  };

  B.draw = function (item, info) {
    var gl = this.gl, u = this.u, o = item.object, kind = item.kind;
    var geometry = o.geometry, material = Array.isArray(o.material) ? o.material[0] : o.material;
    var sprite = kind === 7;
    if (!geometry || !material) return;
    var position = geometry.getAttribute("position");
    if (!position) return;
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
    gl.uniform1i(u.u_mapSrgb, map && map.colorSpace === "srgb" ? 1 : 0);
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
      gl.activeTexture(gl.TEXTURE0);
      if (glE) {
        if (emap.matrixAutoUpdate !== false && emap.updateMatrix) emap.updateMatrix();
        gl.uniformMatrix3fv(u.u_euvt, false, emap.matrix.elements);
      }
    }
    gl.uniform1i(u.u_hasEmap, glE ? 1 : 0);
    gl.uniform1i(u.u_emapSrgb, emap && emap.colorSpace === "srgb" ? 1 : 0);
    gl.uniform1i(u.u_opaque, item.transparent ? 0 : 1);
    gl.uniform1i(u.u_tone, material.toneMapped === false ? 0 : this.tone);
    gl.uniform1f(u.u_pointSize, (material.size || 1) * this.renderer._pixelRatio);
    gl.uniform1i(u.u_sizeAtt, material.sizeAttenuation === false ? 0 : 1);
    // side and winding (a mirrored transform turns the winding round)
    var side = mu[16];
    if (!mesh || side === 2) gl.disable(gl.CULL_FACE);
    else { gl.enable(gl.CULL_FACE); gl.cullFace(side === 1 ? gl.FRONT : gl.BACK); }
    gl.frontFace(o.matrixWorld.determinant() < 0 ? gl.CW : gl.CCW);
    if (item.transparent) {
      gl.enable(gl.BLEND);
      gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.depthMask(false);
    } else {
      gl.disable(gl.BLEND);
      gl.depthMask(material.depthWrite !== false);
    }
    if (material.depthTest === false) gl.disable(gl.DEPTH_TEST); else gl.enable(gl.DEPTH_TEST);
    var index = geometry.index, count = index ? index.count : position.count;
    var range = geometry.drawRange;
    if (range && isFinite(range.count)) count = Math.min(count, range.start + range.count);
    var start = range ? range.start : 0;
    var mode = gl.TRIANGLES;
    if (kind === 3) mode = gl.LINE_STRIP;
    else if (kind === 4) mode = gl.LINES;
    else if (kind === 5) mode = gl.LINE_LOOP;
    else if (kind === 6) mode = gl.POINTS;
    var each = Math.max(1, instances);
    if (mesh && material.wireframe) {
      var wi = this.wireIndex(geometry, index, count);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, wi.buf);
      if (instances > 0) gl.drawElementsInstanced(gl.LINES, wi.count, gl.UNSIGNED_INT, 0, instances);
      else gl.drawElements(gl.LINES, wi.count, gl.UNSIGNED_INT, 0);
      info.lines += (wi.count / 2) * each;
    } else if (index) {
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.buffer(index, gl.ELEMENT_ARRAY_BUFFER));
      if (instances > 0) gl.drawElementsInstanced(mode, count - start, gl.UNSIGNED_INT, start * 4, instances);
      else gl.drawElements(mode, count - start, gl.UNSIGNED_INT, start * 4);
    } else {
      if (instances > 0) gl.drawArraysInstanced(mode, start, count - start, instances);
      else gl.drawArrays(mode, start, count - start);
    }
    var drawn = count - start;
    if (!(mesh && material.wireframe)) {
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
