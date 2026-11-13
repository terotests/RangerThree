// Playground examples: plain three.js code. `renderer` is a ready
// THREE.WebGLRenderer on the page's canvas.
var EXAMPLES = {
  "Torus knot": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1b1e26);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 1.5, 5);
camera.lookAt(0, 0, 0);

const knot = new THREE.Mesh(
  new THREE.TorusKnotGeometry(1, 0.32, 96, 10),
  new THREE.MeshStandardMaterial({ color: 0xff8a3d })
);
scene.add(knot);
scene.add(new THREE.AmbientLight(0xffffff, 0.25));
const sun = new THREE.DirectionalLight(0xffffff, 2.5);
sun.position.set(3, 4, 2);
scene.add(sun);

renderer.setAnimationLoop((t) => {
  knot.rotation.x = t / 2000;
  knot.rotation.y = t / 1300;
  renderer.render(scene, camera);
});`,

  "Extruded shape": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0xeef1f6);
const camera = new THREE.PerspectiveCamera(45, 4 / 3, 0.1, 200);
camera.position.set(0, 0, 40);

const heart = new THREE.Shape();
heart.moveTo(2.5, 2.5);
heart.bezierCurveTo(2.5, 2.5, 2, 0, 0, 0);
heart.bezierCurveTo(-3, 0, -3, 3.5, -3, 3.5);
heart.bezierCurveTo(-3, 5.5, -1, 7.7, 2.5, 9.5);
heart.bezierCurveTo(6, 7.7, 8, 5.5, 8, 3.5);
heart.bezierCurveTo(8, 3.5, 8, 0, 5, 0);
heart.bezierCurveTo(3.5, 0, 2.5, 2.5, 2.5, 2.5);

const geometry = new THREE.ExtrudeGeometry(heart, { depth: 2, bevelSegments: 3, bevelSize: 0.6, bevelThickness: 0.8 });
geometry.center();
const mesh = new THREE.Mesh(geometry, new THREE.MeshPhongMaterial({ color: 0xd8344f }));
mesh.rotation.z = Math.PI;
scene.add(mesh);
scene.add(new THREE.HemisphereLight(0xffffff, 0x445566, 1.2));
const key = new THREE.DirectionalLight(0xffffff, 1.5);
key.position.set(-5, 10, 20);
scene.add(key);

renderer.setAnimationLoop((t) => {
  mesh.rotation.y = Math.sin(t / 900) * 0.8;
  renderer.render(scene, camera);
});`,

  "Tube along a curve": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x10131a);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 3, 9);
camera.lookAt(0, 0, 0);

const path = new THREE.CatmullRomCurve3([
  new THREE.Vector3(-3, 0, 0), new THREE.Vector3(-1, 2, 1),
  new THREE.Vector3(1, -1, 2), new THREE.Vector3(3, 1, 0),
  new THREE.Vector3(1, 0, -2), new THREE.Vector3(-1, -1, -1)
], true);
const tube = new THREE.Mesh(new THREE.TubeGeometry(path, 160, 0.35, 10, true), new THREE.MeshNormalMaterial());
scene.add(tube);

renderer.setAnimationLoop((t) => {
  tube.rotation.y = t / 2500;
  renderer.render(scene, camera);
});`,

  "Primitives and lights": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x222831);
const camera = new THREE.PerspectiveCamera(55, 4 / 3, 0.1, 100);
camera.position.set(0, 4, 9);
camera.lookAt(0, 0, 0);

const floor = new THREE.Mesh(new THREE.PlaneGeometry(14, 14), new THREE.MeshLambertMaterial({ color: 0x8892a0 }));
floor.rotation.x = -Math.PI / 2;
floor.position.y = -1;
scene.add(floor);

const shapes = [
  new THREE.BoxGeometry(1.4, 1.4, 1.4),
  new THREE.SphereGeometry(0.9, 32, 16),
  new THREE.ConeGeometry(0.9, 1.8, 24),
  new THREE.IcosahedronGeometry(0.9, 0),
  new THREE.TorusGeometry(0.7, 0.28, 12, 32),
];
const colors = [0x4cc9f0, 0xf72585, 0xffd166, 0x06d6a0, 0xf8961e];
const group = new THREE.Group();
shapes.forEach((g, i) => {
  const m = new THREE.Mesh(g, new THREE.MeshStandardMaterial({ color: colors[i] }));
  const a = (i / shapes.length) * Math.PI * 2;
  m.position.set(Math.cos(a) * 3, 0, Math.sin(a) * 3);
  group.add(m);
});
scene.add(group);
scene.add(new THREE.AmbientLight(0xffffff, 0.2));
const lamp = new THREE.PointLight(0xffffff, 3);
lamp.position.set(0, 3, 0);
scene.add(lamp);

renderer.setAnimationLoop((t) => {
  group.rotation.y = t / 3000;
  group.children.forEach((m, i) => { m.rotation.x = t / (900 + i * 200); m.rotation.y = t / 1100; });
  renderer.render(scene, camera);
});`,

  "Edges and wireframe": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0xf4f4f0);
const camera = new THREE.PerspectiveCamera(45, 4 / 3, 0.1, 100);
camera.position.set(0, 2.5, 7);
camera.lookAt(0, 0, 0);

// EdgesGeometry keeps the edges between faces that meet at more than
// the threshold angle; WireframeGeometry keeps every triangle edge
const geometry = new THREE.DodecahedronGeometry(1.3, 0);
const solid = new THREE.Mesh(geometry, new THREE.MeshLambertMaterial({ color: 0x5b8def, polygonOffset: true, polygonOffsetFactor: 1 }));
solid.add(new THREE.LineSegments(new THREE.EdgesGeometry(geometry), new THREE.LineBasicMaterial({ color: 0x1b2a4a })));
solid.position.x = -1.8;
const wire = new THREE.LineSegments(new THREE.WireframeGeometry(new THREE.TorusKnotGeometry(0.9, 0.3, 64, 8)), new THREE.LineBasicMaterial({ color: 0xd8344f }));
wire.position.x = 1.8;
scene.add(solid, wire);
scene.add(new THREE.AmbientLight(0xffffff, 1.5));
const light = new THREE.DirectionalLight(0xffffff, 4);
light.position.set(2, 3, 4);
scene.add(light);

