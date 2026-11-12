// SPDX-License-Identifier: MIT
//
// three.js's curves (Curve and its kinds, CurvePath, Path, Shape,
// ShapeUtils) over the crate's `Curve` (three/src/extras). A facade curve
// keeps three.js's fields (`v1`, `points`, `curves`...) as the truth, so
// code that edits them works as in three.js; each call hands the crate
// curve the current fields. The arc-length caches live on the crate curve
// (`cacheArcLengths` and `needsUpdate` read and write it), so they last
// between calls exactly as three.js's do.
//
// A user subclass that overrides `getPoint` (TubeGeometry's usual input)
// has no crate kind; the base methods then run the same algorithms here
// over its `getPoint`.
(function (T, R) {
  "use strict";
  var F = T.__facade;
  var def = F.def;
  var RC = R.Curve;
  var CATMULL = { centripetal: 0, chordal: 1, catmullrom: 2 };

  function v3(v) { return R.Vector3.new_(v.x, v.y, v.z || 0); }
  function out(is2d, p, target) {
    if (target) { if (is2d) target.set(p.x, p.y); else target.set(p.x, p.y, p.z); return target; }
    return is2d ? new T.Vector2(p.x, p.y) : new T.Vector3(p.x, p.y, p.z);
  }
  function outs(is2d, list) { return list.map(function (p) { return out(is2d, p); }); }
  function struct(C, fields) { var o = new C(); for (var k in fields) o[k] = fields[k]; return o; }

  // ---- Curve -------------------------------------------------------------
  function Curve() {
    this.type = "Curve";
    def(this, "__core", RC.path(false));
    this.__core.arc_length_divisions = 200;
  }
  T.Curve = Curve;
  var CP = Curve.prototype;
  Object.defineProperty(CP, "arcLengthDivisions", {
    get: function () { return this.__core.arc_length_divisions; },
    set: function (v) { this.__core.arc_length_divisions = v; }, configurable: true
  });
  Object.defineProperty(CP, "needsUpdate", {
    get: function () { return this.__core.needs_update; },
    set: function (v) { this.__core.needs_update = !!v; }, configurable: true
  });
  Object.defineProperty(CP, "cacheArcLengths", {
    get: function () { var a = this.__core.cache_arc_lengths; return a.length ? a : null; },
    set: function (v) { this.__core.cache_arc_lengths = v || []; }, configurable: true
  });
  // the crate curve with the current fields; null for a user subclass
  CP._core = function () {
    if (!this._kind) return null;
    var c = this.__core;
    c.kind = this._kind();
    c.is_2d = !!this._is2d;
    return c;
  };
  CP._custom = function () { return !this._kind; };
  CP.getPoint = function () { console.warn("THREE.Curve: .getPoint() not implemented."); return null; };
  CP.getPointAt = function (u, target) {
    if (this._custom()) return this.getPoint(this.getUtoTmapping(u), target);
    return out(this._is2d, this._core().get_point_at(u), target);
  };
  CP.getPoints = function (divisions) {
    if (divisions === undefined) divisions = 5;
    if (this._custom()) { var pts = []; for (var d = 0; d <= divisions; d++) pts.push(this.getPoint(d / divisions)); return pts; }
    return outs(this._is2d, this._core().get_points(divisions));
  };
  CP.getSpacedPoints = function (divisions) {
    if (divisions === undefined) divisions = 5;
    if (this._custom()) { var pts = []; for (var d = 0; d <= divisions; d++) pts.push(this.getPointAt(d / divisions)); return pts; }
    return outs(this._is2d, this._core().get_spaced_points(divisions));
  };
  CP.getLength = function () {
    if (this._custom()) { var l = this.getLengths(); return l[l.length - 1]; }
    return this._core().get_length();
  };
  CP.getLengths = function (divisions) {
    if (divisions === undefined) divisions = this.arcLengthDivisions;
    if (!this._custom()) return this._core().get_lengths(divisions);
    var c = this.__core;
    if (c.cache_arc_lengths.length === divisions + 1 && !c.needs_update) return c.cache_arc_lengths;
    c.needs_update = false;
    var cache = [0], last = this.getPoint(0), sum = 0;
    for (var p = 1; p <= divisions; p++) {
      var cur = this.getPoint(p / divisions);
      sum += cur.distanceTo(last);
      cache.push(sum);
      last = cur;
    }
    c.cache_arc_lengths = cache;
    return cache;
  };
  CP.updateArcLengths = function () {
    if (this._custom()) { this.needsUpdate = true; this.getLengths(); return; }
    this._core().update_arc_lengths();
  };
  CP.getUtoTmapping = function (u, distance) {
    if (!this._custom()) return this._core().get_u_to_t_mapping(u, distance || 0);
    var arc = this.getLengths(), il = arc.length;
    var target = distance ? distance : u * arc[il - 1];
    var low = 0, high = il - 1, i, cmp;
    while (low <= high) {
      i = Math.floor(low + (high - low) / 2);
      cmp = arc[i] - target;
      if (cmp < 0) low = i + 1; else if (cmp > 0) high = i - 1; else { high = i; break; }
    }
    i = high;
    if (arc[i] === target) return i / (il - 1);
    return (i + (target - arc[i]) / (arc[i + 1] - arc[i])) / (il - 1);
  };
  CP.getTangent = function (t, target) {
    if (!this._custom()) return out(this._is2d, this._core().get_tangent(t), target);
    var t1 = Math.max(0, t - 0.0001), t2 = Math.min(1, t + 0.0001);
    var p1 = this.getPoint(t1), p2 = this.getPoint(t2);
    var tan = target || (p1.isVector2 ? new T.Vector2() : new T.Vector3());
    return tan.copy(p2).sub(p1).normalize();
  };
  CP.getTangentAt = function (u, target) {
    if (!this._custom()) return out(this._is2d, this._core().get_tangent_at(u), target);
    return this.getTangent(this.getUtoTmapping(u), target);
  };
  CP.computeFrenetFrames = function (segments, closed) {
    var f;
    if (!this._custom()) f = this._core().compute_frenet_frames(segments, closed === true);
    else {
      // a crate curve through this curve's tangents does the transport
      var tangents = [];
      for (var i = 0; i <= segments; i++) tangents.push(v3(this.getTangentAt(i / segments)));
      f = R.ExtrasCurves.frames_from_tangents(tangents, segments, closed === true);
    }
    return { tangents: outs(false, f.tangents), normals: outs(false, f.normals), binormals: outs(false, f.binormals) };
  };
  CP.clone = function () { return new this.constructor().copy(this); };
  CP.copy = function (source) { this.arcLengthDivisions = source.arcLengthDivisions; return this; };
  CP.toJSON = function () {
    return { metadata: { version: 4.7, type: "Curve", generator: "Curve.toJSON" }, arcLengthDivisions: this.arcLengthDivisions, type: this.type };
  };
  CP.fromJSON = function (json) { this.arcLengthDivisions = json.arcLengthDivisions; return this; };

  function subclass(name, Base, ctor, proto) {
    var K = function () {
      Base.call(this);
      this.type = name;
      this["is" + name] = true;
      ctor.apply(this, arguments);
    };
    Object.defineProperty(K, "name", { value: name });
    K.prototype = Object.create(Base.prototype);
    K.prototype.constructor = K;
    for (var k in proto) K.prototype[k] = proto[k];
    T[name] = K;
    return K;
  }
  function vecFields(names, is2d) {
    return {
      copy: function (s) {
        Curve.prototype.copy.call(this, s);
        for (var i = 0; i < names.length; i++) this[names[i]].copy(s[names[i]]);
        return this;
      },
      toJSON: function () {
        var d = Curve.prototype.toJSON.call(this);
        for (var i = 0; i < names.length; i++) d[names[i]] = this[names[i]].toArray();
        return d;
      },
      fromJSON: function (j) {
        Curve.prototype.fromJSON.call(this, j);
        for (var i = 0; i < names.length; i++) this[names[i]].fromArray(j[names[i]]);
        return this;
      }
    };
  }
  function mix(a, b) { for (var k in b) a[k] = b[k]; return a; }
  function V2(v) { return v || new T.Vector2(); }
  function V3(v) { return v || new T.Vector3(); }

  function line(name, is2d) {
    var V = is2d ? V2 : V3;
    subclass(name, Curve, function (v1, v2) { this.v1 = V(v1); this.v2 = V(v2); }, mix(vecFields(["v1", "v2"]), {
      _is2d: is2d,
      _kind: function () { return new R.CurveKind_Line(struct(R.LineData, { v1: v3(this.v1), v2: v3(this.v2) })); },
      getPoint: function (t, target) { return out(is2d, this._core().get_point(t), target); }
    }));
  }
  line("LineCurve", true);
  line("LineCurve3", false);

  function bezier(name, is2d, cubic) {
    var V = is2d ? V2 : V3;
    var names = cubic ? ["v0", "v1", "v2", "v3"] : ["v0", "v1", "v2"];
    subclass(name, Curve, function (a, b, c, d) {
      this.v0 = V(a); this.v1 = V(b); this.v2 = V(c); if (cubic) this.v3 = V(d);
    }, mix(vecFields(names), {
      _is2d: is2d,
      _kind: function () {
        var b = struct(R.BezierData, { v0: v3(this.v0), v1: v3(this.v1), v2: v3(this.v2), v3: cubic ? v3(this.v3) : R.Vector3.new_(0, 0, 0) });
        return cubic ? new R.CurveKind_CubicBezier(b) : new R.CurveKind_QuadraticBezier(b);
      },
      getPoint: function (t, target) { return out(is2d, this._core().get_point(t), target); }
    }));
  }
  bezier("QuadraticBezierCurve", true, false);
  bezier("QuadraticBezierCurve3", false, false);
  bezier("CubicBezierCurve", true, true);
  bezier("CubicBezierCurve3", false, true);

  var ELLIPSE = ["aX", "aY", "xRadius", "yRadius", "aStartAngle", "aEndAngle", "aClockwise", "aRotation"];
  var ellipseProto = {
    _is2d: true,
    _kind: function () {
      return new R.CurveKind_Ellipse(struct(R.EllipseData, {
        x: this.aX, y: this.aY, x_radius: this.xRadius, y_radius: this.yRadius,
        start_angle: this.aStartAngle, end_angle: this.aEndAngle, clockwise: this.aClockwise === true, rotation: this.aRotation
      }));
    },
    getPoint: function (t, target) { return out(true, this._core().get_point(t), target); },
    copy: function (s) { Curve.prototype.copy.call(this, s); for (var i = 0; i < ELLIPSE.length; i++) this[ELLIPSE[i]] = s[ELLIPSE[i]]; return this; },
    toJSON: function () { var d = Curve.prototype.toJSON.call(this); for (var i = 0; i < ELLIPSE.length; i++) d[ELLIPSE[i]] = this[ELLIPSE[i]]; return d; },
    fromJSON: function (j) { Curve.prototype.fromJSON.call(this, j); for (var i = 0; i < ELLIPSE.length; i++) this[ELLIPSE[i]] = j[ELLIPSE[i]]; return this; }
  };
  function ellipseInit(aX, aY, xRadius, yRadius, aStartAngle, aEndAngle, aClockwise, aRotation) {
    this.aX = aX || 0; this.aY = aY || 0;
    this.xRadius = xRadius === undefined ? 1 : xRadius; this.yRadius = yRadius === undefined ? 1 : yRadius;
    this.aStartAngle = aStartAngle || 0; this.aEndAngle = aEndAngle === undefined ? Math.PI * 2 : aEndAngle;
    this.aClockwise = aClockwise || false; this.aRotation = aRotation || 0;
  }
  var EllipseCurve = subclass("EllipseCurve", Curve, ellipseInit, ellipseProto);
  subclass("ArcCurve", EllipseCurve, function (aX, aY, aRadius, aStartAngle, aEndAngle, aClockwise) {
    ellipseInit.call(this, aX, aY, aRadius, aRadius, aStartAngle, aEndAngle, aClockwise);
  }, {});
  T.ArcCurve.prototype.isEllipseCurve = true;

  function pointsFields(is2d) {
    return {
      copy: function (s) { Curve.prototype.copy.call(this, s); this.points = s.points.map(function (p) { return p.clone(); }); return this; },
      toJSON: function () { var d = Curve.prototype.toJSON.call(this); d.points = this.points.map(function (p) { return p.toArray(); }); return d; },
      fromJSON: function (j) {
        Curve.prototype.fromJSON.call(this, j);
        this.points = j.points.map(function (a) { return (is2d ? new T.Vector2() : new T.Vector3()).fromArray(a); });
        return this;
      }
    };
  }
  subclass("SplineCurve", Curve, function (points) { this.points = points || []; }, mix(pointsFields(true), {
    _is2d: true,
    _kind: function () { return new R.CurveKind_Spline(this.points.map(v3)); },
    getPoint: function (t, target) { return out(true, this._core().get_point(t), target); }
  }));

  var crFields = pointsFields(false);
  subclass("CatmullRomCurve3", Curve, function (points, closed, curveType, tension) {
    this.points = points || [];
    this.closed = closed || false;
    this.curveType = curveType || "centripetal";
    this.tension = tension === undefined ? 0.5 : tension;
  }, {
    _is2d: false,
    _kind: function () {
      var t = CATMULL[this.curveType];
      return new R.CurveKind_CatmullRom(struct(R.CatmullRomData, {
        points: this.points.map(v3), closed: this.closed === true, curve_type: t === undefined ? 0 : t, tension: this.tension
      }));
    },
    getPoint: function (t, target) { return out(false, this._core().get_point(t), target); },
    copy: function (s) { crFields.copy.call(this, s); this.closed = s.closed; this.curveType = s.curveType; this.tension = s.tension; return this; },
    toJSON: function () { var d = crFields.toJSON.call(this); d.closed = this.closed; d.curveType = this.curveType; d.tension = this.tension; return d; },
    fromJSON: function (j) { crFields.fromJSON.call(this, j); this.closed = j.closed; this.curveType = j.curveType; this.tension = j.tension; return this; }
  });

  // ---- CurvePath / Path / Shape ------------------------------------------
  // the path's own crate curve keeps its curve-length cache; its curves are
  // the facade curves' crate curves, so their caches last too
  function pathKind(p) {
    var old = p.__core.kind && p.__core.kind.f0;
    var d = struct(R.PathData, {
      curves: p.curves.map(function (c) {
        if (c._custom()) throw new Error("THREE.CurvePath: curves need a built-in kind (" + c.type + ")");
        return c._core();
      }),
      auto_close: p.autoClose === true,
      current_point: p.currentPoint ? v3(p.currentPoint) : R.Vector3.new_(0, 0, 0),
      cache_lengths: old && old.cache_lengths && old.cache_lengths.length === p.curves.length ? old.cache_lengths : [],
      holes: (p.holes || []).map(function (h) { return h._core(); })
    });
    return new R.CurveKind_Path(d);
  }
  function pathIs2d(p) {
    if (p.curves.length) return !!p.curves[0]._is2d;
    return p.type !== "CurvePath";
  }
  var CurvePath = subclass("CurvePath", Curve, function () { this.curves = []; this.autoClose = false; }, {
    _kind: function () { return pathKind(this); },
    _core: function () { var c = this.__core; c.kind = pathKind(this); c.is_2d = pathIs2d(this); return c; },
    add: function (curve) { this.curves.push(curve); },
    closePath: function () {
      var s = this.curves[0].getPoint(0), e = this.curves[this.curves.length - 1].getPoint(1);
      if (!s.equals(e)) this.curves.push(s.isVector2 ? new T.LineCurve(e, s) : new T.LineCurve3(e, s));
      return this;
    },
    getPoint: function (t, target) { return out(pathIs2d(this), this._core().get_point(t), target); },
    getCurveLengths: function () { return this._core().get_curve_lengths(); },
    updateArcLengths: function () { this._core().update_arc_lengths(); },
    getPoints: function (divisions) { return outs(pathIs2d(this), this._core().get_points(divisions === undefined ? 12 : divisions)); },
    getSpacedPoints: function (divisions) { return outs(pathIs2d(this), this._core().get_spaced_points(divisions === undefined ? 40 : divisions)); },
    getPointAt: function (u, target) { return out(pathIs2d(this), this._core().get_point_at(u), target); },
    getTangent: function (t, target) { return out(pathIs2d(this), this._core().get_tangent(t), target); },
    getTangentAt: function (u, target) { return out(pathIs2d(this), this._core().get_tangent_at(u), target); },
    copy: function (s) {
      Curve.prototype.copy.call(this, s);
      this.curves = s.curves.map(function (c) { return c.clone(); });
      this.autoClose = s.autoClose;
      return this;
    },
    toJSON: function () {
      var d = Curve.prototype.toJSON.call(this);
      d.autoClose = this.autoClose;
      d.curves = this.curves.map(function (c) { return c.toJSON(); });
      return d;
    },
    fromJSON: function (j) {
      Curve.prototype.fromJSON.call(this, j);
      this.autoClose = j.autoClose;
      this.curves = j.curves.map(function (c) { return new T[c.type]().fromJSON(c); });
      return this;
    }
  });
  // getPoints of a path picks a resolution per curve kind in the crate
  var Path = subclass("Path", CurvePath, function (points) {
    CurvePath.call(this);
    this.type = "Path";
    this.currentPoint = new T.Vector2();
    if (points) this.setFromPoints(points);
  }, {
    setFromPoints: function (points) {
      this.moveTo(points[0].x, points[0].y);
      for (var i = 1; i < points.length; i++) this.lineTo(points[i].x, points[i].y);
      return this;
    },
    moveTo: function (x, y) { this.currentPoint.set(x, y); return this; },
    lineTo: function (x, y) {
      this.curves.push(new T.LineCurve(this.currentPoint.clone(), new T.Vector2(x, y)));
      this.currentPoint.set(x, y);
      return this;
    },
    quadraticCurveTo: function (cx, cy, x, y) {
      this.curves.push(new T.QuadraticBezierCurve(this.currentPoint.clone(), new T.Vector2(cx, cy), new T.Vector2(x, y)));
      this.currentPoint.set(x, y);
      return this;
    },
    bezierCurveTo: function (c1x, c1y, c2x, c2y, x, y) {
      this.curves.push(new T.CubicBezierCurve(this.currentPoint.clone(), new T.Vector2(c1x, c1y), new T.Vector2(c2x, c2y), new T.Vector2(x, y)));
      this.currentPoint.set(x, y);
      return this;
    },
    splineThru: function (pts) {
      this.curves.push(new T.SplineCurve([this.currentPoint.clone()].concat(pts)));
      this.currentPoint.copy(pts[pts.length - 1]);
      return this;
    },
    arc: function (x, y, r, a0, a1, cw) { this.absarc(x + this.currentPoint.x, y + this.currentPoint.y, r, a0, a1, cw); return this; },
    absarc: function (x, y, r, a0, a1, cw) { this.absellipse(x, y, r, r, a0, a1, cw); return this; },
    ellipse: function (x, y, rx, ry, a0, a1, cw, rot) {
      this.absellipse(x + this.currentPoint.x, y + this.currentPoint.y, rx, ry, a0, a1, cw, rot);
      return this;
    },
    absellipse: function (x, y, rx, ry, a0, a1, cw, rot) {
      var curve = new T.EllipseCurve(x, y, rx, ry, a0, a1, cw, rot);
      if (this.curves.length > 0) {
        var first = curve.getPoint(0);
        if (!first.equals(this.currentPoint)) this.lineTo(first.x, first.y);
      }
      this.curves.push(curve);
      this.currentPoint.copy(curve.getPoint(1));
      return this;
    },
    copy: function (s) { CurvePath.prototype.copy.call(this, s); this.currentPoint.copy(s.currentPoint); return this; },
    toJSON: function () { var d = CurvePath.prototype.toJSON.call(this); d.currentPoint = this.currentPoint.toArray(); return d; },
    fromJSON: function (j) { CurvePath.prototype.fromJSON.call(this, j); this.currentPoint.fromArray(j.currentPoint); return this; }
  });
  delete T.Path.prototype.isPath;
  subclass("Shape", Path, function (points) {
    Path.call(this, points);
    this.type = "Shape";
    this.uuid = T.MathUtils.generateUUID();
    this.holes = [];
  }, {
    getPointsHoles: function (divisions) { return this.holes.map(function (h) { return h.getPoints(divisions); }); },
    extractPoints: function (divisions) { return { shape: this.getPoints(divisions), holes: this.getPointsHoles(divisions) }; },
    copy: function (s) { Path.prototype.copy.call(this, s); this.holes = s.holes.map(function (h) { return h.clone(); }); return this; },
    toJSON: function () {
      var d = Path.prototype.toJSON.call(this);
      d.uuid = this.uuid;
      d.holes = this.holes.map(function (h) { return h.toJSON(); });
      return d;
    },
    fromJSON: function (j) {
      Path.prototype.fromJSON.call(this, j);
      this.uuid = j.uuid;
      this.holes = j.holes.map(function (h) { return new T.Path().fromJSON(h); });
      return this;
    }
  });
  delete T.Shape.prototype.isShape;
  T.Curves = {
    ArcCurve: T.ArcCurve, CatmullRomCurve3: T.CatmullRomCurve3, CubicBezierCurve: T.CubicBezierCurve,
    CubicBezierCurve3: T.CubicBezierCurve3, EllipseCurve: T.EllipseCurve, LineCurve: T.LineCurve,
    LineCurve3: T.LineCurve3, QuadraticBezierCurve: T.QuadraticBezierCurve,
    QuadraticBezierCurve3: T.QuadraticBezierCurve3, SplineCurve: T.SplineCurve
  };


  // ---- geometries from curves and from other geometries --------------------
  var S = R.GeometriesShapes, klass = F.klass;
  function frenetStruct(f) {
    return struct(R.FrenetFrames, { tangents: f.tangents.map(v3), normals: f.normals.map(v3), binormals: f.binormals.map(v3) });
  }
  function shapeCores(shapes) { return (Array.isArray(shapes) ? shapes : [shapes]).map(function (s) { return s._core(); }); }
  T.ShapeGeometry = klass(T.BufferGeometry, function (shapes, curveSegments) {
    if (shapes === undefined) shapes = new T.Shape([new T.Vector2(0, 0.5), new T.Vector2(-0.5, -0.5), new T.Vector2(0.5, -0.5)]);
    var g = S.shape_geometry(shapeCores(shapes), curveSegments === undefined ? 12 : curveSegments, Array.isArray(shapes));
    g.id;
    return g;
  }, function (shapes) {
    this.parameters.shapes = shapes === undefined ? null : shapes;
  });
  // options as three.js names them; a UVGenerator other than the world one is not supported
  function extrudeOptions(options) {
    var o = R.ExtrudeOptions.new_();
    if (options.curveSegments !== undefined) o.curve_segments = options.curveSegments;
    if (options.steps !== undefined) o.steps = options.steps;
    if (options.depth !== undefined) o.depth = options.depth;
    if (options.bevelEnabled !== undefined) o.bevel_enabled = options.bevelEnabled;
    if (options.bevelThickness !== undefined) o.bevel_thickness = options.bevelThickness;
    o.bevel_size = options.bevelSize !== undefined ? options.bevelSize : o.bevel_thickness - 0.1;
    if (options.bevelOffset !== undefined) o.bevel_offset = options.bevelOffset;
    if (options.bevelSegments !== undefined) o.bevel_segments = options.bevelSegments;
    if (options.UVGenerator !== undefined) console.warn("THREE.ExtrudeGeometry: only the world UV generator is supported");
    var path = options.extrudePath;
    if (path) {
      var closed = path.isCatmullRomCurve3 ? path.closed === true : false;
      if (!path._custom()) o.extrude_along(path._core(), closed);
      else {
        o.extrude_by_path = true;
        o.path_points = path.getSpacedPoints(o.steps).map(v3);
        o.path_frames = frenetStruct(path.computeFrenetFrames(o.steps, closed));
      }
    }
    return o;
  }
  T.ExtrudeGeometry = klass(T.BufferGeometry, function (shapes, options) {
    if (shapes === undefined) shapes = new T.Shape([new T.Vector2(0.5, 0.5), new T.Vector2(-0.5, 0.5), new T.Vector2(-0.5, -0.5), new T.Vector2(0.5, -0.5)]);
    var g = S.extrude_geometry(shapeCores(shapes), extrudeOptions(options || {}));
    g.id;
    return g;
  }, function (shapes, options) {
    this.parameters.shapes = shapes === undefined ? null : shapes;
    this.parameters.options = options || {};
  });
  T.TubeGeometry = klass(T.BufferGeometry, function (path, tubularSegments, radius, radialSegments, closed) {
    if (path === undefined) path = new T.QuadraticBezierCurve3(new T.Vector3(-1, -1, 0), new T.Vector3(-1, 1, 0), new T.Vector3(1, 1, 0));
    var segs = tubularSegments === undefined ? 64 : tubularSegments;
    closed = closed === true;
    var frames = path.computeFrenetFrames(segs, closed);
    var points = [];
    for (var i = 0; i <= segs; i++) points.push(v3(path.getPointAt(i / segs)));
    var g = S.tube_from_frames(points, frenetStruct(frames), segs, radius === undefined ? 1 : radius,
      radialSegments === undefined ? 8 : radialSegments, closed);
    g.id;
    def(g, "__frames", frames);
    return g;
  }, function (path, tubularSegments, radius, radialSegments, closed) {
    this.parameters.path = path === undefined ? null : path;
    this.parameters.closed = closed === true;
    this.tangents = this.__frames.tangents;
    this.normals = this.__frames.normals;
    this.binormals = this.__frames.binormals;
  });
  T.EdgesGeometry = klass(T.BufferGeometry, function (geometry, thresholdAngle) {
    var t = thresholdAngle === undefined ? 1 : thresholdAngle;
    var g;
    if (geometry) g = S.edges_geometry(geometry, t);
    else { g = R.BufferGeometry.new_(); g.type_name = "EdgesGeometry"; g.parameters.put("thresholdAngle", t); }
    g.id;
    return g;
  }, function (geometry) { this.parameters.geometry = geometry || null; });
  T.WireframeGeometry = klass(T.BufferGeometry, function (geometry) {
    var g;
    if (geometry) g = S.wireframe_geometry(geometry);
    else { g = R.BufferGeometry.new_(); g.type_name = "WireframeGeometry"; }
    g.id;
    return g;
  }, function (geometry) { this.parameters.geometry = geometry || null; });

  // ---- ShapeUtils / Earcut -----------------------------------------------
  function v2(v) { return R.Vector2.new_(v.x, v.y); }
  function dropDupEnd(points) {
    var l = points.length;
    if (l > 2 && points[l - 1].equals(points[0])) points.pop();
  }
  T.ShapeUtils = {
    area: function (contour) { return R.ExtrasShapeUtils.area(contour.map(v2)); },
    isClockWise: function (pts) { return R.ExtrasShapeUtils.is_clock_wise(pts.map(v2)); },
    // like three.js, drops a repeated end point from the arrays it is given
    triangulateShape: function (contour, holes) {
      dropDupEnd(contour);
      holes.forEach(dropDupEnd);
      return R.ExtrasShapeUtils.triangulate_shape(contour.map(v2), holes.map(function (h) { return h.map(v2); }));
    }
  };
  T.Earcut = {
    triangulate: function (data, holeIndices, dim) {
      dim = dim || 2;
      var flat = data;
      if (dim !== 2) { flat = []; for (var i = 0; i < data.length; i += dim) flat.push(data[i], data[i + 1]); }
      return R.ExtrasEarcut.triangulate(Array.prototype.slice.call(flat), holeIndices ? Array.prototype.slice.call(holeIndices) : []);
    }
  };
})(THREE, RangerThreeCore);
