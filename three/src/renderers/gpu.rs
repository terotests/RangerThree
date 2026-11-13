//! The GPU command stream: what a GPU backend runs for one frame, written by
//! the crate (gpu_frame.rs) and played by a small executor per target:
//! js/gpu-webgl2.js (WebGL2), gpu-gl/ (OpenGL ES 3 / GL 3.3 in Rust, for
//! SDL3, Android and iOS hosts), later WebGPU and Metal. The executor keeps
//! the GPU objects and the state cache; every decision (programs, passes,
//! texture units, uniform values, draw order) is made here.
//!
//! A stream is four lists that are read in step:
//!
//! - `ints`: each command is an opcode followed by its integer arguments;
//! - `floats`: the float data of the commands, in command order;
//! - `texts`: shader sources and uniform names, referred to by index;
//! - `bytes`: 8-bit texel data, referred to by offset and length.
//!
//! A host sends the lists as they are (`Int32Array`, `Float32Array`, strings,
//! `Uint8Array`): in one process the executor reads them directly, across a
//! boundary (CErXes `__hostSync`, a worker, another process) they travel as
//! four typed blocks with no per-call encoding.
//!
//! Resource ids are chosen by the writer (from 1), so making an object needs
//! no answer from the executor. GL enum values are used for formats, blend
//! factors and depth functions; a non-GL executor maps them.
//!
//! Commands (ints after the opcode; F = floats taken, in order):
//!
//! | op | name | ints | floats |
//! |---|---|---|---|
//! | 1 | BUFFER | id, kind (0 f32 vertex data, 1 u32 index), dynamic, n | kind 0: n values (kind 1: n ints follow in `ints`) |
//! | 2 | PROGRAM | id, vs text, fs text, uniform count, then that many name texts | |
//! | 3 | TEXTURE | id, cube, internal format, format, type, w, h, min filter, mag filter, wrap s, wrap t, compare (0/1), data (0 none, 1 floats, 2 bytes), offset, n | data 1: n values |
//! | 4 | PASS | target (0 the screen), w, h, clear (bit 1 colour, 2 depth) | r, g, b, a, depth |
//! | 5 | USE | program id | |
//! | 6 | UNIFORM | location (index in the program's names), kind, count | kind 1 f, 2 vec2, 3 vec3, 4 vec4, 5 mat3, 6 mat4: count values each that size; kind 7 int: count ints follow in `ints` |
//! | 7 | STATE | blend (0 off, 1 on), eq rgb, eq alpha, src rgb, dst rgb, src alpha, dst alpha, depth test, depth func, depth write, cull (0 none, 1 back, 2 front), front ccw, colour write, polygon offset | blend r, g, b, a, offset factor, offset units |
//! | 8 | ATTRIB | location, buffer (0: constant), size, stride, offset, divisor | buffer 0: four values |
//! | 9 | DRAW | mode (GL), first, count, instances (0 none), index buffer (0 none) | |
//! | 10 | BIND | unit, texture id (0 none), cube | |
//! | 11 | RELEASE | id | |
//!
//! The stream's shape follows WebGPU where that costs nothing (ids from the
//! writer, programs made once and used by id, state set as one block, passes
//! explicit). Uniforms are still set one by one by name, as the GLSL ES 3.00
//! programs declare them; uniform blocks come with the shader modularisation's
//! stage 4, and then a UNIFORM BLOCK command maps to WebGPU bind groups.

use ranger::prelude::*;

pub const OP_BUFFER: i64 = 1;
pub const OP_PROGRAM: i64 = 2;
pub const OP_TEXTURE: i64 = 3;
pub const OP_PASS: i64 = 4;
pub const OP_USE: i64 = 5;
pub const OP_UNIFORM: i64 = 6;
pub const OP_STATE: i64 = 7;
pub const OP_ATTRIB: i64 = 8;
pub const OP_DRAW: i64 = 9;
pub const OP_BIND: i64 = 10;
pub const OP_RELEASE: i64 = 11;

