// SPDX-License-Identifier: MIT
//
// Plays the crate's GPU command stream (three/src/renderers/gpu.rs, written
// by renderers/gpu_frame.rs) on a WebGL2 context. It keeps the GPU objects by
// the ids the stream gives them and nothing else: programs, uniform values,
// state and draw order all come from the crate. gpu-gl/ is the same player
// for OpenGL ES 3 / GL 3.3.
//
//   var gpu = new GpuWebGL2(gl);
//   GpuWebGL2.uploadImages(renderer, scene, decode);   // texture pixels the crate asks for
//   gpu.run(frame.commands);          // a GpuCommands (ints, floats, texts, bytes)
(function (root) {
  "use strict";
  var SIZES = { 1: 1, 2: 2, 3: 3, 4: 4, 5: 9, 6: 16 };

  function GpuWebGL2(gl) {
    this.gl = gl;
    this.obj = {};
    this.vao = gl.createVertexArray();
    this.stats = { runs: 0, commands: 0 };
  }

  // the crate's Vec<u8> in JS ({ byteLength, _view: DataView }) or a plain array, as bytes
  function byteArray(b) {
    if (!b) return new Uint8Array(0);
    if (b instanceof Uint8Array) return b;
    if (b._view) return new Uint8Array(b._view.buffer, b._view.byteOffset, b.byteLength);
    return Uint8Array.from(b);
  }

  GpuWebGL2.prototype.run = function (c) {
    var gl = this.gl, ints = c.ints, floats = c.floats instanceof Float32Array ? c.floats : Float32Array.from(c.floats);
    var texts = c.texts, bytes = byteArray(c.bytes), obj = this.obj;
    var i = 0, f = 0, n = ints.length, prog = null;
    this.stats.runs++;
    gl.bindVertexArray(this.vao);
    while (i < n) {
      var op = ints[i++];
      this.stats.commands++;
      switch (op) {
        case 1: { // BUFFER id, kind, dynamic, n
          var id = ints[i], index = ints[i + 1] === 1, dyn = ints[i + 2] === 1, cnt = ints[i + 3];
          i += 4;
          var target = index ? gl.ELEMENT_ARRAY_BUFFER : gl.ARRAY_BUFFER, data;
          if (index) { data = new Uint32Array(cnt); for (var k = 0; k < cnt; k++) data[k] = ints[i + k]; i += cnt; }
          else { data = floats.subarray(f, f + cnt); f += cnt; }
          var e = obj[id];
          if (!e) e = obj[id] = { buf: gl.createBuffer(), length: -1 };
          gl.bindBuffer(target, e.buf);
          if (e.length === cnt) gl.bufferSubData(target, 0, data);
          else gl.bufferData(target, data, dyn ? gl.DYNAMIC_DRAW : gl.STATIC_DRAW);
          e.length = cnt;
          break;
        }
        case 2: { // PROGRAM id, vs, fs, names
          var pid = ints[i], vs = texts[ints[i + 1]], fs = texts[ints[i + 2]], nn = ints[i + 3];
          i += 4;
          var p = gl.createProgram();
          gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vs));
          gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fs));
          gl.linkProgram(p);
          if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error("gpu-webgl2 program: " + gl.getProgramInfoLog(p));
          var locs = [];
          for (var q = 0; q < nn; q++) locs.push(gl.getUniformLocation(p, texts[ints[i + q]]));
          i += nn;
          obj[pid] = { prog: p, locs: locs };
          break;
        }
        case 3: { // TEXTURE id, cube, internal, format, type, w, h, min, mag, wrap s, wrap t, flags, data, offset, n
          var tid = ints[i], cube = ints[i + 1] === 1, internal = ints[i + 2], format = ints[i + 3], type = ints[i + 4];
          var w = ints[i + 5], h = ints[i + 6], dk = ints[i + 12], off = ints[i + 13], dn = ints[i + 14];
          var tgt = cube ? gl.TEXTURE_CUBE_MAP : gl.TEXTURE_2D, px = null;
          if (dk === 1) { px = floats.subarray(f, f + dn); f += dn; }
          // WebGL takes no view of a resizable buffer (the crate's growable Vec<u8>): copy then
          else if (dk === 2) px = bytes.buffer.resizable ? bytes.slice(off, off + dn) : bytes.subarray(off, off + dn);
          var t = obj[tid] ? obj[tid].tex : gl.createTexture();
          gl.bindTexture(tgt, t);
          gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
          gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
          gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
          if (cube) for (var face = 0; face < 6; face++) gl.texImage2D(gl.TEXTURE_CUBE_MAP_POSITIVE_X + face, 0, internal, w, h, 0, format, type, px);
          else gl.texImage2D(gl.TEXTURE_2D, 0, internal, w, h, 0, format, type, px);
          gl.texParameteri(tgt, gl.TEXTURE_MIN_FILTER, ints[i + 7]);
          gl.texParameteri(tgt, gl.TEXTURE_MAG_FILTER, ints[i + 8]);
          gl.texParameteri(tgt, gl.TEXTURE_WRAP_S, ints[i + 9]);
          gl.texParameteri(tgt, gl.TEXTURE_WRAP_T, ints[i + 10]);
          if (ints[i + 11] & 1) {
            gl.texParameteri(tgt, gl.TEXTURE_COMPARE_MODE, gl.COMPARE_REF_TO_TEXTURE);
            gl.texParameteri(tgt, gl.TEXTURE_COMPARE_FUNC, gl.LEQUAL);
          }
          if (ints[i + 11] & 2) gl.generateMipmap(tgt);
          gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
          gl.bindTexture(tgt, null);
          obj[tid] = { tex: t };
          i += 15;
          break;
        }
        case 4: { // PASS target, w, h, clear | rgba, depth
          var fb = ints[i] ? obj[ints[i]].fb : null, pw = ints[i + 1], ph = ints[i + 2], clear = ints[i + 3];
          i += 4;
          gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
          gl.viewport(0, 0, pw, ph);
          gl.clearColor(floats[f], floats[f + 1], floats[f + 2], floats[f + 3]);
          gl.clearDepth(floats[f + 4]);
          f += 5;
          gl.depthMask(true);
          gl.colorMask(true, true, true, true);
          gl.clear((clear & 1 ? gl.COLOR_BUFFER_BIT : 0) | (clear & 2 ? gl.DEPTH_BUFFER_BIT : 0));
          break;
        }
        case 5: // USE id
          prog = obj[ints[i++]];
          gl.useProgram(prog.prog);
          break;
        case 6: { // UNIFORM location, kind, count
          var loc = prog.locs[ints[i]], kind = ints[i + 1], count = ints[i + 2];
          i += 3;
          if (kind === 7) { if (loc != null) gl.uniform1i(loc, ints[i]); i += count; break; }
          var sz = SIZES[kind] * count, v = floats.subarray(f, f + sz);
          f += sz;
          if (loc == null) break;
          if (kind === 6) gl.uniformMatrix4fv(loc, false, v);
          else if (kind === 5) gl.uniformMatrix3fv(loc, false, v);
          else if (kind === 4) gl.uniform4fv(loc, v);
          else if (kind === 3) gl.uniform3fv(loc, v);
          else if (kind === 2) gl.uniform2fv(loc, v);
          else gl.uniform1fv(loc, v);
          break;
        }
        case 7: { // STATE
          if (ints[i]) {
            gl.enable(gl.BLEND);
            gl.blendEquationSeparate(ints[i + 1], ints[i + 2]);
            gl.blendFuncSeparate(ints[i + 3], ints[i + 4], ints[i + 5], ints[i + 6]);
            gl.blendColor(floats[f], floats[f + 1], floats[f + 2], floats[f + 3]);
          } else gl.disable(gl.BLEND);
          if (ints[i + 7]) { gl.enable(gl.DEPTH_TEST); gl.depthFunc(ints[i + 8]); } else gl.disable(gl.DEPTH_TEST);
          gl.depthMask(ints[i + 9] === 1);
          if (ints[i + 10] === 0) gl.disable(gl.CULL_FACE);
          else { gl.enable(gl.CULL_FACE); gl.cullFace(ints[i + 10] === 2 ? gl.FRONT : gl.BACK); }
          gl.frontFace(ints[i + 11] ? gl.CCW : gl.CW);
          var cw = ints[i + 12] === 1;
          gl.colorMask(cw, cw, cw, cw);
          if (ints[i + 13]) { gl.enable(gl.POLYGON_OFFSET_FILL); gl.polygonOffset(floats[f + 4], floats[f + 5]); }
          else gl.disable(gl.POLYGON_OFFSET_FILL);
          i += 14; f += 6;
          break;
        }
        case 8: { // ATTRIB location, buffer, size, stride, offset, divisor
          var al = ints[i], ab = ints[i + 1];
          if (ab === 0) {
            gl.disableVertexAttribArray(al);
            gl.vertexAttrib4f(al, floats[f], floats[f + 1], floats[f + 2], floats[f + 3]);
            f += 4;
          } else {
            gl.bindBuffer(gl.ARRAY_BUFFER, obj[ab].buf);
            gl.enableVertexAttribArray(al);
            gl.vertexAttribPointer(al, ints[i + 2], gl.FLOAT, false, ints[i + 3], ints[i + 4]);
            gl.vertexAttribDivisor(al, ints[i + 5]);
          }
          i += 6;
          break;
        }
        case 9: { // DRAW mode, first, count, instances, index buffer
          var mode = ints[i], first = ints[i + 1], dc = ints[i + 2], inst = ints[i + 3], ib = ints[i + 4];
          i += 5;
          if (ib) {
            gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, obj[ib].buf);
            if (inst > 0) gl.drawElementsInstanced(mode, dc, gl.UNSIGNED_INT, first * 4, inst);
            else gl.drawElements(mode, dc, gl.UNSIGNED_INT, first * 4);
          } else if (inst > 0) gl.drawArraysInstanced(mode, first, dc, inst);
          else gl.drawArrays(mode, first, dc);
          break;
        }
        case 10: { // BIND unit, texture, cube
          gl.activeTexture(gl.TEXTURE0 + ints[i]);
          var bt = ints[i + 1] ? obj[ints[i + 1]].tex : null;
          gl.bindTexture(ints[i + 2] ? gl.TEXTURE_CUBE_MAP : gl.TEXTURE_2D, bt);
          gl.activeTexture(gl.TEXTURE0);
          i += 3;
          break;
        }
        case 11: { // RELEASE id
          var r = obj[ints[i++]];
          if (r) {
            if (r.buf) gl.deleteBuffer(r.buf);
            if (r.tex) gl.deleteTexture(r.tex);
            if (r.prog) gl.deleteProgram(r.prog);
            if (r.fb) gl.deleteFramebuffer(r.fb);
          }
          break;
        }
        default:
          throw new Error("gpu-webgl2: unknown command " + op + " at " + (i - 1));
      }
    }
    if (f !== floats.length) throw new Error("gpu-webgl2: " + (floats.length - f) + " floats left over");
  };

  // Hands the crate's GpuRenderer the pixels of the textures it names
  // (textures_to_upload), decoded by `decode(image, texture)` -> { width,
  // height, data: RGBA bytes, rows from the top } or null while not ready
  // (THREE.WebGLRenderer.decodeImage: data textures, images, canvases).
  GpuWebGL2.uploadImages = function (renderer, scene, decode) {
    var list = renderer.textures_to_upload(scene);
    for (var k = 0; k < list.length; k++) {
      var tex = list[k], px = decode(tex.image, tex);
      if (!px) continue;
      var d = px.data, buf = { byteLength: d.byteLength, _view: new DataView(d.buffer, d.byteOffset, d.byteLength) };
      renderer.set_texture_image(tex.id, tex.version, px.width, px.height, buf);
    }
  };

  function compile(gl, type, src) {
    var s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error("gpu-webgl2 shader: " + gl.getShaderInfoLog(s));
    return s;
  }

  root.GpuWebGL2 = GpuWebGL2;
})(typeof globalThis !== "undefined" ? globalThis : this);
