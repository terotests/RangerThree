// SPDX-License-Identifier: MIT
//
// The reference picture of a parity scene: the {t, a, c} tree built with
// three.js itself by React Three Fiber's own rules (applyProps, attach and
// <Canvas>'s defaults, from @react-three/fiber 9's source), drawn by
// THREE.WebGLRenderer. Test reference only: runs in the meter's Chromium
// page, never in M-Files.
import * as THREE from "./three.module.js";
import { TeapotGeometry } from "/addons/geometries/TeapotGeometry.js";

// what a .tsx gives R3F with extend(): three's addons by their JSX names
const EXTENDED = { TeapotGeometry };

const CANVAS_TONE = THREE.ACESFilmicToneMapping;
const COLOR_MAPS = ["map", "emissiveMap", "sheenColorMap", "specularColorMap", "envMap"];
const SHADOWS = { basic: THREE.BasicShadowMap, percentage: THREE.PCFShadowMap, soft: THREE.PCFSoftShadowMap, variance: THREE.VSMShadowMap };

// "name@N": a THREE.DataTexture of the scene's texture `name`
function textureOf(ref, textures, cache) {
  if (cache[ref]) return cache[ref];
  const name = ref.split("@")[0], t = textures[name];
  if (!t) return null;
  const tex = new THREE.DataTexture(new Uint8Array(t.data), t.w, t.h);
  tex.colorSpace = t.cs === "srgb" ? THREE.SRGBColorSpace : THREE.NoColorSpace;
  tex.needsUpdate = true;
  cache[ref] = tex;
  return tex;
}
// a value as the JSX would have given it: constants by name, textures by name
function value(v, textures, cache) {
  if (typeof v === "string") {
    if (/^[A-Za-z][A-Za-z0-9]*$/.test(v) && typeof THREE[v] === "number" && /Side|Blending|Wrapping|Filter|Mapping|ToneMapping|ShadowMap|Space|Equation|Factor|Depth|Format|Type|Usage$/.test(v)) return THREE[v];
    if (/^[a-z]+@\d+$/.test(v) && textures[v.split("@")[0]]) return textureOf(v, textures, cache);
  }
  return v;
}
function resolve(root, key) {
  if (!key.includes("-") || key in root) return { root, key, target: root[key] };
  let target = root;
  const parts = key.split("-");
  for (let i = 0; i < parts.length - 1; i++) {
    if (target == null || typeof target !== "object") break;
    target = target[parts[i]];
  }
  const last = parts[parts.length - 1];
  return { root: target, key: last, target: target ? target[last] : undefined };
}
const RESERVED = ["args", "attach", "object", "key", "children", "makeDefault", "dispose", "onUpdate", "ref"];
function applyProps(obj, props, textures, cache) {
  for (const prop of Object.keys(props)) {
    if (RESERVED.includes(prop) || /^on[A-Z]/.test(prop)) continue;
    let v = value(props[prop], textures, cache);
    if (v === undefined) continue;
    if (prop === "lookAt" && obj.lookAt) { obj.lookAt(...v); continue; }
    const { root, key, target } = resolve(obj, prop);
    if (root == null) continue;
    if (target instanceof THREE.Color && (typeof v === "string" || typeof v === "number" || v instanceof THREE.Color)) target.set(v);
    else if (target instanceof THREE.Color && Array.isArray(v)) target.fromArray(v);
    else if (target && typeof target === "object" && typeof target.set === "function" && typeof target.copy === "function" && v && v.constructor === target.constructor) target.copy(v);
    else if (target && typeof target === "object" && typeof target.set === "function" && Array.isArray(v)) { if (typeof target.fromArray === "function") target.fromArray(v); else target.set(...v); }
    else if (target && typeof target === "object" && typeof target.set === "function" && typeof v === "number") { if (typeof target.setScalar === "function") target.setScalar(v); else target.set(v); }
    else {
      root[key] = v;
      if (COLOR_MAPS.includes(key) && root[key] && root[key].isTexture && root[key].format === THREE.RGBAFormat && root[key].type === THREE.UnsignedByteType && !props.__linear) root[key].colorSpace = THREE.SRGBColorSpace;
    }
    if (root && root.isTexture) root.needsUpdate = true;
    if (root && root.isMaterial) root.needsUpdate = true;
  }
}
// constructor arguments: JSON stand-ins for what a .tsx makes with THREE
function argsOf(tag, args, textures, cache, attach) {
  if (!args) return [];
  const v2 = (p) => new THREE.Vector2(p.x ?? p[0], p.y ?? p[1]);
  const v3 = (p) => new THREE.Vector3(p[0], p[1], p[2]);
  const shape = (d) => { const s = new THREE.Shape(d.shape.map(v2)); for (const h of d.holes || []) s.holes.push(new THREE.Path(h.map(v2))); return s; };
  const geom = (d) => new THREE[d.geometry[0][0].toUpperCase() + d.geometry[0].slice(1)](...d.geometry[1]);
  switch (tag) {
    case "latheGeometry": return [args[0].map(v2), ...args.slice(1)];
    case "extrudeGeometry": case "shapeGeometry": return [shape(args[0]), ...args.slice(1)];
    case "tubeGeometry": return [new THREE.CatmullRomCurve3(args[0].curve.map(v3)), ...args.slice(1)];
    case "edgesGeometry": case "wireframeGeometry": return [geom(args[0])];
    case "bufferAttribute": return [(attach === "index" ? new Uint16Array(args[0]) : new Float32Array(args[0])), args[1]];
    case "dataTexture": return args;
    default: return args.map((a) => value(a, textures, cache));
  }
}
function className(tag) {
  if (tag === "lOD") return "LOD";
  return tag[0].toUpperCase() + tag.slice(1);
}
function build(node, ctx) {
  const tag = node.t, a = node.a || {};
  let obj;
  if (tag === "dataTexture") {
    obj = textureOf(a.image, ctx.textures, ctx.cache).clone();
    obj.needsUpdate = true;
  } else if (tag === "texture") {
    obj = textureOf(a.map, ctx.textures, ctx.cache).clone();
    obj.needsUpdate = true;
  } else {
    const C = THREE[className(tag)] || EXTENDED[className(tag)];
    if (!C) { ctx.unknown.push(tag); return null; }
    obj = new C(...argsOf(tag, a.args, ctx.textures, ctx.cache, a.attach));
  }
  // props in the JSX's order (R3F applies them so: map before map-repeat);
  // a texture element's own picture (map / image) was taken above
  const props = { ...a };
  if (tag === "texture") delete props.map;
  if (tag === "dataTexture") delete props.image;
  applyProps(obj, props, ctx.textures, ctx.cache);
  if (obj.isTexture) obj.needsUpdate = true;
  for (const c of node.c || []) {
    const child = build(c, ctx);
    if (!child) continue;
    let attach = (c.a || {}).attach;
    if (attach === undefined) { if (child.isBufferGeometry) attach = "geometry"; else if (child.isMaterial) attach = "material"; }
    if (attach) {
      if (/-\d+$/.test(attach)) { const base = attach.replace(/-\d+$/, ""); const r = resolve(obj, base); if (!Array.isArray(r.root[r.key])) r.root[r.key] = []; }
      const r = resolve(obj, attach);
      r.root[r.key] = child;
      if (r.root.isBufferGeometry || r.root.isMaterial) { r.root.needsUpdate = true; }
    } else if (child.isObject3D && obj.isObject3D) {
      obj.add(child);
      if (child.isCamera && (!ctx.camera || (c.a || {}).makeDefault)) ctx.camera = child;
    }
  }
  return obj;
}

