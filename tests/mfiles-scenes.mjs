// Runs MFilesAPI's Three parity scenes (web/three/parity/scenes.mjs drawn by its
// tools/r3f-ref.js R3F rules) with three.js r185 and with this checkout's
// WebGLRenderer, all on one reused renderer as the meter does, and prints
// the mean difference per scene (0..255, RGBA), worst first.
//
//   MF=<MFilesAPI checkout> node tests/mfiles-scenes.mjs [--grep=re] [--png=dir]
import fs from "node:fs"; import zlib from "node:zlib"; import path from "node:path"; import { createRequire } from "node:module";
const ROOT = path.join(path.dirname(new URL(import.meta.url).pathname), "..");
const MF = process.env.MF, RT = process.env.RT || ROOT, REF = process.env.REF || path.join(ROOT, "tmp/three-ref/three");
if (!MF) { console.error("set MF to an MFilesAPI checkout"); process.exit(2); }
const { SCENE_ITEMS, TEXTURES, texBytes, W, H } = await import(path.join(MF, "web/three/parity/scenes.mjs"));
const arg = (k, d) => (process.argv.find((a) => a.startsWith(`--${k}=`)) || "").split("=").slice(1).join("=") || d;
const grep = new RegExp(arg("grep", "."));
const parts = ["build/rangerthree-core.js", "js/three.js", "js/three-scene.js", "js/three-curves.js", "js/three-renderer.js", "js/three-webgl.js"];
const bundle = parts.map((f) => fs.readFileSync(path.join(RT, f), "utf8")).join("\n");
const files = { "/rangerthree.js": bundle, "/three.module.js": fs.readFileSync(path.join(REF, "build/three.module.js")), "/three.core.js": fs.readFileSync(path.join(REF, "build/three.core.js")), "/r3f-ref.js": fs.readFileSync(path.join(MF, "web/three/parity/tools/r3f-ref.js")) };
const { chromium } = createRequire(import.meta.url)("playwright");
const browser = await chromium.launch({ args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage();
page.on("pageerror", (e) => console.log("pageerror", e.message)); if (process.env.CON) page.on("console", (m) => console.log("console", m.text().slice(0, 400)));
if (process.env.DBG) await page.addInitScript(() => { const P = WebGL2RenderingContext.prototype, d = P.drawElements; P.drawElements = function (...a) { d.apply(this, a); const e = this.getError(); if (e) { const units = []; for (let i = 0; i < 16; i++) { this.activeTexture(this.TEXTURE0 + i); units.push(i + ":" + (this.getParameter(this.TEXTURE_BINDING_2D) ? "2d" : "-") + (this.getParameter(this.TEXTURE_BINDING_CUBE_MAP) ? "c" : "")); } this.activeTexture(this.TEXTURE0); const pr = this.getParameter(this.CURRENT_PROGRAM), n = this.getProgramParameter(pr, this.ACTIVE_UNIFORMS), sm = []; for (let k = 0; k < n; k++) { const inf = this.getActiveUniform(pr, k); if (/SAMPLER/.test(Object.keys(WebGL2RenderingContext).find((q) => WebGL2RenderingContext[q] === inf.type) || "")) sm.push(inf.name + "@" + this.getUniform(pr, this.getUniformLocation(pr, inf.name)) + ":" + inf.type.toString(16)); } console.log("samplers " + sm.join(" ")); console.log("glerr " + e + " fbo " + !!this.getParameter(this.FRAMEBUFFER_BINDING) + " " + units.join(" ") + " " + new Error().stack.split("\n").slice(2, 5).join(" | ")); } }; });
await page.route("http://parity.local/**", (route) => {
  const p = new URL(route.request().url()).pathname;
  if (p === "/") return route.fulfill({ contentType: "text/html", body: `<!doctype html><script type="importmap">{"imports":{"three":"/three.module.js"}}</script><script src="/rangerthree.js"></script><script type="module">import { render, makeRender } from "/r3f-ref.js"; window.__ref = render; window.__rt = makeRender(window.THREE, {});</script>` });
  const f = p.startsWith("/addons/") ? fs.readFileSync(path.join(REF, "examples/jsm", p.slice(8))) : files[p];
  if (!f) return route.fulfill({ status: 404, body: "" });
  return route.fulfill({ contentType: "text/javascript", body: f });
});
await page.goto("http://parity.local/");
await page.waitForFunction(() => typeof window.__rt === "function");
const textures = {};
for (const n of Object.keys(TEXTURES)) textures[n] = { w: TEXTURES[n].w, h: TEXTURES[n].h, cs: TEXTURES[n].cs, data: Array.from(texBytes(n)) };
await page.evaluate((t) => { window.__tex = t; }, textures);
const rows = [];
for (const it of SCENE_ITEMS) {
  if (!grep.test(it.id)) continue;
  const r = await page.evaluate(({ it, W, H }) => {
    const res = {};
    try { res.ref = Array.from(window.__ref(it, window.__tex, W, H).px); } catch (e) { res.err = "ref " + e.message; }
    try { res.rt = Array.from(window.__rt(it, window.__tex, W, H).px); } catch (e) { res.err = "rt " + e.message + " @ " + String(e.stack).split("\n").slice(1, 4).join(" | "); }
    return res;
  }, { it, W, H });
  if (r.err) { rows.push([it.id, -1, r.err]); continue; }
  if (arg("png", "")) { const d = arg("png", ""); fs.mkdirSync(d, { recursive: true }); for (const k of ["ref", "rt"]) fs.writeFileSync(path.join(d, it.id.replace(/[^a-z0-9.]/gi, "_") + "." + k + ".png"), png(r[k], W, H)); }
  const c = compare(r.ref, r.rt, W, H);
  rows.push([it.id, c.mean, c.ok ? "" : "FAIL far " + c.far]);
}
rows.sort((a, b) => b[1] - a[1]);
for (const r of rows) console.log(r[1].toFixed ? r[1].toFixed(1) : r[1], r[0], r[2] || "");
const measured = rows.filter((r) => r[1] >= 0);
console.log(`match ${measured.filter((r) => !r[2]).length}/${measured.length}`);
await browser.close();

// the M-Files meter's rule (web/three/parity/run.mjs): over the pixels that
// differ from the reference's corner, mean channel difference <= 6 and at
// most 3 % off by more than 40, edges of the reference not counted
function compare(a, b, w, h) {
  const diff = (p, q, i, j) => Math.max(Math.abs(p[i] - q[j]), Math.abs(p[i + 1] - q[j + 1]), Math.abs(p[i + 2] - q[j + 2]), Math.abs(p[i + 3] - q[j + 3]));
  let sum = 0, far = 0, n = 0;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 4;
    if (diff(a, b, i, 0) <= 8 && diff(b, b, i, 0) <= 8) continue;
    n++;
    for (let c = 0; c < 4; c++) sum += Math.abs(a[i + c] - b[i + c]);
    if (diff(a, b, i, i) <= 40) continue;
    let edge = false;
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const xx = x + dx, yy = y + dy;
      if (xx < 0 || yy < 0 || xx >= w || yy >= h) continue;
      if (diff(b, b, i, (yy * w + xx) * 4) > 40) { edge = true; break; }
    }
    if (!edge) far++;
  }
  if (!n) return { ok: true, mean: 0, far: 0 };
  const mean = sum / (n * 4), share = far / n;
  return { ok: mean <= 6 && share <= 0.03, mean, far: +share.toFixed(3) };
}

function png(px, w, h) {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) { raw[y * (w * 4 + 1)] = 0; for (let i = 0; i < w * 4; i++) raw[y * (w * 4 + 1) + 1 + i] = px[y * w * 4 + i]; }
  const chunk = (t, d) => { const b = Buffer.alloc(8 + d.length + 4); b.writeUInt32BE(d.length, 0); b.write(t, 4); d.copy(b, 8); const crc = zlib.crc32(Buffer.concat([Buffer.from(t), d])); b.writeUInt32BE(crc >>> 0, 8 + d.length); return b; };
  const ih = Buffer.alloc(13); ih.writeUInt32BE(w, 0); ih.writeUInt32BE(h, 4); ih[8] = 8; ih[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ih), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}