pub const U_FLOAT: i64 = 1;
pub const U_VEC2: i64 = 2;
pub const U_VEC3: i64 = 3;
pub const U_VEC4: i64 = 4;
pub const U_MAT3: i64 = 5;
pub const U_MAT4: i64 = 6;
pub const U_INT: i64 = 7;

/// One frame's commands (or any run of them).
#[derive(Clone)]
pub struct GpuCommands {
    pub ints: Vec<i64>,
    pub floats: Vec<f64>,
    pub texts: Vec<String>,
    pub bytes: Vec<u8>,
    /// commands written (for stats and tests)
    pub count: i64,
}

impl Default for GpuCommands {
    fn default() -> GpuCommands {
        GpuCommands::new()
    }
}

impl GpuCommands {
    pub fn new() -> GpuCommands {
        GpuCommands { ints: Vec::new(), floats: Vec::new(), texts: Vec::new(), bytes: Vec::new(), count: 0 }
    }

    /// Empties the lists for the next frame.
    pub fn clear(&mut self) {
        self.ints = Vec::new();
        self.floats = Vec::new();
        self.texts = Vec::new();
        self.bytes = Vec::new();
        self.count = 0;
    }

    fn op(&mut self, code: i64) {
        self.ints.push(code);
        self.count += 1;
    }

    fn text(&mut self, s: &str) -> i64 {
        self.texts.push(s.to_string());
        (self.texts.len() as i64) - 1
    }

    /// Vertex data (`index` false) or a u32 index list.
    pub fn buffer(&mut self, id: i64, index: bool, dynamic: bool, data: &Vec<f64>) {
        self.op(OP_BUFFER);
        self.ints.push(id);
        self.ints.push(if index { 1 } else { 0 });
        self.ints.push(if dynamic { 1 } else { 0 });
        self.ints.push(data.len() as i64);
        if index {
            for v in data.iter() {
                self.ints.push(*v as i64);
            }
        } else {
            for v in data.iter() {
                self.floats.push(*v);
            }
        }
    }

    /// A program from GLSL ES 3.00 sources; `names` are its uniforms, which
    /// UNIFORM commands then name by position.
    pub fn program(&mut self, id: i64, vs: &str, fs: &str, names: &Vec<String>) {
        let v = self.text(vs);
        let f = self.text(fs);
        self.op(OP_PROGRAM);
        self.ints.push(id);
        self.ints.push(v);
        self.ints.push(f);
        self.ints.push(names.len() as i64);
        for n in names.iter() {
            let t = self.text(n);
            self.ints.push(t);
        }
    }

    /// A texture; `params` = cube, internal format, format, type, w, h, min
    /// filter, mag filter, wrap s, wrap t, compare. `data` float texels (may be
    /// empty: storage only).
    pub fn texture(&mut self, id: i64, params: &Vec<i64>, data: &Vec<f64>) {
        self.op(OP_TEXTURE);
        self.ints.push(id);
        for p in params.iter() {
            self.ints.push(*p);
        }
        if data.is_empty() {
            self.ints.push(0);
            self.ints.push(0);
            self.ints.push(0);
        } else {
            self.ints.push(1);
            self.ints.push(0);
            self.ints.push(data.len() as i64);
            for v in data.iter() {
                self.floats.push(*v);
            }
        }
    }

    /// A texture from 8-bit texels (`params` as for `texture`).
    pub fn texture_bytes(&mut self, id: i64, params: &Vec<i64>, data: &Vec<u8>) {
        self.op(OP_TEXTURE);
        self.ints.push(id);
        for p in params.iter() {
            self.ints.push(*p);
        }
        self.ints.push(2);
        self.ints.push(self.bytes.len() as i64);
        self.ints.push(data.len() as i64);
        for b in data.iter() {
            self.bytes.push(*b);
        }
    }

