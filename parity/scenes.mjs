// SPDX-License-Identifier: MIT
//
// The render items (axis "render") and the declarative ones (axis "r3f"):
// each a scene written once as React Three Fiber JSX would give it, the
// {t, a, c} tree web/three's fiber_tree reads. The reference draws the same
// tree with three.js itself through tools/r3f-ref.js (R3F's rules: <scene3d>
// is R3F's <Canvas>, so ACES tone mapping, sRGB output, PCF soft shadows,
// the camera at [0, 0, 5] fov 75 unless the scene has one); the meter draws
// it with rangerthree (GPU and software) and compares the pictures.
//
// Textures are made in code (tex()), RGBA rows from the bottom as
// THREE.DataTexture takes them, so both sides get the same bytes.

export const W = 160, H = 120;

const h = (t, a = {}, ...c) => ({ t, a, c: c.flat().filter(Boolean) });
const geo = (t, args) => h(t, args ? { args } : {});
const mat = (t, a = {}) => h(t, a);
const mesh = (a, g, m, ...kids) => h("mesh", a, g, m, ...kids);
const cam = (a = {}) => h("perspectiveCamera", { position: [0, 1.2, 4], lookAt: [0, 0, 0], fov: 50, ...a });
const key = (a = {}) => h("directionalLight", { position: [3, 4, 5], intensity: 2.5, ...a });
const amb = (i = 0.3) => h("ambientLight", { intensity: i });
const bg = (c = "#20252e") => h("color", { attach: "background", args: [c] });
const scene = (a, ...c) => h("scene3d", a, ...c);
// a lit stage: background, camera, ambient + key light, then the subject
const stage = (subject, extra = {}) => scene(extra.canvas || {}, bg(extra.bg), extra.cam || cam(), amb(extra.amb ?? 0.3), extra.noKey ? null : key(extra.key), subject, ...(extra.more || []));
const std = (a = {}) => mat("meshStandardMaterial", { color: "#4f8fe0", roughness: 0.5, ...a });

// ---- textures --------------------------------------------------------------
export const TEXTURES = {
  checker: { w: 16, h: 16, cs: "srgb", px: (x, y) => ((x >> 2) + (y >> 2)) % 2 ? [235, 230, 220, 255] : [200, 60, 40, 255] },
  gradient: { w: 32, h: 32, cs: "srgb", px: (x, y) => [x * 8, y * 8, 255 - x * 4, 255] },
  stripes: { w: 16, h: 4, cs: "srgb", px: (x) => (x % 4 < 2 ? [250, 250, 250, 255] : [30, 90, 200, 255]) },
  disc: { w: 32, h: 32, cs: "srgb", px: (x, y) => { const d = Math.hypot(x - 15.5, y - 15.5) / 16; return [255, 220, 120, Math.max(0, Math.round(255 * (1 - d)))]; } },
  ring: { w: 32, h: 32, cs: "srgb", px: (x, y) => { const d = Math.hypot(x - 15.5, y - 15.5) / 16; return d > 0.55 && d < 0.95 ? [230, 200, 160, 200] : [0, 0, 0, 0]; } },
  glyph: { w: 8, h: 8, cs: "srgb", px: (x, y) => ((x * 2 - 7) ** 2 + (y * 2 - 7) ** 2 < 40 ? [60, 140, 255, 255] : [255, 255, 255, 0]) },
  bumps: { w: 32, h: 32, cs: "", px: (x, y) => { const v = Math.round(128 + 127 * Math.sin(x * 0.8) * Math.sin(y * 0.8)); return [v, v, v, 255]; } },
  normal: { w: 32, h: 32, cs: "", px: (x, y) => { const nx = Math.sin(x * 0.8) * 0.5, ny = Math.sin(y * 0.8) * 0.5, nz = Math.sqrt(Math.max(0, 1 - nx * nx - ny * ny)); return [Math.round((nx * 0.5 + 0.5) * 255), Math.round((ny * 0.5 + 0.5) * 255), Math.round((nz * 0.5 + 0.5) * 255), 255]; } },
  rough: { w: 16, h: 16, cs: "", px: (x) => { const v = x < 8 ? 40 : 230; return [v, v, v, 255]; } },
  mask: { w: 16, h: 16, cs: "", px: (x, y) => { const v = (x + y) % 8 < 4 ? 255 : 0; return [v, v, v, 255]; } },
  emit: { w: 16, h: 16, cs: "srgb", px: (x, y) => (Math.abs(x - 8) < 2 || Math.abs(y - 8) < 2 ? [255, 160, 40, 255] : [0, 0, 0, 255]) },
  stars: { w: 64, h: 32, cs: "srgb", px: (x, y) => ((x * 73856093 ^ y * 19349663) % 23 === 0 ? [255, 255, 255, 255] : [5, 6, 12, 255]) },
};
export function texBytes(name) {
  const t = TEXTURES[name], out = new Uint8Array(t.w * t.h * 4);
  for (let y = 0; y < t.h; y++) for (let x = 0; x < t.w; x++) out.set(t.px(x, y), (y * t.w + x) * 4);
  return out;
}
const map = (name, extra = {}) => ({ map: name + "@1", ...extra });
const attr = (attach, array, size) => h("bufferAttribute", { attach, args: [array, size] });
const pts = (list) => attr("attributes-position", list.flat(), 3);
// a colour per vertex of an n-vertex geometry (hue by index)
const hueColors = (n) => Array.from({ length: n }, (_, i) => { const a = i / n * 6.283; return [0.5 + 0.5 * Math.cos(a), 0.5 + 0.5 * Math.cos(a - 2.09), 0.5 + 0.5 * Math.cos(a + 2.09)]; }).flat();
// lineDistance of a polyline (what computeLineDistances writes)
const lineDist = (list) => { let d = 0; return list.map((p, i) => (i ? (d += Math.hypot(p[0] - list[i - 1][0], p[1] - list[i - 1][1], p[2] - list[i - 1][2])) : 0)); };

// ---- render items ------------------------------------------------------------
const items = [];
const add = (axis, id, title, tree, extra = {}) => items.push({ id: `${axis}:${id}`, axis, title, scene: tree, ...extra });
const R = (id, title, tree, extra) => add("render", id, title, tree, extra);

