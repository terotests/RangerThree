// The main program drawn twice in headless Chromium (SwiftShader): its GLSL
// with WebGL2 and its WGSL (three/src/renderers/wgsl.rs) with WebGPU, with
// the same uniforms, textures and geometry, and the pixels compared. Each
// variant is drawn as a WebGPU canvas pass (y down) and as a texture pass
// (u_target.z -1, rows in GL's order), so the y conventions (clip y,
// gl_FragCoord, dFdy, front faces) are checked both ways. The texture pass
// rasterizes as GL does and matches exactly; a canvas pass is y-mirrored, so
// a few pixels whose coordinates round across a texel edge (the 4x4 test
// textures are sampled nearest) may differ.
//
//   bash scripts/build-js.sh && node tests/wgsl-draw.mjs
//   PNG=dir (writes gl / wgpu images of each case)
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import zlib from "node:zlib";
import { createRequire } from "node:module";

const { chromium } = createRequire(import.meta.url)("playwright");
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const ctx = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, "build/rangerthree-core.js"), "utf8") + ";this.R = RangerThreeCore;", ctx);
const S = ctx.R.RenderersShaders;
const KEYS = S.key_names();
const FRAME_NAMES = Array.from(S.frame_block_names());
const FRAME_AT = Array.from(S.frame_block_offsets()).map(Number);
const SIZE = 128;

const lights = { K_NAMB: 1, K_NHEMI: 1, K_NDIR: 2, K_NPOINT: 1, K_NSPOT: 1, K_NRECT: 0 };
const base = { u_hasMap: 1, u_hasNormal: 1, u_opaque: 1, u_tone: 4, u_receive: 1, K_SHSLOTS: 1, K_LIT: 1, ...lights };
const CASES = [
  ["Basic, map", { u_hasMap: 1, u_hasNormal: 1, u_opaque: 1 }],
  ["Basic, map, alpha map, equirect env, fog, dashed", { u_hasMap: 1, u_maps: 1 | 16, K_ENV: 2, K_FOG: 1, u_dashed: 1 }],
  ["Lambert, lights, PCF shadow slot", { ...base }],
  ["Lambert, flat shading (dFdx/dFdy)", { ...base, K_FLAT: 1, u_tone: 2 }],
  ["Phong, normal + ao maps, exp2 fog", { ...base, u_maps: 8 | 16, K_PHONG: 1, K_FOG: 2, u_tone: 6 }],
  ["Standard, all maps, CubeUV env", { ...base, u_maps: 1 | 2 | 4 | 8 | 16, K_PHYS: 1, K_ENV: 1, u_hasEmap: 1 }],
  ["Physical, sheen, transmission, rect light", { ...base, u_maps: 2 | 4, K_PHYS: 1, K_SHEEN: 1, K_TRANS: 1, K_RECT: 1, K_NRECT: 1, u_tone: 7 }],
  ["Physical, transmission alone", { ...base, K_PHYS: 1, K_TRANS: 1 }],
  ["Physical, rect light alone, AgX tone", { ...base, K_PHYS: 1, K_RECT: 1, K_NRECT: 1, u_tone: 6 }],
  ["Standard, bump map, many lights (loop)", { ...base, u_maps: 128, K_PHYS: 1, K_NDIR: 5, K_NPOINT: 5, u_tone: 1 }],
  ["Toon, gradient map", { ...base, u_maps: 64, K_MODE: 1 }],
  ["Matcap", { u_hasMap: 1, u_hasNormal: 1, u_maps: 32, K_MODE: 4 }],
  ["Normal material", { u_hasNormal: 1, K_MODE: 2 }],
  ["Depth material", { K_MODE: 3 }],
  ["Shadow material", { ...base, u_hasMap: 0, K_MODE: 5 }],
  ["Instanced, vertex colors, displacement", { ...base, u_instanced: 1, u_hasDisp: 1, u_linearOut: 1 }],
];
const programs = [];
for (const [name, v] of CASES) {
  const vals = KEYS.map((k) => v[k] | 0);
  for (const st of name.includes("shadow") ? [1, 2, 3] : [1]) {
    const vs = S.main_vertex(vals), fs = S.main_fragment(st, vals);
    const w = ctx.R.RenderersWgsl.translate(vs, fs);
    programs.push({ name: name + (st > 1 ? " / shadow type " + st : ""), vals, vs, fs, wvs: w[0], wfs: w[1], layout: w[2] });
  }
}

