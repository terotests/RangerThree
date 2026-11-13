// Renders every playground example at a fixed time with three.js r185's
// own WebGLRenderer (tmp/three-ref, parity/tools/fetch-three.sh) and with
// RangerThree's GPU, GPU command and software backends, in headless Chromium, and
// prints the mean per-channel difference (0..255) of each against
// three.js. Math.random is seeded; examples that also draw random numbers
// inside three.js (uuids) differ for that reason alone.
//
//   bash scripts/build-playground.sh && node tests/render-compare.mjs [--grep=re] [--t=ms] [--tone=n] [--alpha] [--png=dir]
//
// --tone sets renderer.toneMapping (4 = ACES filmic, as React Three Fiber's
// Canvas does); --alpha makes an alpha canvas and compares alpha too.
import { createServer } from "node:http";
import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const arg = (k, d) => (process.argv.find((a) => a.startsWith(`--${k}=`)) || "").split("=")[1] || d;
const grep = new RegExp(arg("grep", "."));
const t = arg("t", "1000");
const extra = (arg("tone", "") ? `&tone=${arg("tone", "")}` : "") + (process.argv.includes("--alpha") ? "&alpha=1" : "");
const channels = process.argv.includes("--alpha") ? 4 : 3;
const files = {
  "/page.html": join(ROOT, "tests/render-compare.html"),
  "/three.cjs": join(ROOT, "tmp/three-ref/three/build/three.cjs"),
};
for (const f of ["rangerthree-core.js", "three.js", "three-scene.js", "three-curves.js", "three-renderer.js", "three-webgl.js", "gpu-webgl2.js", "examples.js"]) files["/" + f] = join(ROOT, "build/site", f);
for (const [k, f] of Object.entries(files)) if (!existsSync(f)) { console.error(`missing ${f} (${k})`); process.exit(2); }
const SHIM = "const T = window.THREE; export const " + ["UniformsLib", "ClampToEdgeWrapping", "DataTexture", "DataUtils", "FloatType", "HalfFloatType", "LinearFilter", "NearestFilter", "RGBAFormat", "UVMapping"].map((k) => `${k} = T.${k}`).join(", ") + ";";
const server = createServer((req, res) => {
  const p = req.url.split("?")[0];
  if (p === "/three-shim.js") { res.writeHead(200, { "content-type": "text/javascript" }); res.end(SHIM); return; }
  if (p.startsWith("/addons/")) { res.writeHead(200, { "content-type": "text/javascript" }); res.end(readFileSync(join(ROOT, "tmp/three-ref/three/examples/jsm", p.slice(8)))); return; }
  const f = files[p];
  if (!f) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { "content-type": f.endsWith(".html") ? "text/html" : "text/javascript" });
  res.end(readFileSync(f));
}).listen(0);
const base = `http://localhost:${server.address().port}`;
const { chromium } = createRequire(import.meta.url)("playwright");
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 640, height: 480 } });

async function shot(query, name) {
  await page.goto(`${base}/page.html?${query}${extra}&t=${t}&ex=${encodeURIComponent(name)}`);
  await page.waitForFunction(() => window.__done, null, { timeout: 30000 });
  await page.waitForTimeout(200);
  const err = await page.evaluate(() => window.__err);
  // pixels of the canvas as RGBA
  const px = await page.evaluate(() => {
    const c = document.getElementById("c"), o = document.createElement("canvas");
    o.width = c.width; o.height = c.height;
    const g = o.getContext("2d"); g.drawImage(c, 0, 0);
    return Array.from(g.getImageData(0, 0, o.width, o.height).data);
  });
  return { err, px };
}
function diff(a, b) {
  let s = 0, n = 0;
  for (let i = 0; i < a.length; i += 4) for (let c = 0; c < channels; c++) { s += Math.abs(a[i + c] - b[i + c]); n++; }
  return s / n;
}

await page.goto(`${base}/page.html?impl=ours&ex=none`);
await page.waitForFunction(() => window.__done);
const names = await page.evaluate(() => Object.keys(EXAMPLES));
console.log("example | gpu | commands | software (mean difference to three.js, 0..255)");
for (const [i, name] of names.entries()) {
  if (!grep.test(name)) continue;
  const ref = await shot("impl=ref", name);
  const gpu = await shot("impl=ours", name);
  const cmd = await shot("impl=ours&backend=commands", name);
  const soft = await shot("impl=ours&backend=software", name);
  for (const [tag, r] of [["ref", ref], ["gpu", gpu], ["commands", cmd], ["soft", soft]]) if (r.err) console.log(`  ${tag} error: ${r.err.split("\n")[0]}`);
  if (arg("png", "")) { const { writeFileSync, mkdirSync } = await import("node:fs"); const d = arg("png", ""); mkdirSync(d, { recursive: true });
    const base64 = async (q) => { await page.goto(`${base}/page.html?${q}${extra}&t=${t}&ex=${encodeURIComponent(name)}`); await page.waitForFunction(() => window.__done); await page.waitForTimeout(200); return page.evaluate(() => document.getElementById("c").toDataURL("image/png").split(",")[1]); };
    for (const [tag, q] of [["ref", "impl=ref"], ["gpu", "impl=ours"], ["commands", "impl=ours&backend=commands"]]) writeFileSync(`${d}/${name.replace(/[^a-z0-9]+/gi, "_")}.${tag}.png`, Buffer.from(await base64(q), "base64")); }
  console.log(`${name} | ${diff(ref.px, gpu.px).toFixed(1)} | ${diff(ref.px, cmd.px).toFixed(1)} | ${diff(ref.px, soft.px).toFixed(1)}`);
}
await browser.close();
server.close();