// geometries: each with Three's defaults and with its own arguments
const GEOS = [
  ["boxGeometry", null, [1.4, 1, 0.8, 2, 2, 2]],
  ["sphereGeometry", null, [1, 32, 16]],
  ["sphereGeometry", "part", [1, 24, 12, 0, Math.PI * 1.5, 0.3, 2]],
  ["cylinderGeometry", null, [0.6, 0.9, 1.6, 24, 1]],
  ["cylinderGeometry", "open", [0.8, 0.8, 1.4, 20, 1, true]],
  ["coneGeometry", null, [0.9, 1.6, 24]],
  ["planeGeometry", null, [2, 1.4, 2, 2]],
  ["circleGeometry", null, [1, 32]],
  ["circleGeometry", "sector", [1, 24, 0.5, 4]],
  ["ringGeometry", null, [0.5, 1, 32]],
  ["torusGeometry", null, [0.8, 0.3, 16, 48]],
  ["torusGeometry", "arc", [0.8, 0.25, 12, 32, Math.PI]],
  ["torusKnotGeometry", null, [0.6, 0.2, 96, 12]],
  ["torusKnotGeometry", "pq", [0.6, 0.15, 128, 10, 3, 5]],
  ["capsuleGeometry", null, [0.5, 1, 8, 16]],
  ["dodecahedronGeometry", null, [1, 0]],
  ["icosahedronGeometry", null, [1, 0]],
  ["icosahedronGeometry", "detail", [1, 3]],
  ["octahedronGeometry", null, [1, 0]],
  ["octahedronGeometry", "detail", [1, 2]],
  ["tetrahedronGeometry", null, [1, 0]],
  ["latheGeometry", null, [[[0, -1], [0.6, -0.8], [0.4, 0], [0.8, 0.6], [0, 1]].map(([x, y]) => ({ x, y })), 24]],
  ["teapotGeometry", null, [0.8, 8]],
];
for (const [g, variant, args] of GEOS) {
  const id = g.replace("Geometry", "") + (variant ? "-" + variant : "");
  if (!variant) R(`geometry.${id}.default`, `${g} with Three's default arguments`, stage(mesh({ rotation: [0.4, 0.6, 0] }, geo(g), std())));
  R(`geometry.${id}`, `${g} args=${JSON.stringify(args).slice(0, 60)}`, stage(mesh({ rotation: [0.4, 0.6, 0] }, geo(g, args), std())));
}
R("geometry.extrude", "extrudeGeometry of a shape with a hole and a bevel", stage(mesh({ rotation: [0.5, 0.4, 0] }, h("extrudeGeometry", { args: [{ shape: [[-0.8, -0.6], [0.8, -0.6], [0.8, 0.6], [-0.8, 0.6]], holes: [[[-0.3, -0.2], [0.3, -0.2], [0, 0.3]]] }, { depth: 0.4, bevelEnabled: true, bevelSize: 0.08, bevelThickness: 0.08, bevelSegments: 2 }] }), std())));
R("geometry.shape", "shapeGeometry (a star)", stage(mesh({}, h("shapeGeometry", { args: [{ shape: Array.from({ length: 10 }, (_, i) => { const r = i % 2 ? 0.45 : 1; const a = i * Math.PI / 5 + Math.PI / 2; return [Math.cos(a) * r, Math.sin(a) * r]; }) }] }), std({ side: "DoubleSide" }))));
R("geometry.tube", "tubeGeometry along a CatmullRom curve", stage(mesh({}, h("tubeGeometry", { args: [{ curve: [[-1.2, -0.5, 0], [-0.4, 0.6, 0.3], [0.4, -0.4, -0.3], [1.2, 0.5, 0]] }, 48, 0.15, 8, false] }), std())));
R("geometry.buffer", "bufferGeometry from position and color attributes", stage(mesh({}, h("bufferGeometry", {}, h("bufferAttribute", { attach: "attributes-position", args: [[-1, -0.8, 0, 1, -0.8, 0, 0, 0.9, 0], 3] }), h("bufferAttribute", { attach: "attributes-color", args: [[1, 0, 0, 0, 1, 0, 0, 0, 1], 3] })), mat("meshBasicMaterial", { vertexColors: true, side: "DoubleSide" }))));
R("geometry.indexed", "indexed bufferGeometry with computed normals", stage(mesh({ rotation: [0.6, 0.5, 0] }, h("bufferGeometry", {}, h("bufferAttribute", { attach: "attributes-position", args: [[0, 1, 0, -1, -0.6, 0.6, 1, -0.6, 0.6, 0, -0.6, -1], 3] }), h("bufferAttribute", { attach: "index", args: [[0, 1, 2, 0, 2, 3, 0, 3, 1, 1, 3, 2], 1] })), std({ flatShading: true }))));
R("geometry.edges", "lineSegments of edgesGeometry", stage(h("lineSegments", { rotation: [0.4, 0.6, 0] }, h("edgesGeometry", { args: [{ geometry: ["boxGeometry", [1.4, 1, 0.8]] }] }), mat("lineBasicMaterial", { color: "#ffd060" }))));
R("geometry.wireframe", "lineSegments of wireframeGeometry", stage(h("lineSegments", { rotation: [0.4, 0.6, 0] }, h("wireframeGeometry", { args: [{ geometry: ["sphereGeometry", [1, 12, 8]] }] }), mat("lineBasicMaterial", { color: "#80ffb0" }))));

