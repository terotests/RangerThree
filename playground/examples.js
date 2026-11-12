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
