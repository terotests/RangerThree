//! A GLSL preprocessor: `#define` (object-like and function-like), `#undef`,
//! `#if` / `#ifdef` / `#ifndef` / `#elif` / `#else` / `#endif` with C's
//! integer expressions and `defined`. The crate runs it on every program it
//! builds, so a driver gets one straight-line source (no branches on
//! constants left for it to keep or drop) and the WGSL translator
//! (wgsl.rs) sees plain GLSL. `#version`, `#extension` and `precision` lines
//! stay; `//` comments and blank lines go.

use ranger::prelude::*;

/// The macros defined so far: name, parameters (comma-joined; `arity` -1 for
/// an object-like macro) and body.
pub struct Macros {
    pub names: Vec<String>,
    pub params: Vec<String>,
    pub arity: Vec<i64>,
    pub bodies: Vec<String>,
}

impl Macros {
    pub fn new() -> Macros {
        Macros { names: Vec::new(), params: Vec::new(), arity: Vec::new(), bodies: Vec::new() }
    }

    fn find(&self, name: &str) -> i64 {
        let mut i = self.names.len() as i64 - 1;
        while i >= 0 {
            if self.names[i as usize] == name {
                return i;
            }
            i -= 1;
        }
        -1
    }

    fn undef(&mut self, name: &str) {
        let at = self.find(name);
        if at >= 0 {
            let k = at as usize;
            self.names.remove(k);
            self.params.remove(k);
            self.arity.remove(k);
            self.bodies.remove(k);
        }
    }

    /// `rest`: what follows `#define ` on its line.
    fn define(&mut self, rest: &str) {
        let b = rest.as_bytes();
        let n = b.len();
        let mut i: usize = 0;
        while i < n && is_ident_byte(b[i]) {
            i += 1;
        }
        let name = rest[0..i].to_string();
        self.undef(&name);
        if i < n && b[i] == 40 {
            // NAME(a, b) body: no space before `(`
            let mut j = i + 1;
            while j < n && b[j] != 41 {
                j += 1;
            }
            let inner = rest[i + 1..j].to_string();
            let mut ps: Vec<String> = Vec::new();
            for p in inner.split(",") {
                let t = p.trim().to_string();
                if !t.is_empty() {
                    ps.push(t);
                }
            }
            let body = if j + 1 < n { rest[j + 1..n].trim().to_string() } else { String::new() };
            self.names.push(name);
            self.params.push(ps.join(","));
            self.arity.push(ps.len() as i64);
            self.bodies.push(body);
        } else {
            let body = rest[i..n].trim().to_string();
            self.names.push(name);
            self.params.push(String::new());
            self.arity.push(-1);
            self.bodies.push(body);
        }
    }
}

