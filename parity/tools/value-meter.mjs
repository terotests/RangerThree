// SPDX-License-Identifier: MIT
// Value items on the .tsx kit: desktop/uix/js/tsx.js (the THREE a document
// sees) and the probes run in CErXes, the engine the UI extension host
// runs documents with; answers {id: {ok, v | err}}.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { valueProbes, PARITY } from "./probes.mjs";
const ROOT = path.resolve(PARITY, "../../..");
export function cerxesBin() {
  if (process.env.CERXES_BIN) return process.env.CERXES_BIN;
  const bin = path.join(ROOT, "tmp/cargo-cerxes/release/cerxes");
  if (!fs.existsSync(bin)) {
    execFileSync("cargo", ["build", "--release", "--bin", "cerxes"], {
      cwd: path.join(ROOT, ".deps/componentengine/cerxes"), stdio: "inherit",
      env: { ...process.env, CARGO_TARGET_DIR: path.join(ROOT, "tmp/cargo-cerxes") },
    });
  }
  return bin;
}
// what THREE is: the kit's (the meter), or with THREE_PARITY_SELF=1 the
// reference three.js itself run in CErXes (checks the harness: every item
// should pass, apart from what CErXes itself lacks)
function implementation() {
  if (process.env.THREE_PARITY_SELF) {
    const ref = process.env.THREE_REF || path.join(ROOT, "tmp/three-ref/three");
    return ["var module = { exports: {} }, exports = module.exports;", fs.readFileSync(path.join(ref, "build/three.cjs"), "utf8"), "var THREE = module.exports;"];
  }
  return [fs.readFileSync(path.join(ROOT, "desktop/uix/js/tsx.js"), "utf8"), "var THREE = window.THREE;"];
}
export function runValues() {
  const probes = valueProbes();
  const src = [
    "var window = globalThis;",
    ...implementation(),
    fs.readFileSync(path.join(PARITY, "probe-prelude.js"), "utf8"),
    "var __out = {};",
    // each probe compiled on its own, so one the engine cannot read fails alone
    ...probes.map((p) => `try { __out[${JSON.stringify(p.id)}] = runProbe(new Function(${JSON.stringify(p.body)})); } catch (e) { __out[${JSON.stringify(p.id)}] = { ok: false, err: 'compile: ' + String(e && e.message || e) }; }`),
    "console.log('@@VALUES ' + JSON.stringify(__out));",
  ].join("\n");
  const file = path.join(os.tmpdir(), `three-parity-${process.pid}.js`);
  fs.writeFileSync(file, src);
  let text = "";
  try {
    text = execFileSync(cerxesBin(), [file], { encoding: "utf8", maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "pipe"], timeout: 600000 });
  } catch (e) {
    text = String(e.stdout || "");
    if (!text.includes("@@VALUES ")) throw new Error("cerxes: " + String(e.stderr || e.message).slice(0, 2000));
  } finally { fs.rmSync(file, { force: true }); }
  const line = text.split("\n").find((l) => l.startsWith("@@VALUES "));
  return JSON.parse(line.slice(9));
}
// equal within float tolerance (three keeps attributes in Float32Array)
export function same(a, b) {
  if (typeof a === "number" && typeof b === "number") return Math.abs(a - b) <= 1e-5 * Math.max(1, Math.abs(b));
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((x, i) => same(x, b[i]));
  if (a && b && typeof a === "object" && typeof b === "object") {
    const ka = Object.keys(a).sort(), kb = Object.keys(b).sort();
    return same(ka, kb) && ka.every((k) => same(a[k], b[k]));
  }
  return a === b;
}