    /// Starts drawing into `target` (0 the screen) of `w` x `h`, clearing
    /// colour (`clear` bit 1) to `rgba` and depth (bit 2) to `depth`.
    pub fn pass(&mut self, target: i64, w: i64, h: i64, clear: i64, rgba: &Vec<f64>, depth: f64) {
        self.op(OP_PASS);
        self.ints.push(target);
        self.ints.push(w);
        self.ints.push(h);
        self.ints.push(clear);
        for i in 0..4 {
            self.floats.push(rgba[i]);
        }
        self.floats.push(depth);
    }

    pub fn use_program(&mut self, id: i64) {
        self.op(OP_USE);
        self.ints.push(id);
    }

    /// `count` values of `kind` (U_FLOAT..U_MAT4) for the uniform at
    /// `location` in the program's names.
    pub fn uniform(&mut self, location: i64, kind: i64, values: &Vec<f64>) {
        let size = if kind == U_VEC2 {
            2
        } else if kind == U_VEC3 {
            3
        } else if kind == U_VEC4 {
            4
        } else if kind == U_MAT3 {
            9
        } else if kind == U_MAT4 {
            16
        } else {
            1
        };
        self.op(OP_UNIFORM);
        self.ints.push(location);
        self.ints.push(kind);
        self.ints.push((values.len() as i64) / size);
        for v in values.iter() {
            self.floats.push(*v);
        }
    }

    pub fn uniform_int(&mut self, location: i64, v: i64) {
        self.op(OP_UNIFORM);
        self.ints.push(location);
        self.ints.push(U_INT);
        self.ints.push(1);
        self.ints.push(v);
    }

    /// The fixed-function state as one block: `s` = blend, eq rgb, eq alpha,
    /// src rgb, dst rgb, src alpha, dst alpha, depth test, depth func, depth
    /// write, cull, front ccw, colour write, polygon offset; `f` = blend
    /// colour rgba, offset factor, offset units.
    pub fn state(&mut self, s: &Vec<i64>, f: &Vec<f64>) {
        self.op(OP_STATE);
        for v in s.iter() {
            self.ints.push(*v);
        }
        for v in f.iter() {
            self.floats.push(*v);
        }
    }

    /// Vertex attribute `location` from `buffer`, or the constant `value`
    /// when `buffer` is 0.
    pub fn attrib(&mut self, location: i64, buffer: i64, size: i64, stride: i64, offset: i64, divisor: i64, value: &Vec<f64>) {
        self.op(OP_ATTRIB);
        self.ints.push(location);
        self.ints.push(buffer);
        self.ints.push(size);
        self.ints.push(stride);
        self.ints.push(offset);
        self.ints.push(divisor);
        if buffer == 0 {
            for i in 0..4 {
                self.floats.push(value[i]);
            }
        }
    }

    pub fn draw(&mut self, mode: i64, first: i64, count: i64, instances: i64, index_buffer: i64) {
        self.op(OP_DRAW);
        self.ints.push(mode);
        self.ints.push(first);
        self.ints.push(count);
        self.ints.push(instances);
        self.ints.push(index_buffer);
    }

    pub fn bind(&mut self, unit: i64, texture: i64, cube: bool) {
        self.op(OP_BIND);
        self.ints.push(unit);
        self.ints.push(texture);
        self.ints.push(if cube { 1 } else { 0 });
    }

    pub fn release(&mut self, id: i64) {
        self.op(OP_RELEASE);
        self.ints.push(id);
    }
}