// materials: each type on a sphere and a box, lit
const MATS = [
  ["meshBasicMaterial", { color: "#e07040" }],
  ["meshLambertMaterial", { color: "#e07040" }],
  ["meshPhongMaterial", { color: "#e07040", shininess: 60, specular: "#ffffff" }],
  ["meshStandardMaterial", { color: "#e07040", roughness: 0.4, metalness: 0.1 }],
  ["meshPhysicalMaterial", { color: "#e07040", roughness: 0.3, clearcoat: 1, clearcoatRoughness: 0.1 }],
  ["meshToonMaterial", { color: "#e07040" }],
  ["meshNormalMaterial", {}],
  ["meshDepthMaterial", {}],
  ["meshMatcapMaterial", { color: "#e07040" }],
];
for (const [m, a] of MATS) {
  const n = m.replace(/^mesh|Material$/g, "").toLowerCase();
  R(`material.${n}`, `${m} on a sphere`, stage(mesh({}, geo("sphereGeometry", [1, 48, 24]), mat(m, a))));
  R(`material.${n}.box`, `${m} on a box (flat faces)`, stage(mesh({ rotation: [0.5, 0.7, 0] }, geo("boxGeometry", [1.3, 1.3, 1.3]), mat(m, a))));
}
const PROPS = [
  ["wireframe", "meshStandardMaterial", { wireframe: true }],
  ["flatShading", "meshStandardMaterial", { flatShading: true }],
  ["flatShading.phong", "meshPhongMaterial", { flatShading: true, color: "#4f8fe0" }],
  ["side.double", "meshStandardMaterial", { side: "DoubleSide" }, "plane"],
  ["side.back", "meshStandardMaterial", { side: "BackSide" }],
  ["emissive", "meshStandardMaterial", { color: "#202020", emissive: "#20ff80", emissiveIntensity: 0.7 }],
  ["emissive.lambert", "meshLambertMaterial", { color: "#202020", emissive: "#ff4020" }],
  ["opacity", "meshStandardMaterial", { transparent: true, opacity: 0.5 }],
  ["opacity.basic", "meshBasicMaterial", { color: "#ff8040", transparent: true, opacity: 0.35 }],
  ["blending.additive", "meshBasicMaterial", { color: "#ff8040", transparent: true, blending: "AdditiveBlending", depthWrite: false }],
  ["blending.subtractive", "meshBasicMaterial", { color: "#406080", transparent: true, blending: "SubtractiveBlending", premultipliedAlpha: true }],
  ["blending.multiply", "meshBasicMaterial", { color: "#ff8040", transparent: true, blending: "MultiplyBlending", premultipliedAlpha: true }],
  ["blending.none", "meshBasicMaterial", { color: "#ff8040", transparent: true, opacity: 0.3, blending: "NoBlending" }],
  ["depthTest.off", "meshBasicMaterial", { color: "#ffd040", depthTest: false }, "behind"],
  ["depthWrite.off", "meshBasicMaterial", { color: "#ffd040", depthWrite: false }, "behind"],
  ["colorWrite.off", "meshBasicMaterial", { colorWrite: false }, "behind"],
  ["toneMapped.off", "meshBasicMaterial", { color: "#ffd040", toneMapped: false }],
  ["fog.off", "meshStandardMaterial", { fog: false }, "fog"],
  ["visible.off", "meshStandardMaterial", { visible: false }],
  ["vertexColors.sphere", "meshStandardMaterial", { vertexColors: true }, "colors"],
  ["alphaTest", "meshBasicMaterial", map("glyph", { alphaTest: 0.5, side: "DoubleSide" }), "plane"],
  ["polygonOffset", "meshBasicMaterial", { color: "#ffffff", polygonOffset: true, polygonOffsetFactor: -1 }, "coplanar"],
  ["dithering", "meshStandardMaterial", { dithering: true }],
  ["shadowSide", "meshStandardMaterial", { shadowSide: "DoubleSide" }],
];
for (const [n, m, a, kind] of PROPS) {
  let subject = mesh({}, geo("sphereGeometry", [1, 48, 24]), mat(m, m === "meshStandardMaterial" && !a.color ? { color: "#4f8fe0", ...a } : a));
  const more = [];
  if (kind === "plane") subject = mesh({ rotation: [0, 2.6, 0] }, geo("planeGeometry", [2, 2]), mat(m, { color: "#4f8fe0", ...a }));
  if (kind === "behind") { subject = mesh({ position: [0, 0, -1] }, geo("sphereGeometry", [0.8, 32, 16]), mat(m, a)); more.push(mesh({ position: [0, 0, 0.5] }, geo("boxGeometry", [1.2, 1.2, 0.2]), std({ color: "#3060c0" }))); }
  if (kind === "coplanar") { subject = mesh({}, geo("planeGeometry", [1, 1]), mat(m, a)); more.push(mesh({}, geo("planeGeometry", [2, 2]), std({ color: "#3060c0" }))); }
  if (kind === "colors") subject = mesh({}, h("sphereGeometry", { args: [1, 32, 16] }, attr("attributes-color", hueColors(33 * 17), 3)), mat(m, a));
  if (kind === "fog") more.push(h("fog", { attach: "fog", args: ["#20252e", 2, 6] }), mesh({ position: [1.6, 0, -2] }, geo("sphereGeometry", [0.6, 24, 12]), std()));
  if (["opacity", "opacity.basic", "blending.additive", "blending.subtractive", "blending.multiply", "blending.none"].includes(n)) more.push(mesh({ position: [0.5, 0.2, -1.5] }, geo("boxGeometry", [1.5, 1.5, 0.3]), std({ color: "#30c060" })));
  R(`material.prop.${n}`, `${m} ${JSON.stringify(a).slice(0, 70)}`, stage(subject, { more }));
}
for (const [r, m] of [[0, 0], [0.5, 0], [1, 0], [0, 1], [0.3, 1], [1, 1], [0.15, 0.5]]) R(`material.pbr.r${r}m${m}`, `meshStandardMaterial roughness ${r} metalness ${m}`, stage(mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ roughness: r, metalness: m, color: "#d0a060" })), { more: [h("pointLight", { position: [-2, 1, 2], intensity: 8, color: "#80a0ff" })] }));
for (const s of [5, 30, 120]) R(`material.phong.shininess${s}`, `meshPhongMaterial shininess ${s}`, stage(mesh({}, geo("sphereGeometry", [1, 48, 24]), mat("meshPhongMaterial", { color: "#4f8fe0", shininess: s, specular: "#ffffff" }))));
R("material.phong.specularColor", "meshPhongMaterial with a coloured specular", stage(mesh({}, geo("sphereGeometry", [1, 48, 24]), mat("meshPhongMaterial", { color: "#303030", shininess: 40, specular: "#ff4000" }))));
R("material.physical.transmission", "meshPhysicalMaterial transmission (glass) over a box", stage(mesh({}, geo("sphereGeometry", [1, 48, 24]), mat("meshPhysicalMaterial", { transmission: 1, roughness: 0.05, thickness: 0.5 })), { more: [mesh({ position: [0, 0, -2] }, geo("boxGeometry", [3, 0.5, 0.2]), std({ color: "#e04040" }))] }));
R("material.physical.sheen", "meshPhysicalMaterial sheen", stage(mesh({}, geo("sphereGeometry", [1, 48, 24]), mat("meshPhysicalMaterial", { color: "#402060", sheen: 1, sheenColor: "#ff80ff", sheenRoughness: 0.4 }))));
R("material.physical.iridescence", "meshPhysicalMaterial iridescence", stage(mesh({}, geo("sphereGeometry", [1, 48, 24]), mat("meshPhysicalMaterial", { color: "#202020", metalness: 1, roughness: 0.2, iridescence: 1 }))));
R("material.shadow", "shadowMaterial catching a shadow", stage(h("group", {}, mesh({ position: [0, 0.3, 0], castShadow: true }, geo("sphereGeometry", [0.6, 32, 16]), std()), mesh({ position: [0, -0.4, 0], rotation: [-Math.PI / 2, 0, 0], receiveShadow: true }, geo("planeGeometry", [5, 5]), mat("shadowMaterial", { opacity: 0.5 }))), { canvas: { shadows: true }, key: { castShadow: true } }));