renderer.setAnimationLoop((t) => {
  solid.rotation.y = t / 1500;
  wire.rotation.y = -t / 2000;
  wire.rotation.x = t / 3000;
  renderer.render(scene, camera);
});`
};

// ---- more involved scenes ------------------------------------------------
// `orbit(camera, target)` is a small drag-to-orbit, wheel-to-zoom helper the
// playground page provides (like the OrbitControls addon).
Object.assign(EXAMPLES, {
  "Instancing: a field of cubes": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0e1116);
scene.fog = new THREE.Fog(0x0e1116, 14, 32);
const camera = new THREE.PerspectiveCamera(55, 4 / 3, 0.1, 100);
camera.position.set(0, 9, 16);
orbit(camera, new THREE.Vector3(0, 0, 0));

const N = 18;
const mesh = new THREE.InstancedMesh(new THREE.BoxGeometry(0.8, 0.8, 0.8), new THREE.MeshStandardMaterial({ roughness: 0.4 }), N * N);
const dummy = new THREE.Object3D();
const color = new THREE.Color();
scene.add(mesh);
scene.add(new THREE.HemisphereLight(0xbfd8ff, 0x202020, 1.2));
const sun = new THREE.DirectionalLight(0xffffff, 1.6);
sun.position.set(5, 10, 4);
scene.add(sun);

renderer.setAnimationLoop((t) => {
  let i = 0;
  for (let x = 0; x < N; x++) for (let z = 0; z < N; z++) {
    const px = x - N / 2 + 0.5, pz = z - N / 2 + 0.5;
    const d = Math.hypot(px, pz);
    const h = Math.sin(d * 0.6 - t / 400) * 0.5 + 0.5;
    dummy.position.set(px, h * 1.5, pz);
    dummy.rotation.set(0, d * 0.2 + t / 2000, 0);
    dummy.scale.set(1, 0.4 + h * 1.6, 1);
    dummy.updateMatrix();
    mesh.setMatrixAt(i, dummy.matrix);
    mesh.setColorAt(i, color.setHSL(0.55 + h * 0.35, 0.7, 0.5));
    i++;
  }
  mesh.instanceMatrix.needsUpdate = true;
  renderer.render(scene, camera);
});`,

  "Ocean waves (vertex animation)": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0xa9c8e8);
scene.fog = new THREE.FogExp2(0xa9c8e8, 0.03);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 200);
camera.position.set(0, 4, 14);
orbit(camera, new THREE.Vector3(0, 0, 0));

const geometry = new THREE.PlaneGeometry(40, 40, 48, 48);
geometry.rotateX(-Math.PI / 2);
const count = geometry.attributes.position.count;
geometry.setAttribute("color", new THREE.Float32BufferAttribute(new Array(count * 3).fill(1), 3));
const sea = new THREE.Mesh(geometry, new THREE.MeshPhongMaterial({ vertexColors: true, shininess: 40, specular: 0x335577 }));
scene.add(sea);

const buoy = new THREE.Mesh(new THREE.CylinderGeometry(0.4, 0.6, 1.4, 16), new THREE.MeshStandardMaterial({ color: 0xe0452b }));
scene.add(buoy);
scene.add(new THREE.AmbientLight(0xffffff, 0.35));
const sun = new THREE.DirectionalLight(0xfff2dd, 2);
sun.position.set(-6, 8, -10);
scene.add(sun);

const wave = (x, z, t) => Math.sin(x * 0.35 + t) * 0.6 + Math.sin(z * 0.5 + t * 1.3) * 0.4 + Math.sin((x + z) * 0.9 + t * 2) * 0.12;
renderer.setAnimationLoop((ms) => {
  const t = ms / 1000;
  const p = geometry.attributes.position, c = geometry.attributes.color;
  for (let i = 0; i < count; i++) {
    const y = wave(p.getX(i), p.getZ(i), t);
    p.setY(i, y);
    const k = (y + 1.1) / 2.2;
    c.setXYZ(i, 0.05 + k * 0.3, 0.25 + k * 0.4, 0.45 + k * 0.4);
  }
  p.needsUpdate = true; c.needsUpdate = true;
  geometry.computeVertexNormals();
  buoy.position.y = wave(0, 0, t) + 0.3;
  buoy.rotation.z = Math.sin(t * 1.3) * 0.2;
  renderer.render(scene, camera);
});`,

  "Solar system (scene graph)": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x02030a);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 500);
camera.position.set(0, 18, 34);
orbit(camera, new THREE.Vector3(0, 0, 0));

// stars
const starPos = [];
for (let i = 0; i < 1500; i++) {
  const v = new THREE.Vector3().randomDirection().multiplyScalar(120 + Math.random() * 60);
  starPos.push(v.x, v.y, v.z);
}
const stars = new THREE.BufferGeometry();
stars.setAttribute("position", new THREE.Float32BufferAttribute(starPos, 3));
scene.add(new THREE.Points(stars, new THREE.PointsMaterial({ color: 0xffffff, size: 1.5, sizeAttenuation: false })));

const sun = new THREE.Mesh(new THREE.SphereGeometry(3, 32, 16), new THREE.MeshBasicMaterial({ color: 0xffcc55 }));
scene.add(sun);
scene.add(new THREE.PointLight(0xffffff, 900, 0, 2));
scene.add(new THREE.AmbientLight(0xffffff, 0.08));

const planets = [
  { r: 0.5, d: 6, s: 1.6, c: 0xb1a79b },
  { r: 0.9, d: 9, s: 1.2, c: 0xe8c27a },
  { r: 1.0, d: 13, s: 1.0, c: 0x3d7bd9, moon: true },
  { r: 0.7, d: 17, s: 0.8, c: 0xc1440e },
  { r: 2.0, d: 24, s: 0.45, c: 0xd2b48c, ring: true },
];
const pivots = planets.map((p) => {
  const pivot = new THREE.Group();
  const planet = new THREE.Mesh(new THREE.SphereGeometry(p.r, 24, 12), new THREE.MeshStandardMaterial({ color: p.c, roughness: 0.8 }));
  planet.position.x = p.d;
  pivot.add(planet);
  if (p.moon) {
    const moonPivot = new THREE.Group();
    moonPivot.position.x = p.d;
    const moon = new THREE.Mesh(new THREE.SphereGeometry(0.27, 16, 8), new THREE.MeshStandardMaterial({ color: 0xcccccc }));
    moon.position.x = 1.8;
    moonPivot.add(moon);
    pivot.add(moonPivot);
    p.moonPivot = moonPivot;
  }
  if (p.ring) {
    const ring = new THREE.Mesh(new THREE.RingGeometry(2.6, 4, 48), new THREE.MeshStandardMaterial({ color: 0xbfa77a, side: THREE.DoubleSide }));
    ring.position.x = p.d;
    ring.rotation.x = -Math.PI / 2.4;
    pivot.add(ring);
  }
  // orbit line
  const pts = [];
  for (let i = 0; i < 96; i++) { const a = (i / 96) * Math.PI * 2; pts.push(new THREE.Vector3(Math.cos(a) * p.d, 0, Math.sin(a) * p.d)); }
  scene.add(new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints(pts), new THREE.LineBasicMaterial({ color: 0x334055 })));
  pivot.rotation.y = Math.random() * Math.PI * 2;
  scene.add(pivot);
  return { pivot, planet, p };
});

renderer.setAnimationLoop(() => {
  pivots.forEach(({ pivot, planet, p }) => {
    pivot.rotation.y += 0.004 * p.s;
    planet.rotation.y += 0.02;
    if (p.moonPivot) p.moonPivot.rotation.y += 0.05;
  });
  renderer.render(scene, camera);
});`,

  "Raycaster: hover to pick": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0xf0f0f0);
