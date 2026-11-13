//! GLSL ES 3.00 to WGSL for the shaders this crate writes (shaders.rs), after
//! glsl_pre.rs has resolved their conditionals and macros. A program's two
//! stages are translated together, so they share one layout:
//!
//! - group 0 binding 0: the `Frame` uniform block (std140 offsets as in GLSL)
//!   followed by `u_target` (width, height, y scale, 0), which only WGSL
//!   reads: WebGPU's framebuffer y runs down and its clip z from 0, so the
//!   vertex stage maps z to 0..1 and scales y by `u_target.z` (-1 when
//!   drawing into a texture, so its rows keep GL's order), and
//!   `gl_FragCoord.y` is counted from the bottom as in GL;
//! - binding 1: `Draw`, the program's other non-texture uniforms;
//! - from binding 2: each texture, then its sampler.
//!
//! `translate` gives [vertex WGSL, fragment WGSL, layout]: the layout lists
//! one item per line (`draw name type offset`, `draw_size n`, `tex name type
//! binding stages` (1 vertex, 2 fragment, 3 both), `attr name type location`, `varying name location`, `note
//! text`), offsets in floats. What WebGPU lacks (point size and
//! gl_PointCoord) is left out and named in a `note`.

use ranger::prelude::*;

const K_IDENT: i64 = 0;
const K_NUM: i64 = 1;
const K_PUNCT: i64 = 2;

