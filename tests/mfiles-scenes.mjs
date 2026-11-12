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
page.on("pageerror", (e) => console.log("pageerror", e.message));
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
    try { res.rt = Array.from(window.__rt(it, window.__tex, W, H).px); } catch (e) { res.err = "rt " + e.message; }
    return res;
  }, { it, W, H });
  if (r.err) { rows.push([it.id, -1, r.err]); continue; }
  if (arg("png", "")) { const d = arg("png", ""); fs.mkdirSync(d, { recursive: true }); for (const k of ["ref", "rt"]) fs.writeFileSync(path.join(d, it.id.replace(/[^a-z0-9.]/gi, "_") + "." + k + ".png"), png(r[k], W, H)); }
  let s = 0; for (let i = 0; i < r.ref.length; i++) s += Math.abs(r.ref[i] - r.rt[i]);
  rows.push([it.id, s / r.ref.length]);
}
rows.sort((a, b) => b[1] - a[1]);
for (const r of rows) console.log(r[1].toFixed ? r[1].toFixed(1) : r[1], r[0], r[2] || "");
await browser.close();

function png(px, w, h) {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) { raw[y * (w * 4 + 1)] = 0; for (let i = 0; i < w * 4; i++) raw[y * (w * 4 + 1) + 1 + i] = px[y * w * 4 + i]; }
  const chunk = (t, d) => { const b = Buffer.alloc(8 + d.length + 4); b.writeUInt32BE(d.length, 0); b.write(t, 4); d.copy(b, 8); const crc = zlib.crc32(Buffer.concat([Buffer.from(t), d])); b.writeUInt32BE(crc >>> 0, 8 + d.length); return b; };
  const ih = Buffer.alloc(13); ih.writeUInt32BE(w, 0); ih.writeUInt32BE(h, 4); ih[8] = 8; ih[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ih), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}