const camera = new THREE.PerspectiveCamera(60, 4 / 3, 0.1, 100);
camera.position.set(0, 0, 14);

const geometry = new THREE.BoxGeometry(1, 1, 1);
const boxes = [];
for (let i = 0; i < 60; i++) {
  const m = new THREE.Mesh(geometry, new THREE.MeshLambertMaterial({ color: Math.random() * 0xffffff }));
  m.position.set((Math.random() - 0.5) * 14, (Math.random() - 0.5) * 9, (Math.random() - 0.5) * 8);
  m.rotation.set(Math.random() * 6, Math.random() * 6, 0);
  m.scale.setScalar(0.6 + Math.random() * 0.8);
  scene.add(m);
  boxes.push(m);
}
scene.add(new THREE.AmbientLight(0xffffff, 0.6));
const light = new THREE.DirectionalLight(0xffffff, 2.5);
light.position.set(1, 1, 1);
scene.add(light);

const raycaster = new THREE.Raycaster();
const pointer = new THREE.Vector2(10, 10);
canvas.onpointermove = (e) => {
  const r = canvas.getBoundingClientRect();
  pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
};
let picked = null;
renderer.setAnimationLoop((t) => {
  const group = scene;
  camera.position.x = Math.sin(t / 4000) * 3;
  camera.lookAt(0, 0, 0);
  raycaster.setFromCamera(pointer, camera);
  const hit = raycaster.intersectObjects(boxes)[0];
  const obj = hit ? hit.object : null;
  if (obj !== picked) {
    if (picked) picked.material.emissive.setHex(0x000000);
    if (obj) obj.material.emissive.setHex(0xff2040);
    picked = obj;
  }
  renderer.render(scene, camera);
});`,

  "Materials: one shape, eight looks": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x30343c);
const camera = new THREE.PerspectiveCamera(40, 4 / 3, 0.1, 100);
camera.position.set(0, 3, 15);
orbit(camera, new THREE.Vector3(0, 0, 0));

// a vase turned on a lathe
const profile = [];
for (let i = 0; i <= 12; i++) profile.push(new THREE.Vector2(0.35 + Math.sin(i * 0.45) * 0.35 + i * 0.03, i * 0.18 - 1.1));
const vase = new THREE.LatheGeometry(profile, 32);
const looks = [
  new THREE.MeshBasicMaterial({ color: 0x4f9dde }),
  new THREE.MeshLambertMaterial({ color: 0x4f9dde }),
  new THREE.MeshPhongMaterial({ color: 0x4f9dde, shininess: 80 }),
  new THREE.MeshStandardMaterial({ color: 0x4f9dde, roughness: 0.9, metalness: 0 }),
  new THREE.MeshStandardMaterial({ color: 0xd8b25a, roughness: 0.3, metalness: 0.6 }),
  new THREE.MeshToonMaterial({ color: 0x4f9dde }),
  new THREE.MeshNormalMaterial(),
  new THREE.MeshPhongMaterial({ color: 0x4f9dde, flatShading: true }),
];
const items = looks.map((m, i) => {
  const mesh = new THREE.Mesh(vase, m);
  mesh.position.set((i % 4) * 3 - 4.5, i < 4 ? 1.6 : -1.6, 0);
  scene.add(mesh);
  return mesh;
});
scene.add(new THREE.AmbientLight(0xffffff, 0.25));
const key = new THREE.DirectionalLight(0xffffff, 2.2);
key.position.set(4, 6, 8);
scene.add(key);
const rim = new THREE.PointLight(0x88aaff, 40);
rim.position.set(-6, 2, -4);
scene.add(rim);

renderer.setAnimationLoop((t) => {
  items.forEach((m, i) => { m.rotation.y = t / 1500 + i; m.rotation.z = Math.sin(t / 1100 + i) * 0.2; });
  renderer.render(scene, camera);
});`,

  "Spotlights on a stage": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x050505);
const camera = new THREE.PerspectiveCamera(45, 4 / 3, 0.1, 100);
camera.position.set(0, 7, 15);
orbit(camera, new THREE.Vector3(0, 1, 0));

const floor = new THREE.Mesh(new THREE.PlaneGeometry(30, 30, 30, 30), new THREE.MeshStandardMaterial({ color: 0x9a9a9a, roughness: 0.7 }));
floor.rotation.x = -Math.PI / 2;
scene.add(floor);
const knot = new THREE.Mesh(new THREE.TorusKnotGeometry(1, 0.35, 120, 14), new THREE.MeshStandardMaterial({ color: 0xffffff, roughness: 0.3, metalness: 0.2 }));
knot.position.y = 2.2;
scene.add(knot);
scene.add(new THREE.AmbientLight(0xffffff, 0.03));

const spots = [0xff3344, 0x33ff66, 0x3366ff].map((c, i) => {
  const s = new THREE.SpotLight(c, 120, 0, Math.PI / 9, 0.4, 2);
  s.position.set(Math.cos(i * 2.1) * 6, 8, Math.sin(i * 2.1) * 6);
  scene.add(s);
  scene.add(s.target);
  return s;
});

renderer.setAnimationLoop((ms) => {
  const t = ms / 1000;
  spots.forEach((s, i) => s.target.position.set(Math.sin(t * 0.7 + i * 2) * 3.5, 0, Math.cos(t * 0.9 + i * 2) * 3.5));
  knot.rotation.y = t * 0.4;
  renderer.render(scene, camera);
});`,

  "Lorenz attractor (lines)": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0b0b12);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 500);
camera.position.set(0, 0, 90);
orbit(camera, new THREE.Vector3(0, 0, 25));

const pts = [], cols = [];
let x = 0.1, y = 0, z = 0;
const c = new THREE.Color();
for (let i = 0; i < 6000; i++) {
  const dt = 0.006;
  const dx = 10 * (y - x), dy = x * (28 - z) - y, dz = x * y - (8 / 3) * z;
  x += dx * dt; y += dy * dt; z += dz * dt;
  pts.push(x, y, z);
  c.setHSL(0.6 - i / 12000, 0.9, 0.55);
  cols.push(c.r, c.g, c.b);
}
const g = new THREE.BufferGeometry();
g.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
g.setAttribute("color", new THREE.Float32BufferAttribute(cols, 3));
const line = new THREE.Line(g, new THREE.LineBasicMaterial({ vertexColors: true }));
scene.add(line);

renderer.setAnimationLoop((t) => {
  line.rotation.z = t / 6000;
  renderer.render(scene, camera);
});`,

  "Galaxy (points)": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x000004);
