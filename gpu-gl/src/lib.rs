//! Plays RangerThree's GPU command stream (three/src/renderers/gpu.rs,
//! written by renderers/gpu_frame.rs) on OpenGL ES 3.0 or desktop OpenGL 3.3
//! core. The same player as js/gpu-webgl2.js: it keeps GPU objects by the ids
//! the stream gives them, and programs, uniform values, state and draw order
//! all come from the crate.
//!
//! ```ignore
//! let gl = gpu_gl::gl::Gl::load(&|name| host_get_proc_address(name))?;
//! let mut player = gpu_gl::Player::new(gl, desktop_core);
//! player.run(&c.ints, &floats_f32, &c.texts, &c.bytes)?;   // each frame
//! ```
pub mod egl;
pub mod gl;

use gl::*;
use std::collections::HashMap;
use std::ffi::c_void;

enum Obj {
    Buffer { buf: GLuint, length: i64 },
    Program { prog: GLuint, locs: Vec<GLint> },
    Texture { tex: GLuint },
    Target { fb: GLuint, rbs: Vec<GLuint> },
}

/// The GPU objects of one GL context, and the stream player for it. The
/// stream's object ids are given by one GpuRenderer, so a player plays the
/// frames of one renderer.
pub struct Player {
    pub gl: Gl,
    objs: HashMap<i64, Obj>,
    vao: GLuint,
    desktop: bool,
    /// The framebuffer a pass with target 0 draws into (the host's window or offscreen target).
    pub screen_fb: GLuint,
    pub commands: u64,
}

const SIZES: [usize; 7] = [0, 1, 2, 3, 4, 9, 16];

fn at(ints: &[i64], i: usize) -> Result<i64, String> {
    ints.get(i).copied().ok_or_else(|| format!("gpu-gl: stream ends inside a command at {}", i))
}

impl Player {
    /// `desktop` is true for an OpenGL 3.3 core context, false for OpenGL ES 3.
    pub fn new(gl: Gl, desktop: bool) -> Player {
        let mut vao = 0;
        // SAFETY: the context `gl` was loaded for is current on this thread.
        unsafe {
            (gl.GenVertexArrays)(1, &mut vao);
            if desktop {
                (gl.Enable)(PROGRAM_POINT_SIZE);
            }
        }
        Player { gl, objs: HashMap::new(), vao, desktop, screen_fb: 0, commands: 0 }
    }

    /// The GL version and renderer strings.
    pub fn describe(&self) -> String {
        let s = |e| unsafe {
            let p = (self.gl.GetString)(e);
            if p.is_null() { String::new() } else { std::ffi::CStr::from_ptr(p as *const _).to_string_lossy().into_owned() }
        };
        format!("{} / {}", s(VERSION), s(RENDERER))
    }

