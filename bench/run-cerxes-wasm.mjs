// Runs a script from bench/make-bench.mjs in CErXes built for wasm32-wasip1
// (componentengine's playground/cerxes-wasm with the `wjit` feature), in
// Node's WebAssembly, with the baseline JIT off or on:
//   COMPONENTENGINE=../componentengine node bench/run-cerxes-wasm.mjs bench.js [--jit] [--min=64]
// The JIT host is componentengine's cerxes/bench/wjit.mjs's, trimmed.
import fs from "node:fs";
import path from "node:path";
const CE = path.resolve(process.env.COMPONENTENGINE || "../componentengine");
const WASM = process.env.CERXES_WASM || path.join(CE, "playground/cerxes-wasm/target-jit/wasm32-wasip1/release/cerxes_wasm.wasm");
const { makeWasi } = await import(path.join(CE, "playground/src/wasi.js"));
const args = process.argv.slice(2);
const file = args.find((a) => !a.startsWith("--"));
const jitOn = args.includes("--jit");
const min = Number((args.find((a) => a.startsWith("--min=")) || "--min=64").slice(6));
const enc = new TextEncoder(), dec = new TextDecoder();
const out = [];
const wasi = makeWasi((_fd, line) => out.push(line));
let x = null;
const NAMES = ["get_elem", "set_elem", "get_prop", "set_prop", "op", "call", "slow", "ret"];
const jit = {
  modules: 0, compileMs: 0, rt: null, helpers: null,
  release(base, n) { const t = x.__indirect_function_table; for (let k = 0; k < n; k++) t.set(base + k, jit.rt.gone); },
  install(ptr, len) {
    const t0 = performance.now();
    const m = new WebAssembly.Module(new Uint8Array(x.memory.buffer, ptr, len).slice());
    const t = x.__indirect_function_table;
    if (!jit.helpers) jit.helpers = NAMES.map((h, k) => t.get(x.cerxes_jit_helper(k)));
    const env = { memory: x.memory, table: t };
    NAMES.forEach((h, k) => (env[h] = jit.helpers[k]));
    const inst = new WebAssembly.Instance(m, jit.rt ? { env, rt: jit.rt } : { env });
    if (inst.exports.look) { jit.rt = inst.exports; jit.compileMs += performance.now() - t0; return 0; }
    let n = 0;
    while (inst.exports["f" + n]) n++;
    const idx = t.grow(n);
    for (let k = 0; k < n; k++) t.set(idx + k, inst.exports["f" + k]);
    jit.modules++;
    jit.compileMs += performance.now() - t0;
    return idx;
  },
};
const t0 = performance.now();
const mod = new WebAssembly.Module(fs.readFileSync(WASM));
const inst = new WebAssembly.Instance(mod, { ...wasi.imports, cerxes_jit: { install: (p, n) => jit.install(p, n), release: (b, n) => jit.release(b, n) } });
x = inst.exports;
wasi.setMemory(x.memory);
if (x._initialize) x._initialize();
const e = x.cx_new();
x.cerxes_jit_mask(0, 0);
x.cx_jit(e, jitOn ? 1 : 0, min);
const src = enc.encode(fs.readFileSync(file, "utf8"));
const p = x.cx_alloc(src.length);
new Uint8Array(x.memory.buffer, p, src.length).set(src);
const t1 = performance.now();
const failed = x.cx_eval(e, p, src.length);
const t2 = performance.now();
const text = (q, n) => dec.decode(new Uint8Array(x.memory.buffer, q, n));
const output = text(x.cx_output_ptr(), x.cx_output_len()) + "\n" + out.join("\n");
if (failed) console.error("failed: " + text(x.cx_result_ptr(), x.cx_result_len()));
const line = output.split("\n").find((l) => l.startsWith("BENCH "));
console.log(line || output.slice(0, 2000));
console.error(`engine ${(t1 - t0).toFixed(0)} ms, script ${(t2 - t1).toFixed(0)} ms, wasm memory ${(x.memory.buffer.byteLength / 1048576).toFixed(1)} MB, jit ${jitOn ? "on" : "off"} modules ${jit.modules} compile ${jit.compileMs.toFixed(0)} ms`);