// ---- the scene both APIs draw ----
const sub = (a, b) => a.map((x, i) => x - b[i]);
const norm = (a) => { const l = Math.hypot(...a); return a.map((x) => x / l); };
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
function lookAt(eye, at) {
  const z = norm(sub(eye, at)), x = norm(cross([0, 1, 0], z)), y = cross(z, x);
  return [x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot(x, eye), -dot(y, eye), -dot(z, eye), 1];
}
function perspective(fov, aspect, n, f) {
  const t = 1 / Math.tan(fov / 2);
  return [t / aspect, 0, 0, 0, 0, t, 0, 0, 0, 0, (f + n) / (n - f), -1, 0, 0, (2 * f * n) / (n - f), 0];
}
const mul = (a, b) => { const o = new Array(16).fill(0); for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) o[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k]; return o; };
const eye = [0.4, 0.9, 2.6];
const view = lookAt(eye, [0, 0, 0]);
const viewInv = (() => { const m = view; const r = [m[0], m[4], m[8], 0, m[1], m[5], m[9], 0, m[2], m[6], m[10], 0, eye[0], eye[1], eye[2], 1]; return r; })();
const proj = perspective(0.9, 1, 0.5, 8);
const bias = [0.5, 0, 0, 0, 0, 0.5, 0, 0, 0, 0, 0.5, 0, 0.5, 0.5, 0.5, 1];
const shMat = mul(bias, mul(proj, view));

function frameBlock(vals) {
  const f = new Float32Array(FRAME_AT[FRAME_AT.length - 1] + 4);
  const put = (name, arr) => f.set(arr, FRAME_AT[FRAME_NAMES.indexOf(name)]);
  put("u_view", view); put("u_viewInv", viewInv); put("u_proj", proj); put("u_cam", eye);
  put("u_exposure", [1.2]); put("u_fog", [0, 0.3, 0.4, 0.5, 2.0, 4.0, 0.25, 0]);
  put("u_pointScale", [48]); put("u_transSize", [4, 4]);
  for (let k = 0; k < 4; k++) {
    f.set(shMat, FRAME_AT[FRAME_NAMES.indexOf("u_shMat")] + 16 * k);
    f.set([1, 0.002, 0.01, 1], FRAME_AT[FRAME_NAMES.indexOf("u_shParam")] + 4 * k);
    f.set([4, 4, 0.5, 10], FRAME_AT[FRAME_NAMES.indexOf("u_shSize")] + 4 * k);
  }
  // lights packed by kind: ambient, hemisphere, directional, point, spot, rect
  const at = FRAME_AT[FRAME_NAMES.indexOf("u_lights")];
  let n = 0;
  for (let kind = 0; kind < 6; kind++) {
    for (let j = 0; j < vals[KEYS.length - 6 + kind]; j++, n++) {
      const a = at + n * 28, s = 0.3 + 0.17 * n;
      const dir = norm([Math.cos(s * 5), 0.8, Math.sin(s * 5)]);
      f.set([1, 0.9 - 0.05 * j, 0.7 + 0.04 * n, 0], a);
      f.set([0.85, 0.95, 0.2, 1.4], a + 4);
      f.set([dir[0] * 2.5, 2, dir[2] * 2.5, 8], a + 8);
      f.set([...dir, 2], a + 12);
      f.set([0.6, 0.4, kind === 2 && j === 0 ? 1 : 0, 0], a + 16);
      f.set([0.5, 0, 0.1, 0], a + 20);
      f.set([0, 0.4, 0.2, 0], a + 24);
    }
  }
  put("u_nlights", [n]);
  return f;
}

