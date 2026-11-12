// SPDX-License-Identifier: MIT
// value-probes.txt -> [{id, body}]: body is a function body that answers
// the probe's value (see the file's head for the two forms).
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
const HERE = path.dirname(fileURLToPath(import.meta.url));
export const PARITY = path.resolve(HERE, "..");
const VARIANT = /(YXZ|ZYX|XYZ|Hex3|Rgba|Number)$/;
function lit(tok) {
  if (tok.startsWith("n:")) return String(Number(tok.slice(2)));
  if (tok.startsWith("s:")) return JSON.stringify(tok.slice(2));
  if (tok.startsWith("b:")) return tok.slice(2) === "true" ? "true" : "false";
  return `P.${tok}()`;
}
export function valueProbes() {
  const out = [];
  const seen = new Set();
  for (const raw of fs.readFileSync(path.join(PARITY, "value-probes.txt"), "utf8").split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    let id, body;
    const eq = line.indexOf(" = ");
    if (eq > 0 && !line.slice(0, eq).includes(" ")) {
      id = line.slice(0, eq);
      body = `return (${line.slice(eq + 3)});`;
    } else {
      const parts = line.split(/\s+/);
      id = parts[0];
      const [cls, member] = id.split(".");
      const method = member.replace(VARIANT, "") || member;
      const args = parts.slice(1).map(lit).join(", ");
      body = `var r = R.${cls}(); var x = r.${method}(${args}); return x === r ? ['<self>', r] : [x, r];`;
    }
    if (seen.has(id)) throw new Error(`value-probes.txt: ${id} twice`);
    seen.add(id);
    out.push({ id: "value:" + id, body });
  }
  return out;
}