fn is_ident_start(c: u8) -> bool {
    (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c == 95
}

fn is_ident_byte(c: u8) -> bool {
    is_ident_start(c) || (c >= 48 && c <= 57)
}

fn is_digit(c: u8) -> bool {
    c >= 48 && c <= 57
}

fn has(list: &Vec<String>, s: &str) -> bool {
    for x in list.iter() {
        if x == s {
            return true;
        }
    }
    false
}

/// `src` with every macro expanded; names in `off` are not expanded again
/// (a macro is off inside its own expansion, as in C).
fn expand(m: &Macros, src: &str, off: &Vec<String>) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = String::new();
    let mut i: usize = 0;
    let mut plain: usize = 0;
    while i < n {
        let c = b[i];
        if is_digit(c) {
            // a number (its exponent or suffix letters are not names)
            while i < n && (is_ident_byte(b[i]) || b[i] == 46) {
                i += 1;
            }
            continue;
        }
        if !is_ident_start(c) {
            i += 1;
            continue;
        }
        let s = i;
        while i < n && is_ident_byte(b[i]) {
            i += 1;
        }
        let name = src[s..i].to_string();
        let at = m.find(&name);
        if at < 0 || has(off, &name) {
            continue;
        }
        let k = at as usize;
        let mut off2 = off.clone();
        off2.push(name.clone());
        if m.arity[k] < 0 {
            out.push_str(&src[plain..s]);
            out.push_str(&expand(m, &m.bodies[k], &off2));
            plain = i;
            continue;
        }
        // a function-like macro is called only when `(` follows
        let mut j = i;
        while j < n && (b[j] == 32 || b[j] == 9) {
            j += 1;
        }
        if j >= n || b[j] != 40 {
            continue;
        }
        let mut args: Vec<String> = Vec::new();
        let mut depth: i64 = 0;
        let mut a = j + 1;
        let mut e = j + 1;
        while e < n {
            let ch = b[e];
            if ch == 40 {
                depth += 1;
            } else if ch == 41 {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            } else if ch == 44 && depth == 0 {
                args.push(src[a..e].trim().to_string());
                a = e + 1;
            }
            e += 1;
        }
        let last = src[a..e].trim().to_string();
        if !last.is_empty() || !args.is_empty() {
            args.push(last);
        }
        // arguments are expanded first, then the body is rescanned
        let mut ex: Vec<String> = Vec::new();
        for x in args.iter() {
            ex.push(expand(m, x, off));
        }
        let mut ps: Vec<String> = Vec::new();
        if !m.params[k].is_empty() {
            for p in m.params[k].split(",") {
                ps.push(p.to_string());
            }
        }
        let body = substitute(&m.bodies[k], &ps, &ex);
        out.push_str(&src[plain..s]);
        out.push_str(&expand(m, &body, &off2));
        i = if e < n { e + 1 } else { n };
        plain = i;
    }
    out.push_str(&src[plain..n]);
    out
}

/// `body` with each parameter name in `ps` replaced by its argument.
fn substitute(body: &str, ps: &Vec<String>, args: &Vec<String>) -> String {
    let b = body.as_bytes();
    let n = b.len();
    let mut out = String::new();
    let mut i: usize = 0;
    let mut plain: usize = 0;
    while i < n {
        if is_digit(b[i]) {
            while i < n && (is_ident_byte(b[i]) || b[i] == 46) {
                i += 1;
            }
            continue;
        }
        if !is_ident_start(b[i]) {
            i += 1;
            continue;
        }
        let s = i;
        while i < n && is_ident_byte(b[i]) {
            i += 1;
        }
        let name = body[s..i].to_string();
        for k in 0..ps.len() {
            if ps[k] == name && k < args.len() {
                out.push_str(&body[plain..s]);
                out.push_str(&args[k]);
                plain = i;
                break;
            }
        }
    }
    out.push_str(&body[plain..n]);
    out
}

/// The tokens of an `#if` expression: numbers, names and operators.
fn expr_tokens(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let n = b.len();
    let mut out: Vec<String> = Vec::new();
    let mut i: usize = 0;
    while i < n {
        let c = b[i];
        if c == 32 || c == 9 {
            i += 1;
            continue;
        }
        let s = i;
        if is_ident_byte(c) {
            while i < n && is_ident_byte(b[i]) {
                i += 1;
            }
            out.push(src[s..i].to_string());
            continue;
        }
        if i + 1 < n {
            let two = src[i..i + 2].to_string();
            if two == "&&" || two == "||" || two == "==" || two == "!=" || two == "<=" || two == ">=" || two == "<<" || two == ">>" {
                out.push(two);
                i += 2;
                continue;
            }
        }
        out.push(src[i..i + 1].to_string());
        i += 1;
    }
    out
}

/// A number token's value (decimal, hex, octal; `u` suffix ignored).
fn number(t: &str) -> i64 {
    let mut s = t.to_string();
    if s.ends_with("u") || s.ends_with("U") {
        s = s[0..s.as_bytes().len() - 1].to_string();
    }
    let b = s.as_bytes();
    let mut v: i64 = 0;
    if b.len() > 2 && b[0] == 48 && (b[1] == 120 || b[1] == 88) {
        for k in 2..b.len() {
            let c = b[k];
            let d: i64 = if c >= 48 && c <= 57 {
                (c - 48) as i64
            } else if c >= 97 && c <= 102 {
                (c - 87) as i64
            } else {
                (c - 55) as i64
            };
            v = v * 16 + d;
        }
        return v;
    }
    let base: i64 = if b.len() > 1 && b[0] == 48 { 8 } else { 10 };
    for k in 0..b.len() {
        v = v * base + (b[k] - 48) as i64;
    }
    v
}

