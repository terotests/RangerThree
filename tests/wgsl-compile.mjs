// The main program's WGSL (three/src/renderers/wgsl.rs) against Chromium's
// WebGPU (Tint, on SwiftShader). The variants: every one the playground
// examples compile when they draw with the WebGL2 backend, and the heaviest
// variant of each kind of material (as tests/sampler-limit.mjs), with each
// shadow type. Each pair of stages must compile without errors and make a
// render pipeline (stages agree on their varyings, bindings fit one layout).
//
//   bash scripts/build-js.sh && node tests/wgsl-compile.mjs
//   SCENES="Torus knot|..." DUMP=dir (writes the WGSL of each variant)
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createRequire } from "node:module";

const { chromium } = createRequire(import.meta.url)("playwright");
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const ctx = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, "build/rangerthree-core.js"), "utf8") + ";this.R = RangerThreeCore;", ctx);
const S = ctx.R.RenderersShaders;
const W = ctx.R.RenderersWgsl;
const KEYS = S.key_names();
const ex = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, "playground/examples.js"), "utf8") + ";this.EXAMPLES=EXAMPLES;", ex);
const SCENES = process.env.SCENES ? process.env.SCENES.split("|") : Object.keys(ex.EXAMPLES);
const LIB = ["build/rangerthree-core.js", "js/three.js", "js/three-scene.js", "js/three-curves.js", "js/three-renderer.js", "js/three-webgl.js"]
  .map((f) => fs.readFileSync(path.join(ROOT, f), "utf8")).join("\n;\n");

// 1. the variants the examples use (WebGL2, SwiftShader)
const variants = new Map();
const add = (name, type, vals) => { const k = type + ":" + vals.join(","); if (!variants.has(k)) variants.set(k, { name, type, vals }); };
const gl = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
for (const name of SCENES) {
  const page = await gl.newPage();
  await page.setContent(`<canvas id="c" width="160" height="120"></canvas>`);
  await page.evaluate((lib) => (0, eval)(lib), LIB);
  const seen = await page.evaluate((code) => {
    const seen = [];
    const S = RangerThreeCore.RenderersShaders, f = S.main_fragment;
    S.main_fragment = function (type, vals) { seen.push([type, Array.from(vals)]); return f.call(this, type, vals); };
    const canvas = document.getElementById("c");
    let loop = null;
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: false });
    renderer.setSize(160, 120, false);
    renderer.setAnimationLoop = (fn) => { loop = fn; };
    window.renderer = renderer;
    window.orbit = (camera, target) => camera.lookAt(target);
    try { (0, eval)("(function(){" + code + "\n})()"); if (loop) loop(1000); } catch (e) { seen.push(["error", String(e.message)]); }
    return seen;
  }, ex.EXAMPLES[name]);
  await page.close();
  for (const [type, vals] of seen) if (type !== "error") add(name, type, vals);
}
await gl.close();

// 2. the heaviest variant of each kind of material
const lights = { K_NAMB: 1, K_NHEMI: 1, K_NDIR: 3, K_NPOINT: 2, K_NSPOT: 3, K_NRECT: 1 };
const base = { u_hasMap: 1, u_hasNormal: 1, u_opaque: 1, u_tone: 4, u_hasEmap: 1, u_receive: 1, K_SHSLOTS: 15, u_hasDisp: 1, K_LIT: 1, K_FOG: 1, ...lights };
const CASES = [
  ["Physical, all maps, sheen, transmission, env, rect", { ...base, u_maps: 31, K_PHYS: 1, K_SHEEN: 1, K_TRANS: 1, K_ENV: 1, K_RECT: 1 }],
  ["Standard with bump, instanced, points of view", { ...base, u_maps: 1 | 2 | 4 | 16 | 128, K_PHYS: 1, K_ENV: 1, K_RECT: 1, u_instanced: 1 }],
  ["Phong, maps, equirect env, exp2 fog", { ...base, u_maps: 1 | 8 | 16, K_PHONG: 1, K_ENV: 2, K_NRECT: 0, K_FOG: 2, u_tone: 6 }],
  ["Lambert, flat, maps, equirect env", { ...base, u_maps: 1 | 128 | 16, K_ENV: 2, K_NRECT: 0, K_FLAT: 1, u_tone: 7 }],
  ["Toon, gradient map", { ...base, u_maps: 1 | 8 | 16 | 64, K_MODE: 1, K_NRECT: 0 }],
  ["Matcap", { ...base, u_maps: 1 | 8 | 32, K_MODE: 4, K_LIT: 0, K_NAMB: 0, K_NHEMI: 0, K_NDIR: 0, K_NPOINT: 0, K_NSPOT: 0, K_NRECT: 0 }],
  ["Normal material", { K_MODE: 2, u_hasNormal: 1 }],
  ["Depth material", { K_MODE: 3 }],
  ["ShadowMaterial", { ...base, u_maps: 0, u_hasMap: 0, u_hasEmap: 0, K_MODE: 5 }],
  ["Basic, map, alpha map, env, dashed", { u_hasMap: 1, u_maps: 1 | 16, K_ENV: 2, K_FOG: 1, u_dashed: 1, u_premult: 1 }],
  ["Points with a map", { u_hasMap: 1, u_points: 1 }],
  ["Sprite", { u_hasMap: 1, u_sprite: 1 }],
  ["Many lights (loops)", { ...base, K_NDIR: 5, K_NPOINT: 5, K_NSPOT: 4, K_NRECT: 0, K_PHYS: 1 }],
  ["Shadow caster pass", { u_hasMap: 1, u_maps: 1, u_instanced: 1, u_hasDisp: 1, K_SHPASS: 1 }],
];
for (const [name, v] of CASES) for (const st of [1, 2, 3]) add(name + " / shadow " + st, st, KEYS.map((k) => v[k] | 0));

