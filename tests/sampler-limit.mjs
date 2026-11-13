// The main program's variants against the fragment texture-unit limit.
// WebGL2 guarantees only 16 fragment texture units (MAX_TEXTURE_IMAGE_UNITS;
// Apple's Metal ANGLE has exactly 16), and a program whose fragment shader
// keeps more samplers active fails to link: the backend then falls back to
// the software renderer. Each case below is the heaviest variant a kind of
// material can have (every map it accepts, shadows of every type, rect
// area lights, environment, transmission, sheen); the test links it in
// headless Chromium and counts the samplers left active (statically used),
// and checks that no two active samplers of different types share a texture
// unit (same-type samplers may: those materials never set both maps).
//
//   bash scripts/build-js.sh && node tests/sampler-limit.mjs
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import vm from "node:vm";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const ctx = {};
vm.runInNewContext(readFileSync(join(ROOT, "build/rangerthree-core.js"), "utf8") + ";this.R = RangerThreeCore;", ctx);
const S = ctx.R.RenderersShaders;
const KEYS = S.key_names();
const LIMIT = 16;

// js/three-webgl.js's texture units (setUnits, MAPS, SHADOW_UNITS)
const UNITS = { u_map: 0, u_emap: 1, u_dfg: 2, u_alphaMap: 3, u_roughMap: 4, u_metalMap: 5, u_normalMap: 6, u_aoMap: 7,
  u_matcap: 4, u_gradMap: 5, u_bumpMap: 6, u_dispMap: 12, u_ltc: 10, u_envMap: 11, u_transMap: 14,
  u_sh0: 8, u_sh1: 9, u_sh2: 13, u_shc0: 15 };
// u_maps bits: 1 alpha, 2 roughness, 4 metalness, 8 normal, 16 ao, 32 matcap, 64 gradient, 128 bump (not with normal)
const lights = { K_NAMB: 1, K_NHEMI: 1, K_NDIR: 3, K_NPOINT: 2, K_NSPOT: 3, K_NRECT: 1 };
const base = { u_hasMap: 1, u_hasNormal: 1, u_opaque: 1, u_tone: 4, u_hasEmap: 1, u_receive: 1, K_SHSLOTS: 15, u_hasDisp: 1, K_LIT: 1, K_FOG: 1, ...lights };
const CASES = [
  { name: "Physical, all maps, sheen, transmission, env, rect", v: { ...base, u_maps: 1 | 2 | 4 | 8 | 16, K_PHYS: 1, K_SHEEN: 1, K_TRANS: 1, K_ENV: 1, K_RECT: 1 } },
  { name: "Physical, all maps, one directional shadow", v: { ...base, K_SHSLOTS: 1, u_maps: 1 | 2 | 4 | 8 | 16, K_PHYS: 1, K_SHEEN: 1, K_TRANS: 1, K_ENV: 1, K_RECT: 1 } },
  { name: "Standard with bump instead of normal map", v: { ...base, u_maps: 1 | 2 | 4 | 16 | 128, K_PHYS: 1, K_ENV: 1, K_RECT: 1 } },
  { name: "Phong, maps, equirect env", v: { ...base, u_maps: 1 | 8 | 16, K_PHONG: 1, K_ENV: 2, K_NRECT: 0 } },
  { name: "Lambert, maps, equirect env", v: { ...base, u_maps: 1 | 128 | 16, K_ENV: 2, K_NRECT: 0 } },
  { name: "Toon, gradient map", v: { ...base, u_maps: 1 | 8 | 16 | 64, K_MODE: 1, K_NRECT: 0 } },
  { name: "Matcap", v: { ...base, u_maps: 1 | 8 | 32, K_MODE: 4, K_LIT: 0, ...Object.fromEntries(Object.keys(lights).map((k) => [k, 0])) } },
  { name: "ShadowMaterial", v: { ...base, u_maps: 0, u_hasMap: 0, u_hasEmap: 0, K_MODE: 5, K_LIT: 1 } },
  { name: "Basic, map, alpha map, env", v: { u_hasMap: 1, u_maps: 1 | 16, u_hasEmap: 0, K_ENV: 2, K_FOG: 1 } },
  { name: "Shadow caster pass", v: { u_hasMap: 1, u_maps: 1, u_instanced: 1, u_hasDisp: 1, K_SHPASS: 1 } },
];
const programs = [];
for (const c of CASES) for (const st of [1, 2, 3]) {
  const vals = KEYS.map((k) => c.v[k] | 0);
  programs.push({ name: c.name + " / shadow " + st, vs: S.main_vertex(vals), fs: S.main_fragment(st, vals) });
}

const { chromium } = createRequire(import.meta.url)("playwright");
const browser = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage();
const out = await page.evaluate(({ programs, UNITS }) => {
  const gl = document.createElement("canvas").getContext("webgl2");
  const SAMPLERS = { 35678: "2D", 35679: "3D", 35680: "cube", 35682: "2D shadow", 36289: "2D array", 36292: "2D array shadow", 36293: "cube shadow" };
  const max = gl.getParameter(gl.MAX_TEXTURE_IMAGE_UNITS);
  return { max, rows: programs.map((p) => {
    const sh = (t, s) => { const o = gl.createShader(t); gl.shaderSource(o, s); gl.compileShader(o); if (!gl.getShaderParameter(o, gl.COMPILE_STATUS)) throw new Error(p.name + ": " + gl.getShaderInfoLog(o)); return o; };
    const prog = gl.createProgram();
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, p.vs));
    gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, p.fs));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return { name: p.name, error: gl.getProgramInfoLog(prog) };
    const active = [];
    for (let i = 0; i < gl.getProgramParameter(prog, gl.ACTIVE_UNIFORMS); i++) {
      const u = gl.getActiveUniform(prog, i);
      if (SAMPLERS[u.type]) active.push({ name: u.name, type: SAMPLERS[u.type] });
    }
    return { name: p.name, active };
  }) };
}, { programs, UNITS });
await browser.close();

let fail = 0;
for (const r of out.rows) {
  if (r.error) { console.log(`FAIL ${r.name}: link: ${r.error}`); fail++; continue; }
  // u_dispMap is the vertex shader's; the rest count against the fragment limit
  const frag = r.active.filter((a) => a.name !== "u_dispMap");
  const byUnit = {};
  const clash = [];
  for (const a of r.active) {
    const unit = UNITS[a.name];
    if (unit === undefined) clash.push(`${a.name} has no unit`);
    else if (byUnit[unit] && byUnit[unit].type !== a.type) clash.push(`${a.name} (${a.type}) and ${byUnit[unit].name} (${byUnit[unit].type}) on unit ${unit}`);
    else byUnit[unit] = a;
  }
  const ok = frag.length <= LIMIT && clash.length === 0;
  if (!ok) fail++;
  if (process.env.SHOW) console.log("     " + r.active.map((a) => a.name + ":" + a.type).join(" "));
  console.log(`${ok ? "ok  " : "FAIL"} ${String(frag.length).padStart(2)}/${LIMIT} ${r.name}${clash.length ? "  " + clash.join("; ") : ""}`);
}
console.log(`${out.rows.length - fail}/${out.rows.length} variants within ${LIMIT} fragment samplers (this context allows ${out.max})`);
process.exit(fail ? 1 : 0);
