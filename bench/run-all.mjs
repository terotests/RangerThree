// The comparison: the playground scenes drawn by the software renderer in
// every engine, frame times and peak memory (resident set of the process):
//   COMPONENTENGINE=../componentengine CERXES_BIN=... node bench/run-all.mjs [W H FRAMES]
// Engines: Rust native, Rust -> wasm32 in Node, the Ranger -> JS build in
// Node's V8, in CErXes (native CLI, register machine) and in CErXes built
// for WebAssembly with the JIT off and on. The checksums of the last frame
// must agree: every engine draws the same picture.
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const [W = "320", H = "240", FRAMES = "20"] = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const CERXES = process.env.CERXES_BIN || path.join(ROOT, "tmp/cargo-cerxes/release/cerxes");
const SCENES = [["torus", "Torus knot"], ["primitives", "Primitives and lights"], ["instancing", "Instancing: a field of cubes"]];
const tmp = fs.mkdtempSync("/tmp/rt-bench-");
// peak RSS of a child, from getrusage
function run(cmd, args, env = {}) {
  const r = spawnSync("python3", ["-c", "import resource,subprocess,sys\np=subprocess.run(sys.argv[1:],capture_output=True,text=True)\nsys.stdout.write(p.stdout)\nsys.stderr.write(p.stderr)\nprint('RSS',resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)", cmd, ...args],
    { encoding: "utf8", env: { ...process.env, ...env }, maxBuffer: 1 << 26 });
  const line = (r.stdout.match(/^BENCH (.*)$/m) || [])[1];
  if (!line) throw new Error(cmd + " " + args.join(" ") + ":\n" + r.stdout.slice(0, 1000) + r.stderr.slice(0, 2000));
  return { ...JSON.parse(line), rssMB: +(Number(r.stdout.match(/^RSS (\d+)/m)[1]) / 1024).toFixed(1) };
}
const rows = [];
for (const [id, name] of SCENES) {
  const js = path.join(tmp, id + ".js"), nr = path.join(tmp, id + "-nr.js");
  spawnSync("node", [path.join(ROOT, "bench/make-bench.mjs"), name, W, H, FRAMES], { stdio: ["ignore", fs.openSync(js, "w"), "inherit"] });
  spawnSync("node", [path.join(ROOT, "bench/make-bench.mjs"), name, W, H, FRAMES, "--no-render"], { stdio: ["ignore", fs.openSync(nr, "w"), "inherit"] });
  const engines = [
    ["Rust natiivi", () => run(path.join(ROOT, "target/release/examples/soft_bench"), [id, W, H, FRAMES])],
    ["Rust → WASM (V8)", () => run("node", ["--no-warnings", path.join(ROOT, "bench/run-wasm.mjs"), id, W, H, FRAMES])],
    ["JS V8 (Node)", () => run("node", [js])],
    ["CErXes natiivi", () => run(CERXES, [js], { CER_REGS: "1" })],
    ["CErXes WASM", () => run("node", [path.join(ROOT, "bench/run-cerxes-wasm.mjs"), js])],
    ["CErXes WASM + JIT", () => run("node", [path.join(ROOT, "bench/run-cerxes-wasm.mjs"), js, "--jit"])],
    ["JS V8, ei piirtoa", () => run("node", [nr])],
    ["CErXes natiivi, ei piirtoa", () => run(CERXES, [nr], { CER_REGS: "1" })],
    ["CErXes WASM + JIT, ei piirtoa", () => run("node", [path.join(ROOT, "bench/run-cerxes-wasm.mjs"), nr, "--jit"])],
  ];
  for (const [engine, f] of engines) {
    const r = f();
    rows.push({ scene: id, engine, ...r });
    console.log(`${id.padEnd(11)} ${engine.padEnd(30)} median ${String(r.median).padStart(8)} ms  mean ${String(r.mean).padStart(8)} ms  setup ${String(r.setup).padStart(7)} ms  rss ${String(r.rssMB).padStart(6)} MB  sum ${r.checksum}`);
  }
}
const out = path.join(ROOT, "bench/results.json");
fs.writeFileSync(out, JSON.stringify({ date: new Date().toISOString(), size: [+W, +H], frames: +FRAMES, rows }, null, 1) + "\n");
console.log("-> " + out);
