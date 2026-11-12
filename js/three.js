// SPDX-License-Identifier: MIT
//
// three.js's API (`THREE`) over RangerThree's three crate as Ranger compiles
// it to JavaScript (build/rangerthree-core.js, `RangerThreeCore`). The crate
// is idiomatic Rust: snake_case methods, values returned instead of written
// into a target argument, enums instead of strings. This file is the thin
// layer that gives a .tsx document or a three.js program the names and
// calling conventions it expects:
//
//   - every snake_case method gets its camelCase name (add_scalar -> addScalar)
//   - `new THREE.Vector3(1, 2, 3)` makes the crate's Vector3 (the facade
//     constructor returns a core instance; prototypes are shared, so
//     instanceof holds)
//   - three.js methods that fill a `target` call the Rust method that
//     returns the value and copy it into the target
//   - strings (Euler orders, colour spaces) become the crate's enums
//
// Nothing here does math of its own: what the crate lacks fails here too,
// so the parity meter measures the crate.
var THREE = (function (R) {
  "use strict";
  var T = {};
  T.REVISION = "185";

  // ---- helpers -----------------------------------------------------------
  function camel(s) {
    return s.replace(/_([a-z0-9])/g, function (_, c) { return c.toUpperCase(); });
  }
  function def(obj, name, value) {
    Object.defineProperty(obj, name, { value: value, writable: true, configurable: true, enumerable: false });
  }
  function accessor(obj, name, get, set) {
    Object.defineProperty(obj, name, { get: get, set: set, configurable: true, enumerable: false });
  }
  // camelCase names for a core class's snake_case methods and statics
  // three.js's isX names are boolean flags (`if (o.isCamera)`), so a crate
  // is_x method gets a camelCase alias only where three.js has that method
  var IS_METHODS = { is_empty: 1, is_enabled: 1, is_front_facing: 1 };
  function aliases(C) {
    var p = C.prototype;
    Object.getOwnPropertyNames(p).forEach(function (k) {
      if (k.slice(0, 3) === "is_" && !IS_METHODS[k]) return;
      if (k.indexOf("_") > 0 && k.slice(0, 3) !== "rs_") {
        var d = Object.getOwnPropertyDescriptor(p, k);
        if (typeof d.value !== "function") return;
        var c = camel(k);
        if (!Object.prototype.hasOwnProperty.call(p, c)) def(p, c, d.value);
      }
    });
    if (p.rs_clone && !Object.prototype.hasOwnProperty.call(p, "clone")) def(p, "clone", p.rs_clone);
  }
  // a facade constructor: makes a core instance and runs init on it
  function make(Core, init) {
    function C() {
      var o = new Core();
      if (init) init.apply(o, arguments);
      return o;
    }
    C.prototype = Core.prototype;
    def(Core.prototype, "constructor", C);
    aliases(Core);
    return C;
  }
  function flag(C, name) {
    accessor(C.prototype, name, function () { return true; });
  }
  // three.js `method(...args, target)`: the core answers the value
  function intoTarget(C, js, rust, nargs) {
    var f = C.prototype[rust];
    def(C.prototype, js, function () {
      var args = Array.prototype.slice.call(arguments, 0, nargs);
      var target = arguments[nargs];
      var v = f.apply(this, args);
      if (v === undefined || v === null) return null;
      if (target === undefined || target === null) return v;
      if (typeof v === "number" || typeof v === "boolean") return v;
      return target.copy(v);
    });
  }
  function fromArrayDefault(C) {
    var f = C.prototype.from_array;
    def(C.prototype, "fromArray", function (array, offset) {
      return f.call(this, Array.prototype.slice.call(array), offset || 0);
    });
  }
  function toArrayInto(C) {
    var f = C.prototype.to_array;
    def(C.prototype, "toArray", function (array, offset) {
      var a = f.call(this);
      if (array === undefined) return a;
      offset = offset || 0;
      for (var i = 0; i < a.length; i++) array[offset + i] = a[i];
      return array;
    });
  }
  function iterate(C, keys) {
    def(C.prototype, Symbol.iterator, function () {
      var self = this, i = 0;
      return { next: function () { return i < keys.length ? { value: self[keys[i++]], done: false } : { value: undefined, done: true }; } };
    });
  }

  // one random source for the API's random() calls (three.js uses Math.random)
  var rng = R.Rng.new_(Math.floor(Math.random() * 4294967296));

  // ---- MathUtils ---------------------------------------------------------
  var MU = R.MathsMathUtils;
  var seeded = R.Rng.new_(1234567);
  var KINDS = { Float32Array: 0, Uint32Array: 1, Uint16Array: 2, Uint8Array: 3, Int32Array: 4, Int16Array: 5, Int8Array: 6 };
  function kindOf(array) {
    var n = array && array.constructor && array.constructor.name;
    if (!(n in KINDS)) throw new Error("Invalid component type.");
    return KINDS[n];
  }
  T.MathUtils = {
    DEG2RAD: MU.DEG2RAD(),
    RAD2DEG: MU.RAD2DEG(),
    generateUUID: function () { return rng.uuid(); },
    clamp: MU.clamp,
    euclideanModulo: MU.euclidean_modulo,
    mapLinear: MU.map_linear,
    inverseLerp: MU.inverse_lerp,
    lerp: MU.lerp,
    damp: MU.damp,
    pingpong: function (x, length) { return MU.pingpong(x, length === undefined ? 1 : length); },
    smoothstep: MU.smoothstep,
    smootherstep: MU.smootherstep,
    randInt: function (low, high) { return rng.rand_int(low, high); },
    randFloat: function (low, high) { return rng.rand_float(low, high); },
    randFloatSpread: function (range) { return rng.rand_float_spread(range); },
    seededRandom: function (s) {
      if (s !== undefined) seeded = R.Rng.new_(s);
      return seeded.next();
    },
    degToRad: MU.deg_to_rad,
    radToDeg: MU.rad_to_deg,
    isPowerOfTwo: function (v) { return MU.is_power_of_two(v); },
    ceilPowerOfTwo: MU.ceil_power_of_two,
    floorPowerOfTwo: MU.floor_power_of_two,
    normalize: function (v, array) { return MU.normalize(v, kindOf(array)); },
    denormalize: function (v, array) { return MU.denormalize(v, kindOf(array)); },
    setQuaternionFromProperEuler: function (q, a, b, c, order) {
      var cos = Math.cos, sin = Math.sin;
      var c2 = cos(b / 2), s2 = sin(b / 2);
      var c13 = cos((a + c) / 2), s13 = sin((a + c) / 2);
      var c1_3 = cos((a - c) / 2), s1_3 = sin((a - c) / 2);
      var c3_1 = cos((c - a) / 2), s3_1 = sin((c - a) / 2);
      switch (order) {
        case "XYX": q.set(c2 * s13, s2 * c1_3, s2 * s1_3, c2 * c13); break;
        case "YZY": q.set(s2 * s1_3, c2 * s13, s2 * c1_3, c2 * c13); break;
        case "ZXZ": q.set(s2 * c1_3, s2 * s1_3, c2 * s13, c2 * c13); break;
        case "XZX": q.set(c2 * s13, s2 * s3_1, s2 * c3_1, c2 * c13); break;
        case "YXY": q.set(s2 * c3_1, c2 * s13, s2 * s3_1, c2 * c13); break;
        case "ZYZ": q.set(s2 * s3_1, s2 * c3_1, c2 * s13, c2 * c13); break;
      }
    }
  };

  // ---- Vector2 / Vector3 / Vector4 --------------------------------------
  T.Vector2 = make(R.Vector2, function (x, y) { this.x = x || 0; this.y = y || 0; });
  flag(T.Vector2, "isVector2");
  accessor(T.Vector2.prototype, "width", function () { return this.x; }, function (v) { this.x = v; });
  accessor(T.Vector2.prototype, "height", function () { return this.y; }, function (v) { this.y = v; });
  T.Vector3 = make(R.Vector3, function (x, y, z) { this.x = x || 0; this.y = y || 0; this.z = z || 0; });
  flag(T.Vector3, "isVector3");
  T.Vector4 = make(R.Vector4, function (x, y, z, w) { this.x = x || 0; this.y = y || 0; this.z = z || 0; this.w = w === undefined ? 1 : w; });
  flag(T.Vector4, "isVector4");
  accessor(T.Vector4.prototype, "width", function () { return this.z; }, function (v) { this.z = v; });
  accessor(T.Vector4.prototype, "height", function () { return this.w; }, function (v) { this.w = v; });
  [[T.Vector2, ["x", "y"]], [T.Vector3, ["x", "y", "z"]], [T.Vector4, ["x", "y", "z", "w"]]].forEach(function (e) {
    var C = e[0], keys = e[1];
    fromArrayDefault(C);
    toArrayInto(C);
    iterate(C, keys);
    var rnd = C.prototype.random;
    if (rnd) def(C.prototype, "random", function () { return rnd.call(this, rng); });
    def(C.prototype, "fromBufferAttribute", function (attr, index) {
      for (var k = 0; k < keys.length; k++) this[keys[k]] = attr.getComponent(index, k);
      return this;
    });
  });
  (function () {
    var p = T.Vector3.prototype;
    var rd = p.random_direction;
    def(p, "randomDirection", function () { return rd.call(this, rng); });
    var ae = p.apply_euler;
    def(p, "applyEuler", function (e) { return ae.call(this, eulerCore(e)); });
    def(p, "project", function (camera) {
      return this.applyMatrix4(camera.matrixWorldInverse).applyMatrix4(camera.projectionMatrix);
    });
    def(p, "unproject", function (camera) {
      return this.applyMatrix4(camera.projectionMatrixInverse).applyMatrix4(camera.matrixWorld);
    });
  })();

  // ---- Matrix3 / Matrix4 -------------------------------------------------
  T.Matrix3 = make(R.Matrix3, function () {
    this.elements = [1, 0, 0, 0, 1, 0, 0, 0, 1];
    if (arguments.length) this.set.apply(this, arguments);
  });
  flag(T.Matrix3, "isMatrix3");
  T.Matrix4 = make(R.Matrix4, function () {
    this.elements = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    if (arguments.length) this.set.apply(this, arguments);
  });
  flag(T.Matrix4, "isMatrix4");
  [T.Matrix3, T.Matrix4].forEach(function (C) {
    fromArrayDefault(C);
    toArrayInto(C);
    def(C.prototype, "identity", C.prototype.make_identity);
    var eb = C.prototype.extract_basis;
    def(C.prototype, "extractBasis", function (x, y, z) {
      var b = eb.call(this);
      x.copy(b[0]); y.copy(b[1]); z.copy(b[2]);
      return this;
    });
  });
  (function () {
    var p3 = T.Matrix3.prototype;
    var tia = p3.transpose_into_array;
    def(p3, "transposeIntoArray", function (r) {
      var a = tia.call(this);
      for (var i = 0; i < 9; i++) r[i] = a[i];
      return this;
    });
    var p = T.Matrix4.prototype;
    var mre = p.make_rotation_from_euler;
    def(p, "makeRotationFromEuler", function (e) { return mre.call(this, eulerCore(e)); });
    var dec = p.decompose;
    def(p, "decompose", function (position, quaternion, scale) {
      var t = dec.call(this);
      if (position === undefined) return t; // the crate's own call
      position.copy(t.position); quaternion.copy(t.quaternion); scale.copy(t.scale);
      return this;
    });
    var sp = p.set_position;
    def(p, "setPosition", function (x, y, z) {
      return x && x.isVector3 ? sp.call(this, x.x, x.y, x.z) : sp.call(this, x, y, z);
    });
    var mp = p.make_perspective;
    def(p, "makePerspective", function (l, r, t, b, n, f) { return mp.call(this, l, r, t, b, n, f); });
  })();

  // ---- Quaternion --------------------------------------------------------
  T.Quaternion = make(R.Quaternion, function (x, y, z, w) {
    this.x = x || 0; this.y = y || 0; this.z = z || 0; this.w = w === undefined ? 1 : w;
  });
  flag(T.Quaternion, "isQuaternion");
  fromArrayDefault(T.Quaternion);
  toArrayInto(T.Quaternion);
  iterate(T.Quaternion, ["x", "y", "z", "w"]);
  (function () {
    var p = T.Quaternion.prototype;
    def(p, "identity", p.make_identity);
    var sfe = p.set_from_euler;
    def(p, "setFromEuler", function (e) { return sfe.call(this, eulerCore(e)); });
    var rnd = p.random;
    def(p, "random", function () { return rnd.call(this, rng); });
    def(p, "toJSON", function () { return this.to_array(); });
    def(p, "fromBufferAttribute", function (attr, index) {
      this.x = attr.getX(index); this.y = attr.getY(index); this.z = attr.getZ(index); this.w = attr.getW(index);
      return this;
    });
    def(p, "_onChange", function (cb) { this._onChangeCallback = cb; return this; });
    T.Quaternion.slerpFlat = function (dst, dstOffset, src0, srcOffset0, src1, srcOffset1, t) {
      var box = { v: Array.prototype.slice.call(dst) };
      R.Quaternion.slerp_flat(box, dstOffset, Array.prototype.slice.call(src0), srcOffset0, Array.prototype.slice.call(src1), srcOffset1, t);
      var out = box.v || box;
      for (var i = 0; i < 4; i++) dst[dstOffset + i] = out[dstOffset + i];
    };
  })();

  // ---- Euler -------------------------------------------------------------
  // The crate's Euler holds its order as an enum; three.js's as a string
  // that changes the rotation's quaternion when set (Object3D listens), so
  // the API's Euler is this small JS class, turned into the crate's for math.
  var ORDERS = ["XYZ", "YXZ", "ZXY", "ZYX", "YZX", "XZY"];
  function orderIndex(o) { var i = ORDERS.indexOf(o); return i < 0 ? 0 : i; }
  function eulerCore(e) {
    if (e instanceof R.Euler) return e;
    return R.Euler.new_(e.x, e.y, e.z, orderIndex(e.order));
  }
  function Euler(x, y, z, order) {
    this._x = x || 0; this._y = y || 0; this._z = z || 0;
    this._order = order || Euler.DEFAULT_ORDER;
    this._onChangeCallback = function () {};
  }
  Euler.DEFAULT_ORDER = "XYZ";
  ["x", "y", "z", "order"].forEach(function (k) {
    accessor(Euler.prototype, k, function () { return this["_" + k]; }, function (v) { this["_" + k] = v; this._onChangeCallback(); });
  });
  flag(Euler, "isEuler");
  function eulerFrom(self, c, update) {
    self._x = c.x; self._y = c.y; self._z = c.z; self._order = ORDERS[c.order];
    if (update !== false) self._onChangeCallback();
    return self;
  }
  Euler.prototype.set = function (x, y, z, order) {
    this._x = x; this._y = y; this._z = z; this._order = order || this._order;
    this._onChangeCallback();
    return this;
  };
  Euler.prototype.clone = function () { return new Euler(this._x, this._y, this._z, this._order); };
  Euler.prototype.copy = function (e) {
    this._x = e.x; this._y = e.y; this._z = e.z; this._order = typeof e.order === "number" ? ORDERS[e.order] : e.order;
    this._onChangeCallback();
    return this;
  };
  Euler.prototype.setFromRotationMatrix = function (m, order, update) {
    var c = R.Euler.new_(0, 0, 0, 0);
    c.set_from_rotation_matrix(m, orderIndex(order || this._order));
    return eulerFrom(this, c, update);
  };
  Euler.prototype.setFromQuaternion = function (q, order, update) {
    var c = R.Euler.new_(0, 0, 0, 0);
    c.set_from_quaternion(q, orderIndex(order || this._order));
    return eulerFrom(this, c, update);
  };
  Euler.prototype.setFromVector3 = function (v, order) { return this.set(v.x, v.y, v.z, order || this._order); };
  Euler.prototype.reorder = function (newOrder) {
    var c = eulerCore(this);
    c.reorder(orderIndex(newOrder));
    return eulerFrom(this, c);
  };
  Euler.prototype.equals = function (e) {
    return e.x === this._x && e.y === this._y && e.z === this._z && e.order === this._order;
  };
  Euler.prototype.fromArray = function (a) {
    this._x = a[0]; this._y = a[1]; this._z = a[2];
    if (a[3] !== undefined) this._order = a[3];
    this._onChangeCallback();
    return this;
  };
  Euler.prototype.toArray = function (array, offset) {
    array = array || []; offset = offset || 0;
    array[offset] = this._x; array[offset + 1] = this._y; array[offset + 2] = this._z; array[offset + 3] = this._order;
    return array;
  };
  Euler.prototype._onChange = function (cb) { this._onChangeCallback = cb; return this; };
  Euler.prototype[Symbol.iterator] = function* () { yield this._x; yield this._y; yield this._z; yield this._order; };
  T.Euler = Euler;

  // ---- Color -------------------------------------------------------------
  var LINEAR = 0, SRGB = 1;
  T.SRGBColorSpace = "srgb";
  T.LinearSRGBColorSpace = "srgb-linear";
  T.NoColorSpace = "";
  T.SRGBTransfer = "srgb";
  T.LinearTransfer = "linear";
  T.Rec709Primaries = "rec709";
  // THREE.ColorManagement: Linear-sRGB is the working space, sRGB the
  // display space (the crate converts on input and output)
  (function () {
    function toLin(c) { return R.MathsColor.srgb_to_linear(c); }
    function toSrgb(c) { return R.MathsColor.linear_to_srgb(c); }
    var spaces = {
      "srgb-linear": { transfer: "linear", primaries: "rec709", luminanceCoefficients: [0.2126, 0.7152, 0.0722], outputColorSpaceConfig: { drawingBufferColorSpace: "srgb" } },
      "srgb": { transfer: "srgb", primaries: "rec709", luminanceCoefficients: [0.2126, 0.7152, 0.0722], outputColorSpaceConfig: { drawingBufferColorSpace: "srgb" } }
    };
    var CM = {
      enabled: true,
      workingColorSpace: "srgb-linear",
      spaces: spaces,
      convert: function (color, source, target) {
        if (this.enabled === false || source === target || !source || !target) return color;
        if (source === "srgb" && target === "srgb-linear") { color.r = toLin(color.r); color.g = toLin(color.g); color.b = toLin(color.b); }
        else if (source === "srgb-linear" && target === "srgb") { color.r = toSrgb(color.r); color.g = toSrgb(color.g); color.b = toSrgb(color.b); }
        return color;
      },
      workingToColorSpace: function (color, target) { return this.convert(color, this.workingColorSpace, target); },
      colorSpaceToWorking: function (color, source) { return this.convert(color, source, this.workingColorSpace); },
      fromWorkingColorSpace: function (color, target) { return this.convert(color, this.workingColorSpace, target); },
      toWorkingColorSpace: function (color, source) { return this.convert(color, source, this.workingColorSpace); },
      getPrimaries: function (cs) { return cs === "" ? "" : (spaces[cs] || spaces["srgb-linear"]).primaries; },
      getTransfer: function (cs) { return cs === "" ? "linear" : (spaces[cs] || spaces["srgb-linear"]).transfer; },
      getToneMappingMode: function () { return "standard"; },
      getLuminanceCoefficients: function (target, cs) { var k = (spaces[cs || this.workingColorSpace] || spaces["srgb-linear"]).luminanceCoefficients; return target.fromArray(k); },
      define: function (defs) { for (var k in defs) spaces[k] = defs[k]; },
      _getMatrix: function (target) { return target.identity(); },
      _getDrawingBufferColorSpace: function () { return "srgb"; },
      _getUnpackColorSpace: function () { return this.workingColorSpace; }
    };
    T.ColorManagement = CM;
  })();
  function space(cs, dflt) {
    if (cs === undefined) return dflt;
    return cs === T.SRGBColorSpace ? SRGB : LINEAR;
  }
  T.Color = make(R.Color, function (r, g, b) {
    this.r = 1; this.g = 1; this.b = 1;
    this.set(r, g, b);
  });
  flag(T.Color, "isColor");
  fromArrayDefault(T.Color);
  toArrayInto(T.Color);
  iterate(T.Color, ["r", "g", "b"]);
  (function () {
    var p = T.Color.prototype;
    var sh = p.set_hex, srgb = p.set_rgb, shsl = p.set_hsl, sst = p.set_style, scn = p.set_color_name;
    var gh = p.get_hex, ghs = p.get_hex_string, ghsl = p.get_hsl, grgb = p.get_rgb;
    def(p, "set", function (r, g, b) {
      if (g === undefined && b === undefined) {
        var v = r;
        if (v && v.isColor) this.copy(v);
        else if (typeof v === "number") sh.call(this, v, SRGB);
        else if (typeof v === "string") this.setStyle(v);
      } else {
        srgb.call(this, r, g, b, LINEAR);
      }
      return this;
    });
    def(p, "setHex", function (hex, cs) { return sh.call(this, Math.floor(hex), space(cs, SRGB)); });
    def(p, "setRGB", function (r, g, b, cs) { return srgb.call(this, r, g, b, space(cs, LINEAR)); });
    def(p, "setHSL", function (h, s, l, cs) { return shsl.call(this, h, s, l, space(cs, LINEAR)); });
    def(p, "setStyle", function (style, cs) {
      if (!sst.call(this, style, space(cs, SRGB))) console.warn("THREE.Color: Unknown color " + style);
      return this;
    });
    def(p, "setColorName", function (style, cs) {
      if (!scn.call(this, style, space(cs, SRGB))) console.warn("THREE.Color: Unknown color " + style);
      return this;
    });
    def(p, "getHex", function (cs) { return gh.call(this, space(cs, SRGB)); });
    def(p, "getHexString", function (cs) { return ghs.call(this, space(cs, SRGB)); });
    def(p, "getHSL", function (target, cs) {
      var h = ghsl.call(this, space(cs, LINEAR));
      target.h = h.h; target.s = h.s; target.l = h.l;
      return target;
    });
    def(p, "getRGB", function (target, cs) { return target.copy(grgb.call(this, space(cs, LINEAR))); });
    def(p, "getStyle", function () { return this.get_style(); });
    def(p, "copySRGBToLinear", p.copy_srgb_to_linear);
    def(p, "copyLinearToSRGB", p.copy_linear_to_srgb);
    def(p, "convertSRGBToLinear", p.convert_srgb_to_linear);
    def(p, "convertLinearToSRGB", p.convert_linear_to_srgb);
    def(p, "offsetHSL", p.offset_hsl);
    def(p, "lerpHSL", p.lerp_hsl);
    def(p, "toJSON", function () { return this.getHex(); });
    def(p, "fromBufferAttribute", function (attr, index) {
      this.r = attr.getX(index); this.g = attr.getY(index); this.b = attr.getZ(index);
      return this;
    });
  })();
  T.Color.NAMES = (function () {
    var out = {};
    R.MathsColor.CSS_NAMES().split(" ").forEach(function (e) {
      var kv = e.split(":");
      out[kv[0]] = parseInt(kv[1], 16);
    });
    return out;
  })();

  // ---- Spherical / Cylindrical ------------------------------------------
  T.Spherical = make(R.Spherical, function (radius, phi, theta) {
    this.radius = radius === undefined ? 1 : radius; this.phi = phi || 0; this.theta = theta || 0;
  });
    T.Cylindrical = make(R.Cylindrical, function (radius, theta, y) {
    this.radius = radius === undefined ? 1 : radius; this.theta = theta || 0; this.y = y || 0;
  });
  
  // ---- bounds ------------------------------------------------------------
  T.Box2 = make(R.Box2, function (min, max) {
    this.min = min || new T.Vector2(Infinity, Infinity);
    this.max = max || new T.Vector2(-Infinity, -Infinity);
  });
  flag(T.Box2, "isBox2");
  intoTarget(T.Box2, "getCenter", "get_center", 0);
  intoTarget(T.Box2, "getSize", "get_size", 0);
  intoTarget(T.Box2, "getParameter", "get_parameter", 1);
  intoTarget(T.Box2, "clampPoint", "clamp_point", 1);

  T.Box3 = make(R.Box3, function (min, max) {
    this.min = min || new T.Vector3(Infinity, Infinity, Infinity);
    this.max = max || new T.Vector3(-Infinity, -Infinity, -Infinity);
  });
  flag(T.Box3, "isBox3");
  intoTarget(T.Box3, "getCenter", "get_center", 0);
  intoTarget(T.Box3, "getSize", "get_size", 0);
  intoTarget(T.Box3, "getParameter", "get_parameter", 1);
  intoTarget(T.Box3, "clampPoint", "clamp_point", 1);
  intoTarget(T.Box3, "getBoundingSphere", "get_bounding_sphere", 0);
  (function () {
    var p = T.Box3.prototype;
    var sfa = p.set_from_array;
    def(p, "setFromArray", function (a) { return sfa.call(this, Array.prototype.slice.call(a)); });
    def(p, "setFromBufferAttribute", function (attr) {
      this.makeEmpty();
      var v = new T.Vector3();
      for (var i = 0; i < attr.count; i++) this.expandByPoint(v.fromBufferAttribute(attr, i));
      return this;
    });
    def(p, "toJSON", function () { return { min: this.min.toArray(), max: this.max.toArray() }; });
    def(p, "fromJSON", function (j) { this.min.fromArray(j.min); this.max.fromArray(j.max); return this; });
  })();

  T.Sphere = make(R.Sphere, function (center, radius) {
    this.center = center || new T.Vector3();
    this.radius = radius === undefined ? -1 : radius;
  });
  flag(T.Sphere, "isSphere");
  intoTarget(T.Sphere, "clampPoint", "clamp_point", 1);
  intoTarget(T.Sphere, "getBoundingBox", "get_bounding_box", 0);
  (function () {
    var p = T.Sphere.prototype;
    var sfp = p.set_from_points, sfpa = p.set_from_points_around;
    def(p, "setFromPoints", function (points, center) {
      return center ? sfpa.call(this, points, center) : sfp.call(this, points);
    });
    def(p, "toJSON", function () { return { radius: this.radius, center: this.center.toArray() }; });
    def(p, "fromJSON", function (j) { this.radius = j.radius; this.center.fromArray(j.center); return this; });
  })();

  T.Plane = make(R.Plane, function (normal, constant) {
    this.normal = normal || new T.Vector3(1, 0, 0);
    this.constant = constant || 0;
  });
  flag(T.Plane, "isPlane");
  intoTarget(T.Plane, "projectPoint", "project_point", 1);
  intoTarget(T.Plane, "intersectLine", "intersect_line", 1);
  intoTarget(T.Plane, "coplanarPoint", "coplanar_point", 0);
  (function () {
    var p = T.Plane.prototype;
    var am = p.apply_matrix4;
    def(p, "applyMatrix4", function (m) { return am.call(this, m); });
  })();

  T.Ray = make(R.Ray, function (origin, direction) {
    this.origin = origin || new T.Vector3();
    this.direction = direction || new T.Vector3(0, 0, -1);
  });
    intoTarget(T.Ray, "at", "at", 1);
  intoTarget(T.Ray, "closestPointToPoint", "closest_point_to_point", 1);
  intoTarget(T.Ray, "intersectSphere", "intersect_sphere", 1);
  intoTarget(T.Ray, "intersectPlane", "intersect_plane", 1);
  intoTarget(T.Ray, "intersectBox", "intersect_box", 1);
  intoTarget(T.Ray, "intersectTriangle", "intersect_triangle", 4);
  (function () {
    var p = T.Ray.prototype;
    var dtp = p.distance_to_plane;
    def(p, "distanceToPlane", function (plane) { var d = dtp.call(this, plane); return d === undefined ? null : d; });
  })();

  T.Line3 = make(R.Line3, function (start, end) {
    this.start = start || new T.Vector3();
    this.end = end || new T.Vector3();
  });
    intoTarget(T.Line3, "getCenter", "get_center", 0);
  intoTarget(T.Line3, "delta", "delta", 0);
  intoTarget(T.Line3, "at", "at", 1);
  intoTarget(T.Line3, "closestPointToPoint", "closest_point_to_point", 2);

  T.Triangle = make(R.Triangle, function (a, b, c) {
    this.a = a || new T.Vector3();
    this.b = b || new T.Vector3();
    this.c = c || new T.Vector3();
  });
    intoTarget(T.Triangle, "getMidpoint", "get_midpoint", 0);
  intoTarget(T.Triangle, "getNormal", "get_normal", 0);
  intoTarget(T.Triangle, "getPlane", "get_plane", 0);
  intoTarget(T.Triangle, "getBarycoord", "get_barycoord", 1);
  intoTarget(T.Triangle, "getInterpolation", "get_interpolation", 4);
  intoTarget(T.Triangle, "closestPointToPoint", "closest_point_to_point", 1);
  (function () {
    var p = T.Triangle.prototype;
    var gi = p.get_interpolation;
    def(p, "getInterpolation", function (point, p1, p2, p3, v1, v2, v3, target) {
      var t = R.Triangle.new_(p1, p2, p3);
      var v = gi.call(t, point, v1, v2, v3);
      if (v === undefined) return null;
      return target.copy(v);
    });
    def(p, "setFromAttributeAndIndices", function (attr, i0, i1, i2) {
      this.a.fromBufferAttribute(attr, i0); this.b.fromBufferAttribute(attr, i1); this.c.fromBufferAttribute(attr, i2);
      return this;
    });
    T.Triangle.getNormal = function (a, b, c, target) { return target.copy(R.Triangle.normal_of(a, b, c)); };
    T.Triangle.getBarycoord = function (p, a, b, c, target) {
      var v = R.Triangle.barycoord_of(p, a, b, c);
      return v === undefined ? null : target.copy(v);
    };
    T.Triangle.containsPoint = function (p, a, b, c) { return R.Triangle.contains_point_of(p, a, b, c); };
  })();

  T.Frustum = make(R.Frustum, function (p0, p1, p2, p3, p4, p5) {
    this.planes = [p0 || new T.Plane(), p1 || new T.Plane(), p2 || new T.Plane(), p3 || new T.Plane(), p4 || new T.Plane(), p5 || new T.Plane()];
  });
    (function () {
    var p = T.Frustum.prototype;
    var st = p.set;
    def(p, "set", function (a, b, c, d, e, f) { return st.call(this, Array.isArray(a) ? a : [a, b, c, d, e, f]); });
  })();

  // the helpers js/three-scene.js builds on
  def(T, "__facade", {
    camel: camel, def: def, accessor: accessor, aliases: aliases, make: make, flag: flag,
    intoTarget: intoTarget, eulerCore: eulerCore, orderIndex: orderIndex, ORDERS: ORDERS,
    rng: rng, kindOf: kindOf, KINDS: KINDS, space: space
  });
  return T;
})(RangerThreeCore);
if (typeof window !== "undefined") window.THREE = THREE;
