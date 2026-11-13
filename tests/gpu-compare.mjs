// The GPU command path against the WebGL2 backend: every playground example
// is drawn in headless Chromium by js/three-webgl.js, then the same scene and
// camera are turned into GPU commands by the crate (renderers/gpu_frame.rs)
// and played by js/gpu-webgl2.js on the same context. Examples the command
// renderer cannot draw yet are listed with what they need.
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
const LIB = ["build/rangerthree-core.js", "js/three.js", "js/three-scene.js", "js/three-curves.js", "js/three-renderer.js", "js/three-webgl.js", "js/gpu-webgl2.js"]
  .map((f) => fs.readFileSync(path.join(ROOT, f), "utf8")).join("\n;\n");

function pageScript(code) {
  return `
var canvas = document.getElementById("c");
var renderer = new THREE.WebGLRenderer({ canvas: canvas, antialias: false, preserveDrawingBuffer: true });
renderer.setSize(${W}, ${H}, false);
var __loop = null, __scene = null, __camera = null;
renderer.setAnimationLoop = function (fn) { __loop = fn; };
var __render = renderer.render.bind(renderer);
renderer.render = function (s, c) { __scene = s; __camera = c; return __render(s, c); };
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

let complete = 0, matched = 0, total = 0;
const rows = [];
for (const name of Object.keys(ctx.EXAMPLES)) {
  if (!GREP.test(name)) continue;
  total++;
  await page.reload();
  await page.setContent(`<!doctype html><canvas id="c" width="${W}" height="${H}"></canvas>`);
  const r = await page.evaluate(({ lib, script, frames }) => {
    (0, eval)(lib + "\n;\n" + script);
    const canvas = document.getElementById("c"), gl = renderer.getContext ? renderer.getContext() : canvas.getContext("webgl2");
    const read = () => { const px = new Uint8Array(canvas.width * canvas.height * 4); gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, px); return px; };
    const R = RangerThreeCore, t = 1000;
    const byteArray = (b) => !b ? new Uint8Array(0) : b._view ? new Uint8Array(b._view.buffer, b._view.byteOffset, b.byteLength) : Uint8Array.from(b);
    const tw = [], tg = [];
    let a, frame;
    for (let i = 0; i < frames; i++) {
      const t0 = performance.now(); __loop(t); a = read(); tw.push(performance.now() - t0);
    }
    if (!__scene) return { error: "no render" };
    const gpu = R.GpuRenderer.new_(), player = new GpuWebGL2(gl);
    const s = R.GpuSettings.new_(canvas.width, canvas.height);
    s.pixel_ratio = renderer._pixelRatio; s.tone_mapping = renderer.toneMapping | 0;
    s.exposure = renderer.toneMappingExposure === undefined ? 1 : renderer.toneMappingExposure;
    s.output_linear = renderer.outputColorSpace === "srgb-linear";
    s.shadows = !!(renderer.shadowMap && renderer.shadowMap.enabled); s.clear_color = renderer._soft.clear_color; s.clear_alpha = renderer._soft.clear_alpha;
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
  }, { lib: LIB, script: pageScript(ctx.EXAMPLES[name]), frames: FRAMES });
  if (r.error) { rows.push(`ERROR ${name}: ${r.error}`); continue; }
  if (r.missing) { rows.push(`  --  ${name}: needs ${r.missing.join(", ")}`); continue; }
  complete++;
  const ok = r.mean <= 0.5 && r.far <= 0.001;
  if (ok) matched++;
  rows.push(`${ok ? "PASS" : "DIFF"}  ${name}: mean ${r.mean.toFixed(3)} max ${r.max} far ${(r.far * 100).toFixed(2)} % | ` +
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
console.log(`commands draw ${complete}/${total} examples; ${matched}/${complete} match the WebGL2 backend`);
process.exit(matched === complete ? 0 : 1);
