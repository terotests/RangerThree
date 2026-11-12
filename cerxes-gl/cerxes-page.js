// SPDX-License-Identifier: MIT
//
// Loads cerxes_gl.wasm in a page (or a worker): the few WASI calls it
// makes, the baseline JIT's host (as componentengine's wjit host does it)
// and the GL bridge (gl-host.js) on the given WebGL2 context.
//
//   const cx = await loadCerxes(bytes, gl, { jit: true });
//   cx.eval(source); cx.call("__frame", "16.7");
import { makeGLHost } from "./gl-host.js";

export async function loadCerxes(bytes, gl, opts = {}) {
  let x = null;
  const mem = () => x.memory;
  const dec = new TextDecoder(), enc = new TextEncoder();
  const out = [];
  let line = "";
  const wasi = {
    fd_write(fd, iovs, n, nw) {
      const v = new DataView(mem().buffer);
      let total = 0;
      for (let i = 0; i < n; i++) {
        const p = v.getUint32(iovs + i * 8, true), l = v.getUint32(iovs + i * 8 + 4, true);
        line += dec.decode(new Uint8Array(mem().buffer, p, l));
        total += l;
      }
      let k;
      while ((k = line.indexOf("\n")) >= 0) { out.push(line.slice(0, k)); line = line.slice(k + 1); }
      v.setUint32(nw, total, true);
      return 0;
    },
    clock_time_get(id, _p, o) { new DataView(mem().buffer).setBigUint64(o, BigInt(Math.round((id === 0 ? Date.now() : performance.now()) * 1e6)), true); return 0; },
    random_get(p, n) { crypto.getRandomValues(new Uint8Array(mem().buffer, p, n)); return 0; },
    environ_sizes_get(c, s) { const v = new DataView(mem().buffer); v.setUint32(c, 0, true); v.setUint32(s, 0, true); return 0; },
    environ_get() { return 0; },
    proc_exit(c) { throw new Error("exit " + c); },
  };
  const NAMES = ["get_elem", "set_elem", "get_prop", "set_prop", "op", "call", "slow", "ret"];
  const jit = {
    modules: 0, rt: null, helpers: null,
    release(base, n) { const t = x.__indirect_function_table; for (let k = 0; k < n; k++) t.set(base + k, jit.rt.gone); },
    install(ptr, len) {
      const m = new WebAssembly.Module(new Uint8Array(x.memory.buffer, ptr, len).slice());
      const t = x.__indirect_function_table;
      if (!jit.helpers) jit.helpers = NAMES.map((h, k) => t.get(x.cerxes_jit_helper(k)));
      const env = { memory: x.memory, table: t };
      NAMES.forEach((h, k) => (env[h] = jit.helpers[k]));
      const inst = new WebAssembly.Instance(m, jit.rt ? { env, rt: jit.rt } : { env });
      if (inst.exports.look) { jit.rt = inst.exports; return 0; }
      let n = 0;
      while (inst.exports["f" + n]) n++;
      const idx = t.grow(n);
      for (let k = 0; k < n; k++) t.set(idx + k, inst.exports["f" + k]);
      jit.modules++;
      return idx;
    },
  };
  const host = makeGLHost(gl);
  const { instance } = await WebAssembly.instantiate(bytes, {
    wasi_snapshot_preview1: wasi,
    cerxes_host: host.imports(mem),
    cerxes_jit: { install: (p, n) => jit.install(p, n), release: (b, n) => jit.release(b, n) },
  });
  x = instance.exports;
  if (x._initialize) x._initialize();
  const e = x.cx_new();
  x.cerxes_jit_mask(0, 0);
  x.cx_jit(e, opts.jit ? 1 : 0, opts.min || 64);
  const str = (s) => { const b = enc.encode(s); const p = x.cx_alloc(b.length); new Uint8Array(x.memory.buffer, p, b.length).set(b); return [p, b.length]; };
  const result = () => dec.decode(new Uint8Array(x.memory.buffer, x.cx_result_ptr(), x.cx_result_len()));
  const output = () => dec.decode(new Uint8Array(x.memory.buffer, x.cx_output_ptr(), x.cx_output_len()));
  return {
    host, jit, out,
    memoryMB: () => x.memory.buffer.byteLength / 1048576,
    eval(src) {
      const [p, n] = str(src);
      const failed = x.cx_eval(e, p, n);
      x.cx_free(p, n);
      const o = output();
      if (o) out.push(o);
      if (failed) throw new Error(result());
      return result();
    },
    call(name, arg = "") {
      const [np, nn] = str(name), [ap, an] = str(arg);
      const failed = x.cx_call(e, np, nn, ap, an);
      x.cx_free(np, nn); x.cx_free(ap, an);
      const o = output();
      if (o) out.push(o);
      if (failed) throw new Error(result());
      return result();
    },
  };
}
