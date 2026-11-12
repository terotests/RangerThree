// Runs the soft_bench example built for wasm32-wasip1 in Node's
// WebAssembly (V8 Liftoff + TurboFan), with Node's WASI:
//   node bench/run-wasm.mjs <scene> [W H FRAMES]
import fs from "fs";
import path from "path";
import { WASI } from "node:wasi";
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const file = process.env.SOFT_BENCH_WASM || path.join(ROOT, "target/wasm32-wasip1/release/examples/soft_bench.wasm");
const wasi = new WASI({ version: "preview1", args: ["soft_bench", ...process.argv.slice(2)], env: {} });
const t0 = performance.now();
const mod = await WebAssembly.compile(fs.readFileSync(file));
const inst = await WebAssembly.instantiate(mod, wasi.getImportObject());
const t1 = performance.now();
wasi.start(inst);
console.error(`compile+instantiate ${(t1 - t0).toFixed(1)} ms, wasm memory ${(inst.exports.memory.buffer.byteLength / 1048576).toFixed(1)} MB`);
