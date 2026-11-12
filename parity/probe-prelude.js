// SPDX-License-Identifier: MIT
//
// Shared by the golden generator (real three.js in Node) and the meter
// (CErXes with the .tsx kit's THREE): the same fixed inputs, the same
// serialisation of results, so a value item compares like with like.
//
// P.<token>() makes a fresh input; ser(v) turns a result into plain JSON.
var P = (function () {
  var T = THREE;
  function v3(x, y, z) { return new T.Vector3(x, y, z); }
  var p = {
    N: function () { return 0.37; },
    N2: function () { return 2.5; },
    I: function () { return 1; },
    I2: function () { return 2; },
    S: function () { return 'abc'; },
    T: function () { return true; },
    V2: function () { return new T.Vector2(0.5, -1.25); },
    V2b: function () { return new T.Vector2(2, 3); },
    V2c: function () { return new T.Vector2(-1, 4); },
    V3: function () { return v3(1.5, -2, 0.75); },
    V3b: function () { return v3(0.3, 0.4, -1.2); },
    V3c: function () { return v3(-2, 1, 3); },
    V3n: function () { return v3(1, 2, -0.5).normalize(); },
    V4: function () { return new T.Vector4(1, -2, 3, 0.5); },
    V4b: function () { return new T.Vector4(0.2, 0.4, -0.6, 2); },
    M3: function () { return new T.Matrix3().set(2, 0.5, 0, -0.3, 1.5, 0.2, 0.1, 0, 1); },
    M3b: function () { return new T.Matrix3().set(1, 0.2, 0.3, 0, 1, 0.4, 0.5, 0, 2); },
    M4: function () { return new T.Matrix4().compose(v3(1, 2, 3), new T.Quaternion().setFromEuler(new T.Euler(0.3, -0.5, 0.2)), v3(1.5, 0.8, 2)); },
    M4b: function () { return new T.Matrix4().compose(v3(-1, 0.5, 2), new T.Quaternion().setFromEuler(new T.Euler(-0.2, 0.9, 0.1)), v3(1, 1, 1)); },
    M4p: function () { return new T.Matrix4().makePerspective(-0.4, 0.4, 0.3, -0.3, 0.5, 50); },
    Q: function () { return new T.Quaternion().setFromEuler(new T.Euler(0.4, 0.2, -0.7)); },
    Qb: function () { return new T.Quaternion().setFromEuler(new T.Euler(-1.1, 0.5, 0.3)); },
    E: function () { return new T.Euler(0.4, -0.3, 1.1, 'XYZ'); },
    Eb: function () { return new T.Euler(-0.2, 0.7, 0.3, 'YXZ'); },
    C: function () { return new T.Color(0.8, 0.3, 0.15); },
    Cb: function () { return new T.Color(0.1, 0.5, 0.9); },
    B2: function () { return new T.Box2(new T.Vector2(-1, -2), new T.Vector2(2, 1)); },
    B2b: function () { return new T.Box2(new T.Vector2(0, 0), new T.Vector2(3, 4)); },
    B3: function () { return new T.Box3(v3(-1, -2, -0.5), v3(2, 1, 3)); },
    B3b: function () { return new T.Box3(v3(0, 0, 0), v3(4, 3, 1)); },
    SPH: function () { return new T.Sphere(v3(0.5, 1, -1), 2); },
    SPHb: function () { return new T.Sphere(v3(3, 0, 0), 1.5); },
    PL: function () { return new T.Plane(v3(0, 1, 0.2).normalize(), -0.5); },
    PLb: function () { return new T.Plane(v3(1, 0, 0), 1); },
    RAY: function () { return new T.Ray(v3(-5, 0.2, 0.1), v3(1, 0.05, 0).normalize()); },
    L3: function () { return new T.Line3(v3(-1, 0, 0), v3(2, 1, 0.5)); },
    L3b: function () { return new T.Line3(v3(0, -1, 1), v3(0.5, 2, -1)); },
    TRI: function () { return new T.Triangle(v3(0, 0, 0), v3(2, 0, 0), v3(0, 2, 0.5)); },
    SPL: function () { return new T.Spherical(2, 0.8, 1.3); },
    CYL: function () { return new T.Cylindrical(2, 0.6, 1.5); },
    ARR: function () { return [0.5, -1, 2, 3.25, 4, -0.75, 1.5, 2, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]; },
    ATTR: function () { return new T.BufferAttribute(new Float32Array([1, 2, 3, -1, 0.5, 2, 4, 0, -2, 0.25, 0.75, 1]), 3); },
    ATTR4: function () { return new T.BufferAttribute(new Float32Array([1, 2, 3, 4, -1, 0.5, 2, 1]), 4); },
    ATTR2: function () { return new T.BufferAttribute(new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), 2); },
    MESH: function () {
      var m = new T.Mesh(new T.BoxGeometry(1, 2, 0.5), new T.MeshBasicMaterial());
      m.position.set(1, 0.5, -2); m.rotation.set(0.2, 0.4, 0); m.scale.set(1, 2, 1); m.name = 'box';
      m.updateMatrixWorld(true);
      return m;
    },
    O3: function () {
      var o = new T.Object3D(); o.name = 'root'; o.position.set(1, 2, 3); o.rotation.set(0.1, 0.5, -0.2); o.scale.set(2, 2, 2);
      var c = new T.Object3D(); c.name = 'child'; c.position.set(0, 1, 0); c.rotation.set(0, 0.3, 0); o.add(c);
      var g = new T.Object3D(); g.name = 'grand'; g.position.set(0.5, 0, 0); c.add(g);
      o.updateMatrixWorld(true);
      return o;
    },
    GEO: function () { return new T.BoxGeometry(1, 2, 3, 2, 1, 1); },
    CAM: function () { var c = new T.PerspectiveCamera(50, 1.5, 0.1, 100); c.position.set(0, 2, 6); c.lookAt(0, 0, 0); c.updateMatrixWorld(true); return c; },
    OCAM: function () { var c = new T.OrthographicCamera(-4, 4, 3, -3, 0.1, 50); c.position.set(0, 0, 10); c.updateMatrixWorld(true); return c; }
  };
  return p;
})();

