// SPDX-License-Identifier: MIT
//
// The page's half of the GL bridge: answers a CErXes script's __hostSync
// (cerxes_host.sync / take, cerxes-gl/src/lib.rs) and plays the commands
// gl-client.js recorded on a real WebGL2 context.
//
//   const host = makeGLHost(gl);          // gl: canvas.getContext("webgl2")
//   imports.cerxes_host = host.imports(() => memory);
export function makeGLHost(gl) {
  const objs = [null];
  const enc = new TextEncoder(), dec = new TextDecoder();
  const constants = {}, methods = [];
  for (let p = Object.getPrototypeOf(gl); p && p !== Object.prototype; p = Object.getPrototypeOf(p)) {
    for (const k of Object.getOwnPropertyNames(p)) {
      if (/^[A-Z0-9_]+$/.test(k) && typeof gl[k] === "number") constants[k] = gl[k];
      else if (k !== "constructor" && typeof Object.getOwnPropertyDescriptor(p, k).value === "function" && !methods.includes(k)) methods.push(k);
    }
  }
  const stats = { commands: 0, syncs: 0, bytes: 0, playMs: 0 };
  const dec1 = (v) => {
    if (v === null || typeof v !== "object") return v;
    if (v.__h !== undefined) return objs[v.__h];
    if (v.__t) return v.__t === "Array" ? v.d : new globalThis[v.__t](v.d);
    return v;
  };
  function play(c) {
    const name = c[0], ret = c[1], args = new Array(c.length - 2);
    for (let i = 2; i < c.length; i++) args[i - 2] = dec1(c[i]);
    if (name === "__resize") { gl.canvas.width = args[0]; gl.canvas.height = args[1]; return null; }
    if (name === "getExtension") return !!gl.getExtension(args[0]);
    if (name === "readPixels") {
      const [x, y, w, h, f, t] = args, out = new Uint8Array(w * h * 4);
      gl.readPixels(x, y, w, h, f, t, out);
      return Array.from(out);
    }
    const r = gl[name].apply(gl, args);
    if (ret) objs[ret] = r;
    return r;
  }
  // an answer as JSON: typed arrays tagged, info objects as plain fields
  function plain(r) {
    if (r === undefined || r === null) return null;
    if (ArrayBuffer.isView(r)) return { __t: r.constructor.name, d: Array.from(r) };
    if (Array.isArray(r)) return r;
    if (typeof r === "object") {
      if (typeof WebGLActiveInfo !== "undefined" && r instanceof WebGLActiveInfo) return { name: r.name, size: r.size, type: r.type };
      if (typeof WebGLShaderPrecisionFormat !== "undefined" && r instanceof WebGLShaderPrecisionFormat) return { rangeMin: r.rangeMin, rangeMax: r.rangeMax, precision: r.precision };
      const k = objs.indexOf(r);
      return k > 0 ? { __h: k } : {};
    }
    return r;
  }
  function answer(text) {
    const kind = text[0];
    stats.bytes += text.length;
    if (kind === "I") return JSON.stringify({ constants, methods });
    const t0 = performance.now();
    let out = "";
    if (kind === "C") {
      const cmds = JSON.parse(text.slice(1));
      for (const c of cmds) play(c);
      stats.commands += cmds.length;
    } else if (kind === "Q") {
      stats.syncs++;
      const r = play(JSON.parse(text.slice(1)));
      out = JSON.stringify(plain(r));
    }
    stats.playMs += performance.now() - t0;
    return out;
  }
  let pending = null;
  return {
    stats,
    imports(memory) {
      return {
        sync(p, n) { pending = enc.encode(answer(dec.decode(new Uint8Array(memory().buffer, p, n)))); return pending.length; },
        take(p) { new Uint8Array(memory().buffer, p, pending.length).set(pending); pending = null; },
      };
    },
  };
}