/// Precedence of a binary operator (higher binds tighter), 0 when `t` is not one.
fn prec(t: &str) -> i64 {
    if t == "||" {
        1
    } else if t == "&&" {
        2
    } else if t == "|" {
        3
    } else if t == "^" {
        4
    } else if t == "&" {
        5
    } else if t == "==" || t == "!=" {
        6
    } else if t == "<" || t == ">" || t == "<=" || t == ">=" {
        7
    } else if t == "<<" || t == ">>" {
        8
    } else if t == "+" || t == "-" {
        9
    } else if t == "*" || t == "/" || t == "%" {
        10
    } else {
        0
    }
}

fn apply(op: &str, a: i64, b: i64) -> i64 {
    if op == "||" {
        if a != 0 || b != 0 { 1 } else { 0 }
    } else if op == "&&" {
        if a != 0 && b != 0 { 1 } else { 0 }
    } else if op == "|" {
        a | b
    } else if op == "^" {
        a ^ b
    } else if op == "&" {
        a & b
    } else if op == "==" {
        if a == b { 1 } else { 0 }
    } else if op == "!=" {
        if a != b { 1 } else { 0 }
    } else if op == "<" {
        if a < b { 1 } else { 0 }
    } else if op == ">" {
        if a > b { 1 } else { 0 }
    } else if op == "<=" {
        if a <= b { 1 } else { 0 }
    } else if op == ">=" {
        if a >= b { 1 } else { 0 }
    } else if op == "<<" {
        a << b
    } else if op == ">>" {
        a >> b
    } else if op == "+" {
        a + b
    } else if op == "-" {
        a - b
    } else if op == "*" {
        a * b
    } else if op == "/" {
        if b == 0 { 0 } else { a / b }
    } else if op == "%" {
        if b == 0 { 0 } else { a % b }
    } else {
        0
    }
}

/// Evaluates `t` from `*at`: unary operators, parentheses, numbers (names left are 0).
fn unary(t: &Vec<String>, at: &mut usize) -> i64 {
    if *at >= t.len() {
        return 0;
    }
    let s = t[*at].clone();
    *at += 1;
    if s == "(" {
        let v = binary(t, at, 1);
        if *at < t.len() && t[*at] == ")" {
            *at += 1;
        }
        return v;
    }
    if s == "!" {
        let v = unary(t, at);
        return if v == 0 { 1 } else { 0 };
    }
    if s == "-" {
        return -unary(t, at);
    }
    if s == "+" {
        return unary(t, at);
    }
    if s == "~" {
        return -unary(t, at) - 1;
    }
    let b = s.as_bytes();
    if !b.is_empty() && is_digit(b[0]) {
        return number(&s);
    }
    0
}

fn binary(t: &Vec<String>, at: &mut usize, min: i64) -> i64 {
    let mut lhs = unary(t, at);
    while *at < t.len() {
        let op = t[*at].clone();
        let p = prec(&op);
        if p == 0 || p < min {
            break;
        }
        *at += 1;
        let rhs = binary(t, at, p + 1);
        lhs = apply(&op, lhs, rhs);
    }
    lhs
}

/// The value of an `#if` / `#elif` condition.
fn condition(m: &Macros, src: &str) -> i64 {
    // `defined X` / `defined(X)` before expansion
    let raw = expr_tokens(src);
    let mut s = String::new();
    let mut i: usize = 0;
    while i < raw.len() {
        if raw[i] == "defined" {
            let mut j = i + 1;
            let paren = j < raw.len() && raw[j] == "(";
            if paren {
                j += 1;
            }
            let name = if j < raw.len() { raw[j].clone() } else { String::new() };
            s.push_str(if m.find(&name) >= 0 { " 1 " } else { " 0 " });
            i = j + 1;
            if paren && i < raw.len() && raw[i] == ")" {
                i += 1;
            }
            continue;
        }
        s.push_str(" ");
        s.push_str(&raw[i]);
        i += 1;
    }
    let none: Vec<String> = Vec::new();
    let t = expr_tokens(&expand(m, &s, &none));
    let mut at: usize = 0;
    binary(&t, &mut at, 1)
}