const camera = new THREE.PerspectiveCamera(60, 4 / 3, 0.1, 100);
camera.position.set(0, 4, 7);
orbit(camera, new THREE.Vector3(0, 0, 0));

const count = 9000, arms = 4;
const pos = [], col = [];
const inner = new THREE.Color(0xffa060), outer = new THREE.Color(0x3060ff), c = new THREE.Color();
for (let i = 0; i < count; i++) {
  const r = Math.pow(Math.random(), 1.6) * 5;
  const a = ((i % arms) / arms) * Math.PI * 2 + r * 1.1;
  const spread = () => Math.pow(Math.random(), 3) * (Math.random() < 0.5 ? 1 : -1) * 0.5 * (r + 0.3);
  pos.push(Math.cos(a) * r + spread(), spread() * 0.4, Math.sin(a) * r + spread());
  c.copy(inner).lerp(outer, r / 5);
  col.push(c.r, c.g, c.b);
}
const g = new THREE.BufferGeometry();
g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
g.setAttribute("color", new THREE.Float32BufferAttribute(col, 3));
const galaxy = new THREE.Points(g, new THREE.PointsMaterial({ size: 0.03, vertexColors: true }));
scene.add(galaxy);

renderer.setAnimationLoop((t) => {
  galaxy.rotation.y = t / 8000;
  renderer.render(scene, camera);
});`,

  "Wireframe and transparency": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x101820);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 1, 7);
orbit(camera, new THREE.Vector3(0, 0, 0));

const core = new THREE.Mesh(new THREE.IcosahedronGeometry(1, 2), new THREE.MeshPhongMaterial({ color: 0xff7a18, shininess: 90 }));
const shell = new THREE.Mesh(new THREE.IcosahedronGeometry(1.8, 1), new THREE.MeshBasicMaterial({ color: 0x5ce1e6, wireframe: true }));
const glass = new THREE.Mesh(new THREE.SphereGeometry(2.4, 32, 16), new THREE.MeshPhongMaterial({ color: 0x88aaff, transparent: true, opacity: 0.25 }));
const edges = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(5, 5, 5)), new THREE.LineBasicMaterial({ color: 0xffffff }));
scene.add(core, shell, glass, edges);
scene.add(new THREE.AmbientLight(0xffffff, 0.3));
const l = new THREE.DirectionalLight(0xffffff, 2);
l.position.set(3, 4, 5);
scene.add(l);

renderer.setAnimationLoop((t) => {
  core.rotation.y = t / 1200;
  shell.rotation.x = t / 2000; shell.rotation.y = -t / 1700;
  edges.rotation.y = t / 4000;
  renderer.render(scene, camera);
});`
});

Object.assign(EXAMPLES, {
  "Textures: canvas, data and repeat": `
// Textures made in code (no downloads): a CanvasTexture on the crate,
// a DataTexture on the globe, a repeating checker floor and an
// alpha-tested fence. TextureLoader works the same way for image files.
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x9fc3e8);
scene.fog = new THREE.Fog(0x9fc3e8, 12, 40);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 3, 9);
orbit(camera, new THREE.Vector3(0, 1, 0));

function canvasTexture(size, draw) {
  const c = document.createElement("canvas");
  c.width = c.height = size;
  draw(c.getContext("2d"), size);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

// crate: planks, a frame and a label
const crateTex = canvasTexture(256, (g, s) => {
  g.fillStyle = "#b07a3c"; g.fillRect(0, 0, s, s);
  for (let i = 0; i < 8; i++) { g.fillStyle = i % 2 ? "#a06c30" : "#bb8646"; g.fillRect(0, i * 32, s, 30); }
  g.strokeStyle = "#5a3a18"; g.lineWidth = 22; g.strokeRect(11, 11, s - 22, s - 22);
  g.beginPath(); g.moveTo(16, 16); g.lineTo(s - 16, s - 16); g.stroke();
  g.fillStyle = "#fff4dc"; g.font = "bold 40px sans-serif"; g.textAlign = "center";
  g.fillText("RANGER", s / 2, s / 2 + 14);
});
const crate = new THREE.Mesh(new THREE.BoxGeometry(2, 2, 2), new THREE.MeshStandardMaterial({ map: crateTex, roughness: 0.8 }));
crate.position.set(-2.4, 1, 0);

// globe: a DataTexture of latitude bands and noise, filtered linearly
const W = 128, H = 64, data = new Uint8Array(W * H * 4);
for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
  const lat = Math.abs(y / H - 0.5) * 2;
  const n = Math.sin(x * 0.31) * Math.cos(y * 0.47) + Math.sin(x * 0.11 + y * 0.2);
  const land = n > 0.35;
  const i = (y * W + x) * 4;
  const ice = lat > 0.82;
  data[i] = ice ? 240 : land ? 70 + lat * 90 : 20;
  data[i + 1] = ice ? 245 : land ? 140 - lat * 30 : 80;
  data[i + 2] = ice ? 250 : land ? 50 : 170;
  data[i + 3] = 255;
}
const globeTex = new THREE.DataTexture(data, W, H, THREE.RGBAFormat);
globeTex.magFilter = THREE.LinearFilter;
globeTex.colorSpace = THREE.SRGBColorSpace;
globeTex.needsUpdate = true;
const globe = new THREE.Mesh(new THREE.SphereGeometry(1.2, 48, 24), new THREE.MeshPhongMaterial({ map: globeTex, shininess: 40 }));
globe.position.set(2.4, 1.3, 0);

// floor: one checker tile repeated 12 times each way
const checker = canvasTexture(64, (g, s) => {
  g.fillStyle = "#e9e4d8"; g.fillRect(0, 0, s, s);
  g.fillStyle = "#6b7a8f"; g.fillRect(0, 0, s / 2, s / 2); g.fillRect(s / 2, s / 2, s / 2, s / 2);
});
checker.wrapS = checker.wrapT = THREE.RepeatWrapping;
checker.repeat.set(12, 12);
checker.magFilter = THREE.NearestFilter;
const floor = new THREE.Mesh(new THREE.PlaneGeometry(40, 40), new THREE.MeshLambertMaterial({ map: checker }));
floor.rotation.x = -Math.PI / 2;

// fence: transparent holes cut with alphaTest
const fenceTex = canvasTexture(128, (g, s) => {
  g.clearRect(0, 0, s, s);
  g.fillStyle = "#8a6440";
  for (let x = 6; x < s; x += 30) g.fillRect(x, 10, 16, s - 10);
  g.fillRect(0, 34, s, 12); g.fillRect(0, 88, s, 12);
});
fenceTex.wrapS = THREE.RepeatWrapping;
fenceTex.repeat.set(4, 1);
const fence = new THREE.Mesh(new THREE.PlaneGeometry(10, 1.6), new THREE.MeshLambertMaterial({ map: fenceTex, alphaTest: 0.5, side: THREE.DoubleSide }));
fence.position.set(0, 0.8, -3);

scene.add(crate, globe, floor, fence);
scene.add(new THREE.HemisphereLight(0xffffff, 0x667755, 1.2));
const sun = new THREE.DirectionalLight(0xffffff, 2);
sun.position.set(4, 8, 5);
scene.add(sun);

renderer.setAnimationLoop((t) => {
  crate.rotation.y = t / 1600;
  crate.rotation.x = Math.sin(t / 2200) * 0.3;
  globe.rotation.y = t / 3000;
  checker.offset.x = t / 20000;
  renderer.render(scene, camera);
});`
});