// lights
const ball = (a = {}) => mesh({}, geo("sphereGeometry", [1, 48, 24]), std(a));
const floor = () => mesh({ position: [0, -1, 0], rotation: [-Math.PI / 2, 0, 0], receiveShadow: true }, geo("planeGeometry", [8, 8]), std({ color: "#c0c0c0" }));
const lit = (id, title, lights, extra = {}) => R(`light.${id}`, title, scene(extra.canvas || {}, bg(), cam(), ...lights, ball(), floor(), ...(extra.more || [])));
lit("ambient", "ambientLight only", [h("ambientLight", { intensity: 1.2, color: "#ffd0a0" })]);
lit("hemisphere", "hemisphereLight sky and ground", [h("hemisphereLight", { args: ["#80c0ff", "#604020", 2] })]);
lit("directional", "directionalLight", [key()]);
lit("directional.color", "directionalLight coloured, low", [key({ color: "#ff8040", position: [-3, 1, 2], intensity: 3 })]);
lit("directional.target", "directionalLight with a moved target", [h("directionalLight", { position: [0, 4, 0], intensity: 3, "target-position": [2, 0, 0] })]);
lit("directional.two", "two directional lights", [key(), key({ position: [-4, 2, -1], color: "#4080ff", intensity: 2 })]);
for (const [d, dist] of [[2, 0], [1, 0], [0, 0], [2, 4]]) lit(`point.decay${d}${dist ? ".dist" + dist : ""}`, `pointLight decay ${d}${dist ? " distance " + dist : ""}`, [amb(0.05), h("pointLight", { position: [1.5, 1, 1.5], intensity: 12, decay: d, distance: dist, color: "#ffe0c0" })]);
lit("point.four", "four point lights", [amb(0.05), ...[["#ff4040", [2, 0, 1]], ["#40ff40", [-2, 0, 1]], ["#4040ff", [0, 2, 1]], ["#ffff40", [0, -0.5, 2]]].map(([c, p]) => h("pointLight", { position: p, intensity: 8, color: c }))]);
lit("point.eight", "eight point lights (more than four)", [amb(0.05), ...Array.from({ length: 8 }, (_, i) => h("pointLight", { position: [Math.cos(i * 0.785) * 2, 0.5, Math.sin(i * 0.785) * 2], intensity: 5, color: ["#ff4040", "#40ff40", "#4040ff", "#ffff40", "#ff40ff", "#40ffff", "#ffffff", "#ff8000"][i] }))]);
lit("spot", "spotLight", [amb(0.05), h("spotLight", { position: [2, 3, 2], intensity: 30, angle: 0.4, penumbra: 0 })]);
lit("spot.penumbra", "spotLight with penumbra", [amb(0.05), h("spotLight", { position: [2, 3, 2], intensity: 30, angle: 0.5, penumbra: 0.8 })]);
lit("spot.decay", "spotLight decay 1 and distance", [amb(0.05), h("spotLight", { position: [0, 4, 0], intensity: 20, angle: 0.6, decay: 1, distance: 8 })]);
lit("rectArea", "rectAreaLight", [amb(0.05), h("rectAreaLight", { position: [0, 2, 2], lookAt: [0, 0, 0], width: 2, height: 1, intensity: 6 })]);
lit("intensity.zero", "a light of intensity 0", [amb(0.1), key({ intensity: 0 })]);
lit("visible.off", "an invisible light", [amb(0.1), key({ visible: false })]);

// shadows
const shadowScene = (id, title, light, canvas = { shadows: true }) => R(`shadow.${id}`, title, scene(canvas, bg(), cam(), amb(0.2), light, mesh({ position: [0, 0, 0], castShadow: true }, geo("torusKnotGeometry", [0.5, 0.16, 64, 8]), std()), floor()));
shadowScene("directional", "directionalLight casting a shadow", key({ castShadow: true }));
shadowScene("directional.mapSize", "shadow-mapSize 2048 and a fitted camera", key({ castShadow: true, "shadow-mapSize": [2048, 2048], "shadow-camera-left": -2, "shadow-camera-right": 2, "shadow-camera-top": 2, "shadow-camera-bottom": -2 }));
shadowScene("directional.bias", "shadow-bias and normalBias", key({ castShadow: true, "shadow-bias": -0.002, "shadow-normalBias": 0.02 }));
shadowScene("directional.radius", "shadow-radius on PCF shadows", key({ castShadow: true, "shadow-radius": 4 }), { shadows: "basic" });
shadowScene("spot", "spotLight casting a shadow", h("spotLight", { position: [2, 4, 2], intensity: 40, angle: 0.5, penumbra: 0.3, castShadow: true }));
shadowScene("point", "pointLight casting a shadow (cube shadow map)", h("pointLight", { position: [1.5, 2, 1], intensity: 15, castShadow: true }));
shadowScene("type.basic", "BasicShadowMap", key({ castShadow: true }), { shadows: "basic" });
shadowScene("type.pcf", "PCFShadowMap", key({ castShadow: true }), { shadows: "percentage" });
shadowScene("type.vsm", "VSMShadowMap", key({ castShadow: true }), { shadows: "variance" });
shadowScene("off", "castShadow on, Canvas shadows off: no shadow", key({ castShadow: true }), { shadows: false });
R("shadow.receiveOnly", "a mesh that receives but does not cast", scene({ shadows: true }, bg(), cam(), amb(0.2), key({ castShadow: true }), mesh({ position: [0, 0.2, 0], castShadow: false, receiveShadow: true }, geo("boxGeometry", [1, 1, 1]), std()), floor()));
R("shadow.selfShadow", "a torus knot shadowing itself", scene({ shadows: true }, bg(), cam(), amb(0.2), key({ castShadow: true }), mesh({ castShadow: true, receiveShadow: true }, geo("torusKnotGeometry", [0.7, 0.2, 96, 12]), std())));

