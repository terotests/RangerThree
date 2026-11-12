// The GPU path: the same playground scenes drawn by RangerThree's WebGL2
// backend (js/three-webgl.js) in headless Chromium, driven by
//   v8          the page's own engine, on the canvas's WebGL2 context
//   cerxes      the same scripts in CErXes (cerxes-gl wasm, interpreter),
//               WebGL2 through the GL bridge (cerxes-gl/gl-client.js ->
//               __hostSync -> gl-host.js -> the canvas's context)
//   cerxes-jit  as cerxes, with CErXes's baseline JIT
// Every frame ends with a 1-pixel readPixels so the GPU's work is counted;
// `script` is the frame's call alone (the engine's work and the GL calls
// issued, before the GPU finishes).
//
//   CERXES_GL_WASM=.../cerxes_gl.wasm node bench/gl-bench.mjs [W H FRAMES]
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const WASM = process.env.CERXES_GL_WASM || path.join(ROOT, "cerxes-gl/target/wasm32-wasip1/release/cerxes_gl.wasm");
const [W = "320", H = "240", FRAMES = "60"] = process.argv.slice(2);
const SCENES = (process.env.SCENES || "Torus knot|Primitives and lights|Instancing: a field of cubes").split("|");
const MODES = (process.env.MODES || "v8,cerxes,cerxes-jit").split(",");
const ctx = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, "playground/examples.js"), "utf8") + ";this.EXAMPLES=EXAMPLES;", ctx);
const LIB = ["build/rangerthree-core.js", "js/three.js", "js/three-scene.js", "js/three-curves.js", "js/three-renderer.js", "js/three-webgl.js"]
  .map((f) => fs.readFileSync(path.join(ROOT, f), "utf8")).join("\n;\n");
const CLIENT = fs.readFileSync(path.join(ROOT, "cerxes-gl/gl-client.js"), "utf8");
function sceneScript(code, cerxes) {
  return `${cerxes ? CLIENT : ""}
var renderer = new THREE.WebGLRenderer({ canvas: ${cerxes ? `__cerxesGL.canvas(${W}, ${H})` : "document.getElementById('c')"}, antialias: false });
renderer.setSize(${W}, ${H}, false);
var __loop = null;
renderer.setAnimationLoop = function (fn) { __loop = fn; };
function orbit(camera, target) { camera.lookAt(target); }
(function () {
${code}
})();
function __frame(t) { __loop(+t); ${cerxes ? "__cerxesGL.flush();" : ""} return renderer.backend; }
`;
}
const PAGE = `<!doctype html><canvas id="c" width="${W}" height="${H}"></canvas><script type="module">
import { loadCerxes } from "/cerxes-gl/cerxes-page.js";
window.runBench = async function (mode, lib, scene, frames) {
  const canvas = document.getElementById("c");
  const gl = canvas.getContext("webgl2", { antialias: false, preserveDrawingBuffer: true });
  const px = new Uint8Array(4);
  let frame, cx = null, backend = "";
  const t0 = performance.now();
  if (mode === "v8") {
    (0, eval)(lib + "\\n;\\n" + scene);
    frame = (t) => window.__frame(t);
  } else {
    const bytes = await (await fetch("/cerxes_gl.wasm")).arrayBuffer();
    cx = await loadCerxes(bytes, gl, { jit: mode === "cerxes-jit" });
    cx.eval(lib + "\\n;\\n" + scene);
    frame = (t) => cx.call("__frame", String(t));
  }
  const setup = performance.now() - t0;
  const times = [], play = [], cpu = [];
  for (let i = 0; i < frames; i++) {
    const p0 = cx ? cx.host.stats.playMs : 0;
    const a = performance.now();
    backend = frame(1000 + i * (1000 / 60));
    cpu.push(performance.now() - a);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
    times.push(performance.now() - a);
    play.push(cx ? cx.host.stats.playMs - p0 : 0);
  }
  const all = new Uint8Array(canvas.width * canvas.height * 4);
  gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, all);
  let sum = 0;
  for (let j = 0; j < all.length; j += 7) sum = (sum * 31 + all[j]) % 1000000007;
  const med = (a) => { const s = a.slice().sort((x, y) => x - y); return s[s.length >> 1]; };
  const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
  return { backend, setup: +setup.toFixed(1), first: +times[0].toFixed(2), median: +med(times).toFixed(2), mean: +mean(times).toFixed(2),
    medianCpu: +med(cpu).toFixed(2), medianHost: +med(play).toFixed(2), checksum: sum, wasmMB: cx ? +cx.memoryMB().toFixed(1) : null,
    jitModules: cx ? cx.jit.modules : null, heapMB: performance.memory ? +(performance.memory.usedJSHeapSize / 1048576).toFixed(1) : null,
    bridge: cx ? { commandsPerFrame: Math.round(cx.host.stats.commands / (frames + 1)), kBPerFrame: +(cx.host.stats.bytes / 1024 / (frames + 1)).toFixed(1), syncs: cx.host.stats.syncs } : null,
    out: cx ? cx.out.join("\\n").slice(0, 500) : "" };
};
window.ready = true;
</script>`;
const browser = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-precise-memory-info"] });
const rows = [];
for (const name of SCENES) {
  for (const mode of MODES) {
    const page = await browser.newPage();
    page.on("pageerror", (e) => console.error("pageerror", e.message));
    await page.route("http://bench.local/**", (route) => {
      const u = new URL(route.request().url()).pathname;
      if (u === "/") return route.fulfill({ contentType: "text/html", body: PAGE });
      if (u === "/cerxes_gl.wasm") return route.fulfill({ contentType: "application/wasm", body: fs.readFileSync(WASM) });
      return route.fulfill({ contentType: "text/javascript", body: fs.readFileSync(path.join(ROOT, u)) });
    });
    await page.goto("http://bench.local/");
    await page.waitForFunction(() => window.ready);
    const r = await page.evaluate(([m, lib, sc, f]) => window.runBench(m, lib, sc, f), [mode, LIB, sceneScript(ctx.EXAMPLES[name], mode !== "v8"), +FRAMES]);
    rows.push({ scene: name, mode, ...r });
    console.log(`${name.slice(0, 12).padEnd(12)} ${mode.padEnd(10)} ${r.backend.padEnd(8)} median ${String(r.median).padStart(7)} ms, script ${String(r.medianCpu).padStart(6)} ms (host GL ${String(r.medianHost).padStart(6)} ms)  mean ${String(r.mean).padStart(7)}  setup ${String(r.setup).padStart(7)} ms  wasm ${r.wasmMB} MB  sum ${r.checksum} ${r.bridge ? JSON.stringify(r.bridge) : ""}`);
    if (r.out) console.log("   out: " + r.out);
    await page.close();
  }
}
await browser.close();
fs.writeFileSync(path.join(ROOT, "bench/gl-results.json"), JSON.stringify({ date: new Date().toISOString(), size: [+W, +H], frames: +FRAMES, rows }, null, 1) + "\n");