Object.assign(EXAMPLES, {
  "Sprites, depth and glow maps": `
// Sprites always face the camera; MeshDepthMaterial shows depth; an
// emissiveMap makes parts of a surface glow; ACES tone mapping as in
// React Three Fiber's Canvas.
renderer.toneMapping = THREE.ACESFilmicToneMapping;
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x101418);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.5, 30);
camera.position.set(0, 2, 9);
orbit(camera, new THREE.Vector3(0, 0.5, 0));

function canvasTexture(size, draw) {
  const c = document.createElement("canvas");
  c.width = c.height = size;
  draw(c.getContext("2d"), size);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

// a soft round dot for the sprites
const dot = canvasTexture(64, (g, s) => {
  const grad = g.createRadialGradient(s / 2, s / 2, 0, s / 2, s / 2, s / 2);
  grad.addColorStop(0, "rgba(255,255,255,1)");
  grad.addColorStop(0.4, "rgba(255,220,150,0.8)");
  grad.addColorStop(1, "rgba(255,160,60,0)");
  g.fillStyle = grad; g.fillRect(0, 0, s, s);
});
const sprites = [];
for (let i = 0; i < 24; i++) {
  const sp = new THREE.Sprite(new THREE.SpriteMaterial({ map: dot, color: new THREE.Color().setHSL(i / 24, 0.8, 0.6) }));
  sp.scale.setScalar(0.6);
  scene.add(sp);
  sprites.push(sp);
}

// windows that glow: an emissiveMap on a dark box
const windows = canvasTexture(128, (g, s) => {
  g.fillStyle = "#000"; g.fillRect(0, 0, s, s);
  for (let y = 0; y < 6; y++) for (let x = 0; x < 4; x++) {
    if ((x * 7 + y * 3) % 5 === 0) continue;
    g.fillStyle = "#ffd27a"; g.fillRect(10 + x * 29, 8 + y * 20, 18, 12);
  }
});
const tower = new THREE.Mesh(new THREE.BoxGeometry(1.4, 3, 1.4), new THREE.MeshStandardMaterial({ color: 0x2a2f3a, roughness: 0.7, emissive: 0xffffff, emissiveMap: windows }));
tower.position.set(-2, 1.5, 0);

// depth: near is white, far is black
const knot = new THREE.Mesh(new THREE.TorusKnotGeometry(0.8, 0.28, 128, 16), new THREE.MeshDepthMaterial());
knot.position.set(2, 1.2, 0);

const ground = new THREE.Mesh(new THREE.PlaneGeometry(20, 20), new THREE.MeshStandardMaterial({ color: 0x3a4a3a }));
ground.rotation.x = -Math.PI / 2;
scene.add(tower, knot, ground);
scene.add(new THREE.HemisphereLight(0xbfd4ff, 0x202020, 1.5));
const moon = new THREE.DirectionalLight(0xffffff, 2);
moon.position.set(-3, 6, 4);
scene.add(moon);

renderer.setAnimationLoop((t) => {
  sprites.forEach((sp, i) => {
    const a = t / 2500 + i / 24 * Math.PI * 2;
    sp.position.set(Math.cos(a) * 3.2, 1 + Math.sin(a * 3) * 0.6, Math.sin(a) * 3.2);
  });
  knot.rotation.y = t / 1800;
  renderer.render(scene, camera);
});`,

  "Environment map: metal and roughness": `
// An environment painted on a canvas as an equirectangular image (sky,
// sun, coloured panels, ground). scene.environment lights every Standard
// material from it, blurred per roughness as PMREMGenerator does; the
// Basic materials in front reflect and refract it directly.
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(45, 4 / 3, 0.1, 100);
camera.position.set(0, 0.6, 9);
orbit(camera, new THREE.Vector3(0, 0, 0));

const c = document.createElement("canvas");
c.width = 1024; c.height = 512;
const g = c.getContext("2d");
const sky = g.createLinearGradient(0, 0, 0, 512);
sky.addColorStop(0, "#1d4f91"); sky.addColorStop(0.47, "#bcd9f2");
sky.addColorStop(0.5, "#6b5a45"); sky.addColorStop(1, "#231d17");
g.fillStyle = sky; g.fillRect(0, 0, 1024, 512);
// the panels sit behind the camera, where the spheres reflect
["#ff5a36", "#ffd23f", "#3ec46d", "#2fa7ff", "#c45cff", "#ffffff"].forEach((col, i) => {
  g.fillStyle = col; g.fillRect(570 + i * 68, 190, 44, 70);
});
g.fillStyle = "#fffbe8"; g.beginPath(); g.arc(300, 80, 28, 0, Math.PI * 2); g.fill();

function envTexture(mapping) {
  const t = new THREE.CanvasTexture(c);
  t.mapping = mapping;
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}
const env = envTexture(THREE.EquirectangularReflectionMapping);
scene.background = env;
scene.environment = env;

// metalness grows up the rows, roughness along them
const ball = new THREE.SphereGeometry(0.42, 48, 24);
for (let row = 0; row < 3; row++) for (let col = 0; col < 6; col++) {
  const m = new THREE.MeshStandardMaterial({ color: row === 2 ? 0xffc870 : 0xffffff, metalness: row / 2, roughness: col / 5 });
  const s = new THREE.Mesh(ball, m);
  s.position.set((col - 2.5) * 1.05, row - 0.4, 0);
  scene.add(s);
}

// a mirror torus and a glass-like ball (Basic materials with envMap)
const mirror = new THREE.Mesh(new THREE.TorusGeometry(0.5, 0.2, 32, 64),
  new THREE.MeshBasicMaterial({ envMap: env, color: 0xffffff }));
mirror.position.set(-1.4, -1.9, 1);
const glass = new THREE.Mesh(new THREE.SphereGeometry(0.6, 48, 24),
  new THREE.MeshBasicMaterial({ envMap: envTexture(THREE.EquirectangularRefractionMapping), refractionRatio: 0.9, color: 0xe8f4ff }));
glass.position.set(1.4, -1.9, 1);
scene.add(mirror, glass);

renderer.setAnimationLoop((t) => {
  mirror.rotation.set(t / 1700, t / 1100, 0);
  renderer.render(scene, camera);
});`,

  "Shadows: sun, spot and point": `
// Three shadow-casting lights: a directional sun (orthographic shadow
// camera), a spotlight and a point light (six-sided cube shadow map).
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x10131a);
const camera = new THREE.PerspectiveCamera(45, 4 / 3, 0.1, 100);
camera.position.set(6, 6, 10);
orbit(camera, new THREE.Vector3(0, 1, 0));

renderer.shadowMap.enabled = true;
renderer.shadowMap.type = THREE.PCFShadowMap;

const ground = new THREE.Mesh(new THREE.PlaneGeometry(30, 30), new THREE.MeshStandardMaterial({ color: 0x9aa3ad, roughness: 0.9 }));
ground.rotation.x = -Math.PI / 2;
ground.receiveShadow = true;
scene.add(ground);

const knot = new THREE.Mesh(new THREE.TorusKnotGeometry(0.8, 0.28, 128, 16), new THREE.MeshStandardMaterial({ color: 0xff7a45, roughness: 0.4 }));
knot.position.y = 2;
const pillars = [];
for (let i = 0; i < 6; i++) {
  const a = i / 6 * Math.PI * 2;
  const p = new THREE.Mesh(new THREE.BoxGeometry(0.5, 1.2 + (i % 3) * 0.6, 0.5), new THREE.MeshStandardMaterial({ color: 0x4f8cff, roughness: 0.6 }));
  p.position.set(Math.cos(a) * 3.2, p.geometry.parameters.height / 2, Math.sin(a) * 3.2);
  pillars.push(p);
}
for (const o of [knot, ...pillars]) { o.castShadow = true; o.receiveShadow = true; scene.add(o); }

scene.add(new THREE.AmbientLight(0xffffff, 0.15));
const sun = new THREE.DirectionalLight(0xfff1dd, 1.6);
sun.position.set(-5, 8, 4);
sun.castShadow = true;
sun.shadow.mapSize.set(1024, 1024);
Object.assign(sun.shadow.camera, { left: -7, right: 7, top: 7, bottom: -7, near: 1, far: 25 });
sun.shadow.camera.updateProjectionMatrix();
scene.add(sun);

const spot = new THREE.SpotLight(0x66ccff, 60, 20, Math.PI / 7, 0.4);
spot.position.set(5, 7, -3);
spot.castShadow = true;
scene.add(spot);

const point = new THREE.PointLight(0xffb347, 12, 12);
point.castShadow = true;
point.shadow.mapSize.set(512, 512);
const bulb = new THREE.Mesh(new THREE.SphereGeometry(0.12, 16, 8), new THREE.MeshBasicMaterial({ color: 0xffd28a }));
point.add(bulb);
scene.add(point);

renderer.setAnimationLoop((t) => {
  knot.rotation.set(t / 2100, t / 1500, 0);
  point.position.set(Math.cos(t / 1300) * 2, 0.9, Math.sin(t / 1300) * 2);
  renderer.render(scene, camera);
});`,

  "Area lights": `
// RectAreaLight: light from a rectangle, evaluated with linearly
// transformed cosines (Standard and Physical materials only). In three.js
// call RectAreaLightUniformsLib.init() from the addons first; the
// playground has it on THREE.
THREE.RectAreaLightUniformsLib.init();
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x050505);
const camera = new THREE.PerspectiveCamera(45, 4 / 3, 1, 1000);
camera.position.set(0, 5, -15);
orbit(camera, new THREE.Vector3(0, 5, 0));

[[0xff0000, -5], [0x00ff00, 0], [0x0000ff, 5]].forEach(([color, x]) => {
  const light = new THREE.RectAreaLight(color, 5, 4, 10);
  light.position.set(x, 5, 5);
  scene.add(light);
  // the panel itself, so the light has a visible shape
  const panel = new THREE.Mesh(new THREE.PlaneGeometry(4, 10), new THREE.MeshBasicMaterial({ color, side: THREE.DoubleSide }));
  panel.position.copy(light.position);
  scene.add(panel);
});

const floor = new THREE.Mesh(new THREE.BoxGeometry(200, 0.1, 200), new THREE.MeshStandardMaterial({ color: 0xbcbcbc, roughness: 0.1, metalness: 0 }));
scene.add(floor);
const knot = new THREE.Mesh(new THREE.TorusKnotGeometry(1.5, 0.5, 200, 16), new THREE.MeshStandardMaterial({ color: 0xffffff, roughness: 0, metalness: 0 }));
knot.position.set(0, 5, 0);
scene.add(knot);

renderer.setAnimationLoop((t) => {
  knot.rotation.y = t / 1000;
  renderer.render(scene, camera);
});`,

  "Physical: glass, sheen and gold": `
// MeshPhysicalMaterial lit by a canvas-made environment: a transmissive
// glass ball refracting the boxes behind it, a sheen (velvet) torus and a
// gold knot. The background is the environment blurred.
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(40, 4 / 3, 0.1, 100);
camera.position.set(0, 1.6, 8);
orbit(camera, new THREE.Vector3(0, 0.6, 0));

const c = document.createElement("canvas");
c.width = 512; c.height = 256;
const g = c.getContext("2d");
const sky = g.createLinearGradient(0, 0, 0, 256);
sky.addColorStop(0, "#2b3a67"); sky.addColorStop(0.45, "#f3d9b1"); sky.addColorStop(0.5, "#5b4636"); sky.addColorStop(1, "#1a1410");
g.fillStyle = sky; g.fillRect(0, 0, 512, 256);
g.fillStyle = "#fff6e0"; g.fillRect(200, 40, 110, 40);
const env = new THREE.CanvasTexture(c);
env.mapping = THREE.EquirectangularReflectionMapping;
env.colorSpace = THREE.SRGBColorSpace;
scene.environment = env;
scene.background = env;
scene.backgroundBlurriness = 0.35;

const colors = [0xff4d4d, 0xffc94d, 0x4dff88, 0x4db8ff];
colors.forEach((col, i) => {
  const b = new THREE.Mesh(new THREE.BoxGeometry(0.5, 0.5, 0.5), new THREE.MeshStandardMaterial({ color: col, roughness: 0.5 }));
  b.position.set(-0.9 + i * 0.6, 0.6, -1.6);
  scene.add(b);
});

const glass = new THREE.Mesh(new THREE.SphereGeometry(0.9, 64, 32),
  new THREE.MeshPhysicalMaterial({ transmission: 1, roughness: 0.05, thickness: 1.2, ior: 1.5, color: 0xffffff }));
glass.position.set(0, 0.9, 0);
const velvet = new THREE.Mesh(new THREE.TorusGeometry(0.55, 0.22, 32, 64),
  new THREE.MeshPhysicalMaterial({ color: 0x3a0b4f, roughness: 0.8, sheen: 1, sheenRoughness: 0.4, sheenColor: 0xff7ad9 }));
velvet.position.set(-2.4, 0.8, 0);
const gold = new THREE.Mesh(new THREE.TorusKnotGeometry(0.45, 0.16, 128, 16),
  new THREE.MeshPhysicalMaterial({ color: 0xffc35a, metalness: 1, roughness: 0.25 }));
gold.position.set(2.4, 0.8, 0);
scene.add(glass, velvet, gold);
scene.add(new THREE.DirectionalLight(0xffffff, 1.2));

renderer.setAnimationLoop((t) => {
  velvet.rotation.set(t / 1900, t / 1300, 0);
  gold.rotation.set(t / 2300, t / 1500, 0);
  renderer.render(scene, camera);
});`,
  "Shader: plasma sphere (ShaderMaterial)": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0b0d12);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 0, 4.2);