// 3. translate, compile, make a pipeline
const programs = [];
for (const v of variants.values()) {
  const out = W.translate(S.main_vertex(v.vals), S.main_fragment(v.type, v.vals));
  programs.push({ name: v.name, key: v.type + ":" + v.vals.join(","), vs: out[0], fs: out[1], layout: out[2] });
}
if (process.env.DUMP) {
  fs.mkdirSync(process.env.DUMP, { recursive: true });
  programs.forEach((p, i) => { fs.writeFileSync(path.join(process.env.DUMP, i + ".vert.wgsl"), p.vs); fs.writeFileSync(path.join(process.env.DUMP, i + ".frag.wgsl"), p.fs); fs.writeFileSync(path.join(process.env.DUMP, i + ".layout"), p.name + "\n" + p.key + "\n" + p.layout); });
}
const gpu = await chromium.launch({ args: ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-webgpu-adapter=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await gpu.newPage();
await page.route("https://wgsl.local/**", (r) => r.fulfill({ contentType: "text/html", body: "<!doctype html>" }));
await page.goto("https://wgsl.local/");
const rows = await page.evaluate(async (programs) => {
  const adapter = await navigator.gpu.requestAdapter();
  const device = await adapter.requestDevice();
  const FORMATS = { float: "float32", vec2: "float32x2", vec3: "float32x3", vec4: "float32x4" };
  const rows = [];
  for (const p of programs) {
    const errs = [];
    const mods = [];
    for (const [stage, code] of [["vertex", p.vs], ["fragment", p.fs]]) {
      const m = device.createShaderModule({ code });
      const info = await m.getCompilationInfo();
      for (const msg of info.messages) if (msg.type === "error") errs.push(`${stage} ${msg.lineNum}:${msg.linePos} ${msg.message} | ${code.split("\n")[msg.lineNum - 1]}`);
      mods.push(m);
    }
    if (!errs.length) {
      // one buffer per attribute, the instance matrix's four columns in one
      const buffers = [], inst = { arrayStride: 64, stepMode: "instance", attributes: [] };
      for (const line of p.layout.split("\n")) {
        const [kind, name, type, loc] = line.split(" ");
        if (kind !== "attr") continue;
        const a = { shaderLocation: +loc, offset: 0, format: FORMATS[type] };
        if (name.startsWith("a_im")) { a.offset = 16 * (+loc - 4); inst.attributes.push(a); } else buffers.push({ arrayStride: 16, attributes: [a] });
      }
      if (inst.attributes.length) buffers.push(inst);
      device.pushErrorScope("validation");
      device.createRenderPipeline({
        layout: "auto",
        vertex: { module: mods[0], entryPoint: "vs_main", buffers },
        fragment: { module: mods[1], entryPoint: "fs_main", targets: [{ format: "rgba8unorm" }] },
        primitive: { topology: "triangle-list" },
        depthStencil: { format: "depth24plus", depthWriteEnabled: true, depthCompare: "less" },
      });
      const e = await device.popErrorScope();
      if (e) errs.push("pipeline: " + e.message.split("\n").slice(0, 4).join(" "));
    }
    rows.push({ name: p.name, errs, notes: p.layout.split("\n").filter((l) => l.startsWith("note ")), lines: p.fs.split("\n").length });
  }
  return rows;
}, programs);
await gpu.close();

let fail = 0;
const notes = new Set();
for (const r of rows) {
  const ok = r.errs.length === 0;
  if (!ok) fail++;
  console.log(`${ok ? "ok  " : "FAIL"} ${String(r.lines).padStart(4)} lines  ${r.name}`);
  for (const n of r.notes) notes.add(n.slice(5));
  for (const e of r.errs.slice(0, 3)) console.log("     " + e);
}
for (const n of notes) console.log("note: " + n);
console.log(`${rows.length - fail}/${rows.length} variants compile to WGSL and make a WebGPU pipeline`);
process.exit(fail ? 1 : 0);