/// The number of ints each opcode takes after itself, before its variable
/// part (the decoders' table; -1 for an unknown opcode).
pub fn fixed_ints(op: i64) -> i64 {
    if op == OP_BUFFER {
        4
    } else if op == OP_PROGRAM {
        4
    } else if op == OP_TEXTURE {
        15
    } else if op == OP_PASS {
        4
    } else if op == OP_USE {
        1
    } else if op == OP_UNIFORM {
        3
    } else if op == OP_STATE {
        14
    } else if op == OP_ATTRIB {
        6
    } else if op == OP_DRAW {
        5
    } else if op == OP_BIND {
        3
    } else if op == OP_RELEASE {
        1
    } else {
        -1
    }
}

/// Walks `c` as a decoder does and returns the number of commands, or -1
/// when the lists do not line up (an encoder bug).
pub fn check_stream(c: &GpuCommands) -> i64 {
    let mut i: usize = 0;
    let mut f: usize = 0;
    let mut n: i64 = 0;
    let len = c.ints.len();
    while i < len {
        let op = c.ints[i];
        let fixed = fixed_ints(op);
        if fixed < 0 || i + 1 + (fixed as usize) > len {
            return -1;
        }
        let a = i + 1;
        let mut next = a + (fixed as usize);
        if op == OP_BUFFER {
            let count = c.ints[a + 3] as usize;
            if c.ints[a + 1] == 1 {
                next += count;
            } else {
                f += count;
            }
        } else if op == OP_PROGRAM {
            next += c.ints[a + 3] as usize;
        } else if op == OP_TEXTURE {
            if c.ints[a + 12] == 1 {
                f += c.ints[a + 14] as usize;
            }
        } else if op == OP_PASS {
            f += 5;
        } else if op == OP_UNIFORM {
            let kind = c.ints[a + 1];
            let count = c.ints[a + 2] as usize;
            if kind == U_INT {
                next += count;
            } else {
                let size: usize = if kind == U_VEC2 {
                    2
                } else if kind == U_VEC3 {
                    3
                } else if kind == U_VEC4 {
                    4
                } else if kind == U_MAT3 {
                    9
                } else if kind == U_MAT4 {
                    16
                } else {
                    1
                };
                f += count * size;
            }
        } else if op == OP_STATE {
            f += 6;
        } else if op == OP_ATTRIB {
            if c.ints[a + 1] == 0 {
                f += 4;
            }
        }
        i = next;
        n += 1;
    }
    if i != len || f != c.floats.len() {
        return -1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_lines_up() {
        let mut c = GpuCommands::new();
        c.buffer(1, false, false, &vec![0.0, 1.0, 2.0]);
        c.buffer(2, true, false, &vec![0.0, 1.0, 2.0]);
        c.program(3, "vs", "fs", &vec![String::from("u_a"), String::from("u_b")]);
        c.texture(4, &vec![0, 33327, 33319, 5126, 2, 2, 9729, 9729, 33071, 33071, 0], &vec![0.5; 8]);
        c.texture_bytes(5, &vec![0, 32856, 6408, 5121, 1, 1, 9728, 9728, 33071, 33071, 0], &vec![1, 2, 3, 4]);
        c.pass(0, 4, 4, 3, &vec![0.0, 0.0, 0.0, 1.0], 1.0);
        c.use_program(3);
        c.uniform(0, U_MAT4, &vec![1.0; 32]);
        c.uniform_int(1, 7);
        c.state(&vec![0, 32774, 32774, 1, 0, 1, 0, 1, 515, 1, 1, 1, 1, 0], &vec![0.0; 6]);
        c.attrib(0, 1, 3, 12, 0, 0, &vec![0.0; 4]);
        c.attrib(1, 0, 3, 0, 0, 0, &vec![0.0, 0.0, 1.0, 0.0]);
        c.bind(2, 4, false);
        c.draw(4, 0, 3, 0, 2);
        c.release(1);
        assert_eq!(check_stream(&c), 15);
        assert_eq!(c.count, 15);
        assert_eq!(c.texts.len(), 4);
        assert_eq!(c.bytes.len(), 4);
        c.floats.push(0.0);
        assert_eq!(check_stream(&c), -1);
    }
}