// a value for each draw uniform (by name; mat3s identity)
function drawValue(name, type) {
  const h = [...name].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
  const r = (k) => 0.2 + (((h >>> (k * 3)) & 63) / 63) * 0.6;
  if (name === "u_model") { const c = Math.cos(0.5), s = Math.sin(0.5); return [c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]; }
  if (name === "u_normalMat") { const c = Math.cos(0.5), s = Math.sin(0.5); return [c, 0, -s, 0, 1, 0, s, 0, c]; }
  if (name === "u_mat") return [0.8, 0.6, 0.45, 1, 0.05, 0.02, 0.0, 0.1, 0.9, 0.9, 0.9, 30, 1, 0.5, 0.5, 0.5, 0.7, 0.4, 1.5, 0.3, 0.45, 0.35, 0.5, 0.5];
  if (name === "u_env") return [1, 0.9, 0.98, 1];
  if (name === "u_envUV") return [4, 1 / (3 * 256), 1 / (4 * 256), 0];
  if (name === "u_trans") return [0.6, 0.5, 1.5, 0.4];
  if (name === "u_sheen") return [0.6, 0.5, 0.4, 0.5];
  if (name === "u_sizeAtt") return [0];
  if (name === "u_dashSize") return [0.05];
  if (name === "u_dashTotal") return [0.08];
  const el = type.replace(/\[.*/, ""), n = +(type.match(/\[(\d+)\]/) || [0, 1])[1];
  const one = el === "mat3" ? [1, 0, 0, 0, 1, 0, 0, 0, 1] : el === "mat4" ? [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    : Array.from({ length: el === "float" || el === "int" ? 1 : +el.slice(-1) }, (_, k) => r(k));
  return Array.from({ length: n }, () => one).flat();
}

// a UV sphere, non-indexed
function sphere() {
  const P = [], N = [], U = [];
  const pt = (i, j) => { const th = (j / 12) * Math.PI, ph = (i / 20) * 2 * Math.PI; const p = [Math.sin(th) * Math.cos(ph), Math.cos(th), Math.sin(th) * Math.sin(ph)]; return { p, u: [i / 20, 1 - j / 12] }; };
  for (let j = 0; j < 12; j++) for (let i = 0; i < 20; i++) {
    const a = pt(i, j), b = pt(i + 1, j), c = pt(i + 1, j + 1), d = pt(i, j + 1);
    for (const v of [a, c, b, a, d, c]) { P.push(...v.p); N.push(...v.p); U.push(...v.u); }
  }
  const n = P.length / 3;
  const C = Array.from({ length: n }, (_, k) => [1, 0.8 + 0.2 * Math.sin(k), 0.9]).flat();
  return { n, attrs: { a_position: P, a_normal: N, a_uv: U, a_color: C, a_icolor: Array(n * 3).fill(1).map((x, k) => (k % 3 === 1 ? 0.9 : 1)),
    a_im0: Array(n).fill([1, 0, 0, 0]).flat(), a_im1: Array(n).fill([0, 1, 0, 0]).flat(), a_im2: Array(n).fill([0, 0, 1, 0]).flat(), a_im3: Array(n).fill([0, 0, 0, 1]).flat(),
    a_lineDistance: Array.from({ length: n }, (_, k) => (k % 6) * 0.03) } };
}
// a 4x4 texture's texels (each texture different)
const texels = (k) => Array.from({ length: 64 }, (_, i) => (((i >> 2) * 37 + k * 71 + (i & 3) * 53 + ((i >> 2) % 4 + (i >> 4)) * 29) % 200) + 40);

const geo = sphere();
const jobs = programs.map((p) => {
  const draws = [];
  for (const line of p.layout.split("\n")) {
    const [kind, name, type, at] = line.split(" ");
    if (kind === "draw") draws.push({ name, type, at: +at, value: drawValue(name, type) });
  }
  return { ...p, frame: Array.from(frameBlock(p.vals)), draws };
});

const browser = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-webgpu-adapter=swiftshader"] });
const page = await browser.newPage();
await page.route("https://wgsl.local/**", (r) => r.fulfill({ contentType: "text/html", body: "<!doctype html>" }));
await page.goto("https://wgsl.local/");
page.on("console", (m) => { if (process.env.CON) console.log("console:", m.text()); });
const results = await page.evaluate(async ({ jobs, geo, SIZE, texelsSrc, FRAME_FLOATS }) => {
  const texels = eval(texelsSrc);
  // the shadow maps' depth: points farther than this from the light are in shadow
  const DEPTH = 0.85;
  // a texture's levels (box filtered; all of them for the transmission map, which is sampled by level)
  const levels = (base, all) => {
    const out = [{ w: 4, data: new Uint8Array(base) }];
    while (all && out[out.length - 1].w > 1) {
      const p = out[out.length - 1], w = p.w / 2, d = new Uint8Array(w * w * 4);
      for (let y = 0; y < w; y++) for (let x = 0; x < w; x++) for (let c = 0; c < 4; c++) {
        const at = (yy, xx) => p.data[(yy * p.w + xx) * 4 + c];
        d[(y * w + x) * 4 + c] = (at(2 * y, 2 * x) + at(2 * y, 2 * x + 1) + at(2 * y + 1, 2 * x) + at(2 * y + 1, 2 * x + 1) + 2) >> 2;
      }
      out.push({ w, data: d });
    }
    return out;
  };
  const adapter = await navigator.gpu.requestAdapter();
  const device = await adapter.requestDevice();
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = SIZE;
  const gl = canvas.getContext("webgl2", { antialias: false, preserveDrawingBuffer: true });
  const out = [];
  const parse = (layout) => layout.split("\n").filter(Boolean).map((l) => l.split(" "));

  function drawGL(job) {
    const sh = (t, s) => { const o = gl.createShader(t); gl.shaderSource(o, s); gl.compileShader(o); if (!gl.getShaderParameter(o, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(o)); return o; };
    const prog = gl.createProgram();
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, job.vs));
    gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, job.fs));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
    gl.useProgram(prog);
    const bi = gl.getUniformBlockIndex(prog, "Frame");
    const ubo = gl.createBuffer();
    if (bi !== gl.INVALID_INDEX) {
      gl.uniformBlockBinding(prog, bi, 0);
      gl.bindBuffer(gl.UNIFORM_BUFFER, ubo);
      gl.bufferData(gl.UNIFORM_BUFFER, new Float32Array(job.frame.slice(0, FRAME_FLOATS)), gl.STATIC_DRAW);
      gl.bindBufferBase(gl.UNIFORM_BUFFER, 0, ubo);
    }
    for (const d of job.draws) {
      const loc = gl.getUniformLocation(prog, d.name);
      if (!loc) continue;
      const el = d.type.replace(/\[.*/, "");
      if (el === "int") gl.uniform1iv(loc, d.value);
      else if (el === "float") gl.uniform1fv(loc, d.value);
      else if (el === "vec2") gl.uniform2fv(loc, d.value);
      else if (el === "vec3") gl.uniform3fv(loc, d.value);
      else if (el === "vec4") gl.uniform4fv(loc, d.value);
      else if (el === "mat3") gl.uniformMatrix3fv(loc, false, d.value);
      else if (el === "mat4") gl.uniformMatrix4fv(loc, false, d.value);
    }
    const textures = [];
    let unit = 0, idx = -1;
    for (const [kind, name, type] of parse(job.layout)) {
      if (kind !== "tex") continue;
      idx++;
      const loc = gl.getUniformLocation(prog, name);
      if (!loc) continue;
      const t = gl.createTexture();
      textures.push(t);
      gl.activeTexture(gl.TEXTURE0 + unit);
      const cube = type.startsWith("samplerCube"), target = cube ? gl.TEXTURE_CUBE_MAP : gl.TEXTURE_2D;
      gl.bindTexture(target, t);
      const faces = cube ? [0, 1, 2, 3, 4, 5].map((f) => gl.TEXTURE_CUBE_MAP_POSITIVE_X + f) : [gl.TEXTURE_2D];
      for (const f of faces) {
        if (type.endsWith("Shadow")) gl.texImage2D(f, 0, gl.DEPTH_COMPONENT32F, 4, 4, 0, gl.DEPTH_COMPONENT, gl.FLOAT, new Float32Array(16).fill(DEPTH));
        else levels(texels(idx), name === "u_transMap").forEach((l, k) => gl.texImage2D(f, k, gl.RGBA8, l.w, l.w, 0, gl.RGBA, gl.UNSIGNED_BYTE, l.data));
      }
      gl.texParameteri(target, gl.TEXTURE_MIN_FILTER, name === "u_transMap" ? gl.NEAREST_MIPMAP_NEAREST : gl.NEAREST);
      gl.texParameteri(target, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texParameteri(target, gl.TEXTURE_WRAP_S, cube ? gl.CLAMP_TO_EDGE : gl.REPEAT);
      gl.texParameteri(target, gl.TEXTURE_WRAP_T, cube ? gl.CLAMP_TO_EDGE : gl.REPEAT);
      if (type.endsWith("Shadow")) { gl.texParameteri(target, gl.TEXTURE_COMPARE_MODE, gl.COMPARE_REF_TO_TEXTURE); gl.texParameteri(target, gl.TEXTURE_COMPARE_FUNC, gl.LEQUAL); }
      gl.uniform1i(loc, unit);
      unit++;
    }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const bufs = [];
    for (const [kind, name, type, loc] of parse(job.layout)) {
      if (kind !== "attr") continue;
      const b = gl.createBuffer(); bufs.push(b);
      gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(geo.attrs[name]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(+loc);
      gl.vertexAttribPointer(+loc, type === "float" ? 1 : +type.slice(-1), gl.FLOAT, false, 0, 0);
    }
    gl.viewport(0, 0, SIZE, SIZE);
    gl.clearColor(0, 0, 0, 0);
    gl.clearDepth(1);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LESS);
    gl.disable(gl.CULL_FACE);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLES, 0, geo.n);
    const px = new Uint8Array(SIZE * SIZE * 4);
    gl.readPixels(0, 0, SIZE, SIZE, gl.RGBA, gl.UNSIGNED_BYTE, px);
    for (const t of textures) gl.deleteTexture(t);
    for (const b of bufs) gl.deleteBuffer(b);
    gl.deleteBuffer(ubo); gl.deleteVertexArray(vao); gl.deleteProgram(prog);
    return px;
  }

  async function drawGPU(job, flip) {
    const items = parse(job.layout);
    const draw = items.find((i) => i[0] === "draw_size");
    const drawFloats = draw ? +draw[1] : 0;
    device.pushErrorScope("validation");
    const vmod = device.createShaderModule({ code: job.wvs }), fmod = device.createShaderModule({ code: job.wfs });
    const entries = [], groupEntries = [];
    const VF = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT;
    const frame = new Float32Array(job.frame);
    frame.set([SIZE, SIZE, flip ? -1 : 1, 0], FRAME_FLOATS);
    const fbuf = device.createBuffer({ size: frame.byteLength, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    device.queue.writeBuffer(fbuf, 0, frame);
    entries.push({ binding: 0, visibility: VF, buffer: { type: "uniform" } });
    groupEntries.push({ binding: 0, resource: { buffer: fbuf } });
    if (job.draws.length) {
      const ab = new ArrayBuffer(drawFloats * 4), f32 = new Float32Array(ab), i32 = new Int32Array(ab);
      for (const d of job.draws) {
        const el = d.type.replace(/\[.*/, "");
        if (el === "int") i32.set(d.value, d.at);
        else if (el === "mat3") {
          const count = d.value.length / 9;
          for (let m = 0; m < count; m++) for (let c = 0; c < 3; c++) f32.set(d.value.slice(m * 9 + c * 3, m * 9 + c * 3 + 3), d.at + m * 12 + c * 4);
        } else if (d.type.includes("[") && el !== "vec4" && el !== "mat4") {
          const w = +el.slice(-1) || 1;
          for (let k = 0; k < d.value.length / w; k++) f32.set(d.value.slice(k * w, k * w + w), d.at + k * 4);
        } else f32.set(d.value, d.at);
      }
      const dbuf = device.createBuffer({ size: ab.byteLength, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
      device.queue.writeBuffer(dbuf, 0, ab);
      entries.push({ binding: 1, visibility: VF, buffer: { type: "uniform" } });
      groupEntries.push({ binding: 1, resource: { buffer: dbuf } });
    }
    let unit = 0;
    for (const [kind, name, type, binding, stages] of items) {
      if (kind !== "tex") continue;
      const vis = (+stages & 1 ? GPUShaderStage.VERTEX : 0) | (+stages & 2 ? GPUShaderStage.FRAGMENT : 0);
      const cube = type.startsWith("samplerCube"), depth = type.endsWith("Shadow");
      const mips = name === "u_transMap" ? 3 : 1;
      const tex = device.createTexture({ size: [4, 4, cube ? 6 : 1], mipLevelCount: mips, format: depth ? "depth32float" : "rgba8unorm",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
      for (let layer = 0; layer < (cube ? 6 : 1); layer++) {
        if (depth) {
          const enc = device.createCommandEncoder();
          enc.beginRenderPass({ colorAttachments: [], depthStencilAttachment: { view: tex.createView({ dimension: "2d", baseArrayLayer: layer, arrayLayerCount: 1 }), depthClearValue: DEPTH, depthLoadOp: "clear", depthStoreOp: "store" } }).end();
          device.queue.submit([enc.finish()]);
        } else levels(texels(unit), mips > 1).forEach((l, k) => device.queue.writeTexture({ texture: tex, mipLevel: k, origin: [0, 0, layer] }, l.data, { bytesPerRow: 4 * l.w }, [l.w, l.w, 1]));
      }
      const smp = device.createSampler(depth ? { compare: "less-equal", magFilter: "nearest", minFilter: "nearest" }
        : { magFilter: "nearest", minFilter: "nearest", mipmapFilter: "nearest", addressModeU: cube ? "clamp-to-edge" : "repeat", addressModeV: cube ? "clamp-to-edge" : "repeat" });
      entries.push({ binding: +binding, visibility: vis, texture: { sampleType: depth ? "depth" : "float", viewDimension: cube ? "cube" : "2d" } });
      entries.push({ binding: +binding + 1, visibility: vis, sampler: { type: depth ? "comparison" : "filtering" } });
      groupEntries.push({ binding: +binding, resource: tex.createView({ dimension: cube ? "cube" : "2d" }) });
      groupEntries.push({ binding: +binding + 1, resource: smp });
      unit++;
    }
    const bgl = device.createBindGroupLayout({ entries });
    const FORMATS = { float: "float32", vec2: "float32x2", vec3: "float32x3", vec4: "float32x4" };
    // one buffer per attribute, the instance matrix's columns in one (WebGPU allows 8 buffers)
    const buffers = [], vbufs = [];
    const im = items.filter((i) => i[0] === "attr" && i[1].startsWith("a_im"));
    if (im.length) {
      buffers.push({ arrayStride: 64, attributes: im.map((i) => ({ shaderLocation: +i[3], offset: 16 * (+i[3] - 4), format: "float32x4" })) });
      const data = new Float32Array(geo.n * 16);
      for (let v = 0; v < geo.n; v++) for (let c = 0; c < 4; c++) data.set(geo.attrs["a_im" + c].slice(v * 4, v * 4 + 4), v * 16 + c * 4);
      const b = device.createBuffer({ size: data.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
      device.queue.writeBuffer(b, 0, data);
      vbufs.push(b);
    }
    for (const [kind, name, type, loc] of items) {
      if (kind !== "attr" || name.startsWith("a_im")) continue;
      const w = type === "float" ? 1 : +type.slice(-1);
      buffers.push({ arrayStride: w * 4, attributes: [{ shaderLocation: +loc, offset: 0, format: FORMATS[type] }] });
      const data = new Float32Array(geo.attrs[name]);
      const b = device.createBuffer({ size: data.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
      device.queue.writeBuffer(b, 0, data);
      vbufs.push(b);
    }
    const pipeline = device.createRenderPipeline({
      layout: device.createPipelineLayout({ bindGroupLayouts: [bgl] }),
      vertex: { module: vmod, entryPoint: "vs_main", buffers },
      fragment: { module: fmod, entryPoint: "fs_main", targets: [{ format: "rgba8unorm" }] },
      primitive: { topology: "triangle-list", cullMode: "none", frontFace: flip ? "cw" : "ccw" },
      depthStencil: { format: "depth24plus", depthWriteEnabled: true, depthCompare: "less" },
    });
    const group = device.createBindGroup({ layout: bgl, entries: groupEntries });
    const color = device.createTexture({ size: [SIZE, SIZE], format: "rgba8unorm", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
    const depthT = device.createTexture({ size: [SIZE, SIZE], format: "depth24plus", usage: GPUTextureUsage.RENDER_ATTACHMENT });
    const enc = device.createCommandEncoder();
    const pass = enc.beginRenderPass({
      colorAttachments: [{ view: color.createView(), clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" }],
      depthStencilAttachment: { view: depthT.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" },
    });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, group);
    vbufs.forEach((b, i) => pass.setVertexBuffer(i, b));
    pass.draw(geo.n);
    pass.end();
    const read = device.createBuffer({ size: SIZE * SIZE * 4, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    enc.copyTextureToBuffer({ texture: color }, { buffer: read, bytesPerRow: SIZE * 4 }, [SIZE, SIZE]);
    device.queue.submit([enc.finish()]);
    const err = await device.popErrorScope();
    if (err) throw new Error(err.message.split("\n").slice(0, 3).join(" "));
    await read.mapAsync(GPUMapMode.READ);
    const px = new Uint8Array(read.getMappedRange().slice(0));
    read.unmap();
    if (flip) return px;
    // a canvas pass is y down: rows to GL's order
    const o = new Uint8Array(px.length), row = SIZE * 4;
    for (let y = 0; y < SIZE; y++) o.set(px.subarray(y * row, y * row + row), (SIZE - 1 - y) * row);
    return o;
  }

  for (const job of jobs) {
    const r = { name: job.name };
    try {
      const a = drawGL(job);
      r.gl = Array.from(a);
      for (const flip of [false, true]) {
        const b = await drawGPU(job, flip);
        let sum = 0, far = 0, max = 0, lit = 0;
        for (let i = 0; i < a.length; i += 4) {
          let d = 0;
          for (let c = 0; c < 4; c++) d = Math.max(d, Math.abs(a[i + c] - b[i + c]));
          sum += d; if (d > 8) far++; if (d > max) max = d; if (a[i + 3] || b[i + 3]) lit++;
        }
        r[flip ? "texture" : "canvas"] = { mean: sum / (a.length / 4), far, max, lit };
        if (!flip) r.wgpu = Array.from(b);
      }
    } catch (e) { r.error = String(e.message || e).slice(0, 400); }
    out.push(r);
  }
  return out;
}, { jobs, geo, SIZE, texelsSrc: texels.toString(), FRAME_FLOATS: FRAME_AT[FRAME_AT.length - 1] });
await browser.close();

function png(file, px) {
  const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE);
  for (let y = 0; y < SIZE; y++) { raw[y * (SIZE * 4 + 1)] = 0; Buffer.from(px.slice((SIZE - 1 - y) * SIZE * 4, (SIZE - y) * SIZE * 4)).copy(raw, y * (SIZE * 4 + 1) + 1); }
  const crc = (b) => { let c = ~0; for (const x of b) { c ^= x; for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)); } return ~c >>> 0; };
  const chunk = (t, d) => { const l = Buffer.alloc(4); l.writeUInt32BE(d.length); const td = Buffer.concat([Buffer.from(t), d]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(SIZE, 0); ihdr.writeUInt32BE(SIZE, 4); ihdr[8] = 8; ihdr[9] = 6;
  fs.writeFileSync(file, Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]));
}

let fail = 0;
console.log(`${"case".padEnd(56)} canvas pass (mean/max/far)   texture pass   lit px`);
results.forEach((r, i) => {
  if (r.error) { fail++; console.log(`FAIL ${r.name}: ${r.error}`); return; }
  const ok = [r.canvas, r.texture].every((c) => c.mean <= 0.5 && c.far <= SIZE * SIZE * 0.002) && r.canvas.lit > 0;
  if (!ok) fail++;
  const f = (c) => `${c.mean.toFixed(2).padStart(5)} ${String(c.max).padStart(3)} ${String(c.far).padStart(4)}`;
  console.log(`${ok ? "ok  " : "FAIL"} ${r.name.padEnd(51)} ${f(r.canvas)}         ${f(r.texture)}   ${r.canvas.lit}`);
  if (process.env.PNG) { fs.mkdirSync(process.env.PNG, { recursive: true }); png(path.join(process.env.PNG, i + "-gl.png"), r.gl); png(path.join(process.env.PNG, i + "-wgpu.png"), r.wgpu); }
});
console.log(`${results.length - fail}/${results.length} cases draw the same pixels with WGSL on WebGPU as with GLSL on WebGL2`);
process.exit(fail ? 1 : 0);
