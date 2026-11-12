// SPDX-License-Identifier: MIT
//
// THREE.WebGLRenderer's API. With a WebGL2 canvas it draws on the GPU
// (three-webgl.js over the crate's frame plan, renderers/plan.rs); otherwise,
// or with { software: true }, the crate's software renderer
// (renderers/soft.rs) draws into a pixel buffer copied to a 2D canvas.
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
    this._gpu = null;
    var canvas = this.domElement;
    if (p.software !== true && T.__WebGL2Backend && canvas && canvas.getContext) {
      var gl = null;
      try {
        gl = canvas.getContext("webgl2", { alpha: !!p.alpha, antialias: p.antialias !== false, depth: true, premultipliedAlpha: p.premultipliedAlpha !== false, preserveDrawingBuffer: !!p.preserveDrawingBuffer, powerPreference: p.powerPreference || "default" });
      } catch (e) { gl = null; }
      if (gl) {
        try { this._gpu = new T.__WebGL2Backend(gl, this); } catch (e) { if (typeof console !== "undefined") console.warn(String(e)); this._gpu = null; }
      }
    }
    // "webgl2" or "software"
    this.backend = this._gpu ? "webgl2" : "software";
    if (this.domElement) this._resize();
  }
  var P = WebGLRenderer.prototype;
  P._resize = function () {
    var w = Math.max(1, Math.floor(this._width * this._pixelRatio)), h = Math.max(1, Math.floor(this._height * this._pixelRatio));
    if (!this._gpu) this._soft.set_size(w, h);
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
  P.getDrawingBufferSize = function (target) {
    if (this._gpu) return (target || new T.Vector2()).set(this._gpu.gl.drawingBufferWidth, this._gpu.gl.drawingBufferHeight);
    return (target || new T.Vector2()).set(this._soft.width, this._soft.height);
  };
  P.setClearColor = function (color, alpha) {
    var c = new T.Color(color);
    this._soft.clear_color = c;
    if (alpha !== undefined) this._soft.clear_alpha = alpha;
  };
  P.getClearColor = function (target) { return (target || new T.Color()).copy(this._soft.clear_color); };
  P.setClearAlpha = function (a) { this._soft.clear_alpha = a; };
  P.getClearAlpha = function () { return this._soft.clear_alpha; };
  // texture pixels: decoded once per texture version and handed to the crate
  function decodeImage(img, tex) {
    if (!img) return null;
    if (img.data && img.width && img.height) {
      var w = img.width, h = img.height, src = img.data, n = w * h, out = new Uint8ClampedArray(n * 4);
      var ch = tex.format === 1023 ? 4 : tex.format === 1022 ? 3 : tex.format === 1030 ? 2 : (tex.format === 1028 || tex.format === 1024) ? 1 : Math.max(1, Math.round(src.length / n));
      var scale = (src instanceof Float32Array || src instanceof Float64Array) ? 255 : 1;
      for (var i = 0; i < n; i++) {
        var o = i * 4, s0 = i * ch;
        var r = src[s0] * scale, g = ch > 2 ? src[s0 + 1] * scale : r, b = ch > 2 ? src[s0 + 2] * scale : r;
        out[o] = r; out[o + 1] = g; out[o + 2] = b;
        out[o + 3] = ch === 4 ? src[s0 + 3] * scale : ch === 2 ? src[s0 + 1] * scale : 255;
      }
      return { width: w, height: h, data: out };
    }
    var iw = img.naturalWidth || img.videoWidth || img.width, ih = img.naturalHeight || img.videoHeight || img.height;
    if (!iw || !ih || img.complete === false) return null;
    var c = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(iw, ih) : (typeof document !== "undefined" ? document.createElement("canvas") : null);
    if (!c) return null;
    c.width = iw; c.height = ih;
    var ctx = c.getContext("2d");
    if (!ctx) return null;
    ctx.drawImage(img, 0, 0, iw, ih);
    return { width: iw, height: ih, data: ctx.getImageData(0, 0, iw, ih).data };
  }
  P._uploadTextures = function (scene) {
    var soft = this._soft, seen = this._seenTextures || (this._seenTextures = {}), count = 0;
    function upload(tex) {
      if (!tex || !tex.image) return;
      var version = tex.version;
      if (soft.texture_version(tex.id) === version) return;
      var px = decodeImage(tex.image, tex);
      if (!px) return;
      // the crate's Vec<u8> is a byte buffer viewed through a DataView
      var bytes = px.data, buf = { byteLength: bytes.byteLength, _view: new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength) };
      soft.set_texture_image(tex.id, version, px.width, px.height, buf, tex.colorSpace === "srgb");
      seen[tex.id] = true;
    }
    scene.traverse(function (o) {
      var m = o.material;
      if (!m) return;
      if (Array.isArray(m)) m.forEach(function (x) { upload(x.map); });
      else upload(m.map);
    });
    for (var k in seen) count++;
    this.info.memory.textures = count;
  };
  P.render = function (scene, camera) {
    if (this._gpu) {
      this._gpu.render(scene, camera);
      this.info.render.frame++;
      return;
    }
    this._uploadTextures(scene);
    this._soft.render(scene, camera);
    this.info.render.frame++;
    this.info.render.calls = 1;
    this.info.render.triangles = this._soft.triangles;
    this.info.render.lines = this._soft.lines;
    this.info.render.points = this._soft.points;
    this._present();
  };
  // the pixel buffer as RGBA bytes
  P.readPixels = function () {
    if (this._gpu) return this._gpu.readPixels();
    return Uint8ClampedArray.from(this._soft.pixels);
  };
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
  P.clear = function () {
    if (this._gpu) {
      var gl = this._gpu.gl, c = this._soft.clear_color, s = R.MathsColor.linear_to_srgb;
      gl.clearColor(s(c.r), s(c.g), s(c.b), this._soft.clear_alpha);
      gl.depthMask(true);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
      return;
    }
    this._soft.clear(this._soft.clear_color, this._soft.clear_alpha); this._present();
  };
  P.compile = function () {};
  P.dispose = function () { this._loop = null; if (this._gpu) this._gpu.dispose(); };
  P.forceContextLoss = function () {};
  // ---- loaders -----------------------------------------------------------
  function Loader(manager) { this.manager = manager || null; this.crossOrigin = "anonymous"; this.withCredentials = false; this.path = ""; this.resourcePath = ""; this.requestHeader = {}; }
  Loader.prototype.setPath = function (p) { this.path = p; return this; };
  Loader.prototype.setResourcePath = function (p) { this.resourcePath = p; return this; };
  Loader.prototype.setCrossOrigin = function (v) { this.crossOrigin = v; return this; };
  Loader.prototype.setWithCredentials = function (v) { this.withCredentials = v; return this; };
  Loader.prototype.setRequestHeader = function (h) { this.requestHeader = h; return this; };
  Loader.prototype.loadAsync = function (url, onProgress) {
    var self = this;
    return new Promise(function (resolve, reject) { self.load(url, resolve, onProgress, reject); });
  };
  function ImageLoader(manager) { Loader.call(this, manager); }
  ImageLoader.prototype = Object.create(Loader.prototype);
  ImageLoader.prototype.constructor = ImageLoader;
  ImageLoader.prototype.load = function (url, onLoad, onProgress, onError) {
    var img = document.createElement("img");
    if (this.crossOrigin && url.slice(0, 5) !== "data:") img.crossOrigin = this.crossOrigin;
    img.addEventListener("load", function () { if (onLoad) onLoad(img); });
    img.addEventListener("error", function (e) { if (onError) onError(e); });
    img.src = this.path + url;
    return img;
  };
  function TextureLoader(manager) { Loader.call(this, manager); }
  TextureLoader.prototype = Object.create(Loader.prototype);
  TextureLoader.prototype.constructor = TextureLoader;
  TextureLoader.prototype.load = function (url, onLoad, onProgress, onError) {
    var texture = new T.Texture();
    var loader = new ImageLoader(this.manager);
    loader.setCrossOrigin(this.crossOrigin).setPath(this.path);
    loader.load(url, function (img) {
      texture.image = img;
      texture.needsUpdate = true;
      if (onLoad) onLoad(texture);
    }, onProgress, onError);
    return texture;
  };
  T.Loader = T.Loader || Loader;
  T.ImageLoader = T.ImageLoader || ImageLoader;
  T.TextureLoader = T.TextureLoader || TextureLoader;
  WebGLRenderer.decodeImage = decodeImage;
  T.WebGLRenderer = WebGLRenderer;
  T.SoftwareRenderer = WebGLRenderer;
})(THREE, RangerThreeCore);