/// `line` without a `//` comment.
fn code_of(line: &str) -> String {
    if let Some(p) = line.find("//") {
        return line[0..p].to_string();
    }
    line.to_string()
}

/// `src` preprocessed: conditionals resolved, macros expanded and their
/// definitions removed.
pub fn preprocess(src: &str) -> String {
    let mut m = Macros::new();
    let mut out = String::new();
    // per open conditional: 1 taking this branch, 0 may take a later one, 2 done (or parent off)
    let mut stack: Vec<i64> = Vec::new();
    let none: Vec<String> = Vec::new();
    for raw in src.split("\n") {
        let line = code_of(raw);
        let t = line.trim().to_string();
        let mut active = true;
        for s in stack.iter() {
            if *s != 1 {
                active = false;
            }
        }
        if t.starts_with("#") {
            let d = t[1..].trim().to_string();
            let b = d.as_bytes();
            let mut w: usize = 0;
            while w < b.len() && is_ident_byte(b[w]) {
                w += 1;
            }
            let word = d[0..w].to_string();
            let rest = d[w..].trim().to_string();
            if word == "if" || word == "ifdef" || word == "ifndef" {
                if !active {
                    stack.push(2);
                    continue;
                }
                let v = if word == "if" {
                    condition(&m, &rest) != 0
                } else if word == "ifdef" {
                    m.find(&rest) >= 0
                } else {
                    m.find(&rest) < 0
                };
                stack.push(if v { 1 } else { 0 });
                continue;
            }
            if word == "elif" || word == "else" || word == "endif" {
                let top = stack.len();
                if top == 0 {
                    continue;
                }
                if word == "endif" {
                    stack.pop();
                    continue;
                }
                let cur = stack[top - 1];
                let next = if cur == 1 || cur == 2 {
                    2
                } else {
                    let mut parent = true;
                    for k in 0..top - 1 {
                        if stack[k] != 1 {
                            parent = false;
                        }
                    }
                    if !parent {
                        2
                    } else if word == "else" || condition(&m, &rest) != 0 {
                        1
                    } else {
                        0
                    }
                };
                stack[top - 1] = next;
                continue;
            }
            if !active {
                continue;
            }
            if word == "define" {
                m.define(&rest);
                continue;
            }
            if word == "undef" {
                m.undef(&rest);
                continue;
            }
            // #version, #extension, #pragma stay
            out.push_str(&t);
            out.push_str("\n");
            continue;
        }
        if !active || t.is_empty() {
            continue;
        }
        let e = expand(&m, &line, &none);
        let trimmed_end = e.trim_end().to_string();
        out.push_str(&trimmed_end);
        out.push_str("\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditionals_and_macros() {
        let src = "#version 300 es\n#define A 2\n#define T(uv, k) tex(m, uv * k) // c\n#if A == 2 && (A & 1) == 0\nvec4 x = T(v, A);\n#elif A == 3\nbad;\n#else\nbad;\n#endif\n#ifdef B\nbad;\n#endif\n#if defined(A) && !defined(B)\nfloat y = 1e-6 + A;\n#endif\n";
        let out = preprocess(src);
        assert_eq!(out, "#version 300 es\nvec4 x = tex(m, v * 2);\nfloat y = 1e-6 + 2;\n");
        // nested branches inside a branch not taken stay off, `#undef` ends a macro
        let n = preprocess("#if 0\n#if 1\nbad;\n#else\nbad;\n#endif\n#elif 1\n#define K 4\nint k = K;\n#undef K\nint j = K;\n#endif\n");
        assert_eq!(n, "int k = 4;\nint j = K;\n");
    }
}