let renderer = null;
// draws one item; answers RGBA rows from the top
export function render(item, textures, W, H) {
  const root = item.scene, ca = root.a || {};
  const dpr = ca.dpr || 1;
  const linear = !!ca.linear, flat = !!ca.flat;
  THREE.ColorManagement.enabled = true;
  // one renderer for every item: a browser keeps only so many WebGL
  // contexts and loses the oldest (the meter's other canvas) past that
  // antialias as R3F's Canvas has it (gl antialias true), as the GPU path draws
  if (!renderer) renderer = new THREE.WebGLRenderer({ canvas: document.createElement("canvas"), antialias: true, alpha: true, preserveDrawingBuffer: true });
  const gl = renderer;
  gl.shadowMap.enabled = false;
  gl.shadowMap.type = THREE.PCFShadowMap;
  gl.toneMappingExposure = 1;
  gl.setClearColor(0x000000, 0);
  gl.setPixelRatio(1);
  gl.setSize(W * dpr, H * dpr, false);
  gl.outputColorSpace = linear ? THREE.LinearSRGBColorSpace : THREE.SRGBColorSpace;
  gl.toneMapping = flat ? THREE.NoToneMapping : CANVAS_TONE;
  if (ca["gl-toneMapping"]) gl.toneMapping = THREE[ca["gl-toneMapping"]];
  if (ca["gl-toneMappingExposure"]) gl.toneMappingExposure = ca["gl-toneMappingExposure"];
  if (ca.shadows) { gl.shadowMap.enabled = true; gl.shadowMap.type = SHADOWS[ca.shadows] ?? THREE.PCFSoftShadowMap; }
  const ctx = { textures, cache: {}, unknown: [], camera: null };
  const scene = new THREE.Scene();
  for (const k of Object.keys(ca)) if (k.startsWith("scene-")) scene[k.slice(6)] = ca[k];
  for (const c of root.c || []) {
    const child = build(c, ctx);
    if (!child) continue;
    const attach = (c.a || {}).attach;
    if (attach) scene[attach] = child;
    else scene.add(child);
    if (child.isCamera && (!ctx.camera || (c.a || {}).makeDefault)) ctx.camera = child;
  }
  let camera = ctx.camera;
  if (!camera) {
    const cp = ca.camera || {};
    camera = ca.orthographic ? new THREE.OrthographicCamera(0, 0, 0, 0, 0.1, 1000) : new THREE.PerspectiveCamera(cp.fov ?? 75, W / H, cp.near ?? 0.1, cp.far ?? 1000);
    if (ca.orthographic) { camera.zoom = 100; camera.left = -W / 2; camera.right = W / 2; camera.top = H / 2; camera.bottom = -H / 2; }
    camera.position.set(...(cp.position || [0, 0, 5]));
    camera.lookAt(0, 0, 0);
  }
  if (camera.isPerspectiveCamera) camera.aspect = W / H;
  camera.updateProjectionMatrix();
  if (scene.environment && scene.environment.isTexture && !scene.environment.mapping) scene.environment.mapping = THREE.EquirectangularReflectionMapping;
  gl.render(scene, camera);
  const g = gl.getContext();
  const px = new Uint8Array(W * dpr * H * dpr * 4);
  g.readPixels(0, 0, W * dpr, H * dpr, g.RGBA, g.UNSIGNED_BYTE, px);
  scene.traverse((o) => { if (o.geometry) o.geometry.dispose(); const ms = Array.isArray(o.material) ? o.material : o.material ? [o.material] : []; for (const m of ms) m.dispose(); });
  for (const t of Object.values(ctx.cache)) t.dispose();
  gl.renderLists.dispose();
  const out = new Uint8Array(px.length), row = W * dpr * 4;
  for (let y = 0; y < H * dpr; y++) out.set(px.subarray((H * dpr - 1 - y) * row, (H * dpr - y) * row), y * row);
  return { px: out, w: W * dpr, h: H * dpr, unknown: ctx.unknown };
}