fn is_ident_start(c: u8) -> bool {
    (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c == 95
}

fn is_ident_byte(c: u8) -> bool {
    is_ident_start(c) || (c >= 48 && c <= 57)
}

fn has(list: &Vec<String>, s: &str) -> bool {
    for x in list.iter() {
        if x == s {
            return true;
        }
    }
    false
}

fn index_of(list: &Vec<String>, s: &str) -> i64 {
    for i in 0..list.len() {
        if list[i] == s {
            return i as i64;
        }
    }
    -1
}

fn byte_len(s: &str) -> usize {
    s.as_bytes().len()
}

/// Words WGSL reserves that a GLSL name may use; such a name gets a `_r` suffix.
fn reserved() -> Vec<String> {
    let words = "alias break case const const_assert continue continuing default diagnostic discard else enable false fn for if let loop override requires return struct switch true var while \
alignas alignof and asm async attribute auto await become binding_array cast catch class coherent column_major common compile concept consteval constexpr constinit crate debugger decltype delete demote do enum explicit export extends extern external filter final finally friend from fxgroup get goto groupshared impl implements import inline instanceof interface macro match meta mod module move mut mutable namespace new nil noexcept noinline nointerpolation noperspective null nullptr of operator package packoffset partition pass patch pixelfragment precise premerge priv protected pub public readonly ref regardless register require resource restrict self set shared sizeof snorm static std subroutine super target template this throw trait try type typedef typeid typename typeof union unless unorm unsafe unsized use using varying virtual volatile wgsl where with writeonly yield \
f32 f16 i32 u32 bool vec2 vec3 vec4 mat2x2 mat3x3 mat4x4 array ptr sampler sampler_comparison select bitcast";
    let mut out: Vec<String> = Vec::new();
    for w in words.split_whitespace() {
        out.push(w.to_string());
    }
    out
}

/// Components of a scalar or vector type (0 for anything else).
fn vec_size(t: &str) -> i64 {
    if t == "float" || t == "int" || t == "uint" || t == "bool" {
        return 1;
    }
    let b = t.as_bytes();
    let n = b.len();
    if n >= 4 && (t.starts_with("vec") || t.starts_with("ivec") || t.starts_with("uvec") || t.starts_with("bvec")) {
        return (b[n - 1] - 48) as i64;
    }
    0
}

fn mat_size(t: &str) -> i64 {
    if t == "mat2" || t == "mat3" || t == "mat4" {
        return (t.as_bytes()[3] - 48) as i64;
    }
    0
}

/// The scalar type of a scalar or vector type.
fn base_of(t: &str) -> String {
    if t.starts_with("ivec") || t == "int" {
        String::from("int")
    } else if t.starts_with("uvec") || t == "uint" {
        String::from("uint")
    } else if t.starts_with("bvec") || t == "bool" {
        String::from("bool")
    } else {
        String::from("float")
    }
}

fn vec_type(base: &str, n: i64) -> String {
    if n == 1 {
        return base.to_string();
    }
    let p = if base == "int" {
        "ivec"
    } else if base == "uint" {
        "uvec"
    } else if base == "bool" {
        "bvec"
    } else {
        "vec"
    };
    let mut s = String::from(p);
    s.push_str(&n.to_string());
    s
}

fn is_sampler(t: &str) -> bool {
    t.starts_with("sampler")
}

fn is_type_name(t: &str) -> bool {
    vec_size(t) > 0 || mat_size(t) > 0 || is_sampler(t) || t == "void"
}

/// An array type `T[N]`: its element type, or empty.
fn elem_of(t: &str) -> String {
    if let Some(p) = t.find("[") {
        return t[0..p].to_string();
    }
    String::new()
}

fn array_len(t: &str) -> String {
    if let Some(p) = t.find("[") {
        return t[p + 1..byte_len(t) - 1].to_string();
    }
    String::new()
}

/// The WGSL spelling of a GLSL type.
fn wtype(t: &str) -> String {
    let e = elem_of(t);
    if !e.is_empty() {
        let mut s = String::from("array<");
        s.push_str(&wtype(&e));
        s.push_str(", ");
        s.push_str(&array_len(t));
        s.push_str(">");
        return s;
    }
    if t == "float" {
        return String::from("f32");
    }
    if t == "int" {
        return String::from("i32");
    }
    if t == "uint" {
        return String::from("u32");
    }
    if t == "bool" {
        return String::from("bool");
    }
    let n = vec_size(t);
    if n > 1 {
        let b = base_of(t);
        let mut s = String::from("vec");
        s.push_str(&n.to_string());
        s.push_str(if b == "int" {
            "i"
        } else if b == "uint" {
            "u"
        } else if b == "bool" {
            "<bool>"
        } else {
            "f"
        });
        return s;
    }
    let m = mat_size(t);
    if m > 0 {
        let mut s = String::from("mat");
        s.push_str(&m.to_string());
        s.push_str("x");
        s.push_str(&m.to_string());
        s.push_str("f");
        return s;
    }
    if t == "sampler2D" {
        return String::from("texture_2d<f32>");
    }
    if t == "samplerCube" {
        return String::from("texture_cube<f32>");
    }
    if t == "sampler2DShadow" {
        return String::from("texture_depth_2d");
    }
    if t == "samplerCubeShadow" {
        return String::from("texture_depth_cube");
    }
    t.to_string()
}

fn sampler_wtype(t: &str) -> String {
    if t.ends_with("Shadow") {
        String::from("sampler_comparison")
    } else {
        String::from("sampler")
    }
}

/// Alignment and size in floats of a type in WGSL's uniform address space.
fn align_of(t: &str) -> i64 {
    let e = elem_of(t);
    if !e.is_empty() {
        let a = align_of(&e);
        return if a < 4 { 4 } else { a };
    }
    let n = vec_size(t);
    if n == 1 {
        return 1;
    }
    if n == 2 {
        return 2;
    }
    if n > 2 {
        return 4;
    }
    let m = mat_size(t);
    if m == 2 {
        return 2;
    }
    4
}

fn size_of(t: &str) -> i64 {
    let e = elem_of(t);
    if !e.is_empty() {
        let mut stride = size_of(&e);
        let a = align_of(t);
        stride = (stride + a - 1) / a * a;
        let n: i64 = array_len(t).parse::<i64>().unwrap_or(0);
        return stride * n;
    }
    let n = vec_size(t);
    if n > 0 {
        return n;
    }
    let m = mat_size(t);
    if m == 2 {
        return 4;
    }
    if m == 3 {
        return 12;
    }
    if m == 4 {
        return 16;
    }
    1
}

fn is_swizzle(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 4 {
        return false;
    }
    let sets = ["xyzw", "rgba", "stpq"];
    for set in sets.iter() {
        let sb = set.as_bytes();
        let mut all = true;
        for c in b.iter() {
            let mut found = false;
            for d in sb.iter() {
                if *c == *d {
                    found = true;
                }
            }
            if !found {
                all = false;
            }
        }
        if all {
            return true;
        }
    }
    false
}

/// A swizzle in WGSL's letters (`stpq` is not WGSL).
fn wswizzle(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    for c in b.iter() {
        let ch = *c;
        out.push_str(if ch == 115 {
            "x"
        } else if ch == 116 {
            "y"
        } else if ch == 112 {
            "z"
        } else if ch == 113 {
            "w"
        } else if ch == 120 || ch == 114 {
            if ch == 120 { "x" } else { "r" }
        } else if ch == 121 || ch == 103 {
            if ch == 121 { "y" } else { "g" }
        } else if ch == 122 || ch == 98 {
            if ch == 122 { "z" } else { "b" }
        } else if ch == 119 {
            "w"
        } else {
            "a"
        });
    }
    out
}

fn xyzw(k: usize) -> String {
    let s = "xyzw";
    s[k..k + 1].to_string()
}

fn number_type(t: &str) -> String {
    let b = t.as_bytes();
    let hex = b.len() > 1 && b[0] == 48 && (b[1] == 120 || b[1] == 88);
    if t.ends_with("u") || t.ends_with("U") {
        return String::from("uint");
    }
    if !hex && (t.contains(".") || t.contains("e") || t.contains("E")) {
        return String::from("float");
    }
    String::from("int")
}

/// The translator's state for one program.
pub struct Wgsl {
    toks: Vec<String>,
    kinds: Vec<i64>,
    pos: usize,
    /// 0 vertex, 1 fragment
    stage: i64,
    // locals: name, type, scope starts
    loc_names: Vec<String>,
    loc_types: Vec<String>,
    loc_marks: Vec<usize>,
    // parameters of the function being translated: name, type, WGSL reference
    par_names: Vec<String>,
    par_types: Vec<String>,
    par_refs: Vec<String>,
    // module-scope names: name, type, WGSL reference
    glob_names: Vec<String>,
    glob_types: Vec<String>,
    glob_refs: Vec<String>,
    // functions: name, return type, parameter types and qualifiers (comma-joined)
    fn_names: Vec<String>,
    fn_rets: Vec<String>,
    fn_params: Vec<String>,
    fn_quals: Vec<String>,
    /// the type of the expression just translated
    ty: String,
    reserved: Vec<String>,
    /// names a stage refers to
    used: Vec<String>,
    helpers: Vec<String>,
    notes: Vec<String>,
    // shared by both stages
    pub frame_names: Vec<String>,
    pub frame_types: Vec<String>,
    pub draw_names: Vec<String>,
    pub draw_types: Vec<String>,
    pub tex_names: Vec<String>,
    pub tex_types: Vec<String>,
    // stage inputs and outputs: name, type, location (-1 none given)
    in_names: Vec<String>,
    in_types: Vec<String>,
    in_locs: Vec<i64>,
    out_names: Vec<String>,
    out_types: Vec<String>,
    /// module-scope consts and private variables of the stage
    decls: String,
}

impl Wgsl {
    pub fn new() -> Wgsl {
        Wgsl {
            toks: Vec::new(),
            kinds: Vec::new(),
            pos: 0,
            stage: 0,
            loc_names: Vec::new(),
            loc_types: Vec::new(),
            loc_marks: Vec::new(),
            par_names: Vec::new(),
            par_types: Vec::new(),
            par_refs: Vec::new(),
            glob_names: Vec::new(),
            glob_types: Vec::new(),
            glob_refs: Vec::new(),
            fn_names: Vec::new(),
            fn_rets: Vec::new(),
            fn_params: Vec::new(),
            fn_quals: Vec::new(),
            ty: String::new(),
            reserved: reserved(),
            used: Vec::new(),
            helpers: Vec::new(),
            notes: Vec::new(),
            frame_names: Vec::new(),
            frame_types: Vec::new(),
            draw_names: Vec::new(),
            draw_types: Vec::new(),
            tex_names: Vec::new(),
            tex_types: Vec::new(),
            in_names: Vec::new(),
            in_types: Vec::new(),
            in_locs: Vec::new(),
            out_names: Vec::new(),
            out_types: Vec::new(),
            decls: String::new(),
        }
    }

    fn note(&mut self, s: &str) {
        if !has(&self.notes, s) {
            self.notes.push(s.to_string());
        }
    }

    fn mark_used(&mut self, s: &str) {
        if !has(&self.used, s) {
            self.used.push(s.to_string());
        }
    }

    fn helper(&mut self, s: &str) {
        if !has(&self.helpers, s) {
            self.helpers.push(s.to_string());
        }
    }

    /// A GLSL name as WGSL may spell it.
    fn name(&self, s: &str) -> String {
        if has(&self.reserved, s) {
            let mut r = s.to_string();
            r.push_str("_r");
            return r;
        }
        s.to_string()
    }

    fn lex(&mut self, src: &str) {
        self.toks = Vec::new();
        self.kinds = Vec::new();
        self.pos = 0;
        // line by line: slicing a whole program by byte offsets re-encodes it per slice in JavaScript
        for line in src.split("\n") {
            let t = line.trim().to_string();
            if t.starts_with("#") {
                // a directive (#version / #extension): not WGSL
                continue;
            }
            self.lex_line(line);
        }
    }

    fn lex_line(&mut self, src: &str) {
        let b = src.as_bytes();
        let n = b.len();
        let mut i: usize = 0;
        while i < n {
            let c = b[i];
            if c == 32 || c == 9 || c == 13 {
                i += 1;
                continue;
            }
            if c == 47 && i + 1 < n && b[i + 1] == 47 {
                break;
            }
            let s = i;
            if is_ident_start(c) {
                while i < n && is_ident_byte(b[i]) {
                    i += 1;
                }
                self.toks.push(src[s..i].to_string());
                self.kinds.push(K_IDENT);
                continue;
            }
            if (c >= 48 && c <= 57) || (c == 46 && i + 1 < n && b[i + 1] >= 48 && b[i + 1] <= 57) {
                let hex = c == 48 && i + 1 < n && (b[i + 1] == 120 || b[i + 1] == 88);
                while i < n {
                    let d = b[i];
                    if is_ident_byte(d) || d == 46 {
                        i += 1;
                    } else if (d == 45 || d == 43) && (b[i - 1] == 101 || b[i - 1] == 69) && !hex {
                        i += 1;
                    } else {
                        break;
                    }
                }
                self.toks.push(src[s..i].to_string());
                self.kinds.push(K_NUM);
                continue;
            }
            if i + 2 < n {
                let three = src[i..i + 3].to_string();
                if three == "<<=" || three == ">>=" {
                    self.toks.push(three);
                    self.kinds.push(K_PUNCT);
                    i += 3;
                    continue;
                }
            }
            if i + 1 < n {
                let two = src[i..i + 2].to_string();
                let ops = ["++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "==", "!=", "<=", ">=", "&&", "||", "^^", "<<", ">>"];
                let mut hit = false;
                for o in ops.iter() {
                    if two == *o {
                        hit = true;
                    }
                }
                if hit {
                    self.toks.push(two);
                    self.kinds.push(K_PUNCT);
                    i += 2;
                    continue;
                }
            }
            self.toks.push(src[i..i + 1].to_string());
            self.kinds.push(K_PUNCT);
            i += 1;
        }
    }

    fn peek(&self) -> String {
        if self.pos < self.toks.len() {
            self.toks[self.pos].clone()
        } else {
            String::new()
        }
    }

    fn peek_at(&self, k: usize) -> String {
        if self.pos + k < self.toks.len() {
            self.toks[self.pos + k].clone()
        } else {
            String::new()
        }
    }

    fn next(&mut self) -> String {
        let t = self.peek();
        self.pos += 1;
        t
    }

    fn eat(&mut self, t: &str) -> bool {
        if self.peek() == t {
            self.pos += 1;
            return true;
        }
        false
    }

    fn expect(&mut self, t: &str) {
        if !self.eat(t) {
            let mut s = String::from("expected ");
            s.push_str(t);
            s.push_str(" before ");
            s.push_str(&self.peek());
            self.note(&s);
            self.pos += 1;
        }
    }

    fn skip_precision(&mut self) {
        while self.peek() == "highp" || self.peek() == "mediump" || self.peek() == "lowp" || self.peek() == "flat" || self.peek() == "smooth" {
            self.pos += 1;
        }
    }

    fn push_scope(&mut self) {
        self.loc_marks.push(self.loc_names.len());
    }

    fn pop_scope(&mut self) {
        if let Some(m) = self.loc_marks.pop() {
            while self.loc_names.len() > m {
                self.loc_names.pop();
                self.loc_types.pop();
            }
        }
    }

    fn add_local(&mut self, name: &str, t: &str) {
        self.loc_names.push(name.to_string());
        self.loc_types.push(t.to_string());
    }

    fn add_global(&mut self, name: &str, t: &str, r: &str) {
        self.glob_names.push(name.to_string());
        self.glob_types.push(t.to_string());
        self.glob_refs.push(r.to_string());
    }

    /// An identifier in an expression: its WGSL reference; sets `ty`.
    fn ident(&mut self, s: &str) -> String {
        let mut i = self.loc_names.len() as i64 - 1;
        while i >= 0 {
            if self.loc_names[i as usize] == s {
                self.ty = self.loc_types[i as usize].clone();
                return self.name(s);
            }
            i -= 1;
        }
        let p = index_of(&self.par_names, s);
        if p >= 0 {
            self.ty = self.par_types[p as usize].clone();
            return self.par_refs[p as usize].clone();
        }
        let g = index_of(&self.glob_names, s);
        if g >= 0 {
            self.mark_used(s);
            self.ty = self.glob_types[g as usize].clone();
            return self.glob_refs[g as usize].clone();
        }
        self.mark_used(s);
        if s == "gl_FragCoord" || s == "gl_Position" {
            self.ty = String::from("vec4");
            return s.to_string();
        }
        if s == "gl_FrontFacing" {
            self.ty = String::from("bool");
            return s.to_string();
        }
        if s == "gl_VertexID" || s == "gl_InstanceID" {
            self.ty = String::from("int");
            return s.to_string();
        }
        if s == "gl_PointSize" {
            self.note("point size: WebGPU draws points one pixel wide");
            self.ty = String::from("float");
            return s.to_string();
        }
        if s == "gl_PointCoord" {
            self.note("gl_PointCoord: WebGPU has none, the point's centre is used");
            self.ty = String::from("vec2");
            return String::from("vec2f(0.5)");
        }
        let mut m = String::from("unknown name ");
        m.push_str(s);
        self.note(&m);
        self.ty = String::new();
        self.name(s)
    }

    /// A type at `pos` (with an optional `[N]`), or empty.
    fn parse_type(&mut self) -> String {
        self.skip_precision();
        let t = self.peek();
        if !is_type_name(&t) {
            return String::new();
        }
        self.pos += 1;
        if self.peek() == "[" {
            self.pos += 1;
            let mut n = String::new();
            while self.peek() != "]" && !self.peek().is_empty() {
                n.push_str(&self.next());
            }
            self.pos += 1;
            let mut s = t.clone();
            s.push_str("[");
            s.push_str(&n);
            s.push_str("]");
            return s;
        }
        t
    }

    // ---- expressions ----

    fn expr(&mut self) -> String {
        self.ternary()
    }

    fn ternary(&mut self) -> String {
        let c = self.binary(1);
        if self.peek() != "?" {
            return c;
        }
        self.pos += 1;
        let a = self.expr();
        let ta = self.ty.clone();
        self.expect(":");
        let b = self.ternary();
        let tb = self.ty.clone();
        // both arms of `select` have one type
        let (a2, b2, t) = self.unify(&a, &ta, &b, &tb);
        self.ty = t;
        let mut s = String::from("select(");
        s.push_str(&b2);
        s.push_str(", ");
        s.push_str(&a2);
        s.push_str(", ");
        s.push_str(&c);
        s.push_str(")");
        s
    }

    /// Two operands of one type for `select` / a builtin: a scalar beside a vector is splat.
    fn unify(&mut self, a: &str, ta: &str, b: &str, tb: &str) -> (String, String, String) {
        let na = vec_size(ta);
        let nb = vec_size(tb);
        if na == 1 && nb > 1 {
            return (self.splat(a, tb), b.to_string(), tb.to_string());
        }
        if nb == 1 && na > 1 {
            return (a.to_string(), self.splat(b, ta), ta.to_string());
        }
        (a.to_string(), b.to_string(), ta.to_string())
    }

    fn splat(&mut self, a: &str, t: &str) -> String {
        let mut s = wtype(t);
        s.push_str("(");
        s.push_str(a);
        s.push_str(")");
        s
    }

    fn prec(t: &str) -> i64 {
        if t == "||" {
            1
        } else if t == "^^" {
            2
        } else if t == "&&" {
            3
        } else if t == "|" {
            4
        } else if t == "^" {
            5
        } else if t == "&" {
            6
        } else if t == "==" || t == "!=" {
            7
        } else if t == "<" || t == ">" || t == "<=" || t == ">=" {
            8
        } else if t == "<<" || t == ">>" {
            9
        } else if t == "+" || t == "-" {
            10
        } else if t == "*" || t == "/" || t == "%" {
            11
        } else {
            0
        }
    }

    fn binary(&mut self, min: i64) -> String {
        let mut lhs = self.unary();
        let mut lt = self.ty.clone();
        loop {
            let op = self.peek();
            let p = Wgsl::prec(&op);
            if p == 0 || p < min || self.kinds.get(self.pos).copied().unwrap_or(0) != K_PUNCT {
                break;
            }
            self.pos += 1;
            let rhs = self.binary(p + 1);
            let rt = self.ty.clone();
            let mut r = rhs.clone();
            if op == "<<" || op == ">>" {
                if base_of(&rt) == "int" {
                    r = String::from("u32(");
                    r.push_str(&rhs);
                    r.push_str(")");
                }
            }
            let wop = if op == "^^" { String::from("!=") } else { op.clone() };
            let mut s = String::from("(");
            s.push_str(&lhs);
            s.push_str(" ");
            s.push_str(&wop);
            s.push_str(" ");
            s.push_str(&r);
            s.push_str(")");
            let t = if p <= 3 || p == 7 || p == 8 {
                String::from("bool")
            } else if p == 9 || p == 4 || p == 5 || p == 6 {
                lt.clone()
            } else {
                Wgsl::arith_type(&lt, &rt, &op)
            };
            if (op == "==" || op == "!=") && vec_size(&lt) > 1 {
                let mut w = String::from(if op == "==" { "all(" } else { "any(" });
                w.push_str(&s);
                w.push_str(")");
                s = w;
            }
            lhs = s;
            lt = t;
        }
        self.ty = lt;
        lhs
    }

    fn arith_type(a: &str, b: &str, op: &str) -> String {
        let ma = mat_size(a);
        let mb = mat_size(b);
        if op == "*" && ma > 0 && vec_size(b) > 1 {
            return b.to_string();
        }
        if op == "*" && mb > 0 && vec_size(a) > 1 {
            return a.to_string();
        }
        if ma > 0 {
            return a.to_string();
        }
        if mb > 0 {
            return b.to_string();
        }
        if vec_size(b) > vec_size(a) {
            return b.to_string();
        }
        a.to_string()
    }

    fn unary(&mut self) -> String {
        let t = self.peek();
        if self.kinds.get(self.pos).copied().unwrap_or(0) == K_PUNCT && (t == "-" || t == "+" || t == "!" || t == "~") {
            self.pos += 1;
            let e = self.unary();
            if t == "+" {
                return e;
            }
            let mut s = String::from("(");
            s.push_str(&t);
            s.push_str(&e);
            s.push_str(")");
            return s;
        }
        if t == "++" || t == "--" {
            // pre-increment: only as a statement (`++i;` == `i++;`)
            self.pos += 1;
            let mut e = self.unary();
            e.push_str(&t);
            return e;
        }
        self.postfix()
    }

    fn postfix(&mut self) -> String {
        let mut e = self.primary();
        loop {
            let t = self.peek();
            if t == "." {
                self.pos += 1;
                let f = self.next();
                let cur = self.ty.clone();
                if vec_size(&cur) >= 1 && is_swizzle(&f) {
                    if vec_size(&cur) == 1 {
                        // scalar.x
                        if byte_len(&f) > 1 {
                            e = self.splat(&e, &vec_type(&base_of(&cur), byte_len(&f) as i64));
                        }
                        continue;
                    }
                    e.push_str(".");
                    e.push_str(&wswizzle(&f));
                    self.ty = vec_type(&base_of(&cur), byte_len(&f) as i64);
                } else if f == "length" {
                    let n = array_len(&cur);
                    e = n;
                    self.ty = String::from("int");
                    if self.peek() == "(" {
                        self.pos += 2;
                    }
                } else {
                    e.push_str(".");
                    e.push_str(&f);
                    self.ty = String::new();
                }
                continue;
            }
            if t == "[" {
                self.pos += 1;
                let cur = self.ty.clone();
                let i = self.expr();
                self.expect("]");
                e.push_str("[");
                e.push_str(&i);
                e.push_str("]");
                let el = elem_of(&cur);
                self.ty = if !el.is_empty() {
                    el
                } else if mat_size(&cur) > 0 {
                    vec_type("float", mat_size(&cur))
                } else {
                    base_of(&cur)
                };
                continue;
            }
            if t == "++" || t == "--" {
                self.pos += 1;
                e.push_str(&t);
                continue;
            }
            break;
        }
        e
    }

    fn args(&mut self) -> (Vec<String>, Vec<String>) {
        let mut a: Vec<String> = Vec::new();
        let mut ts: Vec<String> = Vec::new();
        self.expect("(");
        if self.eat(")") {
            return (a, ts);
        }
        loop {
            let e = self.expr();
            a.push(e);
            ts.push(self.ty.clone());
            if self.eat(",") {
                continue;
            }
            self.expect(")");
            break;
        }
        (a, ts)
    }

    fn primary(&mut self) -> String {
        let t = self.next();
        let k = self.kinds.get(self.pos - 1).copied().unwrap_or(K_PUNCT);
        if t == "(" {
            let e = self.expr();
            self.expect(")");
            let mut s = String::from("(");
            s.push_str(&e);
            s.push_str(")");
            return s;
        }
        if k == K_NUM {
            self.ty = number_type(&t);
            let mut s = t.clone();
            if s.ends_with("f") || s.ends_with("F") {
                s = s[0..byte_len(&s) - 1].to_string();
            }
            return s;
        }
        if t == "true" || t == "false" {
            self.ty = String::from("bool");
            return t;
        }
        if k != K_IDENT {
            let mut m = String::from("unexpected ");
            m.push_str(&t);
            self.note(&m);
            self.ty = String::new();
            return t;
        }
        // an array constructor: T[](...) / T[N](...)
        if is_type_name(&t) && self.peek() == "[" {
            self.pos += 1;
            while self.peek() != "]" {
                self.pos += 1;
            }
            self.pos += 1;
            let (a, _ts) = self.args();
            let mut ty = t.clone();
            ty.push_str("[");
            ty.push_str(&a.len().to_string());
            ty.push_str("]");
            let mut s = wtype(&ty);
            s.push_str("(");
            s.push_str(&a.join(", "));
            s.push_str(")");
            self.ty = ty;
            return s;
        }
        if self.peek() == "(" {
            if is_type_name(&t) {
                let (a, ts) = self.args();
                return self.construct(&t, &a, &ts);
            }
            let f = index_of(&self.fn_names, &t);
            if f >= 0 {
                let (a, ts) = self.args();
                return self.call_user(f as usize, &a, &ts);
            }
            let (a, ts) = self.args();
            return self.builtin(&t, &a, &ts);
        }
        self.ident(&t)
    }

    fn construct(&mut self, t: &str, a: &Vec<String>, ts: &Vec<String>) -> String {
        self.ty = t.to_string();
        let n = vec_size(t);
        let w = wtype(t);
        if n == 1 {
            let mut s = w.clone();
            s.push_str("(");
            s.push_str(&a.join(", "));
            s.push_str(")");
            return s;
        }
        if n > 1 {
            let b = base_of(t);
            let mut parts: Vec<String> = Vec::new();
            if a.len() == 1 && vec_size(&ts[0]) > n {
                let mut s = String::from("(");
                s.push_str(&a[0]);
                s.push_str(").");
                s.push_str(&"xyzw"[0..n as usize]);
                parts.push(if base_of(&ts[0]) == b {
                    s
                } else {
                    let mut c = wtype(&vec_type(&b, n));
                    c.push_str("(");
                    c.push_str(&s);
                    c.push_str(")");
                    c
                });
            } else {
                for i in 0..a.len() {
                    let at = ts[i].clone();
                    if base_of(&at) == b || at.is_empty() || (vec_size(&at) == 1 && self.is_literal(&a[i])) {
                        parts.push(a[i].clone());
                    } else {
                        let mut c = wtype(&vec_type(&b, vec_size(&at)));
                        c.push_str("(");
                        c.push_str(&a[i]);
                        c.push_str(")");
                        parts.push(c);
                    }
                }
            }
            let mut s = w.clone();
            s.push_str("(");
            s.push_str(&parts.join(", "));
            s.push_str(")");
            return s;
        }
        let m = mat_size(t);
        if m > 0 && a.len() == 1 {
            let am = mat_size(&ts[0]);
            if am > 0 && am != m {
                let mut h = String::from("mat");
                h.push_str(&m.to_string());
                h.push_str("_from");
                h.push_str(&am.to_string());
                h.push_str("_");
                self.helper(&h);
                h.push_str("(");
                h.push_str(&a[0]);
                h.push_str(")");
                return h;
            }
            if am == 0 {
                let mut h = String::from("mat");
                h.push_str(&m.to_string());
                h.push_str("_diag_");
                self.helper(&h);
                h.push_str("(");
                h.push_str(&a[0]);
                h.push_str(")");
                return h;
            }
        }
        let mut s = w.clone();
        s.push_str("(");
        s.push_str(&a.join(", "));
        s.push_str(")");
        s
    }

    fn is_literal(&self, s: &str) -> bool {
        let b = s.as_bytes();
        if b.is_empty() {
            return false;
        }
        let mut k: usize = 0;
        if b[0] == 40 || b[0] == 45 {
            k = 1;
            while k < b.len() && (b[k] == 40 || b[k] == 45) {
                k += 1;
            }
        }
        k < b.len() && ((b[k] >= 48 && b[k] <= 57) || b[k] == 46) && !s.contains("u")
    }

    fn call_user(&mut self, f: usize, a: &Vec<String>, ts: &Vec<String>) -> String {
        let quals: Vec<String> = self.fn_quals[f].split(",").map(|q| q.to_string()).collect();
        let mut parts: Vec<String> = Vec::new();
        for i in 0..a.len() {
            if is_sampler(&ts[i]) {
                parts.push(a[i].clone());
                let mut s = a[i].clone();
                s.push_str("_s");
                parts.push(s);
                continue;
            }
            let q = if i < quals.len() { quals[i].clone() } else { String::new() };
            if q == "out" || q == "inout" {
                let mut s = String::from("&");
                s.push_str(&a[i]);
                parts.push(s);
            } else {
                parts.push(a[i].clone());
            }
        }
        self.ty = self.fn_rets[f].clone();
        let mut s = self.name(&self.fn_names[f].clone());
        if s == "main" {
            s = String::from("main_");
        }
        s.push_str("(");
        s.push_str(&parts.join(", "));
        s.push_str(")");
        s
    }

    fn call(&self, f: &str, a: &Vec<String>) -> String {
        let mut s = f.to_string();
        s.push_str("(");
        s.push_str(&a.join(", "));
        s.push_str(")");
        s
    }

    fn builtin(&mut self, f: &str, a: &Vec<String>, ts: &Vec<String>) -> String {
        let t0 = if ts.is_empty() { String::new() } else { ts[0].clone() };
        if f == "texture" || f == "textureLod" || f == "textureGrad" || f == "textureSize" || f == "texelFetch" {
            return self.texture(f, a, ts);
        }
        if f == "length" || f == "distance" || f == "dot" {
            self.ty = String::from("float");
            return self.call(f, a);
        }
        if f == "cross" {
            self.ty = String::from("vec3");
            return self.call(f, a);
        }
        if f == "any" || f == "all" {
            self.ty = String::from("bool");
            return self.call(f, a);
        }
        if f == "lessThan" || f == "greaterThan" || f == "lessThanEqual" || f == "greaterThanEqual" || f == "equal" || f == "notEqual" {
            let op = if f == "lessThan" {
                "<"
            } else if f == "greaterThan" {
                ">"
            } else if f == "lessThanEqual" {
                "<="
            } else if f == "greaterThanEqual" {
                ">="
            } else if f == "equal" {
                "=="
            } else {
                "!="
            };
            self.ty = vec_type("bool", vec_size(&t0));
            let mut s = String::from("(");
            s.push_str(&a[0]);
            s.push_str(" ");
            s.push_str(op);
            s.push_str(" ");
            s.push_str(&a[1]);
            s.push_str(")");
            return s;
        }
        if f == "mod" {
            let rt = Wgsl::arith_type(&t0, &ts[1], "-");
            self.ty = rt;
            let mut s = String::from("((");
            s.push_str(&a[0]);
            s.push_str(") - (");
            s.push_str(&a[1]);
            s.push_str(") * floor((");
            s.push_str(&a[0]);
            s.push_str(") / (");
            s.push_str(&a[1]);
            s.push_str(")))");
            return s;
        }
        if f == "atan" && a.len() == 2 {
            self.ty = t0;
            return self.call("atan2", a);
        }
        if f == "inverse" {
            let m = mat_size(&t0);
            let mut h = String::from("inverse");
            h.push_str(&m.to_string());
            h.push_str("_");
            self.helper(&h);
            self.ty = t0;
            return self.call(&h, a);
        }
        if f == "min" || f == "max" || f == "clamp" || f == "mix" || f == "step" || f == "smoothstep" {
            // WGSL wants one type: a scalar among vectors is splat (mix's weight may stay scalar)
            let mut widest = t0.clone();
            for i in 0..ts.len() {
                if f == "mix" && i == 2 {
                    continue;
                }
                if vec_size(&ts[i]) > vec_size(&widest) {
                    widest = ts[i].clone();
                }
            }
            let mut parts: Vec<String> = Vec::new();
            for i in 0..a.len() {
                if vec_size(&widest) > 1 && vec_size(&ts[i]) == 1 && !(f == "mix" && i == 2) {
                    parts.push(self.splat(&a[i], &widest));
                } else {
                    parts.push(a[i].clone());
                }
            }
            self.ty = widest;
            return self.call(f, &parts);
        }
        if f == "dFdy" {
            // GL's window y runs up, WebGPU's framebuffer y down (unless the pass flips y: u_target.z -1)
            self.ty = t0;
            let mut s = String::from("(dpdy(");
            s.push_str(&a[0]);
            s.push_str(if self.frame_names.is_empty() { ") * -1.0)" } else { ") * -frame_.u_target.z)" });
            return s;
        }
        let w = if f == "inversesqrt" {
            "inverseSqrt"
        } else if f == "dFdx" {
            "dpdx"
        } else if f == "dFdy" {
            "dpdy"
        } else if f == "faceforward" {
            "faceForward"
        } else if f == "roundEven" {
            "round"
        } else {
            f
        };
        self.ty = t0;
        self.call(w, a)
    }

    fn texture(&mut self, f: &str, a: &Vec<String>, ts: &Vec<String>) -> String {
        let st = ts[0].clone();
        let tex = a[0].clone();
        let mut smp = tex.clone();
        smp.push_str("_s");
        let shadow = st.ends_with("Shadow");
        if f == "textureSize" {
            self.ty = String::from("ivec2");
            let mut s = String::from("vec2i(textureDimensions(");
            s.push_str(&tex);
            if a.len() > 1 {
                s.push_str(", ");
                s.push_str(&a[1]);
            }
            s.push_str("))");
            return s;
        }
        if f == "texelFetch" {
            self.ty = String::from("vec4");
            let v = vec![tex, a[1].clone(), a[2].clone()];
            return self.call("textureLoad", &v);
        }
        let vertex = self.stage == 0;
        if shadow {
            self.ty = String::from("float");
            let p = a[1].clone();
            let cube = st.starts_with("samplerCube");
            let mut c = String::from("(");
            c.push_str(&p);
            c.push_str(if cube { ").xyz" } else { ").xy" });
            let mut r = String::from("(");
            r.push_str(&p);
            r.push_str(if cube { ").w" } else { ").z" });
            let name = if vertex || f == "textureLod" { "textureSampleCompareLevel" } else { "textureSampleCompare" };
            let v = vec![tex, smp, c, r];
            return self.call(name, &v);
        }
        self.ty = String::from("vec4");
        if f == "textureLod" {
            let v = vec![tex, smp, a[1].clone(), a[2].clone()];
            return self.call("textureSampleLevel", &v);
        }
        if f == "textureGrad" {
            let v = vec![tex, smp, a[1].clone(), a[2].clone(), a[3].clone()];
            return self.call("textureSampleGrad", &v);
        }
        if vertex {
            let v = vec![tex, smp, a[1].clone(), String::from("0.0")];
            return self.call("textureSampleLevel", &v);
        }
        if a.len() > 2 {
            let v = vec![tex, smp, a[1].clone(), a[2].clone()];
            return self.call("textureSampleBias", &v);
        }
        let v = vec![tex, smp, a[1].clone()];
        self.call("textureSample", &v)
    }

    // ---- statements ----

    fn indent(d: usize) -> String {
        let mut s = String::new();
        for _ in 0..d {
            s.push_str("  ");
        }
        s
    }

    fn is_assign_op(t: &str) -> bool {
        t == "=" || t == "+=" || t == "-=" || t == "*=" || t == "/=" || t == "%=" || t == "&=" || t == "|=" || t == "^=" || t == "<<=" || t == ">>="
    }

    /// A declaration or an expression statement, without `;` (for-loop parts too).
    fn simple(&mut self, d: usize) -> String {
        let save = self.pos;
        let is_const = self.eat("const");
        self.skip_precision();
        if is_type_name(&self.peek()) && self.peek_at(1) != "(" && !(self.peek_at(1) == "[" && self.peek_at(2) == "]") {
            return self.declaration(d, is_const);
        }
        if is_type_name(&self.peek()) && self.peek_at(1) == "[" {
            // T[N] name = ...
            let mut k = 2;
            while self.peek_at(k) != "]" && !self.peek_at(k).is_empty() {
                k += 1;
            }
            if self.kinds.get(self.pos + k + 1).copied().unwrap_or(0) == K_IDENT {
                return self.declaration(d, is_const);
            }
        }
        self.pos = save;
        let lhs = self.ternary();
        let lt = self.ty.clone();
        let op = self.peek();
        if !Wgsl::is_assign_op(&op) {
            let mut s = Wgsl::indent(d);
            if !lhs.ends_with("++") && !lhs.ends_with("--") && !lhs.ends_with(")") {
                s.push_str("_ = ");
            }
            s.push_str(&lhs);
            return s;
        }
        self.pos += 1;
        let rhs = self.expr();
        let rt = self.ty.clone();
        let mut r = rhs.clone();
        if (op == "<<=" || op == ">>=") && base_of(&rt) == "int" {
            r = String::from("u32(");
            r.push_str(&rhs);
            r.push_str(")");
        }
        let _ = lt;
        // `v.xy op= e`: WGSL assigns one component at a time
        let mut dot_at: i64 = -1;
        let lb = lhs.as_bytes();
        for k in 0..lb.len() {
            if lb[k] == 46 {
                dot_at = k as i64;
            }
        }
        if dot_at >= 0 {
            let dot = dot_at as usize;
            let swz = lhs[dot + 1..].to_string();
            if byte_len(&swz) > 1 && is_swizzle(&swz) && !swz.contains(")") {
                let base = lhs[0..dot].to_string();
                let mut s = Wgsl::indent(d);
                s.push_str("{ let t_ = ");
                if op == "=" {
                    s.push_str(&r);
                } else {
                    s.push_str("(");
                    s.push_str(&lhs);
                    s.push_str(" ");
                    s.push_str(&op[0..byte_len(&op) - 1]);
                    s.push_str(" ");
                    s.push_str(&r);
                    s.push_str(")");
                }
                s.push_str(";");
                let sb = swz.as_bytes();
                for k in 0..sb.len() {
                    s.push_str(" ");
                    s.push_str(&base);
                    s.push_str(".");
                    s.push_str(&swz[k..k + 1]);
                    s.push_str(" = t_.");
                    s.push_str(&xyzw(k));
                    s.push_str(";");
                }
                s.push_str(" }");
                return s;
            }
        }
        let mut s = Wgsl::indent(d);
        s.push_str(&lhs);
        s.push_str(" ");
        s.push_str(&op);
        s.push_str(" ");
        s.push_str(&r);
        s
    }

    fn declaration(&mut self, d: usize, is_const: bool) -> String {
        let t0 = self.parse_type();
        let mut out = String::new();
        loop {
            let name = self.next();
            let mut t = t0.clone();
            if self.peek() == "[" {
                self.pos += 1;
                let mut n = String::new();
                while self.peek() != "]" && !self.peek().is_empty() {
                    n.push_str(&self.next());
                }
                self.pos += 1;
                t.push_str("[");
                t.push_str(&n);
                t.push_str("]");
            }
            let mut init = String::new();
            if self.eat("=") {
                init = self.expr();
                let it = self.ty.clone();
                // `T[] x = T[](...)`: the size from the initialiser
                if t.ends_with("[]") && !elem_of(&it).is_empty() {
                    t = it.clone();
                }
            }
            if !out.is_empty() {
                out.push_str("\n");
            }
            out.push_str(&Wgsl::indent(d));
            out.push_str(if is_const { "const " } else { "var " });
            out.push_str(&self.name(&name));
            out.push_str(": ");
            out.push_str(&wtype(&t));
            if !init.is_empty() {
                out.push_str(" = ");
                out.push_str(&init);
            }
            self.add_local(&name, &t);
            if self.eat(",") {
                if !out.is_empty() {
                    out.push_str(";");
                }
                continue;
            }
            break;
        }
        out
    }

    /// One statement as WGSL lines at depth `d` (each line ends with a newline).
    fn statement(&mut self, d: usize) -> String {
        let t = self.peek();
        if t == "{" {
            self.pos += 1;
            let mut s = Wgsl::indent(d);
            s.push_str("{\n");
            s.push_str(&self.block_body(d + 1));
            s.push_str(&Wgsl::indent(d));
            s.push_str("}\n");
            return s;
        }
        if t == ";" {
            self.pos += 1;
            return String::new();
        }
        if t == "if" {
            return self.if_statement(d, false);
        }
        if t == "for" {
            self.pos += 1;
            self.expect("(");
            self.push_scope();
            let init = if self.peek() == ";" { String::new() } else { self.simple(0) };
            self.expect(";");
            let cond = if self.peek() == ";" { String::new() } else { self.expr() };
            self.expect(";");
            let upd = if self.peek() == ")" { String::new() } else { self.simple(0) };
            self.expect(")");
            let mut s = Wgsl::indent(d);
            s.push_str("for (");
            s.push_str(&init);
            s.push_str("; ");
            s.push_str(&cond);
            s.push_str("; ");
            s.push_str(&upd);
            s.push_str(") {\n");
            s.push_str(&self.body(d + 1));
            s.push_str(&Wgsl::indent(d));
            s.push_str("}\n");
            self.pop_scope();
            return s;
        }
        if t == "while" {
            self.pos += 1;
            self.expect("(");
            let c = self.expr();
            self.expect(")");
            let mut s = Wgsl::indent(d);
            s.push_str("while (");
            s.push_str(&c);
            s.push_str(") {\n");
            s.push_str(&self.body(d + 1));
            s.push_str(&Wgsl::indent(d));
            s.push_str("}\n");
            return s;
        }
        if t == "do" {
            self.pos += 1;
            let b = self.body(d + 1);
            self.expect("while");
            self.expect("(");
            let c = self.expr();
            self.expect(")");
            self.expect(";");
            let mut s = Wgsl::indent(d);
            s.push_str("loop {\n");
            s.push_str(&b);
            s.push_str(&Wgsl::indent(d + 1));
            s.push_str("continuing { break if !(");
            s.push_str(&c);
            s.push_str("); }\n");
            s.push_str(&Wgsl::indent(d));
            s.push_str("}\n");
            return s;
        }
        if t == "return" {
            self.pos += 1;
            let mut s = Wgsl::indent(d);
            s.push_str("return");
            if self.peek() != ";" {
                s.push_str(" ");
                s.push_str(&self.expr());
            }
            self.expect(";");
            s.push_str(";\n");
            return s;
        }
        if t == "break" || t == "continue" || t == "discard" {
            self.pos += 1;
            self.expect(";");
            let mut s = Wgsl::indent(d);
            s.push_str(&t);
            s.push_str(";\n");
            return s;
        }
        let mut s = self.simple(d);
        self.expect(";");
        s.push_str(";\n");
        // WebGPU has no point size: its writes are dropped
        if s.trim().starts_with("gl_PointSize ") {
            return String::new();
        }
        s
    }

    fn if_statement(&mut self, d: usize, chained: bool) -> String {
        self.pos += 1;
        self.expect("(");
        let c = self.expr();
        self.expect(")");
        let mut s = if chained { String::new() } else { Wgsl::indent(d) };
        s.push_str("if (");
        s.push_str(&c);
        s.push_str(") {\n");
        s.push_str(&self.body(d + 1));
        s.push_str(&Wgsl::indent(d));
        s.push_str("}");
        if self.eat("else") {
            if self.peek() == "if" {
                s.push_str(" else ");
                s.push_str(&self.if_statement(d, true));
                return s;
            }
            s.push_str(" else {\n");
            s.push_str(&self.body(d + 1));
            s.push_str(&Wgsl::indent(d));
            s.push_str("}");
        }
        s.push_str("\n");
        s
    }

    /// The body of an if / loop: a block's statements, or one statement.
    fn body(&mut self, d: usize) -> String {
        if self.eat("{") {
            return self.block_body(d);
        }
        self.push_scope();
        let s = self.statement(d);
        self.pop_scope();
        s
    }

    /// Statements up to the `}` that closes a block (consumed).
    fn block_body(&mut self, d: usize) -> String {
        self.push_scope();
        let mut s = String::new();
        while self.pos < self.toks.len() && self.peek() != "}" {
            s.push_str(&self.statement(d));
        }
        self.pos += 1;
        self.pop_scope();
        s
    }

    // ---- module scope ----

    /// Whether `name` is assigned in the tokens from `from` to `to`.
    fn assigned(&self, name: &str, from: usize, to: usize) -> bool {
        let mut i = from;
        while i < to {
            if self.toks[i] == name && (i == 0 || self.toks[i - 1] != ".") {
                let mut j = i + 1;
                loop {
                    if j + 1 < to && self.toks[j] == "." {
                        j += 2;
                        continue;
                    }
                    if j < to && self.toks[j] == "[" {
                        let mut depth = 0;
                        while j < to {
                            if self.toks[j] == "[" {
                                depth += 1;
                            } else if self.toks[j] == "]" {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            j += 1;
                        }
                        j += 1;
                        continue;
                    }
                    break;
                }
                if j < to && (Wgsl::is_assign_op(&self.toks[j]) || self.toks[j] == "++" || self.toks[j] == "--") {
                    return true;
                }
                if i > 0 && (self.toks[i - 1] == "++" || self.toks[i - 1] == "--") {
                    return true;
                }
            }
            i += 1;
        }
        false
    }

    /// The token index of the `}` matching the `{` at `from`.
    fn block_end(&self, from: usize) -> usize {
        let mut depth = 0;
        let mut i = from;
        while i < self.toks.len() {
            if self.toks[i] == "{" {
                depth += 1;
            } else if self.toks[i] == "}" {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            i += 1;
        }
        self.toks.len()
    }

    fn function(&mut self, ret: &str, name: &str) -> String {
        self.expect("(");
        let mut pnames: Vec<String> = Vec::new();
        let mut ptypes: Vec<String> = Vec::new();
        let mut pquals: Vec<String> = Vec::new();
        while self.peek() != ")" && !self.peek().is_empty() {
            let mut q = String::from("in");
            if self.peek() == "in" || self.peek() == "out" || self.peek() == "inout" || self.peek() == "const" {
                let w = self.next();
                if w != "const" {
                    q = w;
                }
            }
            let t = self.parse_type();
            if t == "void" {
                break;
            }
            let pn = self.next();
            let mut pt = t.clone();
            if self.peek() == "[" {
                self.pos += 1;
                let n = self.next();
                self.pos += 1;
                pt.push_str("[");
                pt.push_str(&n);
                pt.push_str("]");
            }
            pnames.push(pn);
            ptypes.push(pt);
            pquals.push(q);
            if !self.eat(",") {
                break;
            }
        }
        self.expect(")");
        let mut sig_types = String::new();
        for (i, t) in ptypes.iter().enumerate() {
            if i > 0 {
                sig_types.push_str(",");
            }
            sig_types.push_str(t);
        }
        if index_of(&self.fn_names, name) < 0 {
            self.fn_names.push(name.to_string());
            self.fn_rets.push(ret.to_string());
            self.fn_params.push(sig_types);
            self.fn_quals.push(pquals.join(","));
        }
        if self.eat(";") {
            // a prototype
            return String::new();
        }
        let end = self.block_end(self.pos);
        let mut head = String::from("fn ");
        head.push_str(&if name == "main" { String::from("main_") } else { self.name(name) });
        head.push_str("(");
        let mut copies = String::new();
        self.par_names = Vec::new();
        self.par_types = Vec::new();
        self.par_refs = Vec::new();
        for i in 0..pnames.len() {
            if i > 0 {
                head.push_str(", ");
            }
            let pn = pnames[i].clone();
            let pt = ptypes[i].clone();
            let wn = self.name(&pn);
            self.par_names.push(pn.clone());
            self.par_types.push(pt.clone());
            if is_sampler(&pt) {
                head.push_str(&wn);
                head.push_str(": ");
                head.push_str(&wtype(&pt));
                head.push_str(", ");
                head.push_str(&wn);
                head.push_str("_s: ");
                head.push_str(&sampler_wtype(&pt));
                self.par_refs.push(wn);
                continue;
            }
            if pquals[i] == "out" || pquals[i] == "inout" {
                head.push_str(&wn);
                head.push_str(": ptr<function, ");
                head.push_str(&wtype(&pt));
                head.push_str(">");
                let mut r = String::from("(*");
                r.push_str(&wn);
                r.push_str(")");
                self.par_refs.push(r);
                continue;
            }
            if self.assigned(&pn, self.pos, end) {
                // WGSL parameters are constants: a copy the body may change
                let mut arg = wn.clone();
                arg.push_str("_in");
                head.push_str(&arg);
                copies.push_str("  var ");
                copies.push_str(&wn);
                copies.push_str(" = ");
                copies.push_str(&arg);
                copies.push_str(";\n");
            } else {
                head.push_str(&wn);
            }
            head.push_str(": ");
            head.push_str(&wtype(&pt));
            self.par_refs.push(wn);
        }
        head.push_str(")");
        if ret != "void" {
            head.push_str(" -> ");
            head.push_str(&wtype(ret));
        }
        head.push_str(" {\n");
        head.push_str(&copies);
        self.expect("{");
        self.loc_names = Vec::new();
        self.loc_types = Vec::new();
        self.loc_marks = Vec::new();
        head.push_str(&self.block_body(1));
        head.push_str("}\n");
        self.par_names = Vec::new();
        self.par_types = Vec::new();
        self.par_refs = Vec::new();
        head
    }

    fn add_unique(names: &mut Vec<String>, types: &mut Vec<String>, n: &str, t: &str) {
        if index_of(names, n) < 0 {
            names.push(n.to_string());
            types.push(t.to_string());
        }
    }

    /// Translates one stage's module scope; returns its functions.
    fn stage_source(&mut self, src: &str, stage: i64) -> String {
        self.stage = stage;
        self.lex(src);
        self.glob_names = Vec::new();
        self.glob_types = Vec::new();
        self.glob_refs = Vec::new();
        self.fn_names = Vec::new();
        self.fn_rets = Vec::new();
        self.fn_params = Vec::new();
        self.fn_quals = Vec::new();
        self.used = Vec::new();
        self.in_names = Vec::new();
        self.in_types = Vec::new();
        self.in_locs = Vec::new();
        self.out_names = Vec::new();
        self.out_types = Vec::new();
        self.decls = String::new();
        let mut fns = String::new();
        while self.pos < self.toks.len() {
            let t = self.peek();
            if t == "precision" {
                while self.next() != ";" {}
                continue;
            }
            let mut loc: i64 = -1;
            let mut std140 = false;
            if t == "layout" {
                self.pos += 1;
                self.expect("(");
                while self.peek() != ")" && !self.peek().is_empty() {
                    let w = self.next();
                    if w == "location" {
                        self.expect("=");
                        loc = self.next().parse::<i64>().unwrap_or(-1);
                    } else if w == "std140" {
                        std140 = true;
                    }
                }
                self.expect(")");
            }
            let q = self.peek();
            if q == "uniform" && self.kinds.get(self.pos + 2).copied().unwrap_or(0) == K_PUNCT && self.peek_at(2) == "{" {
                // a uniform block (the Frame block)
                self.pos += 3;
                while self.peek() != "}" {
                    let mt = self.parse_type();
                    loop {
                        let mn = self.next();
                        let mut full = mt.clone();
                        if self.peek() == "[" {
                            self.pos += 1;
                            let n = self.next();
                            self.pos += 1;
                            full.push_str("[");
                            full.push_str(&n);
                            full.push_str("]");
                        }
                        Wgsl::add_unique(&mut self.frame_names, &mut self.frame_types, &mn, &full);
                        let mut r = String::from("frame_.");
                        r.push_str(&mn);
                        self.add_global(&mn, &full, &r);
                        if !self.eat(",") {
                            break;
                        }
                    }
                    self.expect(";");
                }
                self.expect("}");
                self.expect(";");
                let _ = std140;
                continue;
            }
            if q == "uniform" || q == "in" || q == "out" {
                self.pos += 1;
                let ty = self.parse_type();
                loop {
                    let n = self.next();
                    let mut full = ty.clone();
                    if self.peek() == "[" {
                        self.pos += 1;
                        let k = self.next();
                        self.pos += 1;
                        full.push_str("[");
                        full.push_str(&k);
                        full.push_str("]");
                    }
                    if q == "uniform" {
                        if is_sampler(&full) {
                            Wgsl::add_unique(&mut self.tex_names, &mut self.tex_types, &n, &full);
                            let r = self.name(&n);
                            self.add_global(&n, &full, &r);
                        } else {
                            Wgsl::add_unique(&mut self.draw_names, &mut self.draw_types, &n, &full);
                            let mut r = String::from("draw_.");
                            r.push_str(&n);
                            self.add_global(&n, &full, &r);
                        }
                    } else if q == "in" {
                        self.in_names.push(n.clone());
                        self.in_types.push(full.clone());
                        self.in_locs.push(loc);
                        let r = self.name(&n);
                        self.add_global(&n, &full, &r);
                    } else {
                        self.out_names.push(n.clone());
                        self.out_types.push(full.clone());
                        let r = self.name(&n);
                        self.add_global(&n, &full, &r);
                    }
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(";");
                continue;
            }
            // a function, or a module-scope const / variable
            let is_const = self.eat("const");
            let ty = self.parse_type();
            if ty.is_empty() {
                let mut m = String::from("unexpected ");
                m.push_str(&self.peek());
                self.note(&m);
                self.pos += 1;
                continue;
            }
            let n = self.next();
            if self.peek() == "(" {
                fns.push_str(&self.function(&ty, &n));
                continue;
            }
            loop {
                let mut full = ty.clone();
                if self.peek() == "[" {
                    self.pos += 1;
                    let k = self.next();
                    self.pos += 1;
                    full.push_str("[");
                    full.push_str(&k);
                    full.push_str("]");
                }
                let mut init = String::new();
                if self.eat("=") {
                    init = self.expr();
                }
                let w = self.name(&n);
                self.decls.push_str(if is_const && elem_of(&full).is_empty() { "const " } else { "var<private> " });
                self.decls.push_str(&w);
                self.decls.push_str(": ");
                self.decls.push_str(&wtype(&full));
                if !init.is_empty() {
                    self.decls.push_str(" = ");
                    self.decls.push_str(&init);
                }
                self.decls.push_str(";\n");
                self.add_global(&n, &full, &w);
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(";");
        }
        fns
    }

    fn helper_source(h: &str) -> String {
        if h == "mat3_from4_" {
            return String::from("fn mat3_from4_(m: mat4x4f) -> mat3x3f { return mat3x3f(m[0].xyz, m[1].xyz, m[2].xyz); }\n");
        }
        if h == "mat4_from3_" {
            return String::from("fn mat4_from3_(m: mat3x3f) -> mat4x4f { return mat4x4f(vec4f(m[0], 0.0), vec4f(m[1], 0.0), vec4f(m[2], 0.0), vec4f(0.0, 0.0, 0.0, 1.0)); }\n");
        }
        if h == "mat3_diag_" {
            return String::from("fn mat3_diag_(s: f32) -> mat3x3f { return mat3x3f(s, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, s); }\n");
        }
        if h == "mat4_diag_" {
            return String::from("fn mat4_diag_(s: f32) -> mat4x4f { return mat4x4f(s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, s); }\n");
        }
        if h == "inverse3_" {
            return String::from(
                "fn inverse3_(m: mat3x3f) -> mat3x3f {\n  let a = m[0]; let b = m[1]; let c = m[2];\n  let r0 = cross(b, c); let r1 = cross(c, a); let r2 = cross(a, b);\n  let det = dot(a, r0);\n  return transpose(mat3x3f(r0, r1, r2)) * (1.0 / det);\n}\n",
            );
        }
        if h == "inverse4_" {
            return String::from(
                "fn inverse4_(m: mat4x4f) -> mat4x4f {\n  let a00 = m[0][0]; let a01 = m[0][1]; let a02 = m[0][2]; let a03 = m[0][3];\n  let a10 = m[1][0]; let a11 = m[1][1]; let a12 = m[1][2]; let a13 = m[1][3];\n  let a20 = m[2][0]; let a21 = m[2][1]; let a22 = m[2][2]; let a23 = m[2][3];\n  let a30 = m[3][0]; let a31 = m[3][1]; let a32 = m[3][2]; let a33 = m[3][3];\n  let b00 = a00 * a11 - a01 * a10; let b01 = a00 * a12 - a02 * a10; let b02 = a00 * a13 - a03 * a10;\n  let b03 = a01 * a12 - a02 * a11; let b04 = a01 * a13 - a03 * a11; let b05 = a02 * a13 - a03 * a12;\n  let b06 = a20 * a31 - a21 * a30; let b07 = a20 * a32 - a22 * a30; let b08 = a20 * a33 - a23 * a30;\n  let b09 = a21 * a32 - a22 * a31; let b10 = a21 * a33 - a23 * a31; let b11 = a22 * a33 - a23 * a32;\n  let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;\n  return mat4x4f(\n    vec4f(a11 * b11 - a12 * b10 + a13 * b09, a02 * b10 - a01 * b11 - a03 * b09, a31 * b05 - a32 * b04 + a33 * b03, a22 * b04 - a21 * b05 - a23 * b03),\n    vec4f(a12 * b08 - a10 * b11 - a13 * b07, a00 * b11 - a02 * b08 + a03 * b07, a32 * b02 - a30 * b05 - a33 * b01, a20 * b05 - a22 * b02 + a23 * b01),\n    vec4f(a10 * b10 - a11 * b08 + a13 * b06, a01 * b08 - a00 * b10 - a03 * b06, a30 * b04 - a31 * b02 + a33 * b00, a21 * b02 - a20 * b04 - a23 * b00),\n    vec4f(a11 * b07 - a10 * b09 - a12 * b06, a00 * b09 - a01 * b07 + a02 * b06, a31 * b01 - a30 * b03 - a32 * b00, a20 * b03 - a21 * b01 + a22 * b00)) * (1.0 / det);\n}\n",
            );
        }
        String::new()
    }

    /// The bindings both stages declare (Frame, Draw, textures).
    fn bindings(&self, tex: &Vec<String>) -> String {
        let mut s = String::new();
        if !self.frame_names.is_empty() {
            s.push_str("struct Frame {\n");
            for i in 0..self.frame_names.len() {
                s.push_str("  ");
                s.push_str(&self.frame_names[i]);
                s.push_str(": ");
                s.push_str(&wtype(&self.frame_types[i]));
                s.push_str(",\n");
            }
            s.push_str("  u_target: vec4f,\n}\n@group(0) @binding(0) var<uniform> frame_: Frame;\n");
        }
        if !self.draw_names.is_empty() {
            s.push_str("struct Draw {\n");
            for i in 0..self.draw_names.len() {
                s.push_str("  ");
                s.push_str(&self.draw_names[i]);
                s.push_str(": ");
                s.push_str(&wtype(&self.draw_types[i]));
                s.push_str(",\n");
            }
            s.push_str("}\n@group(0) @binding(1) var<uniform> draw_: Draw;\n");
        }
        for i in 0..tex.len() {
            let b = 2 + 2 * i;
            let k = index_of(&self.tex_names, &tex[i]) as usize;
            let n = self.name(&tex[i]);
            s.push_str("@group(0) @binding(");
            s.push_str(&b.to_string());
            s.push_str(") var ");
            s.push_str(&n);
            s.push_str(": ");
            s.push_str(&wtype(&self.tex_types[k]));
            s.push_str(";\n@group(0) @binding(");
            s.push_str(&(b + 1).to_string());
            s.push_str(") var ");
            s.push_str(&n);
            s.push_str("_s: ");
            s.push_str(&sampler_wtype(&self.tex_types[k]));
            s.push_str(";\n");
        }
        s
    }

    fn helpers_source(&self) -> String {
        let mut s = String::new();
        for h in self.helpers.iter() {
            s.push_str(&Wgsl::helper_source(h));
        }
        s
    }
}

fn private(s: &mut String, n: &str, t: &str) {
    s.push_str("var<private> ");
    s.push_str(n);
    s.push_str(": ");
    s.push_str(&wtype(t));
    s.push_str(";\n");
}

/// The WGSL of a program's two GLSL ES 3.00 stages (preprocessed) and their
/// layout: [vertex, fragment, layout] (see the module comment).
pub fn translate(vs: &str, fs: &str) -> Vec<String> {
    let mut w = Wgsl::new();
    let vfns = w.stage_source(vs, 0);
    let v_used = w.used.clone();
    let v_ins = w.in_names.clone();
    let v_in_types = w.in_types.clone();
    let v_in_locs = w.in_locs.clone();
    let v_outs = w.out_names.clone();
    let v_out_types = w.out_types.clone();
    let v_decls = w.decls.clone();
    let v_helpers = w.helpers_source();
    w.helpers = Vec::new();
    let ffns = w.stage_source(fs, 1);
    let f_used = w.used.clone();
    let f_ins = w.in_names.clone();
    let f_in_types = w.in_types.clone();
    let f_outs = w.out_names.clone();
    let f_out_types = w.out_types.clone();
    let f_decls = w.decls.clone();
    let f_helpers = w.helpers_source();
    let has_frame = !w.frame_names.is_empty();
    // the textures either stage samples
    let mut tex: Vec<String> = Vec::new();
    for t in w.tex_names.iter() {
        if has(&v_used, t) || has(&f_used, t) {
            tex.push(t.clone());
        }
    }
    let bind = w.bindings(&tex);
    let mut layout = String::new();

    // the varyings: the vertex outputs either stage refers to, located in order
    let mut vary: Vec<String> = Vec::new();
    let mut vary_t: Vec<String> = Vec::new();
    for i in 0..v_outs.len() {
        if has(&v_used, &v_outs[i]) || has(&f_used, &v_outs[i]) {
            vary.push(v_outs[i].clone());
            vary_t.push(v_out_types[i].clone());
        }
    }

    // ---- vertex ----
    let mut v = String::from("// generated by three/src/renderers/wgsl.rs\n");
    v.push_str(&bind);
    let mut vin = String::new();
    let mut vcopy = String::new();
    let mut next_loc: i64 = 0;
    for i in 0..v_ins.len() {
        let n = w.name(&v_ins[i]);
        private(&mut v, &n, &v_in_types[i]);
        let loc = if v_in_locs[i] >= 0 { v_in_locs[i] } else { next_loc };
        next_loc = loc + 1;
        if !has(&v_used, &v_ins[i]) {
            continue;
        }
        vin.push_str("  @location(");
        vin.push_str(&loc.to_string());
        vin.push_str(") ");
        vin.push_str(&n);
        vin.push_str(": ");
        vin.push_str(&wtype(&v_in_types[i]));
        vin.push_str(",\n");
        vcopy.push_str("  ");
        vcopy.push_str(&n);
        vcopy.push_str(" = in_.");
        vcopy.push_str(&n);
        vcopy.push_str(";\n");
        layout.push_str("attr ");
        layout.push_str(&v_ins[i]);
        layout.push_str(" ");
        layout.push_str(&v_in_types[i]);
        layout.push_str(" ");
        layout.push_str(&loc.to_string());
        layout.push_str("\n");
    }
    for i in 0..v_outs.len() {
        private(&mut v, &w.name(&v_outs[i]), &v_out_types[i]);
    }
    v.push_str("var<private> gl_Position: vec4f;\nvar<private> gl_PointSize: f32;\nvar<private> gl_VertexID: i32;\nvar<private> gl_InstanceID: i32;\n");
    v.push_str(&v_decls);
    v.push_str(&v_helpers);
    v.push_str(&vfns);
    if !vin.is_empty() {
        v.push_str("struct VIn {\n");
        v.push_str(&vin);
        v.push_str("}\n");
    }
    v.push_str("struct VOut {\n  @builtin(position) pos_: vec4f,\n");
    for i in 0..vary.len() {
        v.push_str("  @location(");
        v.push_str(&i.to_string());
        v.push_str(") ");
        v.push_str(&w.name(&vary[i]));
        v.push_str(": ");
        v.push_str(&wtype(&vary_t[i]));
        v.push_str(",\n");
        layout.push_str("varying ");
        layout.push_str(&vary[i]);
        layout.push_str(" ");
        layout.push_str(&i.to_string());
        layout.push_str("\n");
    }
    v.push_str("}\n@vertex fn vs_main(");
    if !vin.is_empty() {
        v.push_str("in_: VIn, ");
    }
    v.push_str("@builtin(vertex_index) vi_: u32, @builtin(instance_index) ii_: u32) -> VOut {\n");
    v.push_str(&vcopy);
    v.push_str("  gl_VertexID = i32(vi_);\n  gl_InstanceID = i32(ii_);\n  main_();\n  var o_: VOut;\n");
    if has_frame {
        v.push_str("  o_.pos_ = vec4f(gl_Position.x, gl_Position.y * frame_.u_target.z, (gl_Position.z + gl_Position.w) * 0.5, gl_Position.w);\n");
    } else {
        v.push_str("  o_.pos_ = vec4f(gl_Position.xy, (gl_Position.z + gl_Position.w) * 0.5, gl_Position.w);\n");
    }
    for i in 0..vary.len() {
        let n = w.name(&vary[i]);
        v.push_str("  o_.");
        v.push_str(&n);
        v.push_str(" = ");
        v.push_str(&n);
        v.push_str(";\n");
    }
    v.push_str("  return o_;\n}\n");

    // ---- fragment ----
    let mut f = String::from("// generated by three/src/renderers/wgsl.rs\ndiagnostic(off, derivative_uniformity);\n");
    f.push_str(&bind);
    let mut fin = String::new();
    let mut fcopy = String::new();
    for i in 0..f_ins.len() {
        let n = w.name(&f_ins[i]);
        private(&mut f, &n, &f_in_types[i]);
        let loc = index_of(&vary, &f_ins[i]);
        if loc < 0 || !has(&f_used, &f_ins[i]) {
            continue;
        }
        fin.push_str("  @location(");
        fin.push_str(&loc.to_string());
        fin.push_str(") ");
        fin.push_str(&n);
        fin.push_str(": ");
        fin.push_str(&wtype(&f_in_types[i]));
        fin.push_str(",\n");
        fcopy.push_str("  ");
        fcopy.push_str(&n);
        fcopy.push_str(" = in_.");
        fcopy.push_str(&n);
        fcopy.push_str(";\n");
    }
    for i in 0..f_outs.len() {
        private(&mut f, &w.name(&f_outs[i]), &f_out_types[i]);
    }
    f.push_str("var<private> gl_FragCoord: vec4f;\nvar<private> gl_FrontFacing: bool;\n");
    f.push_str(&f_decls);
    f.push_str(&f_helpers);
    f.push_str(&ffns);
    fin.push_str("  @builtin(position) fc_: vec4f,\n  @builtin(front_facing) ff_: bool,\n");
    f.push_str("struct FIn {\n");
    f.push_str(&fin);
    f.push_str("}\n@fragment fn fs_main(in_: FIn)");
    let has_out = !f_outs.is_empty();
    if has_out {
        f.push_str(" -> @location(0) ");
        f.push_str(&wtype(&f_out_types[0]));
    }
    f.push_str(" {\n");
    f.push_str(&fcopy);
    if has_frame {
        f.push_str("  gl_FragCoord = vec4f(in_.fc_.x, select(frame_.u_target.y - in_.fc_.y, in_.fc_.y, frame_.u_target.z < 0.0), in_.fc_.z, 1.0 / in_.fc_.w);\n");
    } else {
        f.push_str("  gl_FragCoord = vec4f(in_.fc_.xyz, 1.0 / in_.fc_.w);\n");
    }
    f.push_str("  gl_FrontFacing = in_.ff_;\n  main_();\n");
    if has_out {
        f.push_str("  return ");
        f.push_str(&w.name(&f_outs[0]));
        f.push_str(";\n");
    }
    f.push_str("}\n");

    // ---- layout ----
    let mut off: i64 = 0;
    for i in 0..w.draw_names.len() {
        let t = w.draw_types[i].clone();
        let a = align_of(&t);
        off = (off + a - 1) / a * a;
        layout.push_str("draw ");
        layout.push_str(&w.draw_names[i]);
        layout.push_str(" ");
        layout.push_str(&t);
        layout.push_str(" ");
        layout.push_str(&off.to_string());
        layout.push_str("\n");
        off += size_of(&t);
    }
    off = (off + 3) / 4 * 4;
    layout.push_str("draw_size ");
    layout.push_str(&off.to_string());
    layout.push_str("\n");
    for i in 0..tex.len() {
        let k = index_of(&w.tex_names, &tex[i]) as usize;
        let stages = (if has(&v_used, &tex[i]) { 1 } else { 0 }) + (if has(&f_used, &tex[i]) { 2 } else { 0 });
        layout.push_str("tex ");
        layout.push_str(&tex[i]);
        layout.push_str(" ");
        layout.push_str(&w.tex_types[k]);
        layout.push_str(" ");
        layout.push_str(&(2 + 2 * i).to_string());
        layout.push_str(" ");
        layout.push_str(&stages.to_string());
        layout.push_str("\n");
    }
    for n in w.notes.iter() {
        layout.push_str("note ");
        layout.push_str(n);
        layout.push_str("\n");
    }
    vec![v, f, layout]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderers::shaders::{key_names, main_fragment, main_vertex};

    fn variant(set: &[(&str, i64)]) -> Vec<i64> {
        let keys = key_names();
        let mut vals: Vec<i64> = vec![0; keys.len()];
        for (k, v) in set.iter() {
            let at = keys.iter().position(|x| x == k).unwrap();
            vals[at] = *v;
        }
        vals
    }

    #[test]
    fn translates_the_main_program() {
        let vals = variant(&[("u_hasMap", 1), ("u_maps", 31), ("u_receive", 1), ("u_instanced", 1), ("K_LIT", 1), ("K_PHYS", 1), ("K_SHEEN", 1), ("K_TRANS", 1), ("K_RECT", 1), ("K_ENV", 1), ("K_SHSLOTS", 15), ("K_NDIR", 2), ("K_NPOINT", 1), ("K_NRECT", 1)]);
        let out = translate(&main_vertex(&vals), &main_fragment(1, &vals));
        assert!(out[0].contains("@vertex fn vs_main("));
        assert!(out[1].contains("@fragment fn fs_main(in_: FIn) -> @location(0) vec4f"));
        assert!(out[1].contains("textureSampleCompare(u_sh0, u_sh0_s, "));
        assert!(out[2].contains("attr a_im0 vec4 4\n"));
        assert!(out[2].contains("draw u_model mat4 "));
        for line in out[2].lines() {
            assert!(!line.starts_with("note unknown") && !line.starts_with("note expected") && !line.starts_with("note unexpected"), "{}", line);
        }
        if std::env::var("WGSL_PRINT").is_ok() {
            println!("{}\n----\n{}\n----\n{}", out[0], out[1], out[2]);
        }
    }

    #[test]
    fn swizzle_assignments_and_conversions() {
        let vs = "#version 300 es\nlayout(location=0) in vec3 a_p;\nuniform mat4 u_m;\nout vec2 v_t;\nfloat f(float x, int n) { x *= 2.0; return x + float(n); }\nvoid main() { vec4 p = u_m * vec4(a_p, 1); p.xy *= 0.5; v_t = vec2(gl_VertexID & 1, f(p.z, 3)); gl_Position = vec4(mat3(u_m) * p.xyz, p.w); }";
        let fs = "#version 300 es\nprecision highp float;\nin vec2 v_t;\nout vec4 o;\nvoid main() { float k = mod(v_t.x, 0.5); o = vec4(clamp(v_t, 0.0, 1.0), k > 0.2 ? 1.0 : 0.0, 1.0); }";
        let out = translate(vs, fs);
        assert!(out[0].contains("{ let t_ = (p.xy * 0.5); p.x = t_.x; p.y = t_.y; }"));
        assert!(out[0].contains("fn f(x_in: f32, n: i32) -> f32 {\n  var x = x_in;"));
        assert!(out[0].contains("vec2f(f32((gl_VertexID & 1)), f(p.z, 3))"));
        assert!(out[0].contains("mat3_from4_(draw_.u_m)"));
        assert!(out[1].contains("clamp(v_t, vec2f(0.0), vec2f(1.0))"));
        assert!(out[1].contains("select(0.0, 1.0, (k > 0.2))"));
    }
}
