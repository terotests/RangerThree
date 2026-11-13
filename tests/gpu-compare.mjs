// The GPU command path against the WebGL2 backend: every playground example
// is drawn in headless Chromium by js/three-webgl.js, then the same scene and
// camera are turned into GPU commands by the crate (renderers/gpu_frame.rs)
// and played by js/gpu-webgl2.js on the same context. Examples the command
// renderer cannot draw yet are listed with what they need. The "(three.js)"
// scenes (shader materials, morph targets), which the WebGL2 backend does not
// draw, are compared with three.js r185 instead. SAVE=dir writes those as ppm.
//
//   node tests/gpu-compare.mjs [--grep=re] [--dump=dir] [--frames=N]
//
// --dump writes each complete example's command stream (JSON) and its WebGL2
// pixels (RGBA, bottom row first) for gpu-gl's native player
// (cargo run --release --example replay -- dir).
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const arg = (k, d) => (process.argv.find((a) => a.startsWith(`--${k}=`)) || "").split("=").slice(1).join("=") || d;
const GREP = new RegExp(arg("grep", "."), "i"), DUMP = arg("dump", ""), FRAMES = +arg("frames", "20");
const W = 320, H = 240;
const ctx = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, "playground/examples.js"), "utf8") + ";this.EXAMPLES=EXAMPLES;", ctx);
// scenes the playground has no example of
Object.assign(ctx.EXAMPLES, {
  "(test) Dashed lines and a dashed circle": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x15171c);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 2, 7); camera.lookAt(0, 0, 0);
const pts = [];
for (let i = 0; i <= 64; i++) { const a = i / 64 * Math.PI * 2; pts.push(new THREE.Vector3(Math.cos(a) * 2.5, Math.sin(a * 3) * 0.6, Math.sin(a) * 2.5)); }
const loop = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), new THREE.LineDashedMaterial({ color: 0xffaa33, dashSize: 0.3, gapSize: 0.15 }));
loop.computeLineDistances();
scene.add(loop);
const box = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(1.6, 1.6, 1.6)), new THREE.LineDashedMaterial({ color: 0x66ccff, dashSize: 0.1, gapSize: 0.1, scale: 2 }));
box.computeLineDistances();
box.rotation.set(0.4, 0.6, 0);
scene.add(box);
renderer.setAnimationLoop(() => renderer.render(scene, camera));
`,
});
// scenes the WebGL2 backend cannot draw (shader materials, morph targets):
// their pixels come from three.js r185 (tmp/three-ref, scripts/deps.sh) instead
const THREE_REF = {
  "(three.js) ShaderMaterial: uniforms, a texture, varyings": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x202428);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 1.2, 4); camera.lookAt(0, 0, 0);
const px = new Uint8Array(16 * 16 * 4);
for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
  const o = (y * 16 + x) * 4, on = ((x >> 2) + (y >> 2)) & 1;
  px[o] = on ? 240 : 30; px[o + 1] = on ? 190 : 80; px[o + 2] = on ? 60 : 160; px[o + 3] = 255;
}
const tex = new THREE.DataTexture(px, 16, 16);
tex.magFilter = THREE.NearestFilter; tex.minFilter = THREE.NearestFilter; tex.needsUpdate = true;
const mat = new THREE.ShaderMaterial({
  uniforms: { time: { value: 0 }, tint: { value: new THREE.Color(0x88ccff) }, map: { value: tex }, light: { value: new THREE.Vector3(0.5, 0.8, 0.6).normalize() } },
  vertexShader: \`varying vec2 vUv;
varying vec3 vN;
uniform float time;
void main() {
  vUv = uv;
  vN = normalize(normalMatrix * normal);
  vec3 p = position + normal * 0.05 * sin(position.y * 6.0 + time);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(p, 1.0);
}\`,
  fragmentShader: \`varying vec2 vUv;
varying vec3 vN;
uniform vec3 tint;
uniform sampler2D map;
uniform vec3 light;
void main() {
  float d = max(dot(normalize(vN), normalize((viewMatrix * vec4(light, 0.0)).xyz)), 0.0);
  gl_FragColor = vec4(texture2D(map, vUv).rgb * tint * (0.3 + 0.7 * d), 1.0);
}\`,
});
const mesh = new THREE.Mesh(new THREE.TorusKnotGeometry(0.8, 0.28, 96, 12), mat);
scene.add(mesh);
renderer.setAnimationLoop(() => { mat.uniforms.time.value = 1.3; mesh.rotation.set(0.3, 0.5, 0); renderer.render(scene, camera); });
`,
  "(three.js) RawShaderMaterial: GLSL 1, an own attribute, blending": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x101418);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 0, 3);
