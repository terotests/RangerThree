// SPDX-License-Identifier: MIT
//
// THREE.WebGLRenderer's API over the crate's software renderer
// (three/src/renderers/soft.rs): the scene is drawn into a pixel buffer and
// copied to the canvas. The GL backend will take the same calls.
(function (T, R) {
  "use strict";
  function WebGLRenderer(parameters) {
    var p = parameters || {};
    this.domElement = p.canvas || (typeof document !== "undefined" ? document.createElement("canvas") : null);
    this._soft = R.SoftRenderer.new_(300, 150);
    this._pixelRatio = 1;
    this._width = 300;
    this._height = 150;
    this._loop = null;
    this.autoClear = true;
    this.outputColorSpace = "srgb";
    this.toneMapping = 0;
    this.toneMappingExposure = 1;
    this.shadowMap = { enabled: false, type: 1 };
    this.info = { render: { frame: 0, calls: 0, triangles: 0, points: 0, lines: 0 }, memory: { geometries: 0, textures: 0 } };
    this.isWebGLRenderer = true;
    if (this.domElement) this._resize();
  }
  var P = WebGLRenderer.prototype;
  P._resize = function () {
    var w = Math.max(1, Math.floor(this._width * this._pixelRatio)), h = Math.max(1, Math.floor(this._height * this._pixelRatio));
    this._soft.set_size(w, h);
    if (this.domElement) { this.domElement.width = w; this.domElement.height = h; }
    this._image = null;
  };
  P.setSize = function (width, height, updateStyle) {
    this._width = width; this._height = height;
    if (this.domElement && updateStyle !== false && this.domElement.style) {
      this.domElement.style.width = width + "px";
      this.domElement.style.height = height + "px";
    }
    this._resize();
  };
  P.getSize = function (target) { return (target || new T.Vector2()).set(this._width, this._height); };
  P.setPixelRatio = function (r) { this._pixelRatio = r || 1; this._resize(); };
  P.getPixelRatio = function () { return this._pixelRatio; };
  P.getDrawingBufferSize = function (target) { return (target || new T.Vector2()).set(this._soft.width, this._soft.height); };
  P.setClearColor = function (color, alpha) {
    var c = new T.Color(color);
    this._soft.clear_color = c;
    if (alpha !== undefined) this._soft.clear_alpha = alpha;
  };
  P.getClearColor = function (target) { return (target || new T.Color()).copy(this._soft.clear_color); };
  P.setClearAlpha = function (a) { this._soft.clear_alpha = a; };
  P.getClearAlpha = function () { return this._soft.clear_alpha; };
  P.render = function (scene, camera) {
    this._soft.render(scene, camera);
    this.info.render.frame++;
    this.info.render.calls = 1;
    this.info.render.triangles = this._soft.triangles;
    this._present();
  };
  // the pixel buffer as RGBA bytes
  P.readPixels = function () { return Uint8ClampedArray.from(this._soft.pixels); };
  P._present = function () {
    var canvas = this.domElement;
    if (!canvas || !canvas.getContext) return;
    var ctx = this._ctx || (this._ctx = canvas.getContext("2d"));
    if (!ctx) return;
    var w = this._soft.width, h = this._soft.height;
    if (!this._image || this._image.width !== w || this._image.height !== h) this._image = ctx.createImageData(w, h);
    var px = this._soft.pixels, d = this._image.data;
    for (var i = 0; i < d.length; i++) d[i] = px[i];
    ctx.putImageData(this._image, 0, 0);
  };
  P.setAnimationLoop = function (fn) {
    var self = this;
    this._loop = fn;
    if (!fn || typeof requestAnimationFrame === "undefined") return;
    var tick = function (t) { if (self._loop !== fn) return; fn(t); requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  };
  P.clear = function () { this._soft.clear(this._soft.clear_color, this._soft.clear_alpha); this._present(); };
  P.compile = function () {};
  P.dispose = function () { this._loop = null; };
  P.forceContextLoss = function () {};
  T.WebGLRenderer = WebGLRenderer;
  T.SoftwareRenderer = WebGLRenderer;
})(THREE, RangerThreeCore);
