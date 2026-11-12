// Triangulations from the crate's earcut against three.js r185's earcut
// (tmp/three-ref, from parity/tools/fetch-three.sh) for small and hashed
// (> 80 points) outlines, with and without holes.
import fs from "node:fs";
import vm from "node:vm";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ref = (await import(pathToFileURL(path.join(ROOT, "tmp/three-ref/three/src/extras/lib/earcut.js")))).default;
const ctx = vm.createContext({ console, TextEncoder, TextDecoder });
vm.runInContext(fs.readFileSync(path.join(ROOT, "build/rangerthree-core.js"), "utf8") + "\nthis.R = RangerThreeCore;", ctx);
const ours = (d, h) => ctx.R.ExtrasEarcut.triangulate(d, h);
function ring(n, r, cx, cy, wobble, seed) {
  const out = [];
  let s = seed;
  for (let i = 0; i < n; i++) {
    s = (s * 16807) % 2147483647;
    const rr = r * (1 + wobble * ((s / 2147483647) - 0.5));
    const a = (i / n) * Math.PI * 2;
    out.push(cx + rr * Math.cos(a), cy + rr * Math.sin(a));
  }
  return out;
}
const cases = {
  square: [[0, 0, 2, 0, 2, 2, 0, 2], []],
  concave: [[0, 0, 4, 0, 4, 4, 2, 1, 0, 4], []],
  squareHole: [[0, 0, 4, 0, 4, 4, 0, 4, 1, 1, 1, 3, 3, 3, 3, 1], [4]],
  star: [ring(10, 3, 0, 0, 1.2, 7), []],
  circle100: [ring(100, 5, 0, 0, 0, 1), []],
  wobbly200: [ring(200, 5, 0, 0, 0.6, 3), []],
  ringHoles: [[...ring(120, 10, 0, 0, 0.2, 5), ...ring(30, 2, -4, 0, 0, 1).reverse(), ...ring(30, 2, 4, 0, 0, 1).reverse()], [120, 150]],
  collinear: [[0, 0, 1, 0, 2, 0, 3, 0, 3, 3, 0, 3], []],
};
let fail = 0;
for (const [name, [d, h]] of Object.entries(cases)) {
  const a = ref(d, h, 2), b = ours(d.slice(), h.slice());
  const ok = a.length === b.length && a.every((v, i) => v === b[i]);
  if (!ok) fail++;
  console.log(`${ok ? "PASS" : "FAIL"} earcut:${name} (${a.length / 3} triangles${ok ? "" : `, ours ${b.length / 3}`})`);
}
console.log(`earcut: ${Object.keys(cases).length - fail}/${Object.keys(cases).length}`);
process.exit(fail ? 1 : 0);