    /// Runs one frame's commands. `floats` are the stream's floats as f32.
    pub fn run(&mut self, ints: &[i64], floats: &[f32], texts: &[String], bytes: &[u8]) -> Result<(), String> {
        let gl = &self.gl;
        let (mut i, mut f, n) = (0usize, 0usize, ints.len());
        let mut prog: Option<i64> = None;
        let fl = |f: usize, k: usize| -> Result<&[f32], String> {
            floats.get(f..f + k).ok_or_else(|| format!("gpu-gl: floats end at {} (need {})", f, k))
        };
        // SAFETY: every GL call below goes to the current context with
        // pointers into slices that outlive the call and lengths checked
        // against those slices.
        unsafe {
            (gl.BindVertexArray)(self.vao);
            while i < n {
                let op = ints[i];
                i += 1;
                self.commands += 1;
                match op {
                    1 => {
                        // BUFFER id, kind, dynamic, n
                        let (id, index, dynamic, cnt) = (at(ints, i)?, at(ints, i + 1)? == 1, at(ints, i + 2)? == 1, at(ints, i + 3)? as usize);
                        i += 4;
                        let target = if index { ELEMENT_ARRAY_BUFFER } else { ARRAY_BUFFER };
                        let idx: Vec<u32>;
                        let (ptr, size) = if index {
                            idx = ints.get(i..i + cnt).ok_or("gpu-gl: index data cut")?.iter().map(|&v| v as u32).collect();
                            i += cnt;
                            (idx.as_ptr() as *const c_void, cnt * 4)
                        } else {
                            let d = fl(f, cnt)?;
                            f += cnt;
                            (d.as_ptr() as *const c_void, cnt * 4)
                        };
                        let e = self.objs.entry(id).or_insert_with(|| {
                            let mut b = 0;
                            (gl.GenBuffers)(1, &mut b);
                            Obj::Buffer { buf: b, length: -1 }
                        });
                        if let Obj::Buffer { buf, length } = e {
                            (gl.BindBuffer)(target, *buf);
                            if *length == cnt as i64 {
                                (gl.BufferSubData)(target, 0, size as isize, ptr);
                            } else {
                                (gl.BufferData)(target, size as isize, ptr, if dynamic { DYNAMIC_DRAW } else { STATIC_DRAW });
                            }
                            *length = cnt as i64;
                        } else {
                            return Err(format!("gpu-gl: id {} is not a buffer", id));
                        }
                    }
                    2 => {
                        // PROGRAM id, vs, fs, names
                        let (pid, vs, fs, nn) = (at(ints, i)?, at(ints, i + 1)? as usize, at(ints, i + 2)? as usize, at(ints, i + 3)? as usize);
                        i += 4;
                        let text = |k: usize| texts.get(k).ok_or_else(|| format!("gpu-gl: no text {}", k));
                        let p = (gl.CreateProgram)();
                        let v = compile(gl, VERTEX_SHADER, &self.source(text(vs)?))?;
                        let fr = compile(gl, FRAGMENT_SHADER, &self.source(text(fs)?))?;
                        (gl.AttachShader)(p, v);
                        (gl.AttachShader)(p, fr);
                        (gl.LinkProgram)(p);
                        (gl.DeleteShader)(v);
                        (gl.DeleteShader)(fr);
                        let mut ok = 0;
                        (gl.GetProgramiv)(p, LINK_STATUS, &mut ok);
                        if ok == 0 {
                            return Err(format!("gpu-gl program: {}", program_log(gl, p)));
                        }
                        let mut locs = Vec::with_capacity(nn);
                        for q in 0..nn {
                            let mut name = text(at(ints, i + q)? as usize)?.clone().into_bytes();
                            name.push(0);
                            locs.push((gl.GetUniformLocation)(p, name.as_ptr()));
                        }
                        i += nn;
                        if let Some(Obj::Program { prog, .. }) = self.objs.get(&pid) {
                            (gl.DeleteProgram)(*prog);
                        }
                        self.objs.insert(pid, Obj::Program { prog: p, locs });
                    }
                    3 => {
                        // TEXTURE id, cube, internal, format, type, w, h, min, mag, wrap s, wrap t, flags, data, offset, n
                        let a = ints.get(i..i + 15).ok_or("gpu-gl: texture command cut")?;
                        i += 15;
                        let (tid, cube) = (a[0], a[1] == 1);
                        let (dk, off, dn) = (a[12], a[13] as usize, a[14] as usize);
                        let tgt = if cube { TEXTURE_CUBE_MAP } else { TEXTURE_2D };
                        let px: *const c_void = match dk {
                            1 => {
                                let d = fl(f, dn)?;
                                f += dn;
                                d.as_ptr() as *const c_void
                            }
                            2 => bytes.get(off..off + dn).ok_or("gpu-gl: texture bytes cut")?.as_ptr() as *const c_void,
                            _ => std::ptr::null(),
                        };
                        let t = match self.objs.get(&tid) {
                            Some(Obj::Texture { tex }) => *tex,
                            _ => {
                                let mut t = 0;
                                (gl.GenTextures)(1, &mut t);
                                t
                            }
                        };
                        (gl.ActiveTexture)(TEXTURE0 + 31); // uploads on a unit no program samples
                        (gl.BindTexture)(tgt, t);
                        (gl.PixelStorei)(UNPACK_ALIGNMENT, 1);
                        let (internal, format, ty, w, h) = (a[2] as GLint, a[3] as GLenum, a[4] as GLenum, a[5] as GLsizei, a[6] as GLsizei);

                        if cube {
                            for face in 0..6 {
                                (gl.TexImage2D)(TEXTURE_CUBE_MAP_POSITIVE_X + face, 0, internal, w, h, 0, format, ty, px);
                            }
                        } else {
                            (gl.TexImage2D)(TEXTURE_2D, 0, internal, w, h, 0, format, ty, px);
                        }
                        (gl.TexParameteri)(tgt, TEXTURE_MIN_FILTER, a[7] as GLint);
                        (gl.TexParameteri)(tgt, TEXTURE_MAG_FILTER, a[8] as GLint);
                        (gl.TexParameteri)(tgt, TEXTURE_WRAP_S, a[9] as GLint);
                        (gl.TexParameteri)(tgt, TEXTURE_WRAP_T, a[10] as GLint);
                        if a[11] & 1 != 0 {
                            (gl.TexParameteri)(tgt, TEXTURE_COMPARE_MODE, COMPARE_REF_TO_TEXTURE);
                            (gl.TexParameteri)(tgt, TEXTURE_COMPARE_FUNC, LEQUAL);
                        }
                        if a[11] & 2 != 0 {
                            (gl.GenerateMipmap)(tgt);
                        }
                        (gl.PixelStorei)(UNPACK_ALIGNMENT, 4);
                        (gl.BindTexture)(tgt, 0);
                        (gl.ActiveTexture)(TEXTURE0);
                        self.objs.insert(tid, Obj::Texture { tex: t });
                    }
                    4 => {
                        // PASS target, w, h, clear | rgba, depth
                        let (target, pw, ph, clear) = (at(ints, i)?, at(ints, i + 1)?, at(ints, i + 2)?, at(ints, i + 3)?);
                        i += 4;
                        let fb = self.framebuffer(target)?;
                        let c = fl(f, 5)?;
                        f += 5;
                        (gl.BindFramebuffer)(FRAMEBUFFER, fb);
                        (gl.Viewport)(0, 0, pw as GLsizei, ph as GLsizei);
                        (gl.ClearColor)(c[0], c[1], c[2], c[3]);
                        (gl.ClearDepthf)(c[4]);
                        (gl.DepthMask)(1);
                        (gl.ColorMask)(1, 1, 1, 1);
                        let mut bits = 0;
                        if clear & 1 != 0 {
                            bits |= COLOR_BUFFER_BIT;
                        }
                        if clear & 2 != 0 {
                            bits |= DEPTH_BUFFER_BIT;
                        }
                        (gl.Clear)(bits);
                    }
                    5 => {
                        // USE id
                        let id = at(ints, i)?;
                        i += 1;
                        match self.objs.get(&id) {
                            Some(Obj::Program { prog: p, .. }) => (gl.UseProgram)(*p),
                            _ => return Err(format!("gpu-gl: id {} is not a program", id)),
                        }
                        prog = Some(id);
                    }
                    6 => {
                        // UNIFORM location, kind, count
                        let (li, kind, count) = (at(ints, i)? as usize, at(ints, i + 1)?, at(ints, i + 2)?);
                        i += 3;
                        let loc = match prog.and_then(|p| self.objs.get(&p)) {
                            Some(Obj::Program { locs, .. }) => locs.get(li).copied().unwrap_or(-1),
                            _ => return Err("gpu-gl: uniform without a program".into()),
                        };
                        if kind == 7 {
                            if loc >= 0 {
                                (gl.Uniform1i)(loc, at(ints, i)? as GLint);
                            }
                            i += count as usize;
                            continue;
                        }
                        let sz = SIZES.get(kind as usize).copied().ok_or_else(|| format!("gpu-gl: uniform kind {}", kind))? * count as usize;
                        let v = fl(f, sz)?;
                        f += sz;
                        if loc < 0 {
                            continue;
                        }
                        let (c, p) = (count as GLsizei, v.as_ptr());
                        match kind {
                            6 => (gl.UniformMatrix4fv)(loc, c, 0, p),
                            5 => (gl.UniformMatrix3fv)(loc, c, 0, p),
                            4 => (gl.Uniform4fv)(loc, c, p),
                            3 => (gl.Uniform3fv)(loc, c, p),
                            2 => (gl.Uniform2fv)(loc, c, p),
                            _ => (gl.Uniform1fv)(loc, c, p),
                        }
                    }
                    7 => {
                        // STATE
                        let s = ints.get(i..i + 14).ok_or("gpu-gl: state command cut")?;
                        let v = fl(f, 6)?;
                        i += 14;
                        f += 6;
                        if s[0] != 0 {
                            (gl.Enable)(BLEND);
                            (gl.BlendEquationSeparate)(s[1] as GLenum, s[2] as GLenum);
                            (gl.BlendFuncSeparate)(s[3] as GLenum, s[4] as GLenum, s[5] as GLenum, s[6] as GLenum);
                            (gl.BlendColor)(v[0], v[1], v[2], v[3]);
                        } else {
                            (gl.Disable)(BLEND);
                        }
                        if s[7] != 0 {
                            (gl.Enable)(DEPTH_TEST);
                            (gl.DepthFunc)(s[8] as GLenum);
                        } else {
                            (gl.Disable)(DEPTH_TEST);
                        }
                        (gl.DepthMask)((s[9] == 1) as GLboolean);
                        if s[10] == 0 {
                            (gl.Disable)(CULL_FACE);
                        } else {
                            (gl.Enable)(CULL_FACE);
                            (gl.CullFace)(if s[10] == 2 { FRONT } else { BACK });
                        }
                        (gl.FrontFace)(if s[11] != 0 { CCW } else { CW });
                        let cw = (s[12] == 1) as GLboolean;
                        (gl.ColorMask)(cw, cw, cw, cw);
                        if s[13] != 0 {
                            (gl.Enable)(POLYGON_OFFSET_FILL);
                            (gl.PolygonOffset)(v[4], v[5]);
                        } else {
                            (gl.Disable)(POLYGON_OFFSET_FILL);
                        }
                    }
                    8 => {
                        // ATTRIB location, buffer, size, stride, offset, divisor
                        let a = ints.get(i..i + 6).ok_or("gpu-gl: attrib command cut")?;
                        i += 6;
                        let al = a[0] as GLuint;
                        if a[1] == 0 {
                            let v = fl(f, 4)?;
                            f += 4;
                            (gl.DisableVertexAttribArray)(al);
                            (gl.VertexAttrib4f)(al, v[0], v[1], v[2], v[3]);
                        } else {
                            match self.objs.get(&a[1]) {
                                Some(Obj::Buffer { buf, .. }) => (gl.BindBuffer)(ARRAY_BUFFER, *buf),
                                _ => return Err(format!("gpu-gl: id {} is not a buffer", a[1])),
                            }
                            (gl.EnableVertexAttribArray)(al);
                            (gl.VertexAttribPointer)(al, a[2] as GLint, FLOAT, 0, a[3] as GLsizei, a[4] as usize as *const c_void);
                            (gl.VertexAttribDivisor)(al, a[5] as GLuint);
                        }
                    }
                    9 => {
                        // DRAW mode, first, count, instances, index buffer
                        let a = ints.get(i..i + 5).ok_or("gpu-gl: draw command cut")?;
                        i += 5;
                        let (mode, first, dc, inst, ib) = (a[0] as GLenum, a[1], a[2] as GLsizei, a[3] as GLsizei, a[4]);
                        if ib != 0 {
                            match self.objs.get(&ib) {
                                Some(Obj::Buffer { buf, .. }) => (gl.BindBuffer)(ELEMENT_ARRAY_BUFFER, *buf),
                                _ => return Err(format!("gpu-gl: id {} is not a buffer", ib)),
                            }
                            let off = (first as usize * 4) as *const c_void;
                            if inst > 0 {
                                (gl.DrawElementsInstanced)(mode, dc, UNSIGNED_INT, off, inst);
                            } else {
                                (gl.DrawElements)(mode, dc, UNSIGNED_INT, off);
                            }
                        } else if inst > 0 {
                            (gl.DrawArraysInstanced)(mode, first as GLint, dc, inst);
                        } else {
                            (gl.DrawArrays)(mode, first as GLint, dc);
                        }
                    }
                    10 => {
                        // BIND unit, texture, cube
                        let a = ints.get(i..i + 3).ok_or("gpu-gl: bind command cut")?;
                        i += 3;
                        let t = match self.objs.get(&a[1]) {
                            Some(Obj::Texture { tex }) => *tex,
                            _ if a[1] == 0 => 0,
                            _ => return Err(format!("gpu-gl: id {} is not a texture", a[1])),
                        };
                        (gl.ActiveTexture)(TEXTURE0 + a[0] as GLenum);
                        (gl.BindTexture)(if a[2] != 0 { TEXTURE_CUBE_MAP } else { TEXTURE_2D }, t);
                        (gl.ActiveTexture)(TEXTURE0);
                    }
                    11 => {
                        // RELEASE id
                        let id = at(ints, i)?;
                        i += 1;
                        if let Some(o) = self.objs.remove(&id) {
                            delete(gl, o);
                        }
                    }
                    12 => {
                        // TARGET id, w, h, samples, colour kind, colour, depth kind, depth
                        let a = ints.get(i..i + 8).ok_or("gpu-gl: target command cut")?;
                        i += 8;
                        if let Some(o) = self.objs.remove(&a[0]) {
                            delete(gl, o);
                        }
                        let (w, h, samples) = (a[1] as GLsizei, a[2] as GLsizei, a[3] as GLsizei);
                        let mut fb = 0;
                        let mut rbs = Vec::new();
                        (gl.GenFramebuffers)(1, &mut fb);
                        (gl.BindFramebuffer)(FRAMEBUFFER, fb);
                        for at in 0..2 {
                            let (kind, v) = (a[4 + at * 2], a[5 + at * 2]);
                            let point = if at == 0 {
                                COLOR_ATTACHMENT0
                            } else if kind == 2 && (v as GLenum == DEPTH24_STENCIL8 || v as GLenum == DEPTH32F_STENCIL8) {
                                DEPTH_STENCIL_ATTACHMENT
                            } else {
                                DEPTH_ATTACHMENT
                            };
                            if kind == 1 || kind >= 3 {
                                let tex = match self.objs.get(&v) {
                                    Some(Obj::Texture { tex }) => *tex,
                                    _ => return Err(format!("gpu-gl: id {} is not a texture", v)),
                                };
                                let face = if kind >= 3 { TEXTURE_CUBE_MAP_POSITIVE_X + (kind - 3) as GLenum } else { TEXTURE_2D };
                                (gl.FramebufferTexture2D)(FRAMEBUFFER, point, face, tex, 0);
                            } else if kind == 2 {
                                let mut rb = 0;
                                (gl.GenRenderbuffers)(1, &mut rb);
                                (gl.BindRenderbuffer)(RENDERBUFFER, rb);
                                if samples > 0 {
                                    (gl.RenderbufferStorageMultisample)(RENDERBUFFER, samples, v as GLenum, w, h);
                                } else {
                                    (gl.RenderbufferStorage)(RENDERBUFFER, v as GLenum, w, h);
                                }
                                (gl.FramebufferRenderbuffer)(FRAMEBUFFER, point, RENDERBUFFER, rb);
                                rbs.push(rb);
                            }
                        }
                        if a[4] == 0 {
                            // depth only: GL 3.3 counts a draw buffer without an image as incomplete
                            (gl.DrawBuffers)(1, &NONE);
                            (gl.ReadBuffer)(NONE);
                        }
                        let status = (gl.CheckFramebufferStatus)(FRAMEBUFFER);
                        (gl.BindFramebuffer)(FRAMEBUFFER, self.screen_fb);
                        self.objs.insert(a[0], Obj::Target { fb, rbs });
                        if status != FRAMEBUFFER_COMPLETE {
                            return Err(format!("gpu-gl: target {} is incomplete (0x{:x})", a[0], status));
                        }
                    }
                    14 => {
                        // MIPMAPS texture
                        let id = at(ints, i)?;
                        i += 1;
                        let tex = match self.objs.get(&id) {
                            Some(Obj::Texture { tex }) => *tex,
                            _ => return Err(format!("gpu-gl: id {} is not a texture", id)),
                        };
                        (gl.ActiveTexture)(TEXTURE0 + 31);
                        (gl.BindTexture)(TEXTURE_2D, tex);
                        (gl.GenerateMipmap)(TEXTURE_2D);
                        (gl.BindTexture)(TEXTURE_2D, 0);
                        (gl.ActiveTexture)(TEXTURE0);
                    }
                    13 => {
                        // COPY source, destination, sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask, filter
                        let a = ints.get(i..i + 12).ok_or("gpu-gl: copy command cut")?;
                        i += 12;
                        let (src, dst) = (self.framebuffer(a[0])?, self.framebuffer(a[1])?);
                        (gl.BindFramebuffer)(READ_FRAMEBUFFER, src);
                        (gl.BindFramebuffer)(DRAW_FRAMEBUFFER, dst);
                        let r: Vec<GLint> = a[2..10].iter().map(|&v| v as GLint).collect();
                        (gl.BlitFramebuffer)(r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], a[10] as GLbitfield, a[11] as GLenum);
                        (gl.BindFramebuffer)(FRAMEBUFFER, self.screen_fb);
                    }
                    _ => return Err(format!("gpu-gl: unknown command {} at {}", op, i - 1)),
                }
            }
        }
        if f != floats.len() {
            return Err(format!("gpu-gl: {} floats left over", floats.len() - f));
        }
        Ok(())
    }

    /// A texture's level 0 as RGBA floats, rows bottom first (for tests and tools).
    pub fn read_texture(&self, id: i64, w: i32, h: i32) -> Result<Vec<f32>, String> {
        let tex = match self.objs.get(&id) {
            Some(Obj::Texture { tex }) => *tex,
            _ => return Err(format!("gpu-gl: id {} is not a texture", id)),
        };
        let gl = &self.gl;
        let mut px = vec![0f32; (w * h * 4) as usize];
        // SAFETY: `px` holds w*h RGBA floats; the framebuffer is made and deleted here.
        unsafe {
            let mut fb = 0;
            (gl.GenFramebuffers)(1, &mut fb);
            (gl.BindFramebuffer)(FRAMEBUFFER, fb);
            (gl.FramebufferTexture2D)(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, tex, 0);
            (gl.ReadPixels)(0, 0, w, h, RGBA, FLOAT, px.as_mut_ptr() as *mut c_void);
            (gl.BindFramebuffer)(FRAMEBUFFER, self.screen_fb);
            (gl.DeleteFramebuffers)(1, &fb);
        }
        Ok(px)
    }

    /// The framebuffer of target `id` (0: the host's).
    fn framebuffer(&self, id: i64) -> Result<GLuint, String> {
        if id == 0 {
            return Ok(self.screen_fb);
        }
        match self.objs.get(&id) {
            Some(Obj::Target { fb, .. }) => Ok(*fb),
            _ => Err(format!("gpu-gl: id {} is not a target", id)),
        }
    }

    /// The crate's shaders are GLSL ES 3.00; desktop core GL takes them as GLSL 3.30.
    fn source(&self, s: &str) -> String {
        if self.desktop && s.starts_with("#version 300 es") {
            format!("#version 330 core{}", &s["#version 300 es".len()..])
        } else {
            s.to_string()
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let gl = &self.gl;
        // SAFETY: the player's context is expected to still be current when it is dropped.
        unsafe {
            for (_, o) in self.objs.drain() {
                delete(gl, o);
            }
        }
    }
}

