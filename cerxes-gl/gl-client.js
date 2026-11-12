// SPDX-License-Identifier: MIT
//
// A WebGL2 context for a script that runs in CErXes, over __hostSync: the
// calls are recorded as commands and sent to the page in one batch (at a
// frame's end, or before a call whose answer the script needs), where
// gl-host.js plays them on a real WebGL2 context.
//
//   var canvas = __cerxesGL.canvas(width, height);
//   var gl = canvas.getContext("webgl2");      // as WebGL2RenderingContext
//   ... gl.drawElements(...) ...
//   __cerxesGL.flush();                          // end of the frame
//
// Objects the page makes (buffers, textures, programs, uniform locations...)
// are numbered here, so making one needs no round trip. Asked synchronously:
// getParameter, getExtension, the shader and program queries, readPixels.
// Typed arrays travel as JSON number arrays (a prototype: a binary channel
// is the next step). Images and canvases as texture sources are not passed.
var __cerxesGL = (function () {
  "use strict";
  var queue = [];
  var nextId = 1;
  var stats = { flushes: 0, commands: 0, bytes: 0, syncs: 0 };
  var info = JSON.parse(__hostSync("I"));
  function Handle(id, kind) { this.__h = id; this.kind = kind; }
  function enc(v) {
    if (v === null || v === undefined || typeof v !== "object") return v === undefined ? null : v;
    if (v instanceof Handle) return { __h: v.__h };
    if (ArrayBuffer.isView(v)) {
      var name = v.constructor && v.constructor.name, a = new Array(v.length);
      for (var i = 0; i < v.length; i++) a[i] = v[i];
      return { __t: name, d: a };
    }
    if (Array.isArray(v)) return { __t: "Array", d: v.slice() };
    return null;
  }
  function push(name, ret, args) {
    var c = [name, ret];
    for (var i = 0; i < args.length; i++) c.push(enc(args[i]));
    queue.push(c);
  }
  function flush() {
    if (!queue.length) return;
    var text = "C" + JSON.stringify(queue);
    stats.flushes++; stats.commands += queue.length; stats.bytes += text.length;
    queue = [];
    __hostSync(text);
  }
  function ask(name, args) {
    flush();
    var c = [name, 0];
    for (var i = 0; i < args.length; i++) c.push(enc(args[i]));
    stats.syncs++;
    return JSON.parse(__hostSync("Q" + JSON.stringify(c)));
  }
  var SYNC = { getParameter: 1, getShaderParameter: 1, getProgramParameter: 1, getShaderInfoLog: 1, getProgramInfoLog: 1,
    getError: 1, getAttribLocation: 1, checkFramebufferStatus: 1, getSupportedExtensions: 1, isContextLost: 1, getContextAttributes: 1 };
  function Context(canvas) {
    this.canvas = canvas;
    this.drawingBufferWidth = canvas._w;
    this.drawingBufferHeight = canvas._h;
  }
  var P = Context.prototype;
  for (var k in info.constants) P[k] = info.constants[k];
  info.methods.forEach(function (name) {
    if (/^create/.test(name)) {
      P[name] = function () { var id = nextId++; push(name, id, arguments); return new Handle(id, name.slice(6)); };
    } else if (name === "getUniformLocation") {
      P[name] = function (program, uname) { var id = nextId++; push(name, id, arguments); return new Handle(id, "UniformLocation"); };
    } else if (SYNC[name]) {
      P[name] = function () { return ask(name, arguments); };
    } else {
      P[name] = function () { push(name, 0, arguments); };
    }
  });
  P.getExtension = function (name) { return ask("getExtension", [name]) ? {} : null; };
  P.readPixels = function (x, y, w, h, format, type, out) {
    var px = ask("readPixels", [x, y, w, h, format, type]);
    for (var i = 0; i < px.length && i < out.length; i++) out[i] = px[i];
  };
  function Canvas(w, h) { this._w = w; this._h = h; this._gl = null; this.style = {}; }
  Object.defineProperty(Canvas.prototype, "width", { get: function () { return this._w; }, set: function (v) { this._w = v; this._resize(); } });
  Object.defineProperty(Canvas.prototype, "height", { get: function () { return this._h; }, set: function (v) { this._h = v; this._resize(); } });
  Canvas.prototype._resize = function () {
    push("__resize", 0, [this._w, this._h]);
    if (this._gl) { this._gl.drawingBufferWidth = this._w; this._gl.drawingBufferHeight = this._h; }
  };
  Canvas.prototype.getContext = function (kind) {
    if (kind !== "webgl2") return null;
    if (!this._gl) { this._gl = new Context(this); this._resize(); }
    return this._gl;
  };
  Canvas.prototype.addEventListener = function () {};
  Canvas.prototype.removeEventListener = function () {};
  return { canvas: function (w, h) { return new Canvas(w || 300, h || 150); }, flush: flush, stats: stats };
})();
