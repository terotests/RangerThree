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
  // the shaders and their variants come from the crate (renderers/shaders.rs), the one source
  // the other GPU backends use too
  var S = R.RenderersShaders;

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

  var NAMES = S.uniform_names();

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
  // become constants (renderers/shaders.rs: key_names lists the values of a
  // key in order after the shadow type: 1 PCF, also without shadows; 2 basic; 3 VSM).
  // (SPEC uniforms and the K_ flags), so the shader holds only the branches that draw takes.
  var SPEC_KEYS = S.key_names();
  B.useMain = function (type, vals) {
    if (!vals) { vals = []; for (var i = 0; i < SPEC_KEYS.length; i++) vals.push(0); }
    var key = type + ":" + vals.join(","), m = this.mains[key], gl = this.gl;
    if (!m) {
      m = this.mains[key] = program(gl, S.main_vertex(vals), S.main_fragment(type, vals), NAMES);
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
      this.pm = program(gl, S.full_vs(), S.pmrem_fs(), ["u_src", "u_pass", "u_size", "u_sizeIn", "u_rough"]);
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
    if (!this.bg) this.bg = program(gl, S.full_vs(), S.background_fs(), ["u_mode", "u_tone", "u_linearOut", "u_exposure", "u_intensity", "u_lod", "u_mipF", "u_sizeA", "u_sizeB",
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
    if (!this.vsm) this.vsm = program(gl, S.full_vs(), S.vsm_blur_fs(), ["u_src", "u_res", "u_radius", "u_samples", "u_horizontal"]);
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