// textures
const quad = (m, a = {}) => mesh({ rotation: [-0.35, 0.3, 0] }, geo("planeGeometry", [2.4, 1.8]), mat(m, a));
const T = (id, title, subject, more) => R(`texture.${id}`, title, stage(subject, { more }));
T("map.nearest", "map, NearestFilter (DataTexture default)", quad("meshBasicMaterial", map("checker")));
T("map.linear", "map, LinearFilter", quad("meshBasicMaterial", map("checker", { "map-magFilter": "LinearFilter", "map-minFilter": "LinearFilter" })));
T("map.mipmap", "map, LinearMipmapLinearFilter, minified", mesh({ rotation: [-1.3, 0, 0], position: [0, -0.5, 0] }, geo("planeGeometry", [8, 8]), mat("meshBasicMaterial", map("checker", { "map-repeat": [16, 16], "map-wrapS": "RepeatWrapping", "map-wrapT": "RepeatWrapping", "map-magFilter": "LinearFilter", "map-minFilter": "LinearMipmapLinearFilter", "map-generateMipmaps": true }))));
T("map.anisotropy", "map anisotropy 8 on a grazing floor", mesh({ rotation: [-1.4, 0, 0], position: [0, -0.5, 0] }, geo("planeGeometry", [8, 8]), mat("meshBasicMaterial", map("stripes", { "map-repeat": [8, 8], "map-wrapS": "RepeatWrapping", "map-wrapT": "RepeatWrapping", "map-magFilter": "LinearFilter", "map-minFilter": "LinearMipmapLinearFilter", "map-generateMipmaps": true, "map-anisotropy": 8 }))));
T("map.repeat", "map-repeat with RepeatWrapping", quad("meshBasicMaterial", map("gradient", { "map-repeat": [3, 2], "map-wrapS": "RepeatWrapping", "map-wrapT": "RepeatWrapping" })));
T("map.mirror", "MirroredRepeatWrapping", quad("meshBasicMaterial", map("gradient", { "map-repeat": [3, 2], "map-wrapS": "MirroredRepeatWrapping", "map-wrapT": "MirroredRepeatWrapping" })));
T("map.clamp", "ClampToEdgeWrapping with repeat", quad("meshBasicMaterial", map("gradient", { "map-repeat": [3, 2] })));
T("map.offset", "map-offset", quad("meshBasicMaterial", map("gradient", { "map-offset": [0.25, 0.5], "map-wrapS": "RepeatWrapping", "map-wrapT": "RepeatWrapping" })));
T("map.rotation", "map-rotation about map-center", quad("meshBasicMaterial", map("checker", { "map-rotation": 0.6, "map-center": [0.5, 0.5], "map-wrapS": "RepeatWrapping", "map-wrapT": "RepeatWrapping" })));
T("map.flipY", "map-flipY (DataTexture: false by default)", quad("meshBasicMaterial", map("gradient", { "map-flipY": true })));
T("map.srgb", "an sRGB map lit", quad("meshStandardMaterial", map("gradient")));
T("map.linearSpace", "a map in linear colour space", quad("meshBasicMaterial", map("gradient", { "map-colorSpace": "" })));
T("map.color", "map tinted by color", quad("meshBasicMaterial", map("checker", { color: "#60a0ff" })));
T("map.sphere", "map on a sphere (equirectangular UVs)", mesh({ rotation: [0.3, 0.8, 0] }, geo("sphereGeometry", [1, 48, 24]), std(map("gradient", { color: "#ffffff" }))));
T("map.transparent", "a map with alpha, transparent", quad("meshBasicMaterial", map("disc", { transparent: true })), [mesh({ position: [0, 0, -1] }, geo("boxGeometry", [3, 0.6, 0.2]), std({ color: "#30c060" }))]);
T("alphaMap", "alphaMap", quad("meshBasicMaterial", { color: "#ff9040", alphaMap: "mask@1", transparent: true }), [mesh({ position: [0, 0, -1] }, geo("boxGeometry", [3, 0.6, 0.2]), std({ color: "#30c060" }))]);
T("emissiveMap", "emissiveMap", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ color: "#202020", emissive: "#ffffff", emissiveMap: "emit@1" })));
T("normalMap", "normalMap", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ normalMap: "normal@1" })));
T("normalMap.scale", "normalMap with normalScale", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ normalMap: "normal@1", normalScale: [2, 2] })));
T("bumpMap", "bumpMap", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ bumpMap: "bumps@1", bumpScale: 3 })));
T("roughnessMap", "roughnessMap", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ roughness: 1, roughnessMap: "rough@1", metalness: 0.3 })));
T("metalnessMap", "metalnessMap", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ metalness: 1, metalnessMap: "rough@1", roughness: 0.3 })));
T("aoMap", "aoMap", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ aoMap: "bumps@1", aoMapIntensity: 1 })));
T("displacementMap", "displacementMap", mesh({}, geo("sphereGeometry", [1, 64, 32]), std({ displacementMap: "bumps@1", displacementScale: 0.2 })));
T("lightMap", "lightMap", quad("meshStandardMaterial", { lightMap: "bumps@1", lightMapIntensity: 1.5 }));
T("specularMap", "specularMap on phong", mesh({}, geo("sphereGeometry", [1, 48, 24]), mat("meshPhongMaterial", { color: "#4f8fe0", shininess: 50, specularMap: "rough@1" })));
T("gradientMap", "toon gradientMap", mesh({}, geo("torusKnotGeometry", [0.6, 0.2, 96, 12]), mat("meshToonMaterial", { color: "#4f8fe0", gradientMap: "stripes@1" })));
T("matcap", "matcap texture", mesh({}, geo("torusKnotGeometry", [0.6, 0.2, 96, 12]), mat("meshMatcapMaterial", { matcap: "gradient@1" })));
T("envMap.equirect", "scene environment from an equirectangular texture", mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ metalness: 1, roughness: 0.15, color: "#ffffff" })), [h("texture", { attach: "environment", map: "gradient@1", mapping: "EquirectangularReflectionMapping" })]);
T("background.texture", "scene background as a texture", mesh({}, geo("boxGeometry", [1, 1, 1]), std()), [h("texture", { attach: "background", map: "stars@1" })]);
T("map.update", "a map given again (needsUpdate) draws the new picture", quad("meshBasicMaterial", { map: "checker@2" }));

// scene, colour and tone
const S = (id, title, canvas, more = [], a = {}) => R(`scene.${id}`, title, scene(canvas, bg(a.bg), cam(), amb(0.3), key(), ...(a.subject ? [a.subject] : [ball(), mesh({ position: [1.6, -0.2, -2.5] }, geo("boxGeometry", [1, 1, 1]), std({ color: "#e0a040" }))]), ...more));
S("background.color", "background colour", {});
S("fog.linear", "fog (linear)", {}, [h("fog", { attach: "fog", args: ["#20252e", 3, 8] })]);
S("fog.exp2", "fogExp2", {}, [h("fogExp2", { attach: "fog", args: ["#20252e", 0.18] })]);
for (const t of ["NoToneMapping", "LinearToneMapping", "ReinhardToneMapping", "CineonToneMapping", "ACESFilmicToneMapping", "AgXToneMapping", "NeutralToneMapping"]) S(`tone.${t.replace("ToneMapping", "").toLowerCase()}`, `gl.toneMapping = ${t}`, { "gl-toneMapping": t });
S("tone.exposure", "toneMappingExposure 2", { "gl-toneMappingExposure": 2 });
S("canvas.flat", "<Canvas flat> (no tone mapping)", { flat: true });
S("canvas.linear", "<Canvas linear> (no sRGB output)", { linear: true });
S("canvas.default", "<Canvas> defaults: ACES, sRGB, camera at [0, 0, 5] fov 75", {}, [], {});
R("scene.camera.default", "no camera in the scene: R3F's default", scene({}, bg(), amb(0.3), key(), ball()));
S("environmentIntensity", "scene environment intensity (<Canvas scene={{ environmentIntensity }}>)", { "scene-environmentIntensity": 0.4 }, [h("texture", { attach: "environment", map: "gradient@1", mapping: "EquirectangularReflectionMapping" })], { subject: mesh({}, geo("sphereGeometry", [1, 48, 24]), std({ metalness: 1, roughness: 0.2, color: "#ffffff" })) });
S("background.blurriness", "background texture blurriness", { "scene-backgroundBlurriness": 0.5 }, [h("texture", { attach: "background", map: "stars@1", mapping: "EquirectangularReflectionMapping" })]);
S("overrideMaterial", "scene.overrideMaterial", {}, [h("meshNormalMaterial", { attach: "overrideMaterial" })]);
R("scene.clearAlpha", "no background: the canvas stays transparent", scene({}, cam(), amb(0.3), key(), ball()));