// receivers: an interesting instance of each class
var R = {
  Vector2: P.V2, Vector3: P.V3, Vector4: P.V4, Matrix3: P.M3, Matrix4: P.M4,
  Quaternion: P.Q, Euler: P.E, Color: P.C, Box2: P.B2, Box3: P.B3, Sphere: P.SPH,
  Plane: P.PL, Ray: P.RAY, Line3: P.L3, Triangle: P.TRI, Spherical: P.SPL,
  Cylindrical: P.CYL, Object3D: P.O3, Mesh: P.MESH, BufferGeometry: P.GEO,
  BufferAttribute: P.ATTR, PerspectiveCamera: P.CAM, OrthographicCamera: P.OCAM,
  Frustum: function () { var c = P.CAM(); return new THREE.Frustum().setFromProjectionMatrix(new THREE.Matrix4().multiplyMatrices(c.projectionMatrix, c.matrixWorldInverse)); }
};

function ser(v, depth) {
  depth = depth || 0;
  if (v === undefined) return '<undefined>';
  if (v === null) return null;
  var t = typeof v;
  if (t === 'number') return isFinite(v) ? v : String(v);
  if (t === 'boolean' || t === 'string') return v;
  if (t === 'function') return '<function>';
  if (depth > 4) return '<deep>';
  if (Array.isArray(v) || (v && typeof v.length === 'number' && v.BYTES_PER_ELEMENT)) {
    var a = []; for (var i = 0; i < v.length && i < 64; i++) a.push(ser(v[i], depth + 1)); return a;
  }
  if (v.isVector2) return ['V2', v.x, v.y];
  if (v.isVector3) return ['V3', v.x, v.y, v.z];
  if (v.isVector4) return ['V4', v.x, v.y, v.z, v.w];
  if (v.isQuaternion) return ['Q', v.x, v.y, v.z, v.w];
  if (v.isEuler) return ['E', v.x, v.y, v.z, v.order];
  if (v.isColor) return ['C', v.r, v.g, v.b];
  if (v.isMatrix3) return ['M3'].concat(ser(v.elements, depth + 1));
  if (v.isMatrix4) return ['M4'].concat(ser(v.elements, depth + 1));
  if (v.isBox2) return ['B2', ser(v.min, depth + 1), ser(v.max, depth + 1)];
  if (v.isBox3) return ['B3', ser(v.min, depth + 1), ser(v.max, depth + 1)];
  if (v.isSphere) return ['SPH', ser(v.center, depth + 1), v.radius];
  if (v.isPlane) return ['PL', ser(v.normal, depth + 1), v.constant];
  if (v.isRay) return ['RAY', ser(v.origin, depth + 1), ser(v.direction, depth + 1)];
  if (v.isLine3) return ['L3', ser(v.start, depth + 1), ser(v.end, depth + 1)];
  if (v.isTriangle) return ['TRI', ser(v.a, depth + 1), ser(v.b, depth + 1), ser(v.c, depth + 1)];
  if (v.isSpherical) return ['SPL', v.radius, v.phi, v.theta];
  if (v.isCylindrical) return ['CYL', v.radius, v.theta, v.y];
  if (v.isFrustum) { var pl = []; for (var k = 0; k < 6; k++) pl.push(ser(v.planes[k], depth + 1)); return ['FR'].concat(pl); }
  if (v.isBufferAttribute) return ['ATTR', v.itemSize, v.count, ser(Array.prototype.slice.call(v.array, 0, 48), depth + 1)];
  if (v.isBufferGeometry) {
    var at = v.attributes || {}, ks = Object.keys(at).sort(), o = {};
    for (var j = 0; j < ks.length; j++) o[ks[j]] = [at[ks[j]].itemSize, at[ks[j]].count, digest(at[ks[j]].array)];
    return ['GEO', v.type, o, v.index ? [v.index.count, digest(v.index.array)] : null, (v.groups || []).length];
  }
  if (v.isObject3D) {
    var kids = []; for (var c = 0; c < (v.children || []).length; c++) kids.push(v.children[c].name || v.children[c].type);
    return ['OBJ', v.type, v.name, ser(v.position, depth + 1), ser(v.quaternion, depth + 1), ser(v.scale, depth + 1), kids];
  }
  if (v.isMaterial) return ['MAT', v.type];
  if (v.isTexture) return ['TEX', v.constructor && v.constructor.name];
  var keys = Object.keys(v).filter(function (k) { return k[0] !== '_' && typeof v[k] !== 'function'; }).sort().slice(0, 24);
  var out = {}; for (var q = 0; q < keys.length; q++) out[keys[q]] = ser(v[keys[q]], depth + 1);
  return out;
}
// a long array as its length, sum and a few samples (rounded), so a
// geometry compares without carrying every vertex
function digest(a) {
  var s = 0, s2 = 0; for (var i = 0; i < a.length; i++) { s += a[i]; s2 += a[i] * ((i % 7) + 1); }
  return [a.length, Math.round(s * 1e4) / 1e4, Math.round(s2 * 1e4) / 1e4, Math.round((a[0] || 0) * 1e4) / 1e4, Math.round((a[a.length - 1] || 0) * 1e4) / 1e4];
}
// runs one probe: its result and, for a method call, the receiver after it
function runProbe(fn) {
  try { return { ok: true, v: ser(fn()) }; }
  catch (e) { return { ok: false, err: String(e && e.message || e) }; }
}
