// Compares RangerThree's generated geometries with three.js's (the parity
// reference in tmp/three-ref): attribute values (as float32), index, groups.
import { createRequire } from 'node:module';
import fs from 'node:fs';
import vm from 'node:vm';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(import.meta.url);
const THREE = require(path.join(root, 'tmp/three-ref/three/build/three.cjs'));
const ctx = { TextEncoder, TextDecoder };
vm.createContext(ctx);
vm.runInContext(fs.readFileSync(path.join(root, 'build/rangerthree-core.js'), 'utf8') + '\nthis.R = RangerThreeCore;', ctx);
const R = ctx.R;
const P = R.GeometriesPrimitives, H = R.GeometriesPolyhedron;
const pi = Math.PI;
const cases = [
  ['box', () => new THREE.BoxGeometry(1, 2, 3, 2, 3, 1), () => P.box_geometry(1, 2, 3, 2, 3, 1)],
  ['plane', () => new THREE.PlaneGeometry(2, 1, 3, 2), () => P.plane_geometry(2, 1, 3, 2)],
  ['sphere', () => new THREE.SphereGeometry(1.5, 12, 8), () => P.sphere_geometry(1.5, 12, 8, 0, 2 * pi, 0, pi)],
  ['sphere-part', () => new THREE.SphereGeometry(1, 9, 5, 0.3, 2, 0.4, 1.5), () => P.sphere_geometry(1, 9, 5, 0.3, 2, 0.4, 1.5)],
  ['cylinder', () => new THREE.CylinderGeometry(0.5, 1, 2, 10, 3), () => P.cylinder_geometry(0.5, 1, 2, 10, 3, false, 0, 2 * pi)],
  ['cone', () => new THREE.ConeGeometry(1, 2, 8), () => P.cone_geometry(1, 2, 8, 1, false, 0, 2 * pi)],
  ['circle', () => new THREE.CircleGeometry(2, 7, 0.2, 4), () => P.circle_geometry(2, 7, 0.2, 4)],
  ['ring', () => new THREE.RingGeometry(0.4, 1.2, 9, 2), () => P.ring_geometry(0.4, 1.2, 9, 2, 0, 2 * pi)],
  ['torus', () => new THREE.TorusGeometry(1, 0.3, 8, 20), () => P.torus_geometry(1, 0.3, 8, 20, 2 * pi, 0, 2 * pi)],
  ['torusKnot', () => new THREE.TorusKnotGeometry(1, 0.3, 40, 6, 2, 3), () => P.torus_knot_geometry(1, 0.3, 40, 6, 2, 3)],
  ['capsule', () => new THREE.CapsuleGeometry(0.5, 1, 4, 8, 2), () => P.capsule_geometry(0.5, 1, 4, 8, 2)],
  ['lathe', () => new THREE.LatheGeometry(), () => P.lathe_geometry(P.lathe_default_points(), 12, 0, 2 * pi)],
  ['tetrahedron', () => new THREE.TetrahedronGeometry(1, 0), () => H.tetrahedron_geometry(1, 0)],
  ['octahedron', () => new THREE.OctahedronGeometry(1, 2), () => H.octahedron_geometry(1, 2)],
  ['icosahedron', () => new THREE.IcosahedronGeometry(1.2, 1), () => H.icosahedron_geometry(1.2, 1)],
  ['dodecahedron', () => new THREE.DodecahedronGeometry(1, 0), () => H.dodecahedron_geometry(1, 0)],
];
const near = (a, b) => Math.abs(a - b) <= 1e-5 * Math.max(1, Math.abs(a));
let pass = 0;
for (const [name, ref, ours] of cases) {
  const a = ref(), b = ours();
  const problems = [];
  for (const attr of ['position', 'normal', 'uv']) {
    const x = a.attributes[attr].array, y = b.attributes.vals[attr].array;
    if (x.length !== y.length) { problems.push(`${attr} length ${y.length} vs ${x.length}`); continue; }
    const i = x.findIndex((v, k) => !near(v, y[k]));
    if (i >= 0) problems.push(`${attr}[${i}] ${y[i]} vs ${x[i]}`);
  }
  const xi = a.index ? Array.from(a.index.array) : [], yi = b.index ? b.index.array : [];
  if (xi.join() !== yi.join()) problems.push(`index differs (${yi.length} vs ${xi.length})`);
  const xg = JSON.stringify(a.groups.map((g) => [g.start, g.count, g.materialIndex]));
  const yg = JSON.stringify(b.groups.map((g) => [g.start, g.count, g.material_index]));
  if (xg !== yg) problems.push(`groups ${yg} vs ${xg}`);
  if (a.type !== b.type_name) problems.push(`type ${b.type_name}`);
  if (problems.length) console.log(`FAIL ${name}: ${problems.join('; ')}`); else pass++;
}
console.log(`geometry ${pass}/${cases.length}`);
process.exit(pass === cases.length ? 0 : 1);