// objects
const O = (id, title, subject, extra = {}) => R(`object.${id}`, title, stage(subject, extra));
O("sprite", "sprite", h("sprite", { scale: [1.2, 1.2, 1] }, mat("spriteMaterial", map("disc", { transparent: true }))));
O("sprite.alphaTest", "sprite with alphaTest", h("sprite", { scale: [1.2, 1.2, 1] }, mat("spriteMaterial", map("glyph", { alphaTest: 0.5 }))));
O("sprite.rotation", "spriteMaterial rotation", h("sprite", { scale: [1.2, 1.2, 1] }, mat("spriteMaterial", map("checker", { rotation: 0.7 }))));
O("sprite.center", "sprite center [0, 0]", h("sprite", { scale: [1.2, 1.2, 1], center: [0, 0] }, mat("spriteMaterial", { color: "#ff9040" })));
O("sprite.sizeAttenuation", "sprite sizeAttenuation false", h("sprite", { scale: [0.2, 0.2, 1] }, mat("spriteMaterial", { color: "#ff9040", sizeAttenuation: false })));
O("sprite.additive", "additive glow sprite over a sphere", h("group", {}, ball({ color: "#202020" }), h("sprite", { scale: [3, 3, 1] }, mat("spriteMaterial", map("disc", { transparent: true, blending: "AdditiveBlending", depthWrite: false })))));
O("points", "points", h("points", {}, h("sphereGeometry", { args: [1, 16, 12] }), mat("pointsMaterial", { size: 0.08, color: "#ffd060" })));
O("points.noAttenuation", "points sizeAttenuation false", h("points", {}, h("sphereGeometry", { args: [1, 16, 12] }), mat("pointsMaterial", { size: 3, sizeAttenuation: false, color: "#ffd060" })));
O("points.map", "points with a round map", h("points", {}, h("sphereGeometry", { args: [1, 12, 8] }), mat("pointsMaterial", map("disc", { size: 0.25, transparent: true, depthWrite: false }))));
O("points.vertexColors", "points with vertex colours", h("points", {}, h("sphereGeometry", { args: [1, 16, 12] }, attr("attributes-color", hueColors(17 * 13), 3)), mat("pointsMaterial", { size: 0.1, vertexColors: true })));
const POLY = [[-1, -0.6, 0], [-0.3, 0.7, 0], [0.4, -0.5, 0], [1.1, 0.6, 0]];
O("line", "line", h("line", {}, h("bufferGeometry", {}, pts(POLY)), mat("lineBasicMaterial", { color: "#80e0ff" })));
O("lineLoop", "lineLoop", h("lineLoop", {}, h("circleGeometry", { args: [1, 6] }), mat("lineBasicMaterial", { color: "#80e0ff" })));
O("lineSegments", "lineSegments", h("lineSegments", {}, h("bufferGeometry", {}, pts([[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0]])), mat("lineBasicMaterial", { color: "#80e0ff" })));
O("line.dashed", "lineDashedMaterial with a lineDistance attribute", h("line", {}, h("bufferGeometry", {}, pts(POLY), attr("attributes-lineDistance", lineDist(POLY), 1)), mat("lineDashedMaterial", { color: "#80e0ff", dashSize: 0.15, gapSize: 0.08 })));
O("group", "nested groups compose transforms", h("group", { position: [0.5, 0, 0], rotation: [0, 0.6, 0] }, h("group", { position: [0, 0.5, 0], scale: 0.7 }, mesh({ position: [-1, 0, 0] }, geo("boxGeometry"), std()), mesh({ position: [1, 0, 0], rotation: [0.4, 0, 0.3] }, geo("boxGeometry"), std({ color: "#e0a040" })))));
O("renderOrder", "renderOrder decides two transparent meshes' order", h("group", {}, mesh({ position: [-0.3, 0, 0], renderOrder: 2 }, geo("planeGeometry", [1.5, 1.5]), mat("meshBasicMaterial", { color: "#ff4040", transparent: true, opacity: 0.6 })), mesh({ position: [0.3, 0, 0.2], renderOrder: 1 }, geo("planeGeometry", [1.5, 1.5]), mat("meshBasicMaterial", { color: "#4040ff", transparent: true, opacity: 0.6 }))));
O("transparentSort", "transparent meshes drawn far to near", h("group", {}, ...[-1.5, -0.5, 0.5].map((z, i) => mesh({ position: [(i - 1) * 0.5, 0, z] }, geo("planeGeometry", [1.6, 1.6]), mat("meshBasicMaterial", { color: ["#ff4040", "#40ff40", "#4040ff"][i], transparent: true, opacity: 0.5 })))));
O("visible", "visible={false} hides a mesh", h("group", {}, mesh({ position: [-0.8, 0, 0], visible: false }, geo("boxGeometry"), std()), mesh({ position: [0.8, 0, 0] }, geo("boxGeometry"), std({ color: "#e0a040" }))));
// a 4x4 plane (Three's vertex order: rows from the top) and a dome as its morph target
const GRID = []; for (let iy = 0; iy <= 4; iy++) for (let ix = 0; ix <= 4; ix++) GRID.push([ix / 2 - 1, 1 - iy / 2]);
O("morphTargets", "morph target influence (morphAttributes-position-0)", mesh({ morphTargetInfluences: [0.7], rotation: [-0.6, 0, 0] }, h("planeGeometry", { args: [2, 2, 4, 4] }, attr("morphAttributes-position-0", GRID.map(([x, y]) => [x, y, 0.8 * (1 - x * x) * (1 - y * y)]).flat(), 3)), std({ side: "DoubleSide" })));
O("frustumCulled", "an object outside the view is culled, inside drawn", h("group", {}, mesh({ position: [0, 0, 0], frustumCulled: true }, geo("boxGeometry"), std()), mesh({ position: [40, 0, 0] }, geo("boxGeometry"), std())));
O("scaleNegative", "a negative scale (mirrored winding)", mesh({ scale: [-1, 1, 1], rotation: [0.4, 0.6, 0] }, geo("torusKnotGeometry", [0.6, 0.2, 96, 12]), std()));

// cameras
const C = (id, title, camera, canvas = {}) => R(`camera.${id}`, title, scene(canvas, bg(), camera, amb(0.3), key(), ball(), mesh({ position: [1.6, -0.2, -2.5] }, geo("boxGeometry", [1, 1, 1]), std({ color: "#e0a040" })), floor()));
C("fov", "perspective fov 30", cam({ fov: 30, position: [0, 2, 8] }));
C("near", "near plane cuts the sphere", cam({ near: 3.6, position: [0, 0, 4.4], lookAt: [0, 0, 0] }));
C("far", "far plane drops the box", cam({ far: 5 }));
C("zoom", "perspective zoom 2", cam({ zoom: 2 }));
C("up", "camera up [1, 0, 0]", cam({ up: [1, 0, 0] }));
C("rotation", "camera rotation instead of lookAt", h("perspectiveCamera", { position: [0, 0.5, 4], rotation: [-0.1, 0.2, 0], fov: 50 }));
C("orthographic", "orthographicCamera", h("orthographicCamera", { position: [0, 2, 6], lookAt: [0, 0, 0], args: [-3, 3, 2.25, -2.25, 0.1, 50] }));
C("orthographic.zoom", "orthographicCamera zoom 2", h("orthographicCamera", { position: [0, 2, 6], lookAt: [0, 0, 0], zoom: 2, args: [-3, 3, 2.25, -2.25, 0.1, 50] }));
C("orthographic.canvas", "<Canvas orthographic> camera", h("orthographicCamera", { makeDefault: true, position: [0, 2, 6], zoom: 50, lookAt: [0, 0, 0] }), { orthographic: true });