let seed = 7;
const rnd = () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; };
const pos = [], shade = [];
for (let i = 0; i < 60 * 3; i++) {
  pos.push(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5);
  shade.push(rnd(), rnd(), rnd(), rnd() * 0.6 + 0.2);
}
const g = new THREE.BufferGeometry();
g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
g.setAttribute("shade", new THREE.Float32BufferAttribute(shade, 4));
const mat = new THREE.RawShaderMaterial({
  uniforms: { scale: { value: 1.6 } },
  side: THREE.DoubleSide, transparent: true, depthWrite: false,
  vertexShader: \`precision mediump float;
uniform mat4 modelViewMatrix;
uniform mat4 projectionMatrix;
uniform float scale;
attribute vec3 position;
attribute vec4 shade;
varying vec4 vC;
void main() {
  vC = shade;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position * scale, 1.0);
}\`,
  fragmentShader: \`precision mediump float;
varying vec4 vC;
void main() { gl_FragColor = vC; }\`,
});
const mesh = new THREE.Mesh(g, mat);
mesh.rotation.set(0.2, 0.4, 0);
scene.add(mesh);
renderer.setAnimationLoop(() => renderer.render(scene, camera));
`,
  "(three.js) ShaderMaterial: GLSL 3, defines, instanced, uniform array": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1b1e26);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 3, 6); camera.lookAt(0, 0, 0);
const mat = new THREE.ShaderMaterial({
  glslVersion: THREE.GLSL3,
  defines: { SCALE: "0.8", USE_TINT: true },
  uniforms: { tints: { value: [new THREE.Vector3(1, 0.4, 0.2), new THREE.Vector3(0.2, 0.6, 1)] } },
  vertexShader: \`out vec3 vN;
flat out int vId;
void main() {
  vN = normal;
  vId = gl_InstanceID;
  gl_Position = projectionMatrix * viewMatrix * modelMatrix * instanceMatrix * vec4(position * SCALE, 1.0);
}\`,
  fragmentShader: \`in vec3 vN;
flat in int vId;
uniform vec3 tints[2];
out vec4 outColor;
void main() {
  vec3 c = 0.5 + 0.5 * vN;
#ifdef USE_TINT
  c *= tints[vId % 2];
#endif
  outColor = vec4(c, 1.0);
}\`,
});
const mesh = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), mat, 9);
const d = new THREE.Object3D();
for (let i = 0; i < 9; i++) {
  d.position.set((i % 3 - 1) * 1.6, 0, (Math.floor(i / 3) - 1) * 1.6);
  d.rotation.set(0, i * 0.3, 0);
  d.updateMatrix();
  mesh.setMatrixAt(i, d.matrix);
}
scene.add(mesh);
renderer.setAnimationLoop(() => renderer.render(scene, camera));
`,
  "(three.js) Morph targets: positions and normals": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x202428);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 0.5, 5); camera.lookAt(0, 0, 0);
function morphed(relative) {
  const geo = new THREE.SphereGeometry(1, 32, 16);
  const p = geo.attributes.position, targets = [[], []];
  for (let i = 0; i < p.count; i++) {
    const x = p.getX(i), y = p.getY(i), z = p.getZ(i);
    const a = [x * (1 + 0.6 * y * y), y * 1.3, z * (1 + 0.6 * y * y)];
    const b = [x + 0.3 * Math.sin(y * 4), y, z + 0.3 * Math.cos(y * 4)];
    targets[0].push(...(relative ? [a[0] - x, a[1] - y, a[2] - z] : a));
    targets[1].push(...(relative ? [b[0] - x, b[1] - y, b[2] - z] : b));
  }
  const normals = targets.map((t) => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(relative ? t.map((v, k) => v + p.array[k]) : t, 3));
    g.setIndex(geo.index); g.computeVertexNormals();
    const n = g.attributes.normal.array, base = geo.attributes.normal.array;
    return new THREE.Float32BufferAttribute(relative ? Array.from(n, (v, k) => v - base[k]) : Array.from(n), 3);
  });
  geo.morphAttributes.position = targets.map((t) => new THREE.Float32BufferAttribute(t, 3));
  geo.morphAttributes.normal = normals;
  geo.morphTargetsRelative = relative;
  return geo;
}
const a = new THREE.Mesh(morphed(false), new THREE.MeshNormalMaterial());
a.morphTargetInfluences[0] = 0.6; a.morphTargetInfluences[1] = 0.3;
a.position.x = -1.3;
const b = new THREE.Mesh(morphed(true), new THREE.MeshNormalMaterial({ flatShading: true }));
b.morphTargetInfluences[0] = 0.2; b.morphTargetInfluences[1] = 0.8;
b.position.x = 1.3;
scene.add(a, b);
renderer.setAnimationLoop(() => renderer.render(scene, camera));
`,
};
Object.assign(ctx.EXAMPLES, THREE_REF);
// playground examples with shader materials or morph targets are checked against three.js too
for (const [name, code] of Object.entries(ctx.EXAMPLES)) if (!THREE_REF[name] && /ShaderMaterial|morphAttributes/.test(code)) THREE_REF[name] = code;
// the shadow example with the other shadow map types
const SHADOWS = ctx.EXAMPLES["Shadows: sun, spot and point"];
if (SHADOWS) {
  ctx.EXAMPLES["(test) Shadows, basic"] = SHADOWS.replace("THREE.PCFShadowMap", "THREE.BasicShadowMap");
  ctx.EXAMPLES["(test) Shadows, VSM"] = SHADOWS.replace("THREE.PCFShadowMap", "THREE.VSMShadowMap");
}
const LIB = ["build/rangerthree-core.js", "js/three.js", "js/three-scene.js", "js/three-curves.js", "js/three-renderer.js", "js/three-webgl.js", "js/gpu-webgl2.js"]
  .map((f) => fs.readFileSync(path.join(ROOT, f), "utf8")).join("\n;\n");

function pageScript(code, skip) {
  return `
