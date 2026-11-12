// SPDX-License-Identifier: MIT
//
// three.js's scene API (Object3D and its kinds, BufferAttribute,
// BufferGeometry and the generated geometries, materials, textures, fog)
// over the three crate, after js/three.js. Same rules as there: the
// crate's objects are the API's objects, this file only renames, adapts
// calling conventions and keeps the JavaScript-only state three.js has
// (event listeners, userData, the reactive `rotation`).
//
// Field adapters: a crate struct's fields are own properties set in its
// constructor. An accessor on the prototype catches those writes, so the
// crate's `None` reads as `null`, a `Map` reads as a plain object
// (`geometry.attributes.position`) and an attribute's numbers live in a
// typed array, without the crate knowing.
(function (T, R) {
  "use strict";
  var F = T.__facade;
  var def = F.def, accessor = F.accessor, aliases = F.aliases, camel = F.camel;
  var eulerCore = F.eulerCore, orderIndex = F.orderIndex, ORDERS = F.ORDERS, rng = F.rng;
  var O = R.CoreObject3d;

  // ---- constants ---------------------------------------------------------
  var C = {
    FrontSide: 0, BackSide: 1, DoubleSide: 2,
    NoBlending: 0, NormalBlending: 1, AdditiveBlending: 2, SubtractiveBlending: 3, MultiplyBlending: 4, CustomBlending: 5,
    UVMapping: 300, CubeReflectionMapping: 301, CubeRefractionMapping: 302,
    EquirectangularReflectionMapping: 303, EquirectangularRefractionMapping: 304, CubeUVReflectionMapping: 306,
    RepeatWrapping: 1000, ClampToEdgeWrapping: 1001, MirroredRepeatWrapping: 1002,
    NearestFilter: 1003, NearestMipmapNearestFilter: 1004, NearestMipMapNearestFilter: 1004,
    NearestMipmapLinearFilter: 1005, NearestMipMapLinearFilter: 1005, LinearFilter: 1006,
    LinearMipmapNearestFilter: 1007, LinearMipMapNearestFilter: 1007, LinearMipmapLinearFilter: 1008, LinearMipMapLinearFilter: 1008,
    UnsignedByteType: 1009, ByteType: 1010, ShortType: 1011, UnsignedShortType: 1012, IntType: 1013, UnsignedIntType: 1014,
    FloatType: 1015, HalfFloatType: 1016, UnsignedInt248Type: 1020,
    AlphaFormat: 1021, RGBFormat: 1022, RGBAFormat: 1023, DepthFormat: 1026, DepthStencilFormat: 1027, RedFormat: 1028,
    StaticDrawUsage: 35044, DynamicDrawUsage: 35048, StreamDrawUsage: 35040,
    NoToneMapping: 0, LinearToneMapping: 1, ReinhardToneMapping: 2, CineonToneMapping: 3, ACESFilmicToneMapping: 4,
    AgXToneMapping: 6, NeutralToneMapping: 7,
    BasicShadowMap: 0, PCFShadowMap: 1, PCFSoftShadowMap: 2, VSMShadowMap: 3,
    MultiplyOperation: 0, MixOperation: 1, AddOperation: 2,
    TangentSpaceNormalMap: 0, ObjectSpaceNormalMap: 1,
    BasicDepthPacking: 3200, RGBADepthPacking: 3201,
    TrianglesDrawMode: 0, TriangleStripDrawMode: 1, TriangleFanDrawMode: 2
  };
  Object.keys(C).forEach(function (k) { T[k] = C[k]; });
  var USAGE = R.CoreBufferAttribute && R.CoreBufferAttribute.STATIC_DRAW_USAGE ? R.CoreBufferAttribute.STATIC_DRAW_USAGE() : 35044;

  // ---- helpers -----------------------------------------------------------
  function setProto(o, C, newTarget) {
    var p = newTarget && newTarget.prototype ? newTarget.prototype : C.prototype;
    if (Object.getPrototypeOf(o) !== p) Object.setPrototypeOf(o, p);
    return o;
  }
  // a facade class whose instances are crate objects made by `create`
  function klass(Base, create, init) {
    function K() {
      var o = create.apply(null, arguments);
      setProto(o, K, new.target);
      if (init) init.apply(o, arguments);
      return o;
    }
    K.prototype = Object.create(Base.prototype);
    def(K.prototype, "constructor", K);
    return K;
  }
  function hidden(name) { return "_$" + name; }
  // a field the crate writes, read through `get` (and written through `set`)
  function adapt(proto, field, get, set) {
    var key = hidden(field);
    Object.defineProperty(proto, field, {
      get: get ? function () { return get.call(this, this[key]); } : function () { return this[key]; },
      set: set ? function (v) { this[key] = set.call(this, v, this[key]); } : function (v) { this[key] = v; },
      configurable: true, enumerable: false
    });
    // the hidden key itself must not show up as data
    def(proto, key, undefined);
  }
  // a crate field kept under three.js's name (own and enumerable, as
  // three.js's plain objects have it)
  function rename(proto, field, jsName) {
    Object.defineProperty(proto, field, {
      get: function () { return this[jsName]; },
      set: function (v) { this[jsName] = v; },
      configurable: true, enumerable: false
    });
  }
  function nullable(proto, fields) {
    fields.forEach(function (f) { adapt(proto, f, function (v) { return v === undefined ? null : v; }); });
  }
  // a crate Map as a plain object: the keys are its own properties, the
  // crate's `vals`, `order`, `put`, `del`, `rs_clone` hidden on it
  function plainMap(m) {
    if (m === undefined || m === null) return m;
    if (m.vals === m) return m;
    var o = m.vals || {};
    def(o, "vals", o);
    Object.defineProperty(o, "order", { get: function () { return Object.keys(this); }, set: function () {}, configurable: true, enumerable: false });
    def(o, "put", function (k, v) { this[k] = v; });
    def(o, "del", function (k) { delete this[k]; });
    def(o, "rs_clone", function () { var c = plainMap({ vals: {} }); for (var k in this) c[k] = this[k]; return c; });
    return o;
  }
  function mapField(proto, fields) {
    fields.forEach(function (f) { adapt(proto, f, null, function (v) { return plainMap(v); }); });
  }
  // a Vector/Quaternion field that keeps its object when the crate assigns
  // a new value (three.js's position etc. are the same object for life)
  function stable(proto, fields) {
    fields.forEach(function (f) {
      adapt(proto, f, null, function (v, old) {
        if (old && v && old !== v && old.copy) { old.copy(v); return old; }
        return v;
      });
    });
  }
  // camelCase accessors for an instance's snake_case fields
  function camelFields(proto, sample, skip) {
    var keys = Object.keys(sample);
    for (var k in sample) if (keys.indexOf(k) < 0) keys.push(k);
    keys.concat(Object.getOwnPropertyNames(Object.getPrototypeOf(sample)).filter(function (k) {
      var d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(sample), k);
      return d && d.get;
    })).forEach(function (k) {
      if (k.indexOf("_") <= 0 || k.slice(0, 2) === "_$" || (skip && skip.indexOf(k) >= 0)) return;
      // Ranger writes a field named like a JavaScript built-in with a trailing _
      var c = k.charAt(k.length - 1) === "_" ? k.slice(0, -1) : camel(k);
      if (c in proto && Object.prototype.hasOwnProperty.call(proto, c)) return;
      accessor(proto, c, function () { return this[k]; }, function (v) { this[k] = v; });
    });
  }
  // a lazily numbered id (three.js counts per kind) and uuid
  function ids(proto, counter) {
    adapt(proto, "id", function (v) {
      if (!v) { v = counter.n++; this[hidden("id")] = v; }
      return v;
    }, function (v, old) { return v ? v : old; });
    adapt(proto, "uuid", function (v) {
      if (!v) { v = rng.uuid(); this[hidden("uuid")] = v; }
      return v;
    }, function (v, old) { return v ? v : old; });
  }
  function newId(o) { o[hidden("id")] = 0; o[hidden("uuid")] = ""; return o.id; }
  function vec3(x, y, z) {
    if (x && x.isVector3) return x;
    return new T.Vector3(x, y, z);
  }
  function color(v) {
    if (v && v.isColor) return v;
    var c = new T.Color();
    if (v !== undefined) c.set(v);
    return c;
  }

  // ---- EventDispatcher ---------------------------------------------------
  var events = {
    addEventListener: function (type, listener) {
      if (!this._listeners) def(this, "_listeners", {});
      var l = this._listeners;
      if (!l[type]) l[type] = [];
      if (l[type].indexOf(listener) === -1) l[type].push(listener);
    },
    hasEventListener: function (type, listener) {
      var l = this._listeners;
      return !!(l && l[type] && l[type].indexOf(listener) !== -1);
    },
    removeEventListener: function (type, listener) {
      var l = this._listeners;
      if (!l || !l[type]) return;
      var i = l[type].indexOf(listener);
      if (i !== -1) l[type].splice(i, 1);
    },
    dispatchEvent: function (event) {
      var l = this._listeners;
      if (!l || !l[event.type]) return;
      event.target = this;
      var a = l[event.type].slice(0);
      for (var i = 0; i < a.length; i++) a[i].call(this, event);
      event.target = null;
    }
  };
  function eventful(proto) { Object.keys(events).forEach(function (k) { def(proto, k, events[k]); }); }
  T.EventDispatcher = function EventDispatcher() {};
  eventful(T.EventDispatcher.prototype);

  // ---- Layers ------------------------------------------------------------
  T.Layers = F.make(R.Layers, function () { this.mask = 1; });

  // ---- BufferAttribute ---------------------------------------------------
  // The crate keeps numbers as f64 rounded to the array kind; here they
  // live in the typed array three.js has (written through in place).
  var TYPED = [Float32Array, Uint32Array, Uint16Array, Uint8Array, Int32Array, Int16Array, Int8Array];
  function typed(array, kind) {
    if (array === undefined || array === null || kind === undefined) return array;
    if (array.BYTES_PER_ELEMENT) return array;
    return new TYPED[kind](array);
  }
  (function () {
    var p = R.BufferAttribute.prototype;
    adapt(p, "array", null, function (v) { return typed(v, this[hidden("kind")]); });
    adapt(p, "kind", null, function (v) {
      var a = this[hidden("array")];
      if (a && !a.BYTES_PER_ELEMENT) this[hidden("array")] = typed(a, v);
      return v;
    });
  })();
  function attribute(array, itemSize, normalized, kindOverride) {
    var kind = kindOverride !== undefined ? kindOverride : F.kindOf(array);
    if (Array.isArray(array)) throw new TypeError("THREE.BufferAttribute: array should be a Typed Array.");
    var a = R.BufferAttribute.new_([], itemSize, kind, normalized === true);
    a.array = array;
    a.count = array.length / itemSize;
    return a;
  }
  T.BufferAttribute = klass({ prototype: R.BufferAttribute.prototype }, function (array, itemSize, normalized) {
    return attribute(array, itemSize, normalized);
  });
  // the crate's BufferAttribute is the base: crate-made attributes are BufferAttributes
  T.BufferAttribute.prototype = R.BufferAttribute.prototype;
  def(R.BufferAttribute.prototype, "constructor", T.BufferAttribute);
  aliases(R.BufferAttribute);
  F.flag(T.BufferAttribute, "isBufferAttribute");
  (function () {
    var p = R.BufferAttribute.prototype;
    accessor(p, "itemSize", function () { return this.item_size; }, function (v) { this.item_size = v; });
    accessor(p, "updateRanges", function () { return this.update_ranges; }, function (v) { this.update_ranges = v; });
    accessor(p, "needsUpdate", function () { return false; }, function (v) { if (v === true) this.version++; });
    accessor(p, "gpuType", function () { return C.FloatType; }, function () {});
    def(p, "setUsage", function (u) { this.usage = u; return this; });
    def(p, "onUpload", function (cb) { this.onUploadCallback = cb; return this; });
    def(p, "onUploadCallback", function () {});
    var cp = p.copy, ca = p.copy_at, carr = p.copy_array, st = p.set;
    def(p, "copy", function (source) {
      this.name = source.name;
      this.array = new source.array.constructor(source.array);
      this.item_size = source.item_size; this.count = source.count; this.normalized = source.normalized;
      this.usage = source.usage; this.kind = source.kind;
      return this;
    });
    def(p, "copyAt", function (i1, attr, i2) { ca.call(this, i1, attr, i2); return this; });
    def(p, "copyArray", function (array) { this.array.set(array); return this; });
    def(p, "set", function (value, offset) { this.array.set(value, offset || 0); return this; });
    def(p, "setXY", p.set_xy); def(p, "setXYZ", p.set_xyz); def(p, "setXYZW", p.set_xyzw);
    ["applyMatrix3", "applyMatrix4", "applyNormalMatrix", "transformDirection", "setComponent",
      "setX", "setY", "setZ", "setW", "setXY", "setXYZ", "setXYZW", "addUpdateRange", "clearUpdateRanges"].forEach(function (k) {
      var f = p[k];
      def(p, k, function () { f.apply(this, arguments); return this; });
    });
    def(p, "clone", function () { return new this.constructor(this.array, this.itemSize).copy(this); });
    def(p, "toJSON", function () {
      var d = { itemSize: this.itemSize, type: this.array.constructor.name, array: Array.prototype.slice.call(this.array), normalized: this.normalized };
      if (this.name !== "") d.name = this.name;
      if (this.usage !== USAGE) d.usage = this.usage;
      return d;
    });
  })();
  [["Float32", Float32Array], ["Uint32", Uint32Array], ["Uint16", Uint16Array], ["Uint8", Uint8Array],
    ["Int32", Int32Array], ["Int16", Int16Array], ["Int8", Int8Array], ["Uint8Clamped", Uint8ClampedArray]].forEach(function (e) {
    var A = e[1];
    T[e[0] + "BufferAttribute"] = klass(T.BufferAttribute, function (array, itemSize, normalized) {
      var kind = A === Uint8ClampedArray ? 3 : F.KINDS[A.name];
      return attribute(new A(array), itemSize, normalized, kind);
    });
  });
  T.InstancedBufferAttribute = klass(T.BufferAttribute, function (array, itemSize, normalized) {
    return attribute(array, itemSize, normalized);
  }, function (array, itemSize, normalized, meshPerAttribute) { this.meshPerAttribute = meshPerAttribute || 1; });
  F.flag(T.InstancedBufferAttribute, "isInstancedBufferAttribute");
  // an interleaved buffer: several attributes reading one array with a stride
  T.InterleavedBuffer = function InterleavedBuffer(array, stride) {
    this.array = array; this.stride = stride; this.count = array.length / stride;
    this.usage = USAGE; this.version = 0; this.uuid = rng.uuid();
  };
  T.InterleavedBuffer.prototype.isInterleavedBuffer = true;
  T.InterleavedBuffer.prototype.setUsage = function (u) { this.usage = u; return this; };
  T.InterleavedBufferAttribute = function InterleavedBufferAttribute(data, itemSize, offset, normalized) {
    this.name = ""; this.data = data; this.itemSize = itemSize; this.offset = offset; this.normalized = normalized === true;
  };
  (function () {
    var p = T.InterleavedBufferAttribute.prototype;
    p.isInterleavedBufferAttribute = true;
    Object.defineProperty(p, "count", { get: function () { return this.data.count; } });
    Object.defineProperty(p, "array", { get: function () { return this.data.array; } });
    p.getComponent = function (i, c) { return this.data.array[i * this.data.stride + this.offset + c]; };
    p.setComponent = function (i, c, v) { this.data.array[i * this.data.stride + this.offset + c] = v; return this; };
    ["X", "Y", "Z", "W"].forEach(function (n, c) {
      p["get" + n] = function (i) { return this.getComponent(i, c); };
      p["set" + n] = function (i, v) { return this.setComponent(i, c, v); };
    });
    p.setXY = function (i, x, y) { return this.setX(i, x).setY(i, y); };
    p.setXYZ = function (i, x, y, z) { return this.setX(i, x).setY(i, y).setZ(i, z); };
    p.setXYZW = function (i, x, y, z, w) { return this.setX(i, x).setY(i, y).setZ(i, z).setW(i, w); };
  })();

  // ---- BufferGeometry ----------------------------------------------------
  var geometryIds = { n: 0 };
  rename(R.GeometryGroup.prototype, "material_index", "materialIndex");
  (function () {
    var p = R.BufferGeometry.prototype;
    mapField(p, ["attributes", "morph_attributes", "parameters"]);
    nullable(p, ["index", "bounding_box", "bounding_sphere"]);
    ids(p, geometryIds);
  })();
  function geometry(create) {
    return function () {
      var g = create.apply(null, arguments);
      g.id;
      return g;
    };
  }
  T.BufferGeometry = function BufferGeometry() {
    var g = R.BufferGeometry.new_();
    setProto(g, T.BufferGeometry, new.target);
    g.id;
    return g;
  };
  T.BufferGeometry.prototype = R.BufferGeometry.prototype;
  def(R.BufferGeometry.prototype, "constructor", T.BufferGeometry);
  aliases(R.BufferGeometry);
  F.flag(T.BufferGeometry, "isBufferGeometry");
  eventful(R.BufferGeometry.prototype);
  (function () {
    var p = R.BufferGeometry.prototype;
    camelFields(p, R.BufferGeometry.new_(), ["type_name"]);
    accessor(p, "type", function () { return this.type_name; }, function (v) { this.type_name = v; });
    accessor(p, "userData", function () {
      if (!this._userData) def(this, "_userData", {});
      return this._userData;
    }, function (v) { def(this, "_userData", v); });
    var si = p.set_index, sia = p.set_index_attribute;
    def(p, "setIndex", function (index) {
      if (Array.isArray(index)) si.call(this, index);
      else sia.call(this, index === null ? undefined : index);
      return this;
    });
    def(p, "getIndex", function () { return this.index; });
    def(p, "setAttribute", function (name, a) { this.set_attribute(name, a); return this; });
    def(p, "getAttribute", function (name) { return this.attributes[name]; });
    def(p, "deleteAttribute", function (name) { delete this.attributes[name]; return this; });
    def(p, "hasAttribute", function (name) { return this.attributes[name] !== undefined; });
    var ag = p.add_group;
    def(p, "addGroup", function (start, count, materialIndex) { ag.call(this, start, count, materialIndex || 0); });
    accessor(p, "drawRange", function () { return this.draw_range; }, function (v) { this.draw_range = v; });
    ["applyMatrix4", "applyQuaternion", "rotateX", "rotateY", "rotateZ", "translate", "scale", "center",
      "computeBoundingBox", "computeBoundingSphere", "computeVertexNormals", "normalizeNormals", "computeTangents"].forEach(function (k) {
      var f = p[k];
      if (f) def(p, k, function () { f.apply(this, arguments); return this; });
    });
    var la = p.look_at;
    def(p, "lookAt", function (v) { la.call(this, v); return this; });
    var sfp = p.set_from_points;
    def(p, "setFromPoints", function (points) {
      if (points.length && points[0].isVector2) {
        points = points.map(function (q) { return new T.Vector3(q.x, q.y, 0); });
      }
      sfp.call(this, points);
      return this;
    });
    var tni = p.to_non_indexed, dc = p.deep_clone, cpy = p.copy_from;
    def(p, "toNonIndexed", function () {
      if (this.index === null) { console.warn("THREE.BufferGeometry.toNonIndexed(): BufferGeometry is already non-indexed."); return this; }
      var g = tni.call(this);
      setProto(g, T.BufferGeometry);
      return g;
    });
    def(p, "copy", function (source) {
      var type = this.type_name, params = this.parameters;
      cpy.call(this, source);
      this.type_name = type; this.parameters = params;
      if (source._userData) def(this, "_userData", JSON.parse(JSON.stringify(source._userData)));
      return this;
    });
    def(p, "clone", function () {
      var g = dc.call(this);
      setProto(g, Object.getPrototypeOf(this).constructor === T.BufferGeometry ? T.BufferGeometry : Object.getPrototypeOf(this).constructor);
      newId(g);
      if (this.instanceCount !== undefined) g.instanceCount = this.instanceCount;
      return g;
    });
    def(p, "dispose", function () { this.dispatchEvent({ type: "dispose" }); });
    def(p, "toJSON", function () {
      var d = { metadata: { version: 4.7, type: "BufferGeometry", generator: "BufferGeometry.toJSON" } };
      d.uuid = this.uuid; d.type = this.type;
      if (this.name !== "") d.name = this.name;
      var params = this.parameters;
      if (Object.keys(params).length) {
        for (var k in params) if (params[k] !== undefined) d[k] = params[k];
        return d;
      }
      d.data = { attributes: {} };
      if (this.index) d.data.index = { type: this.index.array.constructor.name, array: Array.prototype.slice.call(this.index.array) };
      for (var n in this.attributes) d.data.attributes[n] = this.attributes[n].toJSON();
      return d;
    });
  })();
  T.InstancedBufferGeometry = klass(T.BufferGeometry, function () {
    var g = R.BufferGeometry.new_();
    g.type_name = "InstancedBufferGeometry";
    g.id;
    return g;
  }, function () { this.instanceCount = Infinity; });
  F.flag(T.InstancedBufferGeometry, "isInstancedBufferGeometry");

  // the generated geometries: three.js's constructor defaults, the crate's generator
  var PI = Math.PI;
  function geo(name, defaults, gen) {
    T[name] = klass(T.BufferGeometry, function () {
      var a = [];
      for (var i = 0; i < defaults.length; i++) a.push(arguments[i] === undefined ? defaults[i] : arguments[i]);
      var g = gen.apply(null, a);
      g.id;
      return g;
    });
  }
  var GP = R.GeometriesPrimitives, GH = R.GeometriesPolyhedron;
  geo("BoxGeometry", [1, 1, 1, 1, 1, 1], GP.box_geometry);
  geo("PlaneGeometry", [1, 1, 1, 1], GP.plane_geometry);
  geo("SphereGeometry", [1, 32, 16, 0, 2 * PI, 0, PI], GP.sphere_geometry);
  geo("CylinderGeometry", [1, 1, 1, 32, 1, false, 0, 2 * PI], GP.cylinder_geometry);
  geo("ConeGeometry", [1, 1, 32, 1, false, 0, 2 * PI], GP.cone_geometry);
  geo("CircleGeometry", [1, 32, 0, 2 * PI], GP.circle_geometry);
  geo("RingGeometry", [0.5, 1, 32, 1, 0, 2 * PI], GP.ring_geometry);
  geo("TorusGeometry", [1, 0.4, 12, 48, 2 * PI, 0, 2 * PI], GP.torus_geometry);
  geo("TorusKnotGeometry", [1, 0.4, 64, 8, 2, 3], GP.torus_knot_geometry);
  geo("CapsuleGeometry", [1, 1, 4, 8, 1], GP.capsule_geometry);
  geo("TetrahedronGeometry", [1, 0], GH.tetrahedron_geometry);
  geo("OctahedronGeometry", [1, 0], GH.octahedron_geometry);
  geo("IcosahedronGeometry", [1, 0], GH.icosahedron_geometry);
  geo("DodecahedronGeometry", [1, 0], GH.dodecahedron_geometry);
  T.PolyhedronGeometry = klass(T.BufferGeometry, function (vertices, indices, radius, detail) {
    var g = GH.polyhedron_geometry(vertices || [], indices || [], radius === undefined ? 1 : radius, detail || 0);
    g.id;
    return g;
  }, function (vertices, indices) { this.parameters.vertices = vertices || []; this.parameters.indices = indices || []; });
  T.LatheGeometry = klass(T.BufferGeometry, function (points, segments, phiStart, phiLength) {
    var pts = points ? points.map(function (q) { return new T.Vector2(q.x, q.y); }) : GP.lathe_default_points();
    var g = GP.lathe_geometry(pts, segments === undefined ? 12 : segments, phiStart || 0, phiLength === undefined ? 2 * PI : phiLength);
    g.id;
    return g;
  }, function (points) { this.parameters.points = points || GP.lathe_default_points(); });

  // ---- Material ----------------------------------------------------------
  var materialIds = { n: 0 };
  var TEXTURE_FIELDS = ["map_", "alpha_map", "ao_map", "light_map", "emissive_map", "bump_map", "normal_map", "displacement_map",
    "specular_map", "env_map", "roughness_map", "metalness_map", "gradient_map", "matcap", "shadow_side"];
  (function () {
    var p = R.Material.prototype;
    nullable(p, TEXTURE_FIELDS);
    ids(p, materialIds);
  })();
  T.Material = function Material() {
    var m = R.Material.new_(0);
    setProto(m, T.Material, new.target);
    m.id;
    return m;
  };
  T.Material.prototype = R.Material.prototype;
  def(R.Material.prototype, "constructor", T.Material);
  aliases(R.Material);
  F.flag(T.Material, "isMaterial");
  eventful(R.Material.prototype);
  (function () {
    var p = R.Material.prototype;
    camelFields(p, R.Material.new_(0), ["type_name"]);
    accessor(p, "type", function () { return this.type_name(); }, function () {});
    accessor(p, "userData", function () {
      if (!this._userData) def(this, "_userData", {});
      return this._userData;
    }, function (v) { def(this, "_userData", v); });
    accessor(p, "needsUpdate", function () { return false; }, function (v) { if (v === true) this.version++; });
    def(p, "setValues", function (values) {
      if (values === undefined) return;
      for (var key in values) {
        var v = values[key];
        if (v === undefined) { console.warn("THREE.Material: parameter '" + key + "' has value of undefined."); continue; }
        var cur = this[key];
        if (cur && cur.isColor) cur.set(v);
        else if (cur && cur.isVector2 && v && v.isVector2) cur.copy(v);
        else this[key] = v;
      }
    });
    def(p, "copy", function (source) {
      var keep = this.kind;
      var m = this.copied_from(source);
      var self = this;
      Object.keys(m).forEach(function (k) { if (k !== "kind") self[k] = m[k]; });
      ["color", "emissive", "specular", "sheen_color", "attenuation_color", "specular_color"].forEach(function (k) { self[k] = source[k].clone(); });
      this.normal_scale = source.normal_scale.clone();
      this.kind = keep;
      if (source._userData) def(this, "_userData", JSON.parse(JSON.stringify(source._userData)));
      return this;
    });
    def(p, "clone", function () {
      var m = new this.constructor();
      return m.copy(this);
    });
    def(p, "dispose", function () { this.dispatchEvent({ type: "dispose" }); });
    def(p, "onBeforeCompile", function () {});
    def(p, "onBeforeRender", function () {});
    def(p, "customProgramCacheKey", function () { return this.onBeforeCompile.toString(); });
    def(p, "toJSON", function () {
      var d = { metadata: { version: 4.7, type: "Material", generator: "Material.toJSON" } };
      d.uuid = this.uuid; d.type = this.type;
      if (this.name !== "") d.name = this.name;
      if (this.color) d.color = this.color.getHex();
      if (this.kind === 3 || this.kind === 4) { d.roughness = this.roughness; d.metalness = this.metalness; }
      if (this.kind === 2) { d.specular = this.specular.getHex(); d.shininess = this.shininess; }
      if (this.emissive && this.emissive.getHex() !== 0) d.emissive = this.emissive.getHex();
      if (this.opacity < 1) d.opacity = this.opacity;
      if (this.transparent) d.transparent = true;
      if (this.side !== 0) d.side = this.side;
      d.depthFunc = 3; d.depthTest = this.depth_test; d.depthWrite = this.depth_write; d.colorWrite = this.color_write;
      return d;
    });
  })();
  var MATERIALS = ["MeshBasicMaterial", "MeshLambertMaterial", "MeshPhongMaterial", "MeshStandardMaterial", "MeshPhysicalMaterial",
    "MeshToonMaterial", "MeshNormalMaterial", "MeshMatcapMaterial", "MeshDepthMaterial", "MeshDistanceMaterial", "ShadowMaterial",
    "LineBasicMaterial", "LineDashedMaterial", "PointsMaterial", "SpriteMaterial", "ShaderMaterial", "RawShaderMaterial"];
  MATERIALS.forEach(function (name, kind) {
    var base = name === "MeshPhysicalMaterial" ? T.MeshStandardMaterial : name === "LineDashedMaterial" ? T.LineBasicMaterial :
      name === "RawShaderMaterial" ? T.ShaderMaterial : T.Material;
    T[name] = klass(base, function () {
      var m = R.Material.new_(kind);
      m.id;
      return m;
    }, function (params) { this.setValues(params); });
    F.flag(T[name], "is" + name);
  });
  (function () {
    var sp = T.ShaderMaterial.prototype;
    ["uniforms", "defines", "vertexShader", "fragmentShader", "lights", "clipping", "glslVersion", "extensions", "uniformsGroups"].forEach(function (k) {
      var dflt = { uniforms: function () { return {}; }, defines: function () { return {}; },
        vertexShader: function () { return "void main() {\n\tgl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );\n}"; },
        fragmentShader: function () { return "void main() {\n\tgl_FragColor = vec4( 1.0, 0.0, 0.0, 1.0 );\n}"; },
        lights: function () { return false; }, clipping: function () { return false; }, glslVersion: function () { return null; },
        extensions: function () { return { clipCullDistance: false, multiDraw: false }; }, uniformsGroups: function () { return []; } }[k];
      accessor(sp, k, function () {
        var key = "_sh_" + k;
        if (!(key in this)) def(this, key, dflt());
        return this[key];
      }, function (v) { def(this, "_sh_" + k, v); });
    });
  })();

  // ---- Texture -----------------------------------------------------------
  var textureIds = { n: 0 };
  (function () {
    var p = R.Texture.prototype;
    ids(p, textureIds);
  })();
  function sourceOf(t) {
    if (!t._source) def(t, "_source", { isSource: true, version: 0, uuid: rng.uuid(), data: null });
    return t._source;
  }
  T.Texture = function Texture(image, mapping, wrapS, wrapT, magFilter, minFilter, format, type, anisotropy, colorSpace) {
    var t = R.Texture.new_();
    setProto(t, T.Texture, new.target);
    t.id;
    if (image !== undefined && image !== null) t.image = image;
    if (mapping !== undefined) t.mapping = mapping;
    if (wrapS !== undefined) t.wrap_s = wrapS;
    if (wrapT !== undefined) t.wrap_t = wrapT;
    if (magFilter !== undefined) t.mag_filter = magFilter;
    if (minFilter !== undefined) t.min_filter = minFilter;
    if (format !== undefined) t.format = format;
    if (type !== undefined) t.data_type = type;
    if (anisotropy !== undefined) t.anisotropy = anisotropy;
    if (colorSpace !== undefined) t.colorSpace = colorSpace;
    return t;
  };
  T.Texture.prototype = R.Texture.prototype;
  def(R.Texture.prototype, "constructor", T.Texture);
  aliases(R.Texture);
  F.flag(T.Texture, "isTexture");
  eventful(R.Texture.prototype);
  T.Texture.DEFAULT_IMAGE = null;
  T.Texture.DEFAULT_MAPPING = 300;
  T.Texture.DEFAULT_ANISOTROPY = 1;
  (function () {
    var p = R.Texture.prototype;
    camelFields(p, R.Texture.new_(), ["data_type", "color_space", "is_cube", "is_data", "wrap_s", "wrap_t", "wrap_r", "flip_y"]);
    accessor(p, "type", function () { return this.data_type; }, function (v) { this.data_type = v; });
    accessor(p, "wrapS", function () { return this.wrap_s; }, function (v) { this.wrap_s = v; });
    accessor(p, "wrapT", function () { return this.wrap_t; }, function (v) { this.wrap_t = v; });
    accessor(p, "wrapR", function () { return this.wrap_r; }, function (v) { this.wrap_r = v; });
    accessor(p, "flipY", function () { return this.flip_y; }, function (v) { this.flip_y = v; });
    accessor(p, "colorSpace", function () { return ["srgb-linear", "srgb", ""][this.color_space] !== undefined && this._noColorSpace ? "" : (this.color_space === 1 ? "srgb" : (this._noColorSpace === false ? "srgb-linear" : "")); },
      function (v) { def(this, "_noColorSpace", v === "" ? true : false); this.color_space = v === "srgb" ? 1 : 0; });
    accessor(p, "source", function () { return sourceOf(this); }, function (v) { def(this, "_source", v); });
    accessor(p, "needsUpdate", function () { return false; }, function (v) { if (v === true) { this.version++; sourceOf(this).version++; } });
    accessor(p, "userData", function () {
      if (!this._userData) def(this, "_userData", {});
      return this._userData;
    }, function (v) { def(this, "_userData", v); });
    adapt(p, "image", function (v) { return v === undefined ? null : v; });
    var um = p.update_matrix, tu = p.transform_uv;
    def(p, "updateMatrix", function () { um.call(this); });
    def(p, "transformUv", function (uv) { return uv.copy(tu.call(this, uv)); });
    def(p, "clone", function () {
      var t = new this.constructor();
      var c = this.rs_clone();
      Object.keys(c).forEach(function (k) { t[k] = c[k]; });
      t[hidden("id")] = 0; t[hidden("uuid")] = "";
      t.id;
      if (this._source) def(t, "_source", this._source);
      return t;
    });
    def(p, "copy", function (source) {
      var c = source.rs_clone(), self = this, id = this.id, uuid = this.uuid;
      Object.keys(c).forEach(function (k) { self[k] = c[k]; });
      this[hidden("id")] = id; this[hidden("uuid")] = uuid;
      return this;
    });
    def(p, "dispose", function () { this.dispatchEvent({ type: "dispose" }); });
    def(p, "setValues", function (values) { for (var k in values) this[k] = values[k]; });
    def(p, "toJSON", function () {
      return {
        metadata: { version: 4.7, type: "Texture", generator: "Texture.toJSON" },
        uuid: this.uuid, name: this.name, image: sourceOf(this).uuid, mapping: this.mapping, channel: 0,
        repeat: [this.repeat.x, this.repeat.y], offset: [this.offset.x, this.offset.y], center: [this.center.x, this.center.y],
        rotation: this.rotation, wrap: [this.wrap_s, this.wrap_t], format: this.format, internalFormat: null, type: this.data_type,
        colorSpace: this.colorSpace, minFilter: this.min_filter, magFilter: this.mag_filter, anisotropy: this.anisotropy,
        flipY: this.flip_y, generateMipmaps: this.generate_mipmaps, premultiplyAlpha: this.premultiply_alpha, unpackAlignment: this.unpack_alignment
      };
    });
  })();
  function dataImage(data, width, height, depth) {
    return { data: data || null, width: width || 1, height: height || 1, depth: depth || 1 };
  }
  T.DataTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; },
    function (data, width, height, format, type, mapping, wrapS, wrapT, magFilter, minFilter, anisotropy, colorSpace) {
      this.image = dataImage(data, width, height);
      this.is_data = true;
      if (format !== undefined) this.format = format;
      if (type !== undefined) this.data_type = type;
      if (mapping !== undefined) this.mapping = mapping;
      if (wrapS !== undefined) this.wrap_s = wrapS;
      if (wrapT !== undefined) this.wrap_t = wrapT;
      this.mag_filter = magFilter !== undefined ? magFilter : C.NearestFilter;
      this.min_filter = minFilter !== undefined ? minFilter : C.NearestFilter;
      if (anisotropy !== undefined) this.anisotropy = anisotropy;
      if (colorSpace !== undefined) this.colorSpace = colorSpace;
      this.generate_mipmaps = false; this.flip_y = false; this.unpack_alignment = 1;
    });
  F.flag(T.DataTexture, "isDataTexture");
  T.Data3DTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; }, function (data, width, height, depth) {
    this.image = dataImage(data, width, height, depth);
    this.mag_filter = C.NearestFilter; this.min_filter = C.NearestFilter; this.wrap_r = C.ClampToEdgeWrapping;
    this.generate_mipmaps = false; this.flip_y = false; this.unpack_alignment = 1;
  });
  F.flag(T.Data3DTexture, "isData3DTexture");
  T.DataArrayTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; }, function (data, width, height, depth) {
    this.image = dataImage(data, width, height, depth);
    this.mag_filter = C.NearestFilter; this.min_filter = C.NearestFilter; this.wrap_r = C.ClampToEdgeWrapping;
    this.generate_mipmaps = false; this.flip_y = false; this.unpack_alignment = 1;
  });
  F.flag(T.DataArrayTexture, "isDataArrayTexture");
  T.CubeTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; }, function (images, mapping) {
    this.image = images || [];
    this.mapping = mapping !== undefined ? mapping : C.CubeReflectionMapping;
    this.flip_y = false; this.is_cube = true;
  });
  F.flag(T.CubeTexture, "isCubeTexture");
  accessor(T.CubeTexture.prototype, "images", function () { return this.image; }, function (v) { this.image = v; });
  T.CanvasTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; }, function (canvas) {
    if (canvas !== undefined) this.image = canvas;
    this.version++;
  });
  F.flag(T.CanvasTexture, "isCanvasTexture");
  T.VideoTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; }, function (video) {
    if (video !== undefined) this.image = video;
    this.mag_filter = C.LinearFilter; this.min_filter = C.LinearFilter; this.generate_mipmaps = false;
  });
  F.flag(T.VideoTexture, "isVideoTexture");
  T.DepthTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; }, function (width, height, type) {
    this.image = dataImage(null, width, height);
    this.format = C.DepthFormat; this.data_type = type !== undefined ? type : C.UnsignedIntType;
    this.mag_filter = C.NearestFilter; this.min_filter = C.NearestFilter; this.flip_y = false; this.generate_mipmaps = false;
  });
  F.flag(T.DepthTexture, "isDepthTexture");
  T.CompressedTexture = klass(T.Texture, function () { var t = R.Texture.new_(); t.id; return t; }, function (mipmaps, width, height, format, type) {
    this.image = { width: width, height: height }; this.mipmaps = mipmaps;
    if (format !== undefined) this.format = format;
    if (type !== undefined) this.data_type = type;
    this.flip_y = false; this.generate_mipmaps = false;
  });
  F.flag(T.CompressedTexture, "isCompressedTexture");
  T.Source = function Source(data) { this.isSource = true; this.data = data === undefined ? null : data; this.version = 0; this.uuid = rng.uuid(); };
  T.RenderTarget = function RenderTarget(width, height, options) {
    options = options || {};
    this.width = width || 1; this.height = height || 1; this.depth = options.depth || 1;
    this.scissor = new T.Vector4(0, 0, this.width, this.height); this.scissorTest = false;
    this.viewport = new T.Vector4(0, 0, this.width, this.height);
    this.texture = new T.Texture({ width: this.width, height: this.height, depth: this.depth });
    this.texture.generate_mipmaps = options.generateMipmaps === true;
    this.texture.min_filter = options.minFilter !== undefined ? options.minFilter : C.LinearFilter;
    this.textures = [this.texture];
    this.depthBuffer = options.depthBuffer !== undefined ? options.depthBuffer : true;
    this.stencilBuffer = options.stencilBuffer === true;
    this.resolveDepthBuffer = true; this.resolveStencilBuffer = true;
    this.depthTexture = options.depthTexture || null;
    this.samples = options.samples || 0;
  };
  T.RenderTarget.prototype = Object.create(T.EventDispatcher.prototype);
  T.RenderTarget.prototype.isRenderTarget = true;
  T.RenderTarget.prototype.setSize = function (width, height, depth) {
    depth = depth || 1;
    if (this.width !== width || this.height !== height || this.depth !== depth) {
      this.width = width; this.height = height; this.depth = depth;
      for (var i = 0; i < this.textures.length; i++) { var im = this.textures[i].image; im.width = width; im.height = height; im.depth = depth; }
    }
    this.viewport.set(0, 0, width, height); this.scissor.set(0, 0, width, height);
  };
  T.RenderTarget.prototype.dispose = function () { this.dispatchEvent({ type: "dispose" }); };
  T.WebGLRenderTarget = function WebGLRenderTarget(width, height, options) { T.RenderTarget.call(this, width, height, options); };
  T.WebGLRenderTarget.prototype = Object.create(T.RenderTarget.prototype);
  T.WebGLRenderTarget.prototype.isWebGLRenderTarget = true;

  // ---- Fog ---------------------------------------------------------------
  T.Fog = function Fog(c, near, far) {
    var f = R.Fog.linear(color(c), near === undefined ? 1 : near, far === undefined ? 1000 : far);
    setProto(f, T.Fog, new.target);
    f.name = "";
    return f;
  };
  T.Fog.prototype = Object.create(R.Fog.prototype);
  def(T.Fog.prototype, "constructor", T.Fog);
  T.Fog.prototype.isFog = true;
  T.Fog.prototype.clone = function () { return new T.Fog(this.color, this.near, this.far); };
  T.Fog.prototype.toJSON = function () { return { type: "Fog", name: this.name, color: this.color.getHex(), near: this.near, far: this.far }; };
  T.FogExp2 = function FogExp2(c, density) {
    var f = R.Fog.exponential(color(c), density === undefined ? 0.00025 : density);
    setProto(f, T.FogExp2, new.target);
    f.name = "";
    return f;
  };
  T.FogExp2.prototype = Object.create(R.Fog.prototype);
  def(T.FogExp2.prototype, "constructor", T.FogExp2);
  T.FogExp2.prototype.isFogExp2 = true;
  T.FogExp2.prototype.clone = function () { return new T.FogExp2(this.color, this.density); };
  T.FogExp2.prototype.toJSON = function () { return { type: "FogExp2", name: this.name, color: this.color.getHex(), density: this.density }; };

  // ---- Object3D ----------------------------------------------------------
  var objectIds = { n: 0 };
  var KIND_CLASS = {};
  (function () {
    var p = R.Object3D.prototype;
    nullable(p, ["parent"]);
    stable(p, ["position", "quaternion", "scale"]);
    ids(p, objectIds);
  })();
  function object(kind) { return R.Object3D.new_(kind); }
  T.Object3D = function Object3D() {
    var o = object(new R.ObjectKind_Object3D());
    setProto(o, T.Object3D, new.target);
    o.id;
    return o;
  };
  T.Object3D.prototype = R.Object3D.prototype;
  def(R.Object3D.prototype, "constructor", T.Object3D);
  aliases(R.Object3D);
  F.flag(T.Object3D, "isObject3D");
  eventful(R.Object3D.prototype);
  T.Object3D.DEFAULT_UP = new T.Vector3(0, 1, 0);
  T.Object3D.DEFAULT_MATRIX_AUTO_UPDATE = true;
  T.Object3D.DEFAULT_MATRIX_WORLD_AUTO_UPDATE = true;
  // the crate object's facade class from its kind (for objects the crate made)
  function classify(o) {
    var K = KIND_CLASS[o.kind.__rg_kind];
    if (K && Object.getPrototypeOf(o) === R.Object3D.prototype && K !== T.Object3D) Object.setPrototypeOf(o, K.prototype);
    return o;
  }
  (function () {
    var p = R.Object3D.prototype;
    camelFields(p, object(new R.ObjectKind_Object3D()), ["rotation_order"]);
    accessor(p, "type", function () {
      return this._type !== undefined ? this._type : this.kind.type_name ? this.kind.type_name() : R.RsImpl_ObjectKind.type_name(this.kind);
    }, function (v) { def(this, "_type", v); });
    accessor(p, "userData", function () {
      if (!this._userData) def(this, "_userData", {});
      return this._userData;
    }, function (v) { def(this, "_userData", v); });
    accessor(p, "animations", function () {
      if (!this._animations) def(this, "_animations", []);
      return this._animations;
    }, function (v) { def(this, "_animations", v); });
    accessor(p, "isObject3D", function () { return true; });
    // three.js's rotation: an Euler that sets the quaternion when changed and
    // follows the quaternion when that changes
    accessor(p, "rotation", function () {
      var e = this._rot;
      if (!e) {
        e = new T.Euler();
        var self = this;
        e._onChange(function () {
          self.quaternion.set_from_euler(eulerCore(e));
          self.rotation_order = orderIndex(e._order);
          var q = self.quaternion;
          def(self, "_rq", [q.x, q.y, q.z, q.w, self.rotation_order]);
        });
        def(this, "_rot", e);
      }
      var q = this.quaternion, s = this._rq;
      if (!s || s[0] !== q.x || s[1] !== q.y || s[2] !== q.z || s[3] !== q.w || s[4] !== this.rotation_order) {
        var c = R.Euler.new_(0, 0, 0, 0);
        c.set_from_quaternion(q, this.rotation_order);
        e._x = c.x; e._y = c.y; e._z = c.z; e._order = ORDERS[this.rotation_order];
        def(this, "_rq", [q.x, q.y, q.z, q.w, this.rotation_order]);
      }
      return e;
    }, function (v) { this.rotation.copy(v); });
    accessor(p, "modelViewMatrix", function () {
      if (!this._mv) def(this, "_mv", new T.Matrix4());
      return this._mv;
    });
    accessor(p, "normalMatrix", function () {
      if (!this._nm) def(this, "_nm", new T.Matrix3());
      return this._nm;
    });
    def(p, "onBeforeRender", function () {});
    def(p, "onAfterRender", function () {});
    def(p, "onBeforeShadow", function () {});
    def(p, "onAfterShadow", function () {});
    ["applyQuaternion", "rotateOnAxis", "rotateOnWorldAxis", "rotateX", "rotateY", "rotateZ", "translateOnAxis", "translateX", "translateY", "translateZ"].forEach(function (k) {
      var f = p[k];
      def(p, k, function () { f.apply(this, arguments); return this; });
    });
    ["applyMatrix4", "setRotationFromAxisAngle", "setRotationFromMatrix", "setRotationFromQuaternion"].forEach(function (k) {
      var f = p[k];
      def(p, k, function () { f.apply(this, arguments); });
    });
    var sre = p.set_rotation_from_euler;
    def(p, "setRotationFromEuler", function (e) { sre.call(this, eulerCore(e)); });
    var ltw = p.local_to_world, wtl = p.world_to_local;
    def(p, "localToWorld", function (v) { O.update_world_matrix(this, true, false); return v.copy(ltw.call(this, v)); });
    def(p, "worldToLocal", function (v) { O.update_world_matrix(this, true, false); return v.copy(wtl.call(this, v)); });
    def(p, "lookAt", function (x, y, z) { O.look_at(this, vec3(x, y, z).clone()); });
    def(p, "add", function (object) {
      if (arguments.length > 1) { for (var i = 0; i < arguments.length; i++) this.add(arguments[i]); return this; }
      if (object === this) { console.error("THREE.Object3D.add: object can't be added as a child of itself.", object); return this; }
      if (object && object.isObject3D) {
        object.removeFromParent();
        O.add(this, object);
        object.dispatchEvent({ type: "added" });
        this.dispatchEvent({ type: "childadded", child: object });
      } else {
        console.error("THREE.Object3D.add: object not an instance of THREE.Object3D.", object);
      }
      return this;
    });
    def(p, "remove", function (object) {
      if (arguments.length > 1) { for (var i = 0; i < arguments.length; i++) this.remove(arguments[i]); return this; }
      if (O.remove(this, object)) {
        object.parent = null;
        object.dispatchEvent({ type: "removed" });
        this.dispatchEvent({ type: "childremoved", child: object });
      }
      return this;
    });
    def(p, "removeFromParent", function () { var parent = this.parent; if (parent !== null) parent.remove(this); return this; });
    def(p, "clear", function () { return this.remove.apply(this, this.children.slice()); });
    def(p, "attach", function (object) {
      O.update_world_matrix(this, true, false);
      var m = this.matrix_world.clone().invert();
      if (object.parent !== null) {
        O.update_world_matrix(object.parent, true, false);
        m.multiply(object.parent.matrix_world);
      }
      object.applyMatrix4(m);
      object.removeFromParent();
      object.parent = this;
      this.children.push(object);
      object.updateWorldMatrix(false, true);
      object.dispatchEvent({ type: "added" });
      this.dispatchEvent({ type: "childadded", child: object });
      return this;
    });
    def(p, "getObjectById", function (id) { return this.getObjectByProperty("id", id); });
    def(p, "getObjectByName", function (name) { return this.getObjectByProperty("name", name); });
    def(p, "getObjectByProperty", function (name, value) {
      if (this[name] === value) return this;
      for (var i = 0; i < this.children.length; i++) {
        var o = this.children[i].getObjectByProperty(name, value);
        if (o !== undefined) return o;
      }
      return undefined;
    });
    def(p, "getObjectsByProperty", function (name, value, result) {
      result = result || [];
      if (this[name] === value) result.push(this);
      for (var i = 0; i < this.children.length; i++) this.children[i].getObjectsByProperty(name, value, result);
      return result;
    });
    def(p, "getWorldPosition", function (t) { O.update_world_matrix(this, true, false); return t.copy(this.world_position()); });
    def(p, "getWorldQuaternion", function (t) { O.update_world_matrix(this, true, false); return t.copy(this.world_quaternion()); });
    def(p, "getWorldScale", function (t) { O.update_world_matrix(this, true, false); return t.copy(this.world_scale()); });
    def(p, "getWorldDirection", function (t) { O.update_world_matrix(this, true, false); return t.copy(this.world_direction()); });
    def(p, "traverse", function (cb) {
      cb(this);
      var c = this.children;
      for (var i = 0; i < c.length; i++) c[i].traverse(cb);
    });
    def(p, "traverseVisible", function (cb) {
      if (this.visible === false) return;
      cb(this);
      var c = this.children;
      for (var i = 0; i < c.length; i++) c[i].traverseVisible(cb);
    });
    def(p, "traverseAncestors", function (cb) {
      var parent = this.parent;
      if (parent !== null) { cb(parent); parent.traverseAncestors(cb); }
    });
    var um = p.update_matrix;
    def(p, "updateMatrix", function () { um.call(this); });
    def(p, "updateMatrixWorld", function (force) { O.update_matrix_world(this, force === true); });
    def(p, "updateWorldMatrix", function (parents, children) { O.update_world_matrix(this, parents === true, children === true); });
    function fix(src, dst) {
      Object.setPrototypeOf(dst, Object.getPrototypeOf(src));
      if (src._type !== undefined) def(dst, "_type", src._type);
      if (src._userData) def(dst, "_userData", JSON.parse(JSON.stringify(src._userData)));
      dst.id;
      if (src.copyExtra) src.copyExtra.call(dst, src);
    }
    function fixTree(src, dst) {
      fix(src, dst);
      for (var i = 0; i < dst.children.length && i < src.children.length; i++) fixTree(src.children[i], dst.children[i]);
    }
    def(p, "clone", function (recursive) {
      var c = O.clone_object(this, recursive !== false);
      fixTree(this, c);
      return c;
    });
    def(p, "copy", function (source, recursive) {
      this.name = source.name;
      this.up.copy(source.up);
      this.position.copy(source.position);
      this.rotation_order = source.rotation_order;
      this.quaternion.copy(source.quaternion);
      this.scale.copy(source.scale);
      this.matrix.copy(source.matrix);
      this.matrix_world.copy(source.matrix_world);
      this.matrix_auto_update = source.matrix_auto_update;
      this.matrix_world_auto_update = source.matrix_world_auto_update;
      this.matrix_world_needs_update = source.matrix_world_needs_update;
      this.layers.mask = source.layers.mask;
      this.visible = source.visible;
      this.cast_shadow = source.cast_shadow;
      this.receive_shadow = source.receive_shadow;
      this.frustum_culled = source.frustum_culled;
      this.render_order = source.render_order;
      if (this.kind.__rg_kind === source.kind.__rg_kind && this.kind.f0 !== undefined) {
        this.kind = R.RsImpl_ObjectKind.cloneOf(source.kind);
      }
      if (source._userData) def(this, "_userData", JSON.parse(JSON.stringify(source._userData)));
      if (recursive !== false) {
        for (var i = 0; i < source.children.length; i++) this.add(source.children[i].clone());
      }
      return this;
    });
    def(p, "toJSON", function (meta) {
      var isRoot = meta === undefined || typeof meta === "string";
      if (isRoot) meta = { geometries: {}, materials: {}, textures: {}, images: {}, shapes: {}, skeletons: {}, animations: {}, nodes: {} };
      var o = { uuid: this.uuid, type: this.type };
      if (this.name !== "") o.name = this.name;
      if (this.cast_shadow) o.castShadow = true;
      if (this.receive_shadow) o.receiveShadow = true;
      if (this.visible === false) o.visible = false;
      if (this.frustum_culled === false) o.frustumCulled = false;
      if (this.render_order !== 0) o.renderOrder = this.render_order;
      if (this._userData && Object.keys(this._userData).length) o.userData = this._userData;
      o.layers = this.layers.mask;
      o.matrix = this.matrix.toArray();
      o.up = this.up.toArray();
      if (this.matrix_auto_update === false) o.matrixAutoUpdate = false;
      if (this.toJSONExtra) this.toJSONExtra(o, meta);
      if (this.children.length > 0) {
        o.children = [];
        for (var i = 0; i < this.children.length; i++) o.children.push(this.children[i].toJSON(meta).object);
      }
      var out = { object: o };
      if (isRoot) {
        out.metadata = { version: 4.7, type: "Object", generator: "Object3D.toJSON" };
        ["geometries", "materials", "textures", "images", "shapes", "skeletons", "animations", "nodes"].forEach(function (k) {
          var vals = Object.keys(meta[k]).map(function (u) { return meta[k][u]; });
          if (vals.length) out[k] = vals;
        });
      }
      return out;
    });
  })();
  KIND_CLASS.ObjectKind_Object3D = T.Object3D;

  // the kinds: a facade class per crate ObjectKind, with its data's fields
  function kindClass(Base, kindName, create, fields, init) {
    var K = klass(Base, function () {
      var o = create.apply(null, arguments);
      o.id;
      return o;
    }, init);
    KIND_CLASS["ObjectKind_" + kindName] = K;
    var p = K.prototype;
    (fields || []).forEach(function (f) {
      var c = camel(f);
      accessor(p, c, function () { var v = this.kind.f0[f]; return v === undefined ? null : v; }, function (v) { this.kind.f0[f] = v === null ? undefined : v; });
    });
    return K;
  }

  T.Group = kindClass(T.Object3D, "Group", function () { return object(new R.ObjectKind_Group()); });
  F.flag(T.Group, "isGroup");
  T.Bone = kindClass(T.Object3D, "Bone", function () { return object(new R.ObjectKind_Bone()); });
  F.flag(T.Bone, "isBone");

  // Scene
  T.Scene = kindClass(T.Object3D, "Scene", function () { return R.Object3D.scene(); },
    ["background_blurriness", "background_intensity", "environment", "environment_intensity", "override_material"]);
  F.flag(T.Scene, "isScene");
  (function () {
    var p = T.Scene.prototype;
    accessor(p, "background", function () {
      var b = this.kind.f0.background;
      return b.__rg_kind === "Background_None" ? null : b.f0;
    }, function (v) {
      var d = this.kind.f0;
      if (v === null || v === undefined) d.background = new R.Background_None();
      else if (v.isColor) d.background = new R.Background_Color(v);
      else d.background = new R.Background_Texture(v);
    });
    accessor(p, "fog", function () { var f = this.kind.f0.fog; return f === undefined ? null : f; }, function (v) { this.kind.f0.fog = v === null ? undefined : v; });
    accessor(p, "backgroundRotation", function () { if (!this._br) def(this, "_br", new T.Euler()); return this._br; });
    accessor(p, "environmentRotation", function () { if (!this._er) def(this, "_er", new T.Euler()); return this._er; });
    def(p, "toJSONExtra", function (o) {
      if (this.fog) o.fog = this.fog.toJSON();
      if (this.background && this.background.isColor) o.background = this.background.getHex();
    });
  })();

  // meshes, points, lines
  function meshFields(K) {
    var p = K.prototype;
    accessor(p, "geometry", function () { return this.kind.f0.geometry; }, function (g) { this.kind.f0.geometry = g; });
    accessor(p, "material", function () {
      var d = this.kind.f0;
      return d.material_is_array ? d.materials : d.materials[0];
    }, function (m) {
      var d = this.kind.f0;
      if (Array.isArray(m)) { d.materials = m; d.material_is_array = true; } else { d.materials = [m]; d.material_is_array = false; }
    });
    accessor(p, "morphTargetInfluences", function () {
      var d = this.kind.f0;
      return d.morph_target_influences.length || Object.keys(this.geometry.morphAttributes).length ? d.morph_target_influences : undefined;
    }, function (v) { this.kind.f0.morph_target_influences = v; });
    accessor(p, "morphTargetDictionary", function () {
      var d = this.kind.f0;
      if (!Object.keys(this.geometry.morphAttributes).length) return undefined;
      var o = {};
      d.morph_target_dictionary.order.forEach(function (k) { o[k] = d.morph_target_dictionary.vals[k]; });
      return o;
    });
    def(p, "updateMorphTargets", function () {
      var d = this.kind.f0, ma = this.geometry.morphAttributes, keys = Object.keys(ma);
      if (!keys.length) return;
      var list = ma[keys[0]];
      d.morph_target_influences = [];
      d.morph_target_dictionary = new (d.morph_target_dictionary.constructor)();
      for (var i = 0; i < list.length; i++) {
        d.morph_target_influences.push(0);
        d.morph_target_dictionary.put(list[i].name || String(i), i);
      }
    });
    def(p, "copyExtra", function () {});
    def(p, "toJSONExtra", function (o, meta) {
      var g = this.geometry, m = this.material;
      if (!meta.geometries[g.uuid]) meta.geometries[g.uuid] = g.toJSON();
      o.geometry = g.uuid;
      var ms = Array.isArray(m) ? m : [m];
      ms.forEach(function (x) { if (!meta.materials[x.uuid]) meta.materials[x.uuid] = x.toJSON(); });
      o.material = Array.isArray(m) ? ms.map(function (x) { return x.uuid; }) : m.uuid;
    });
  }
  function defaultGeometry() { var g = R.BufferGeometry.new_(); g.id; return g; }
  function meshArgs(geometry, material, dflt) {
    var g = geometry || defaultGeometry();
    var ms = material === undefined ? (dflt ? new dflt() : new T.MeshBasicMaterial()) : material;
    return [g, Array.isArray(ms) ? (ms[0] || new T.MeshBasicMaterial()) : ms, Array.isArray(ms) ? ms : null];
  }
  function meshKind(name, factory, dflt, flagName, Base) {
    var K = kindClass(Base || T.Object3D, name, function (geometry, material) {
      var a = meshArgs(geometry, material, dflt);
      var o = factory(a[0], a[1]);
      if (a[2]) { o.kind.f0.materials = a[2]; o.kind.f0.material_is_array = true; }
      return o;
    }, [], function () { this.updateMorphTargets(); });
    meshFields(K);
    F.flag(K, flagName);
    return K;
  }
  T.Mesh = meshKind("Mesh", R.Object3D.mesh, null, "isMesh");
  T.Points = meshKind("Points", R.Object3D.points, T.PointsMaterial, "isPoints");
  T.Line = meshKind("Line", R.Object3D.line, T.LineBasicMaterial, "isLine");
  T.LineSegments = meshKind("LineSegments", R.Object3D.line_segments, T.LineBasicMaterial, "isLineSegments", T.Line);
  T.LineLoop = meshKind("LineLoop", R.Object3D.line_loop, T.LineBasicMaterial, "isLineLoop", T.Line);
  T.InstancedMesh = kindClass(T.Mesh, "InstancedMesh", function (geometry, material, count) {
    var a = meshArgs(geometry, material);
    var o = R.Object3D.instanced_mesh(a[0], a[1], count === undefined ? 1 : count);
    if (a[2]) { o.kind.f0.materials = a[2]; o.kind.f0.material_is_array = true; }
    return o;
  }, ["count", "instance_matrix", "instance_color"]);
  F.flag(T.InstancedMesh, "isInstancedMesh");
  (function () {
    var p = T.InstancedMesh.prototype;
    var MI = R.CoreObject3d;
    var B = R.CoreBounds;
    def(p, "setMatrixAt", function (i, m) { B.set_instance_matrix_at(this.kind.f0, i, m); return this; });
    def(p, "getMatrixAt", function (i, m) { return m.copy(B.instance_matrix_at(this.kind.f0, i)); });
    def(p, "setColorAt", function (i, c) {
      O.ensure_instance_colors(this.kind.f0);
      this.instanceColor.setXYZ(i, c.r, c.g, c.b);
      return this;
    });
    def(p, "getColorAt", function (i, c) { return c.fromBufferAttribute(this.instanceColor, i); });
    def(p, "computeBoundingBox", function () { def(this, "_bb", B.instance_bounding_box(this.kind.f0)); });
    def(p, "computeBoundingSphere", function () { def(this, "_bs", B.instance_bounding_sphere(this.kind.f0)); });
    accessor(p, "boundingBox", function () { return this._bb === undefined ? null : this._bb; }, function (v) { def(this, "_bb", v); });
    accessor(p, "boundingSphere", function () { return this._bs === undefined ? null : this._bs; }, function (v) { def(this, "_bs", v); });
    def(p, "dispose", function () { this.dispatchEvent({ type: "dispose" }); return this; });
  })();

  def(T.Mesh.prototype, "getVertexPosition", function (i, target) { return target.copy(O.vertex_position(this.kind.f0, i)); });
  def(T.Line.prototype, "computeLineDistances", function () { O.compute_line_distances(this.geometry, false); return this; });
  def(T.LineSegments.prototype, "computeLineDistances", function () { O.compute_line_distances(this.geometry, true); return this; });
  (function () {
    var B = R.CoreBounds, b3 = T.Box3.prototype, fr = T.Frustum.prototype;
    def(b3, "setFromObject", function (o, precise) { return this.copy(B.box_from_object(o, precise === true)); });
    def(b3, "expandByObject", function (o, precise) { B.expand_by_object(this, o, precise === true); return this; });
    def(fr, "intersectsObject", function (o) { var s = B.world_bounding_sphere(o); return s ? this.intersects_sphere(s) : false; });
    def(fr, "intersectsSprite", function (o) { var s = B.world_bounding_sphere(o); return s ? this.intersects_sphere(s) : false; });
  })();

  // Sprite
  T.Sprite = kindClass(T.Object3D, "Sprite", function (material) {
    return R.Object3D.sprite(material || new T.SpriteMaterial());
  }, ["geometry", "material", "center"]);
  F.flag(T.Sprite, "isSprite");

  // LOD
  T.LOD = kindClass(T.Object3D, "Lod", function () { return R.Object3D.lod(); }, ["levels", "auto_update"]);
  F.flag(T.LOD, "isLOD");
  (function () {
    var p = T.LOD.prototype;
    def(p, "addLevel", function (object, distance, hysteresis) {
      object.removeFromParent();
      O.lod_add_level(this, object, distance || 0, hysteresis || 0);
      object.dispatchEvent({ type: "added" });
      this.dispatchEvent({ type: "childadded", child: object });
      return this;
    });
    def(p, "removeLevel", function (distance) {
      var levels = this.levels;
      for (var i = 0; i < levels.length; i++) {
        if (levels[i].distance === distance) { var r = levels.splice(i, 1); this.remove(r[0].object); return true; }
      }
      return false;
    });
    def(p, "getCurrentLevel", function () { return this.kind.f0.current_level; });
    def(p, "getObjectForDistance", function (d) { var o = O.lod_object_for_distance(this, d); return o === undefined ? null : o; });
    def(p, "update", function (camera) { O.lod_update(this, camera); });
  })();

  // cameras
  [["full_width", "fullWidth"], ["full_height", "fullHeight"], ["offset_x", "offsetX"], ["offset_y", "offsetY"]].forEach(function (e) {
    rename(R.ViewOffset.prototype, e[0], e[1]);
  });
  var CAMERA_FIELDS = ["fov", "aspect", "near", "far", "zoom", "focus", "film_gauge", "film_offset", "left", "right", "top", "bottom",
    "view", "projection_matrix", "projection_matrix_inverse", "matrix_world_inverse"];
  T.Camera = klass(T.Object3D, function () {
    var o = R.Object3D.perspective_camera(50, 1, 0.1, 2000);
    o.id;
    return o;
  });
  F.flag(T.Camera, "isCamera");
  CAMERA_FIELDS.forEach(function (f) {
    accessor(T.Camera.prototype, camel(f), function () { var v = this.kind.f0[f]; return v === undefined ? null : v; }, function (v) { this.kind.f0[f] = v === null ? undefined : v; });
  });
  (function () {
    var p = T.Camera.prototype;
    accessor(p, "coordinateSystem", function () { return 2000; });
    def(p, "updateProjectionMatrix", function () { this.update_projection_matrix(); });
    def(p, "setViewOffset", function (fw, fh, x, y, w, h) { this.set_view_offset(fw, fh, x, y, w, h); });
    def(p, "clearViewOffset", function () { this.clear_view_offset(); });
    var umw = R.Object3D.prototype.updateMatrixWorld, uwm = R.Object3D.prototype.updateWorldMatrix;
    def(p, "getFocalLength", function () { return this.kind.f0.focal_length(); });
    def(p, "setFocalLength", function (f) { this.kind.f0.set_focal_length(f); this.update_projection_matrix(); });
    def(p, "getEffectiveFOV", function () { return this.kind.f0.effective_fov(); });
    def(p, "getFilmWidth", function () { return this.kind.f0.film_width(); });
    def(p, "getFilmHeight", function () { return this.kind.f0.film_height(); });
    def(p, "getViewBounds", function (distance, min, max) {
      var b = this.kind.f0.view_bounds(distance);
      min.copy(b[0]); max.copy(b[1]);
    });
    def(p, "getViewSize", function (distance, target) {
      var b = this.kind.f0.view_bounds(distance);
      return target.subVectors(b[1], b[0]);
    });
    def(p, "toJSONExtra", function (o) {
      var d = this.kind.f0;
      if (this.kind.__rg_kind === "ObjectKind_PerspectiveCamera") {
        o.fov = d.fov; o.zoom = d.zoom; o.near = d.near; o.far = d.far; o.focus = d.focus; o.aspect = d.aspect;
        o.filmGauge = d.film_gauge; o.filmOffset = d.film_offset;
      } else {
        o.zoom = d.zoom; o.left = d.left; o.right = d.right; o.top = d.top; o.bottom = d.bottom; o.near = d.near; o.far = d.far;
      }
    });
  })();
  T.PerspectiveCamera = kindClass(T.Camera, "PerspectiveCamera", function (fov, aspect, near, far) {
    var o = R.Object3D.perspective_camera(fov === undefined ? 50 : fov, aspect === undefined ? 1 : aspect, near === undefined ? 0.1 : near, far === undefined ? 2000 : far);
    return o;
  });
  F.flag(T.PerspectiveCamera, "isPerspectiveCamera");
  T.OrthographicCamera = kindClass(T.Camera, "OrthographicCamera", function (left, right, top, bottom, near, far) {
    return R.Object3D.orthographic_camera(left === undefined ? -1 : left, right === undefined ? 1 : right, top === undefined ? 1 : top,
      bottom === undefined ? -1 : bottom, near === undefined ? 0.1 : near, far === undefined ? 2000 : far);
  });
  F.flag(T.OrthographicCamera, "isOrthographicCamera");
  T.ArrayCamera = klass(T.PerspectiveCamera, function () {
    var o = R.Object3D.perspective_camera(50, 1, 0.1, 2000);
    o.id;
    return o;
  }, function (cameras) { this.cameras = cameras || []; });
  F.flag(T.ArrayCamera, "isArrayCamera");

  // lights
  var LIGHT_FIELDS = ["color", "intensity", "ground_color", "distance", "decay", "angle", "penumbra", "width", "height", "target", "shadow", "map_"];
  T.Light = klass(T.Object3D, function (c, intensity) {
    var o = R.Object3D.ambient_light(color(c === undefined ? 0xffffff : c), intensity === undefined ? 1 : intensity);
    o.id;
    return o;
  });
  F.flag(T.Light, "isLight");
  LIGHT_FIELDS.forEach(function (f) {
    accessor(T.Light.prototype, f === "map_" ? "map" : camel(f), function () { var v = this.kind.f0[f]; return v === undefined ? null : v; }, function (v) {
      if ((f === "color" || f === "ground_color") && !(v && v.isColor)) { this.kind.f0[f].set(v); return; }
      this.kind.f0[f] = v === null ? undefined : v;
    });
  });
  (function () {
    var p = T.Light.prototype;
    def(p, "dispose", function () {});
    def(p, "toJSONExtra", function (o) {
      var d = this.kind.f0;
      o.color = d.color.getHex(); o.intensity = d.intensity;
      var k = this.kind.__rg_kind;
      if (k === "ObjectKind_HemisphereLight") o.groundColor = d.ground_color.getHex();
      if (k === "ObjectKind_PointLight" || k === "ObjectKind_SpotLight") { o.distance = d.distance; o.decay = d.decay; }
      if (k === "ObjectKind_SpotLight") { o.angle = d.angle; o.penumbra = d.penumbra; }
      if (k === "ObjectKind_RectAreaLight") { o.width = d.width; o.height = d.height; }
      if (d.target) o.target = d.target.uuid;
    });
    // the shadow camera is a crate object: give it its facade class
    var sh = Object.getOwnPropertyDescriptor(p, "shadow");
    accessor(p, "shadow", function () {
      var s = sh.get.call(this);
      if (s && s.camera) classify(s.camera);
      if (s && !s._f) {
        def(s, "_f", true);
        accessor(s, "mapSize", function () { return this.map_size; }, function (v) { this.map_size = v; });
        accessor(s, "normalBias", function () { return this.normal_bias; }, function (v) { this.normal_bias = v; });
        accessor(s, "blurSamples", function () { return this.blur_samples; }, function (v) { this.blur_samples = v; });
        accessor(s, "autoUpdate", function () { return this.auto_update; }, function (v) { this.auto_update = v; });
        accessor(s, "needsUpdate", function () { return this.needs_update; }, function (v) { this.needs_update = v; });
      }
      return s;
    }, sh.set);
    var tg = Object.getOwnPropertyDescriptor(p, "target");
    accessor(p, "target", function () { var t = tg.get.call(this); if (t) { classify(t); t.id; } return t; }, tg.set);
  })();
  function light(name, factory, defaults, init) {
    var K = kindClass(T.Light, name, function () {
      var a = [];
      for (var i = 0; i < defaults.length; i++) a.push(arguments[i] === undefined ? defaults[i] : arguments[i]);
      a[0] = color(a[0]);
      if (name === "HemisphereLight") a[1] = color(a[1]);
      return factory.apply(null, a);
    }, [], init);
    F.flag(K, "is" + name);
    return K;
  }
  T.AmbientLight = light("AmbientLight", R.Object3D.ambient_light, [0xffffff, 1]);
  T.HemisphereLight = light("HemisphereLight", R.Object3D.hemisphere_light, [0xffffff, 0xffffff, 1], function () { this.position.copy(T.Object3D.DEFAULT_UP); this.updateMatrix(); });
  T.DirectionalLight = light("DirectionalLight", R.Object3D.directional_light, [0xffffff, 1], function () { this.position.copy(T.Object3D.DEFAULT_UP); this.updateMatrix(); });
  T.PointLight = light("PointLight", R.Object3D.point_light, [0xffffff, 1, 0, 2]);
  T.SpotLight = light("SpotLight", R.Object3D.spot_light, [0xffffff, 1, 0, Math.PI / 3, 0, 2], function () { this.position.copy(T.Object3D.DEFAULT_UP); this.updateMatrix(); });
  T.RectAreaLight = light("RectAreaLight", R.Object3D.rect_area_light, [0xffffff, 1, 10, 10]);
  accessor(T.PointLight.prototype, "power", function () { return this.intensity * 4 * Math.PI; }, function (v) { this.intensity = v / (4 * Math.PI); });
  accessor(T.SpotLight.prototype, "power", function () { return this.intensity * Math.PI; }, function (v) { this.intensity = v / Math.PI; });
  accessor(T.RectAreaLight.prototype, "power", function () { return this.intensity * this.width * this.height * Math.PI; }, function (v) { this.intensity = v / (this.width * this.height * Math.PI); });
  // ---- Raycaster ---------------------------------------------------------
  T.Raycaster = function Raycaster(origin, direction, near, far) {
    var r = R.Raycaster.new_(origin ? origin.clone() : new T.Vector3(), direction ? direction.clone() : new T.Vector3(0, 0, -1),
      near === undefined ? 0 : near, far === undefined ? Infinity : far);
    setProto(r, T.Raycaster, new.target);
    return r;
  };
  T.Raycaster.prototype = R.Raycaster.prototype;
  def(R.Raycaster.prototype, "constructor", T.Raycaster);
  aliases(R.Raycaster);
  nullable(R.Raycaster.prototype, ["camera"]);
  (function () {
    var p = R.Raycaster.prototype;
    accessor(p, "params", function () {
      if (!this._params) {
        var self = this, line = {}, points = {};
        accessor(line, "threshold", function () { return self.line_threshold; }, function (v) { self.line_threshold = v; });
        accessor(points, "threshold", function () { return self.points_threshold; }, function (v) { self.points_threshold = v; });
        Object.defineProperty(line, "threshold", { enumerable: true });
        Object.defineProperty(points, "threshold", { enumerable: true });
        def(this, "_params", { Mesh: {}, Line: line, LOD: {}, Points: points, Sprite: {} });
      }
      return this._params;
    }, function (v) {
      if (v && v.Line && v.Line.threshold !== undefined) this.line_threshold = v.Line.threshold;
      if (v && v.Points && v.Points.threshold !== undefined) this.points_threshold = v.Points.threshold;
    });
    var sfc = p.set_from_camera, io = p.intersect_object, ios = p.intersect_objects;
    def(p, "setFromCamera", function (coords, camera) {
      if (!camera.isPerspectiveCamera && !camera.isOrthographicCamera) { console.error("THREE.Raycaster: Unsupported camera type: " + camera.type); return; }
      sfc.call(this, coords, camera);
    });
    def(p, "setFromXRController", function (controller) {
      var m = new T.Matrix4().identity().extractRotation(controller.matrixWorld);
      this.ray.origin.setFromMatrixPosition(controller.matrixWorld);
      this.ray.direction.set(0, 0, -1).applyMatrix4(m);
      return this;
    });
    // a crate Intersection as the plain object three.js answers
    function hit(h) {
      var o = { distance: h.distance, point: h.point, object: h.object };
      if (h.face !== undefined && h.face !== null) {
        if (h.uv !== undefined) o.uv = h.uv;
        if (h.uv1 !== undefined) o.uv1 = h.uv1;
        if (h.normal !== undefined) o.normal = h.normal;
        var f = h.face;
        o.face = { a: f.a, b: f.b, c: f.c, normal: f.normal, materialIndex: f.material_index };
        o.barycoord = h.barycoord;
        o.faceIndex = h.face_index;
        if (h.instance_id !== undefined) o.instanceId = h.instance_id;
      } else if (h.index !== undefined) {
        if (h.distance_to_ray !== undefined) o.distanceToRay = h.distance_to_ray;
        o.index = h.index; o.face = null; o.faceIndex = null; o.barycoord = null;
      } else {
        o.uv = h.uv === undefined ? null : h.uv; o.face = null;
      }
      return o;
    }
    function collect(list, intersects) {
      for (var i = 0; i < list.length; i++) intersects.push(hit(list[i]));
      intersects.sort(function (a, b) { return a.distance - b.distance; });
      return intersects;
    }
    def(p, "intersectObject", function (object, recursive, intersects) {
      if (this.camera === null && hasSprite(object, recursive !== false)) console.error("THREE.Sprite: \"Raycaster.camera\" needs to be set in order to raycast against sprites.");
      return collect(io.call(this, object, recursive !== false), intersects || []);
    });
    def(p, "intersectObjects", function (objects, recursive, intersects) {
      return collect(ios.call(this, objects, recursive !== false), intersects || []);
    });
    function hasSprite(o, deep) {
      if (o.isSprite) return true;
      if (deep) for (var i = 0; i < o.children.length; i++) if (hasSprite(o.children[i], true)) return true;
      return false;
    }
  })();

  // ---- Clock / Timer -----------------------------------------------------
  function now() { return typeof performance !== "undefined" && performance.now ? performance.now() : Date.now(); }
  T.Clock = function Clock(autoStart) {
    var c = R.Clock.new_(autoStart !== false);
    setProto(c, T.Clock, new.target);
    return c;
  };
  T.Clock.prototype = R.Clock.prototype;
  def(R.Clock.prototype, "constructor", T.Clock);
  (function () {
    var p = R.Clock.prototype;
    camelFields(p, R.Clock.new_(true));
    var st = p.start, sp = p.stop, ge = p.get_elapsed_time, gd = p.get_delta;
    def(p, "start", function () { st.call(this, now()); });
    def(p, "stop", function () { sp.call(this, now()); });
    def(p, "getElapsedTime", function () { return ge.call(this, now()); });
    def(p, "getDelta", function () { return gd.call(this, now()); });
  })();
  T.Timer = function Timer() {
    var t = R.Timer.new_(now());
    setProto(t, T.Timer, new.target);
    return t;
  };
  T.Timer.prototype = R.Timer.prototype;
  def(R.Timer.prototype, "constructor", T.Timer);
  (function () {
    var p = R.Timer.prototype;
    var up = p.update, rs = p.reset, sts = p.set_timescale;
    def(p, "getDelta", p.get_delta);
    def(p, "getElapsed", p.get_elapsed);
    def(p, "getTimescale", p.get_timescale);
    def(p, "setTimescale", function (s) { sts.call(this, s); return this; });
    def(p, "reset", function () { rs.call(this, now()); return this; });
    def(p, "update", function (timestamp) {
      if (this._doc && this._doc.hidden === true) { this.delta = 0; return this; }
      up.call(this, timestamp !== undefined ? timestamp : now());
      return this;
    });
    def(p, "connect", function (doc) { def(this, "_doc", doc); });
    def(p, "disconnect", function () { def(this, "_doc", null); });
    def(p, "dispose", function () { def(this, "_doc", null); });
  })();
  F.klass = klass;
})(THREE, RangerThreeCore);
