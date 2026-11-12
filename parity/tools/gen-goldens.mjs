// SPDX-License-Identifier: MIT
// The goldens: each value probe run on three.js itself (the pinned
// reference, tools/fetch-three.sh; never shipped) -> goldens/value.json.
//
//   node web/three/parity/tools/gen-goldens.mjs
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { valueProbes, PARITY } from "./probes.mjs";
const REF = process.env.THREE_REF || path.resolve(PARITY, "../../../tmp/three-ref/three");
const T = await import(path.join(REF, "build/three.module.js"));
const ctx = { THREE: { ...T }, console, Float32Array, Uint8Array, Uint16Array, Int8Array, Uint32Array, Math, JSON };
vm.createContext(ctx);
vm.runInContext(fs.readFileSync(path.join(PARITY, "probe-prelude.js"), "utf8"), ctx);
const gold = {}, bad = [];
const warn = console.warn; console.warn = () => {};
for (const p of valueProbes()) {
  const r = vm.runInContext(`runProbe(function () { ${p.body} })`, ctx);
  if (!r.ok) bad.push(`${p.id}: ${r.err}`);
  else gold[p.id] = r.v;
}
console.warn = warn;
fs.mkdirSync(path.join(PARITY, "goldens"), { recursive: true });
fs.writeFileSync(path.join(PARITY, "goldens/value.json"), JSON.stringify({ three: T.REVISION, values: gold }, null, 0) + "\n");
console.log(`goldens: ${Object.keys(gold).length} values from three.js r${T.REVISION}${bad.length ? `, ${bad.length} probes throw on three.js itself:` : ""}`);
for (const b of bad) console.log("  " + b);
process.exit(bad.length ? 1 : 0);