var __skip = ${skip ? "true" : "false"};
var canvas = document.getElementById("c");
var renderer = new THREE.WebGLRenderer({ canvas: canvas, antialias: false, preserveDrawingBuffer: true });
renderer.setSize(${W}, ${H}, false);
var __loop = null, __scene = null, __camera = null;
renderer.setAnimationLoop = function (fn) { __loop = fn; };
var __render = renderer.render.bind(renderer);
renderer.render = function (s, c) { __scene = s; __camera = c; if (!__skip) return __render(s, c); };
function orbit(camera, target) { camera.lookAt(target); }
(function () {
${code}
})();
`;
}

const browser = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage();
page.on("pageerror", (e) => console.log("pageerror", e.message));
await page.setContent(`<!doctype html><canvas id="c" width="${W}" height="${H}"></canvas>`);
if (DUMP) fs.mkdirSync(DUMP, { recursive: true });
const REF = path.join(ROOT, "tmp/three-ref/three/build");
const refPage = await browser.newPage();
refPage.on("pageerror", (e) => console.log("three.js pageerror", e.message));
await refPage.route("http://ref.local/**", (route) => {
  const u = new URL(route.request().url());
  if (u.pathname === "/") return route.fulfill({ contentType: "text/html", body: `<!doctype html><canvas id="c" width="${W}" height="${H}"></canvas>` });
  route.fulfill({ contentType: "text/javascript", body: fs.readFileSync(path.join(REF, path.basename(u.pathname))) });
});
// the scene drawn by three.js r185 (antialias off, as the backend above)
async function threeRef(code) {
  await refPage.goto("http://ref.local/");
  return refPage.evaluate(async ({ code, W, H }) => {
    const THREE = await import("/three.module.js");
    const canvas = document.getElementById("c");
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, preserveDrawingBuffer: true });
    renderer.setSize(W, H, false);
    let loop = null;
    renderer.setAnimationLoop = (fn) => { loop = fn; };
    new Function("THREE", "renderer", "canvas", "orbit", code)(THREE, renderer, canvas, (camera, target) => camera.lookAt(target || new THREE.Vector3()));
    loop(1000); loop(1000);
    const gl = renderer.getContext(), px = new Uint8Array(W * H * 4);
    gl.readPixels(0, 0, W, H, gl.RGBA, gl.UNSIGNED_BYTE, px);
    return Array.from(px);
  }, { code, W, H });
}

let complete = 0, matched = 0, total = 0;
const rows = [];
for (const name of Object.keys(ctx.EXAMPLES)) {
  if (!GREP.test(name)) continue;
  total++;
  await page.reload();
  await page.setContent(`<!doctype html><canvas id="c" width="${W}" height="${H}"></canvas>`);
  const ref = THREE_REF[name] ? await threeRef(THREE_REF[name]) : null;
  const r = await page.evaluate(({ lib, script, frames, ref }) => {
    (0, eval)(lib + "\n;\n" + script);
    const canvas = document.getElementById("c"), gl = renderer.getContext ? renderer.getContext() : canvas.getContext("webgl2");
    const read = () => { const px = new Uint8Array(canvas.width * canvas.height * 4); gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, px); return px; };
    const R = RangerThreeCore, t = 1000;
    const byteArray = (b) => !b ? new Uint8Array(0) : b._view ? new Uint8Array(b._view.buffer, b._view.byteOffset, b.byteLength) : Uint8Array.from(b);
    const tw = [], tg = [];
    let a, frame;
    for (let i = 0; i < frames; i++) {
      const t0 = performance.now(); __loop(t); a = ref ? Uint8Array.from(ref) : read(); tw.push(performance.now() - t0);
    }
    if (!__scene) return { error: "no render" };
    const gpu = R.GpuRenderer.new_(), player = new GpuWebGL2(gl);
    const s = R.GpuSettings.new_(canvas.width, canvas.height);
    s.pixel_ratio = renderer._pixelRatio; s.tone_mapping = renderer.toneMapping | 0;
    s.exposure = renderer.toneMappingExposure === undefined ? 1 : renderer.toneMappingExposure;
    s.output_linear = renderer.outputColorSpace === "srgb-linear";
    s.shadows = !!(renderer.shadowMap && renderer.shadowMap.enabled); s.half_float = !!gl.getExtension("EXT_color_buffer_float"); s.shadow_type = renderer.shadowMap ? renderer.shadowMap.type | 0 : 1; s.float_linear = !!gl.getExtension("OES_texture_float_linear"); s.samples = Math.min(4, gl.getParameter(gl.MAX_SAMPLES)); s.clear_color = renderer._soft.clear_color; s.clear_alpha = renderer._soft.clear_alpha;
    let b, first = null; const pl = [], py = [];
    for (let i = 0; i < frames; i++) {
      const t0 = performance.now();
      GpuWebGL2.uploadImages(gpu, __scene, THREE.WebGLRenderer.decodeImage);
      frame = gpu.render(__scene, __camera, s);
      const t1 = performance.now();
      if (frame.missing.length) return { missing: frame.missing };
      if (!first) first = frame.commands;
      player.run(frame.commands);
      b = read();
      const t2 = performance.now();
      pl.push(t1 - t0); py.push(t2 - t1); tg.push(t2 - t0);
    }
    let sum = 0, far = 0, max = 0;
    for (let i = 0; i < a.length; i++) { const d = Math.abs(a[i] - b[i]); sum += d; if (d > max) max = d; if (d > 3) far++; }
    const med = (v) => { const x = v.slice().sort((p, q) => p - q); return x[x.length >> 1]; };
    const c = frame.commands;
    return { mean: sum / a.length, max, far: far / a.length, webgl: med(tw), gpu: med(tg), plan: med(pl), play: med(py),
      commands: c.count, ints: c.ints.length, floats: c.floats.length,
      dump: { ints: first.ints, floats: Array.from(first.floats), texts: first.texts, bytes: Array.from(byteArray(first.bytes)), pixels: Array.from(b), w: canvas.width, h: canvas.height } };
  }, { lib: LIB, script: pageScript(ctx.EXAMPLES[name], !!ref), frames: FRAMES, ref });
  if (r.error) { rows.push(`ERROR ${name}: ${r.error}`); continue; }
  if (r.missing) { rows.push(`  --  ${name}: needs ${r.missing.join(", ")}`); continue; }
  if (process.env.SAVE && ref) {
    const ppm = (px) => { const o = [Buffer.from(`P6\n${W} ${H}\n255\n`)]; const b = Buffer.alloc(W * H * 3); for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) for (let k = 0; k < 3; k++) b[(y * W + x) * 3 + k] = px[((H - 1 - y) * W + x) * 4 + k]; o.push(b); return Buffer.concat(o); };
    const slug = name.replace(/[^A-Za-z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase();
    fs.mkdirSync(process.env.SAVE, { recursive: true });
    fs.writeFileSync(path.join(process.env.SAVE, slug + ".three.ppm"), ppm(ref));
    fs.writeFileSync(path.join(process.env.SAVE, slug + ".gpu.ppm"), ppm(r.dump.pixels));
  }
  complete++;
  const ok = r.mean <= 0.5 && r.far <= 0.001;
  if (ok) matched++;
  rows.push(`${ok ? "PASS" : "DIFF"}  ${name}${ref ? " (vs three.js)" : ""}: mean ${r.mean.toFixed(3)} max ${r.max} far ${(r.far * 100).toFixed(2)} % | ` +
    `WebGL2 backend ${r.webgl.toFixed(2)} ms, commands ${r.gpu.toFixed(2)} ms (crate ${r.plan.toFixed(2)} + player ${r.play.toFixed(2)}) | ` +
    `${r.commands} commands, ${r.ints} ints, ${r.floats} floats per frame (medians)`);
  if (DUMP) {
    const slug = name.replace(/[^A-Za-z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase();
    const d = r.dump;
    fs.writeFileSync(path.join(DUMP, slug + ".json"), JSON.stringify({ name, w: d.w, h: d.h, ints: d.ints, floats: d.floats, texts: d.texts, bytes: d.bytes }));
    fs.writeFileSync(path.join(DUMP, slug + ".rgba"), Buffer.from(d.pixels));
  }
}
await browser.close();
for (const row of rows) console.log(row);
console.log(`commands draw ${complete}/${total} examples; ${matched}/${complete} match their reference (WebGL2 backend or three.js r185)`);
process.exit(matched === complete ? 0 : 1);