orbit(camera);

// your own GLSL: three.js adds the matrices, position, normal and uv
const material = new THREE.ShaderMaterial({
  uniforms: {
    time: { value: 0 },
    colorA: { value: new THREE.Color(0xff3d7f) },
    colorB: { value: new THREE.Color(0x3dd6ff) },
  },
  vertexShader: \`uniform float time;
varying vec3 vNormal;
varying vec3 vPos;
void main() {
  vec3 p = position;
  float wave = sin(p.x * 4.0 + time) * sin(p.y * 4.0 + time * 1.3) * sin(p.z * 4.0 + time * 0.7);
  p += normal * wave * 0.12;
  vPos = p;
  vNormal = normalize(normalMatrix * normal);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(p, 1.0);
}\`,
  fragmentShader: \`uniform float time;
uniform vec3 colorA;
uniform vec3 colorB;
varying vec3 vNormal;
varying vec3 vPos;
void main() {
  float v = sin(vPos.x * 6.0 + time) + sin(vPos.y * 7.0 - time * 1.2) + sin(length(vPos.xz) * 9.0 + time * 0.8);
  vec3 c = mix(colorA, colorB, 0.5 + 0.5 * sin(v * 1.6));
  float rim = pow(1.0 - abs(vNormal.z), 2.0);
  gl_FragColor = vec4(c + rim * 0.6, 1.0);
}\`,
});
const ball = new THREE.Mesh(new THREE.IcosahedronGeometry(1.2, 32), material);
scene.add(ball);

renderer.setAnimationLoop((t) => {
  material.uniforms.time.value = t / 1000;
  ball.rotation.y = t / 4000;
  renderer.render(scene, camera);
});`,

  "Shader: dissolve with a noise texture": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x14161c);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 100);
