// SPDX-License-Identifier: MIT
// The value axis on RangerThree: every probe of value-probes.txt runs on
// `THREE` = js/three.js over build/rangerthree-core.js (the three crate as
// Ranger compiled it), and its answer is compared with three.js r185's
// (goldens/value.json).
//
//   node parity/value.mjs                 in Node
//   node parity/value.mjs --cerxes        in CErXes (CERXES_BIN or scripts/deps.sh's build)
//   node parity/value.mjs --grep=Vector3  only some items; --show prints the misses
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import vm from "node:vm";
import { execFileSync } from "node:child_process";
import { valueProbes, PARITY } from "./tools/probes.mjs";
import { same } from "./tools/same.mjs";

const ROOT = path.resolve(PARITY, "..");
const args = process.argv.slice(2);
const opt = (k) => { const a = args.find((x) => x.startsWith(`--${k}`)); return a ? (a.split("=")[1] ?? true) : null; };

function sources() {
  return [
    "var window = globalThis;",
    fs.readFileSync(path.join(ROOT, "build/rangerthree-core.js"), "utf8"),
    fs.readFileSync(path.join(ROOT, "js/three.js"), "utf8"),
    fs.readFileSync(path.join(ROOT, "js/three-scene.js"), "utf8"),
    fs.readFileSync(path.join(PARITY, "probe-prelude.js"), "utf8"),
  ];
}

function runNode(probes) {
  const ctx = vm.createContext({ console, Float32Array, Uint16Array, Uint32Array, Int8Array, Int16Array, Int32Array, Uint8Array, Uint8ClampedArray, Float64Array, TextEncoder, TextDecoder });
  vm.runInContext(sources().join("\n"), ctx, { filename: "three-parity.js" });
  const out = {};
  for (const p of probes) {
    try { out[p.id] = vm.runInContext(`runProbe(function () { ${p.body} })`, ctx, { timeout: 5000 }); }
    catch (e) { out[p.id] = { ok: false, err: "compile: " + String(e && e.message || e) }; }
  }
  // through JSON as the CErXes run is (NaN and Infinity become null)
  return JSON.parse(JSON.stringify(out));
}

function runCerxes(probes) {
  let bin = process.env.CERXES_BIN;
  if (!bin) {
    bin = execFileSync("bash", ["-c", `source "${ROOT}/scripts/deps.sh" && ensure_cerxes_cli >&2 && echo "$CERXES_BIN"`], { encoding: "utf8" }).trim();
  }
  const src = [
    ...sources(),
    "var __out = {};",
    ...probes.map((p) => `try { __out[${JSON.stringify(p.id)}] = runProbe(new Function(${JSON.stringify(p.body)})); } catch (e) { __out[${JSON.stringify(p.id)}] = { ok: false, err: 'compile: ' + String(e && e.message || e) }; }`),
    "console.log('@@VALUES ' + JSON.stringify(__out));",
  ].join("\n");
  const file = path.join(os.tmpdir(), `rangerthree-values-${process.pid}.js`);
  fs.writeFileSync(file, src);
  let text = "";
  try {
    text = execFileSync(bin, [file], { encoding: "utf8", maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "pipe"], timeout: 600000 });
  } catch (e) {
    text = String(e.stdout || "");
    if (!text.includes("@@VALUES ")) throw new Error("cerxes: " + String(e.stderr || e.message).slice(0, 2000));
  } finally { fs.rmSync(file, { force: true }); }
  return JSON.parse(text.split("\n").find((l) => l.startsWith("@@VALUES ")).slice(9));
}

const golden = JSON.parse(fs.readFileSync(path.join(PARITY, "goldens/value.json"), "utf8")).values;
let probes = valueProbes();
const grep = opt("grep");
if (grep) { const re = new RegExp(grep); probes = probes.filter((p) => re.test(p.id)); }
const engine = opt("cerxes") ? "cerxes" : "node";
const got = engine === "cerxes" ? runCerxes(probes) : runNode(probes);
const byClass = {};
let pass = 0;
const lines = [];
for (const p of probes) {
  const g = golden[p.id], r = got[p.id];
  const ok = !!(g !== undefined && r && r.ok && same(r.v, g));
  const cls = p.id.slice(6).split(".")[0];
  byClass[cls] = byClass[cls] || [0, 0];
  byClass[cls][1]++;
  if (ok) { pass++; byClass[cls][0]++; }
  lines.push(`${ok ? "PASS" : "FAIL"} ${p.id}`);
  if (!ok && opt("show")) {
    console.log(`FAIL ${p.id}\n  want ${String(JSON.stringify(g)).slice(0, 300)}\n  got  ${r && r.ok ? JSON.stringify(r.v).slice(0, 300) : "error: " + (r && r.err)}`);
  }
}
const summary = Object.entries(byClass).map(([k, [a, b]]) => `${k} ${a}/${b}`).join(", ");
console.log(`value (${engine}): ${pass}/${probes.length}\n${summary}`);
if (!grep) {
  fs.mkdirSync(path.join(PARITY, "results"), { recursive: true });
  fs.writeFileSync(path.join(PARITY, "results", `value-${engine}.txt`), `value (${engine}): ${pass}/${probes.length}\n${summary}\n\n${lines.join("\n")}\n`);
}