// ---- r3f items (declarative semantics, drawn and compared too) -----------------
const X = (id, title, tree) => add("r3f", id, title, tree);
const sub = (a, m = std()) => mesh(a, geo("boxGeometry", [1, 1, 1]), m);
X("props.position", "position={[x, y, z]}", stage(sub({ position: [0.8, -0.3, 0.2] })));
X("props.positionDash", "position-x / position-y pierced props", stage(sub({ "position-x": 0.8, "position-y": -0.3 })));
X("props.rotationOrder", "rotation={[x, y, z, 'YXZ']} keeps the order", stage(sub({ rotation: [0.6, 0.8, 0.2, "YXZ"] })));
X("props.rotationDash", "rotation-y", stage(sub({ "rotation-y": 0.7, "rotation-x": 0.3 })));
X("props.scaleScalar", "scale={1.6} sets all three", stage(sub({ scale: 1.6 })));
X("props.scaleDash", "scale-y", stage(sub({ "scale-y": 1.8 })));
X("props.quaternion", "quaternion={[x, y, z, w]}", stage(sub({ quaternion: [0.2, 0.3, 0.1, 0.927] })));
X("props.lookAt", "lookAt on a mesh", stage(sub({ lookAt: [2, 2, 2] })));
X("props.up", "up and lookAt", stage(sub({ up: [1, 0, 0], lookAt: [0, 2, 2] })));
X("props.colorName", "color=\"hotpink\" (CSS name)", stage(sub({}, std({ color: "hotpink" }))));
X("props.colorHex", "color={0xff8800} (number)", stage(sub({}, std({ color: 0xff8800 }))));
X("props.colorArray", "color={[r, g, b]}", stage(sub({}, std({ color: [0.2, 0.8, 0.4] }))));
X("props.colorHsl", "color=\"hsl(200, 80%, 50%)\"", stage(sub({}, std({ color: "hsl(200, 80%, 50%)" }))));
X("props.colorRgb", "color=\"rgb(255, 120, 40)\"", stage(sub({}, std({ color: "rgb(255, 120, 40)" }))));
X("props.colorDash", "material-color on the mesh", stage(sub({ "material-color": "#ff4080" })));
X("props.materialDash", "material-roughness / material-metalness on the mesh", stage(mesh({ "material-roughness": 0.1, "material-metalness": 1 }, geo("sphereGeometry", [1, 48, 24]), std({ color: "#d0a060" }))));
X("props.geometryArgs", "args re-make the geometry", stage(mesh({}, geo("boxGeometry", [2, 0.5, 0.5]), std())));
X("props.sideConstant", "side={THREE.DoubleSide} as a number (2)", stage(mesh({ rotation: [0, 2.6, 0] }, geo("planeGeometry", [2, 2]), std({ side: 2 }))));
X("props.visibleFalse", "visible={false}", stage(sub({ visible: false })));
X("props.userData", "userData passes through", stage(sub({ userData: { a: 1 } })));
X("props.name", "name", stage(sub({ name: "box" })));
X("props.castShadow", "castShadow / receiveShadow booleans", scene({ shadows: true }, bg(), cam(), amb(0.2), key({ castShadow: true }), sub({ castShadow: true }), floor()));
X("props.shadowDash", "shadow-mapSize-width dash path", scene({ shadows: true }, bg(), cam(), amb(0.2), key({ castShadow: true, "shadow-mapSize-width": 1024, "shadow-mapSize-height": 1024 }), sub({ castShadow: true }), floor()));
X("attach.material", "attach=\"material\" (default for materials)", stage(mesh({}, geo("boxGeometry"), h("meshStandardMaterial", { attach: "material", color: "#40c080" }))));
X("attach.geometry", "attach=\"geometry\" (default for geometries)", stage(mesh({}, h("sphereGeometry", { attach: "geometry", args: [1, 24, 12] }), std())));
X("attach.materialArray", "attach=\"material-0\" … per face group", stage(mesh({ rotation: [0.5, 0.7, 0] }, geo("boxGeometry", [1.3, 1.3, 1.3]), ...["#ff4040", "#40ff40", "#4040ff", "#ffff40", "#ff40ff", "#40ffff"].map((c, i) => h("meshStandardMaterial", { attach: `material-${i}`, color: c })))));
X("attach.background", "<color attach=\"background\">", scene({}, h("color", { attach: "background", args: ["#503020"] }), cam(), amb(), key(), ball()));
X("attach.fog", "<fog attach=\"fog\">", scene({}, bg(), cam(), amb(), key(), ball(), h("fog", { attach: "fog", args: ["#20252e", 3, 5] })));
X("attach.map", "<dataTexture attach=\"map\"> inside a material", stage(mesh({ rotation: [-0.35, 0.3, 0] }, geo("planeGeometry", [2.4, 1.8]), h("meshBasicMaterial", {}, h("dataTexture", { attach: "map", image: "checker@1" })))));
X("attach.bufferAttribute", "<bufferAttribute attach=\"attributes-position\">", R_last());
X("group.keys", "children reordered by key keep their objects", stage(h("group", {}, sub({ key: "b", position: [0.8, 0, 0] }), sub({ key: "a", position: [-0.8, 0, 0] }, std({ color: "#e0a040" })))));
X("camera.makeDefault", "makeDefault on a second camera", scene({}, bg(), cam(), h("perspectiveCamera", { makeDefault: true, position: [3, 2, 3], lookAt: [0, 0, 0], fov: 40 }), amb(), key(), ball(), floor()));
X("camera.canvasProp", "<Canvas camera={{ position, fov }}>", scene({ camera: { position: [2, 2, 4], fov: 40 } }, bg(), amb(), key(), ball(), floor()));
X("canvas.dpr", "<Canvas dpr={2}> draws at twice the pixels", scene({ dpr: 2 }, bg(), cam(), amb(), key(), mesh({ rotation: [0.4, 0.6, 0] }, geo("torusKnotGeometry", [0.6, 0.2, 96, 12]), std())));
X("canvas.shadowsSoft", "<Canvas shadows=\"soft\"> (PCSS)", scene({ shadows: "soft" }, bg(), cam(), amb(0.2), key({ castShadow: true }), sub({ castShadow: true }), floor()));
X("canvas.glToneMapping", "<Canvas gl={{ toneMapping: NoToneMapping }}>", scene({ "gl-toneMapping": "NoToneMapping" }, bg(), cam(), amb(), key(), ball()));
X("canvas.frameloopDemand", "<Canvas frameloop=\"demand\"> draws on invalidate", stage(sub({})));
X("constants.byName", "constants given by name (\"DoubleSide\", \"AdditiveBlending\")", stage(mesh({ rotation: [0, 2.6, 0] }, geo("planeGeometry", [2, 2]), std({ side: "DoubleSide" }))));
// r3f behaviour that is not a picture: run in the kit (the meter's kit pass)
const K = (id, title, test) => add("r3f", id, title, null, { kit: test });
K("hooks.useFrame", "useFrame((state, delta) => ...) runs each frame with delta", "useFrame");
K("hooks.useFramePriority", "useFrame with a priority takes over rendering (manual render)", "useFramePriority");
K("hooks.useThree", "useThree() → { camera, scene, size, viewport, gl, clock }", "useThree");
K("hooks.useThreeSelector", "useThree(state => state.size) selector", "useThreeSelector");
K("hooks.useLoaderTexture", "useLoader(TextureLoader, url) with Suspense", "useLoaderTexture");
K("hooks.useLoaderGLTF", "useLoader(GLTFLoader, url)", "useLoaderGLTF");
K("hooks.useGraph", "useGraph(scene) → nodes and materials by name", "useGraph");
K("hooks.ref", "ref={r} gives the THREE object (r.current.position.x += …)", "ref");
K("hooks.refMutation", "mutating ref.current in useFrame moves the object without re-render", "refMutation");
K("events.onClick", "onClick on a mesh (raycast from the pointer)", "onClick");
K("events.onPointerOver", "onPointerOver / onPointerOut", "onPointerOver");
K("events.onPointerMove", "onPointerMove gives point, uv, distance", "onPointerMove");
K("events.onPointerDown", "onPointerDown / onPointerUp", "onPointerDown");
K("events.onDoubleClick", "onDoubleClick", "onDoubleClick");
K("events.onContextMenu", "onContextMenu", "onContextMenu");
K("events.onWheel", "onWheel", "onWheel");
K("events.stopPropagation", "e.stopPropagation() stops the nearer-first bubbling", "stopPropagation");
K("events.pointerMissed", "onPointerMissed on the Canvas", "onPointerMissed");
K("events.eventObject", "event.object vs event.eventObject in groups", "eventObject");
K("events.instanceId", "events on instancedMesh give instanceId", "instanceId");
K("state.invalidate", "invalidate() asks one frame in demand mode", "invalidate");
K("state.advance", "advance(timestamp) steps a frame by hand", "advance");
K("state.clock", "state.clock.elapsedTime grows", "clock");
K("state.viewport", "viewport.width/height in world units at z 0", "viewport");
K("state.pointer", "state.pointer in normalized device coordinates", "pointer");
K("state.raycaster", "state.raycaster set from the pointer", "raycaster");
K("extend", "extend({ MyMesh }) registers a custom element", "extend");
K("portal", "createPortal(children, scene) draws into another scene", "portal");
K("dispose", "unmounting disposes geometry, material and textures", "dispose");
K("disposeNull", "dispose={null} keeps a shared resource", "disposeNull");
K("suspense", "Suspense fallback while a loader runs", "suspense");
K("onUpdate", "onUpdate={(self) => ...} after props apply", "onUpdate");
K("args.change", "changing args re-creates the object, keeps its children", "argsChange");
K("textureProps", "texture-repeat / map-wrapS set on a texture prop", "textureProps");
K("instancedMesh", "instancedMesh matrices set from a ref (setMatrixAt) are drawn", "instancedMesh");
K("instancedMeshColors", "instancedMesh setColorAt colours are drawn", "instancedMeshColors");
K("skinnedMesh", "a skinnedMesh with a skeleton made in code bends with its bone", "skinnedMesh");
K("lod", "<lOD> levels added in code switch by distance", "lod");
K("primitive", "<primitive object={mesh}> mounts an object made with THREE", "primitive");
K("viewOffset", "camera.setViewOffset from a ref draws the tile", "viewOffset");
K("cameraManual", "camera manual={true} keeps its aspect", "cameraManual");
K("resize", "resizing the canvas updates the camera aspect and size", "resize");

