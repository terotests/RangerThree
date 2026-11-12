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
const P = R.GeometriesPrimitives, H = R.GeometriesPolyhedron, S = R.GeometriesShapes;
const V3 = (x, y, z) => R.Vector3.new_(x, y, z || 0);
// the same shape for both: a square with a round hole, plus a heart of Bezier curves
function refShapes() {
  const s = new THREE.Shape(); s.moveTo(0, 0); s.lineTo(2, 0); s.lineTo(2, 2); s.lineTo(0, 2); s.lineTo(0, 0);
  const h = new THREE.Path(); h.absarc(1, 1, 0.5, 0, Math.PI * 2, true); s.holes.push(h);
  const t = new THREE.Shape(); t.moveTo(2.5, 2.5); t.bezierCurveTo(2.5, 2.5, 2, 0, 0, 0); t.bezierCurveTo(-3, 0, -3, 3.5, -3, 3.5);
  t.bezierCurveTo(-3, 5.5, -1, 7.7, 2.5, 9.5); t.quadraticCurveTo(6, 7.7, 8, 4.5); t.splineThru([new THREE.Vector2(6, 2), new THREE.Vector2(2.5, 2.5)]);
  return [s, t];
}
function ourShapes() {
  const s = R.Curve.path(true); s.move_to(0, 0); s.line_to(2, 0); s.line_to(2, 2); s.line_to(0, 2); s.line_to(0, 0);
  const h = R.Curve.path(true); h.absarc(1, 1, 0.5, 0, Math.PI * 2, true); s.add_hole(h);
  const t = R.Curve.path(true); t.move_to(2.5, 2.5); t.bezier_curve_to(2.5, 2.5, 2, 0, 0, 0); t.bezier_curve_to(-3, 0, -3, 3.5, -3, 3.5);
  t.bezier_curve_to(-3, 5.5, -1, 7.7, 2.5, 9.5); t.quadratic_curve_to(6, 7.7, 8, 4.5); t.spline_thru([V3(6, 2), V3(2.5, 2.5)]);
  return [s, t];
}
function extrudeOpts(o) { const e = R.ExtrudeOptions.new_(); for (const k in o) e[k] = o[k]; return e; }
const refKnot = () => new THREE.CatmullRomCurve3([new THREE.Vector3(-2, 0, 0), new THREE.Vector3(0, 2, 1), new THREE.Vector3(2, 0, 0), new THREE.Vector3(0, -2, -1)], true);
const ourKnot = () => R.Curve.catmull_rom([V3(-2, 0, 0), V3(0, 2, 1), V3(2, 0, 0), V3(0, -2, -1)], true, 0, 0.5);
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
  ['shape', () => new THREE.ShapeGeometry(refShapes()[0], 6), () => S.shape_geometry([ourShapes()[0]], 6, false)],
  ['shapes', () => new THREE.ShapeGeometry(refShapes(), 5), () => S.shape_geometry(ourShapes(), 5, true)],
  ['extrude', () => new THREE.ExtrudeGeometry(refShapes()[0], { depth: 0.5, curveSegments: 6 }), () => S.extrude_geometry([ourShapes()[0]], extrudeOpts({ depth: 0.5, curve_segments: 6 }))],
  ['extrude-flat', () => new THREE.ExtrudeGeometry(refShapes(), { depth: 2, steps: 3, bevelEnabled: false }), () => S.extrude_geometry(ourShapes(), extrudeOpts({ depth: 2, steps: 3, bevel_enabled: false }))],
  ['extrude-bevel', () => new THREE.ExtrudeGeometry(refShapes()[1], { depth: 1, bevelThickness: 0.5, bevelSize: 0.3, bevelOffset: 0.1, bevelSegments: 4, curveSegments: 4 }),
    () => S.extrude_geometry([ourShapes()[1]], extrudeOpts({ depth: 1, bevel_thickness: 0.5, bevel_size: 0.3, bevel_offset: 0.1, bevel_segments: 4, curve_segments: 4 }))],
  ['extrude-path', () => new THREE.ExtrudeGeometry(refShapes()[0], { steps: 20, extrudePath: refKnot() }),
    () => { const o = extrudeOpts({ steps: 20 }); o.extrude_along(ourKnot(), true); return S.extrude_geometry([ourShapes()[0]], o); }],
  ['tube', () => new THREE.TubeGeometry(refKnot(), 32, 0.3, 6, true), () => S.tube_geometry(ourKnot(), 32, 0.3, 6, true)],
  ['tube-open', () => new THREE.TubeGeometry(), () => S.tube_geometry(R.Curve.quadratic_bezier(V3(-1, -1, 0), V3(-1, 1, 0), V3(1, 1, 0), false), 64, 1, 8, false)],
  ['edges', () => new THREE.EdgesGeometry(new THREE.BoxGeometry(1, 2, 3, 2, 2, 2)), () => S.edges_geometry(P.box_geometry(1, 2, 3, 2, 2, 2), 1)],
  ['edges-sphere', () => new THREE.EdgesGeometry(new THREE.SphereGeometry(1, 8, 6), 20), () => S.edges_geometry(P.sphere_geometry(1, 8, 6, 0, 2 * pi, 0, pi), 20)],
  ['wireframe', () => new THREE.WireframeGeometry(new THREE.BoxGeometry(1, 1, 1, 2, 1, 1)), () => S.wireframe_geometry(P.box_geometry(1, 1, 1, 2, 1, 1))],
  ['wireframe-soup', () => new THREE.WireframeGeometry(new THREE.TorusGeometry(1, 0.3, 4, 6).toNonIndexed()), () => S.wireframe_geometry(P.torus_geometry(1, 0.3, 4, 6, 2 * pi, 0, 2 * pi).to_non_indexed())],
];
const near = (a, b) => Math.abs(a - b) <= 1e-5 * Math.max(1, Math.abs(a));
let pass = 0;
for (const [name, ref, ours] of cases) {
  const a = ref(), b = ours();
  const problems = [];
  for (const attr of ['position', 'normal', 'uv']) {
    if (!a.attributes[attr] && !b.attributes.vals[attr]) continue;
    if (!a.attributes[attr] || !b.attributes.vals[attr]) { problems.push(`${attr} missing`); continue; }
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
