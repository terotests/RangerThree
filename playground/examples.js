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
camera.position.set(3, 2.5, 4);
camera.lookAt(0, 0, 0);

const geometry = new THREE.DodecahedronGeometry(1.3, 0);
const solid = new THREE.Mesh(geometry, new THREE.MeshLambertMaterial({ color: 0x5b8def }));
scene.add(solid);
scene.add(new THREE.AmbientLight(0xffffff, 0.5));
const light = new THREE.DirectionalLight(0xffffff, 1.5);
light.position.set(2, 3, 4);
scene.add(light);

// the edges are computed in Rust; this renderer does not draw lines yet
const edges = new THREE.EdgesGeometry(geometry);
const wire = new THREE.WireframeGeometry(geometry);
console.log("edge segments", edges.attributes.position.count / 2, "wire segments", wire.attributes.position.count / 2);

renderer.setAnimationLoop((t) => {
  solid.rotation.y = t / 1500;
  renderer.render(scene, camera);
});`
};