// ---- examples (three.js's own examples, ported as .tsx documents) -------------
export const EXAMPLES = [
  "webgl_animation_keyframes", "webgl_animation_skinning_blending", "webgl_animation_skinning_morph",
  "webgl_buffergeometry", "webgl_buffergeometry_custom_attributes_particles", "webgl_buffergeometry_drawrange",
  "webgl_buffergeometry_instancing", "webgl_buffergeometry_lines", "webgl_buffergeometry_points",
  "webgl_camera", "webgl_camera_array", "webgl_clipping", "webgl_decals", "webgl_effects_anaglyph",
  "webgl_geometries", "webgl_geometry_colors", "webgl_geometry_convex", "webgl_geometry_cube",
  "webgl_geometry_extrude_shapes", "webgl_geometry_shapes", "webgl_geometry_teapot", "webgl_geometry_text",
  "webgl_instancing_dynamic", "webgl_instancing_raycast", "webgl_interactive_cubes", "webgl_interactive_points",
  "webgl_lensflares", "webgl_lightprobe", "webgl_lights_hemisphere", "webgl_lights_physical",
  "webgl_lights_pointlights", "webgl_lights_spotlight", "webgl_lines_dashed", "webgl_lod",
  "webgl_materials_bumpmap", "webgl_materials_car", "webgl_materials_envmaps", "webgl_materials_normalmap",
  "webgl_materials_physical_clearcoat", "webgl_materials_physical_transmission", "webgl_materials_texture_filters",
  "webgl_materials_toon", "webgl_math_obb", "webgl_morphtargets", "webgl_points_sprites",
  "webgl_postprocessing_unreal_bloom", "webgl_shadowmap", "webgl_shadowmap_pointlight", "webgl_sprites",
  "webgl_loader_gltf",
];
for (const ex of EXAMPLES) add("example", ex, `three.js example ${ex} as a .tsx document`, null, { example: ex });

// ---- performance (the GPU path against three.js in the same browser) ----------
for (const [id, title] of [
  ["frame.1kMeshes", "1000 meshes: frame time within 1.5x of three.js"],
  ["frame.100kTris", "100k triangles: frame time within 1.5x of three.js"],
  ["frame.instanced10k", "10k instances: frame time within 1.5x of three.js"],
  ["frame.shadows", "shadowed scene: frame time within 1.5x of three.js"],
  ["frame.textures", "32 textures: frame time within 1.5x of three.js"],
  ["update.sceneDiff", "a frame that moves 100 objects sends a diff, not the scene"],
  ["memory.stable", "1000 frames of an animated scene: memory stable"],
  ["startup.module", "first frame of a scene under 300 ms after the module is in"],
]) add("perf", id, title, null, { perf: id });

function R_last() {
  return stage(mesh({}, h("bufferGeometry", {}, h("bufferAttribute", { attach: "attributes-position", args: [[-1, -0.8, 0, 1, -0.8, 0, 0, 0.9, 0], 3] })), mat("meshBasicMaterial", { color: "#40e0a0", side: "DoubleSide" })));
}

export const SCENE_ITEMS = items;