camera.position.set(0, 1, 5);
orbit(camera);

// tiling value noise in a DataTexture (seeded, so every run looks the same)
const N = 128, G = 16, px = new Uint8Array(N * N * 4);
let seed = 3;
const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const grid = [];
for (let i = 0; i < G * G; i++) grid.push(rnd());
const at = (x, y) => grid[(y % G) * G + (x % G)];
for (let y = 0; y < N; y++) for (let x = 0; x < N; x++) {
  let v = 0, amp = 0.5, f = 4;
  for (let o = 0; o < 3; o++, amp *= 0.5, f *= 2) {
    const gx = x / N * f, gy = y / N * f, ix = Math.floor(gx), iy = Math.floor(gy);
    const sx = (gx - ix) * (gx - ix) * (3 - 2 * (gx - ix)), sy = (gy - iy) * (gy - iy) * (3 - 2 * (gy - iy));
    const a = at(ix, iy) * (1 - sx) + at(ix + 1, iy) * sx, b = at(ix, iy + 1) * (1 - sx) + at(ix + 1, iy + 1) * sx;
    v += (a * (1 - sy) + b * sy) * amp;
  }
  const o = (y * N + x) * 4, n = Math.round(v / 0.875 * 255);
  px[o] = px[o + 1] = px[o + 2] = n; px[o + 3] = 255;
}
const noise = new THREE.DataTexture(px, N, N);
noise.wrapS = noise.wrapT = THREE.RepeatWrapping;
noise.magFilter = noise.minFilter = THREE.LinearFilter;
noise.needsUpdate = true;

const material = new THREE.ShaderMaterial({
  side: THREE.DoubleSide,
  uniforms: { noise: { value: noise }, threshold: { value: 0.5 }, edgeColor: { value: new THREE.Color(0xff7a1a) } },
  vertexShader: \`varying vec2 vUv;
varying vec3 vN;
void main() {
  vUv = uv;
  vN = normalize(normalMatrix * normal);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}\`,
  fragmentShader: \`uniform sampler2D noise;
uniform float threshold;
uniform vec3 edgeColor;
varying vec2 vUv;
varying vec3 vN;
void main() {
  float n = texture2D(noise, vUv * vec2(4.0, 1.0)).r;
  if (n < threshold) discard;
  float edge = smoothstep(threshold + 0.06, threshold, n);
  vec3 nn = normalize(vN) * (gl_FrontFacing ? 1.0 : -1.0);
  float light = 0.3 + 0.7 * max(dot(nn, normalize(vec3(0.4, 0.8, 0.5))), 0.0);
  gl_FragColor = vec4(mix(vec3(0.75, 0.78, 0.85) * light, edgeColor * 2.0, edge), 1.0);
}\`,
});
const knot = new THREE.Mesh(new THREE.TorusKnotGeometry(1, 0.35, 160, 20), material);
scene.add(knot);
// a glowing core shows through the holes
const core = new THREE.Mesh(new THREE.SphereGeometry(0.55, 32, 16), new THREE.MeshStandardMaterial({ color: 0x111111, emissive: 0xff5a00, emissiveIntensity: 1.5 }));
scene.add(core);

renderer.setAnimationLoop((t) => {
  material.uniforms.threshold.value = 0.45 + 0.4 * Math.sin(t / 1500);
  knot.rotation.set(t / 5000, t / 3000, 0);
  renderer.render(scene, camera);
});`,

  "Raw shader: a swarm of triangles (RawShaderMaterial)": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x101010);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 10);
