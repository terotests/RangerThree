// Cold start: the first frame of each playground example in a fresh page,
// with the shader programs it compiles. For each example and engine:
//   setup   the scene script (renderer, geometry, materials)
//   first   the first frame, readPixels included (compiles every program)
//   second  the second frame (programs cached)
//   progs   programs linked, and the shader source they compiled (KB)
//   gl      the time spent in the compile / link / status calls
// Engines: `rt` RangerThree's WebGL2 backend, `three` the original three.js
// r185 (tmp/three-ref, parity/tools/fetch-three.sh) when present.
// SwiftShader compiles on the CPU, so absolute times are not a real GPU's;
// the program counts and source sizes are the same anywhere.
//
//   bash scripts/build-js.sh && node bench/cold-start.mjs [W H]
//   SCENES="Torus knot|..." ENGINES=rt,three JSON=out.json
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const [W = "320", H = "240"] = process.argv.slice(2);
const ctx = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, "playground/examples.js"), "utf8") + ";this.EXAMPLES=EXAMPLES;", ctx);
const SCENES = process.env.SCENES ? process.env.SCENES.split("|") : Object.keys(ctx.EXAMPLES);
const LIB = ["build/rangerthree-core.js", "js/three.js", "js/three-scene.js", "js/three-curves.js", "js/three-renderer.js", "js/three-webgl.js"]
  .map((f) => fs.readFileSync(path.join(ROOT, f), "utf8")).join("\n;\n");
const THREE_REF = path.join(ROOT, "tmp/three-ref/three/build/three.cjs");
const THREE_LIB = fs.existsSync(THREE_REF) ? "var module = { exports: {} }, exports = module.exports;\n" + fs.readFileSync(THREE_REF, "utf8") + "\n;var THREE = module.exports;" : "";
const ENGINES = (process.env.ENGINES || (THREE_LIB ? "rt,three" : "rt")).split(",");

const PAGE = `<!doctype html><canvas id="c" width="${W}" height="${H}"></canvas><script>
window.run = function (lib, code) {
  const canvas = document.getElementById("c");
  const gl = canvas.getContext("webgl2", { antialias: false, preserveDrawingBuffer: true });
  // time spent in the calls that compile and link (the driver may defer
  // the work to the first status query or draw, so all of them count)
  const P = WebGL2RenderingContext.prototype, st = { ms: 0, progs: 0, kb: 0 };
  for (const n of ["shaderSource", "compileShader", "linkProgram", "getShaderParameter", "getProgramParameter", "getUniformLocation", "useProgram"]) {
    const f = P[n];
    P[n] = function () {
      const a = performance.now(); const r = f.apply(this, arguments); st.ms += performance.now() - a;
      if (n === "linkProgram") st.progs++;
      if (n === "shaderSource") st.kb += arguments[1].length / 1024;
      return r;
    };
  }
  const px = new Uint8Array(4);
  let loop = null;
  const t0 = performance.now();
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: false });
  renderer.setSize(${W}, ${H}, false);
  renderer.setAnimationLoop = (fn) => { loop = fn; };
  window.renderer = renderer;
  window.orbit = (camera, target) => camera.lookAt(target);
  (0, eval)("(function(){" + code + "\\n})()");
  const setup = performance.now() - t0;
  const glSetup = st.ms;
  const frame = (t) => { const a = performance.now(); loop(t); gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px); return performance.now() - a; };
  const first = frame(1000), glFirst = st.ms - glSetup, progs = st.progs, kb = st.kb;
  const second = frame(1016);
  return { setup: +setup.toFixed(1), first: +first.toFixed(1), second: +second.toFixed(1), gl: +(glSetup + glFirst).toFixed(1), progs, kb: +kb.toFixed(0) };
};
window.ready = true;
</script>`;

const browser = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const rows = [];
const tot = {};
console.log(`${"example".padEnd(34)} ${ENGINES.map((e) => `${e.padStart(5)}: first  2nd progs   KB   gl`).join("   ")}`);
for (const name of SCENES) {
  const line = [];
  for (const eng of ENGINES) {
    const page = await browser.newPage();
    let err = null;
    page.on("pageerror", (e) => { err = e.message; });
    await page.route("http://bench.local/**", (r) => r.fulfill({ contentType: "text/html", body: PAGE }));
    await page.goto("http://bench.local/");
    await page.waitForFunction(() => window.ready);
    await page.evaluate((lib) => (0, eval)(lib), eng === "rt" ? LIB : THREE_LIB);
    let r;
    try { r = await page.evaluate(([c]) => window.run(null, c), [ctx.EXAMPLES[name]]); } catch (e) { r = { error: String(e.message).split("\n")[0] }; }
    await page.close();
    rows.push({ example: name, engine: eng, ...r });
    if (r.error) { line.push(`${eng.padStart(5)}: ${r.error.slice(0, 40)}`); continue; }
    const t = tot[eng] || (tot[eng] = { first: 0, progs: 0, kb: 0, gl: 0, n: 0 });
    t.first += r.first; t.progs += r.progs; t.kb += r.kb; t.gl += r.gl; t.n++;
    line.push(`${eng.padStart(5)}: ${String(r.first).padStart(5)} ${String(r.second).padStart(4)} ${String(r.progs).padStart(5)} ${String(r.kb).padStart(4)} ${String(r.gl).padStart(5)}`);
  }
  console.log(`${name.slice(0, 34).padEnd(34)} ${line.join("   ")}`);
}
await browser.close();
for (const [e, t] of Object.entries(tot)) console.log(`${e}: ${t.n} examples, first frames ${t.first.toFixed(0)} ms, ${t.progs} programs, ${t.kb} KB source, ${t.gl.toFixed(0)} ms in compile/link calls`);
if (process.env.JSON) fs.writeFileSync(process.env.JSON, JSON.stringify({ date: new Date().toISOString(), size: [+W, +H], rows }, null, 1) + "\n");