unsafe fn delete(gl: &Gl, o: Obj) {
    match o {
        Obj::Buffer { buf, .. } => (gl.DeleteBuffers)(1, &buf),
        Obj::Texture { tex } => (gl.DeleteTextures)(1, &tex),
        Obj::Program { prog, .. } => (gl.DeleteProgram)(prog),
        Obj::Target { fb, rbs } => {
            (gl.DeleteFramebuffers)(1, &fb);
            if !rbs.is_empty() {
                (gl.DeleteRenderbuffers)(rbs.len() as GLsizei, rbs.as_ptr());
            }
        }
    }
}

unsafe fn compile(gl: &Gl, kind: GLenum, src: &str) -> Result<GLuint, String> {
    let s = (gl.CreateShader)(kind);
    let p = src.as_ptr();
    let len = src.len() as GLint;
    (gl.ShaderSource)(s, 1, &p, &len);
    (gl.CompileShader)(s);
    let mut ok = 0;
    (gl.GetShaderiv)(s, COMPILE_STATUS, &mut ok);
    if ok == 0 {
        let mut n = 0;
        (gl.GetShaderiv)(s, INFO_LOG_LENGTH, &mut n);
        let mut log = vec![0u8; n.max(1) as usize];
        (gl.GetShaderInfoLog)(s, n, std::ptr::null_mut(), log.as_mut_ptr());
        (gl.DeleteShader)(s);
        return Err(format!("gpu-gl shader: {}", String::from_utf8_lossy(&log).trim_end_matches('\0')));
    }
    Ok(s)
}

unsafe fn program_log(gl: &Gl, p: GLuint) -> String {
    let mut n = 0;
    (gl.GetProgramiv)(p, INFO_LOG_LENGTH, &mut n);
    let mut log = vec![0u8; n.max(1) as usize];
    (gl.GetProgramInfoLog)(p, n, std::ptr::null_mut(), log.as_mut_ptr());
    String::from_utf8_lossy(&log).trim_end_matches('\0').to_string()
}

/// Reads the bound framebuffer as RGBA8, bottom row first (as glReadPixels gives it).
pub fn read_rgba(gl: &Gl, w: i32, h: i32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    // SAFETY: `px` holds w*h RGBA8 pixels and PACK_ALIGNMENT 4 fits RGBA8 rows.
    unsafe {
        (gl.PixelStorei)(PACK_ALIGNMENT, 4);
        (gl.ReadPixels)(0, 0, w, h, RGBA, UNSIGNED_BYTE, px.as_mut_ptr() as *mut c_void);
    }
    px
}