camera.position.z = 2;
orbit(camera);

// 500 triangles, each corner with its own RGBA colour
let seed = 11;
const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const positions = [], colors = [];
for (let i = 0; i < 500 * 3; i++) {
  positions.push(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5);
  colors.push(rnd(), rnd(), rnd(), rnd());
}
const geometry = new THREE.BufferGeometry();
geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 4));

// RawShaderMaterial: nothing is added, the shaders declare all they use
const material = new THREE.RawShaderMaterial({
  uniforms: { time: { value: 1.0 } },
  side: THREE.DoubleSide,
  transparent: true,
  vertexShader: \`precision mediump float;
uniform mat4 modelViewMatrix;
uniform mat4 projectionMatrix;
attribute vec3 position;
attribute vec4 color;
varying vec3 vPosition;
varying vec4 vColor;
void main() {
  vPosition = position;
  vColor = color;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}\`,
  fragmentShader: \`precision mediump float;
uniform float time;
varying vec3 vPosition;
varying vec4 vColor;
void main() {
  vec4 color = vec4(vColor);
  color.r += sin(vPosition.x * 10.0 + time) * 0.5;
  gl_FragColor = color;
}\`,
});
const swarm = new THREE.Mesh(geometry, material);
scene.add(swarm);

renderer.setAnimationLoop((t) => {
  swarm.rotation.y = t * 0.0005;
  material.uniforms.time.value = t * 0.005;
  renderer.render(scene, camera);
});`,

  "Morph targets: shape-shifting blob": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1a1d24);
const camera = new THREE.PerspectiveCamera(45, 4 / 3, 0.1, 100);
camera.position.set(0, 0.6, 5);
orbit(camera);

// two morph targets of a sphere: a rounded cube and a spiky ball
const geometry = new THREE.SphereGeometry(1, 64, 32);
const p = geometry.attributes.position;
const cube = [], spiky = [];
for (let i = 0; i < p.count; i++) {
  const x = p.getX(i), y = p.getY(i), z = p.getZ(i);
  const m = Math.max(Math.abs(x), Math.abs(y), Math.abs(z));
  cube.push(x / m * 0.85, y / m * 0.85, z / m * 0.85);
  const r = 1 + 0.18 * Math.sin(Math.atan2(z, x) * 8) * Math.sin(Math.atan2(Math.hypot(x, z), y) * 6);
  spiky.push(x * r, y * r, z * r);
}
// each target brings its own normals, so the lighting follows the shape
const normalsOf = (pos) => {
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.setIndex(geometry.index);
  g.computeVertexNormals();
  return g.attributes.normal;
};
geometry.morphAttributes.position = [new THREE.Float32BufferAttribute(cube, 3), new THREE.Float32BufferAttribute(spiky, 3)];
geometry.morphAttributes.normal = [normalsOf(cube), normalsOf(spiky)];

const blob = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ color: 0x6ad0ff, roughness: 0.35, metalness: 0.2 }));
scene.add(blob);
scene.add(new THREE.AmbientLight(0xffffff, 0.3));
const key = new THREE.DirectionalLight(0xffffff, 2.2);
key.position.set(3, 4, 5);
scene.add(key);
const fill = new THREE.DirectionalLight(0xff9a6a, 1.0);
fill.position.set(-4, -1, -2);
scene.add(fill);

renderer.setAnimationLoop((t) => {
  blob.morphTargetInfluences[0] = 0.5 + 0.5 * Math.sin(t / 900);
  blob.morphTargetInfluences[1] = 0.5 + 0.5 * Math.sin(t / 1300 + 1);
  blob.rotation.y = t / 3000;
  renderer.render(scene, camera);
});`,

  "Shader instancing: a GLSL 3 wave of boxes": `
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0e1116);
const camera = new THREE.PerspectiveCamera(50, 4 / 3, 0.1, 200);
camera.position.set(0, 14, 22);
orbit(camera);

const GRID = 24;
// GLSL 3 sources (in / out), a #define, and gl_InstanceID to tell the boxes apart
const material = new THREE.ShaderMaterial({
  glslVersion: THREE.GLSL3,
  defines: { GRID: GRID.toFixed(1) },
  uniforms: { time: { value: 0 } },
  vertexShader: \`uniform float time;
out vec3 vColor;
out vec3 vNormal;
vec3 hsv(float h, float s, float v) {
  vec3 k = clamp(abs(mod(h * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0);
  return v * mix(vec3(1.0), k, s);
}
void main() {
  float x = mod(float(gl_InstanceID), GRID) - GRID * 0.5 + 0.5;
  float z = floor(float(gl_InstanceID) / GRID) - GRID * 0.5 + 0.5;
  float h = 0.5 + 0.5 * sin(length(vec2(x, z)) * 0.5 - time);
  vec3 p = position * vec3(1.0, 0.3 + h * 3.0, 1.0) + vec3(0.0, (0.3 + h * 3.0) * 0.5, 0.0);
  vColor = hsv(0.55 + h * 0.35, 0.75, 0.9);
  vNormal = normalize(normalMatrix * mat3(instanceMatrix) * normal);
  gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(p, 1.0);
}\`,
  fragmentShader: \`in vec3 vColor;
in vec3 vNormal;
out vec4 outColor;
void main() {
  vec3 l = normalize((viewMatrix * vec4(0.4, 1.0, 0.3, 0.0)).xyz);
  outColor = vec4(vColor * (0.35 + 0.65 * max(dot(normalize(vNormal), l), 0.0)), 1.0);
}\`,
});
const boxes = new THREE.InstancedMesh(new THREE.BoxGeometry(0.8, 1, 0.8), material, GRID * GRID);
const dummy = new THREE.Object3D();
for (let i = 0; i < GRID * GRID; i++) {
  dummy.position.set((i % GRID) - GRID / 2 + 0.5, 0, Math.floor(i / GRID) - GRID / 2 + 0.5);
  dummy.updateMatrix();
  boxes.setMatrixAt(i, dummy.matrix);
}
scene.add(boxes);

renderer.setAnimationLoop((t) => {
  material.uniforms.time.value = t / 400;
  renderer.render(scene, camera);
});`
});
