// Writes one self-contained script that runs a playground example with the
// software renderer for a number of frames and prints the frame times:
//   node bench/make-bench.mjs "<example name>" W H FRAMES [--no-render] > out.js
// The same file runs in Node (V8) and in CErXes (`cerxes out.js`).
import fs from "fs";
import path from "path";
import vm from "vm";
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const args = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const [name, W = "320", H = "240", FRAMES = "30"] = args;
// --no-render: the frame's JavaScript alone (scene.updateMatrixWorld in
// place of drawing), what stays in the engine when a native or GPU
// renderer draws
const noRender = process.argv.includes("--no-render");
const ctx = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, "playground/examples.js"), "utf8") + ";this.EXAMPLES=EXAMPLES;", ctx);
const code = ctx.EXAMPLES[name];
if (!code) { console.error("no example " + name + "; have: " + Object.keys(ctx.EXAMPLES).join(" | ")); process.exit(1); }
const parts = ["build/rangerthree-core.js", "js/three.js", "js/three-scene.js", "js/three-curves.js", "js/three-renderer.js"]
  .map((f) => fs.readFileSync(path.join(ROOT, f), "utf8"));
const now = "(typeof performance !== 'undefined' && performance.now ? performance.now() : Date.now())";
const out = `var __t0 = ${now};
${parts.join("\n;\n")}
;var __tLoad = ${now};
var __W = ${W | 0}, __H = ${H | 0}, __FRAMES = ${FRAMES | 0};
var renderer = new THREE.WebGLRenderer({ software: true });
renderer.setSize(__W, __H);
renderer._present = function () {};
${noRender ? "renderer.render = function (s) { s.updateMatrixWorld(); };" : ""}
var __loop = null;
renderer.setAnimationLoop = function (fn) { __loop = fn; };
function orbit(camera, target) { camera.lookAt(target); }
(function () {
${code}
})();
var __tSetup = ${now};
var __times = [];
for (var __i = 0; __i < __FRAMES; __i++) {
  var __a = ${now};
  __loop(1000 + __i * (1000 / 60));
  __times.push(${now} - __a);
}
var __px = renderer._soft.pixels, __sum = 0;
for (var __j = 0; __j < __px.length; __j += 7) __sum = (__sum * 31 + __px[__j]) % 1000000007;
var __s = __times.slice().sort(function (a, b) { return a - b; });
var __tot = 0; for (var __k = 0; __k < __times.length; __k++) __tot += __times[__k];
console.log("BENCH " + JSON.stringify({ load: +(__tLoad - __t0).toFixed(1), setup: +(__tSetup - __tLoad).toFixed(1),
  first: +__times[0].toFixed(2), median: +__s[__s.length >> 1].toFixed(2), mean: +(__tot / __times.length).toFixed(2),
  tris: renderer.info.render.triangles, checksum: __sum }));
`;
process.stdout.write(out);
