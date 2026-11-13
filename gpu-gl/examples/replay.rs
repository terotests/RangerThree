//! Plays the command streams tests/gpu-compare.mjs --dump wrote, in a
//! headless OpenGL ES 3 and OpenGL 3.3 core context, and compares the pixels
//! with what the same stream drew in Chromium's WebGL2.
//!
//!   node tests/gpu-compare.mjs --dump=tmp/gpu-dump
//!   cd gpu-gl && cargo run --release --example replay -- ../tmp/gpu-dump [--save]
//!
//! --save writes each native frame next to the dump as <name>.<es|gl>.ppm.
use gpu_gl::{egl::Headless, read_rgba, Player};
use std::time::Instant;

enum J {
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
    Other,
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn value(&mut self) -> J {
        self.ws();
        match self.s[self.i] {
            b'{' => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b'}' {
                        self.i += 1;
                        return J::Obj(v);
                    }
                    let k = match self.value() {
                        J::Str(k) => k,
                        _ => panic!("object key at {}", self.i),
                    };
                    self.ws();
                    self.i += 1; // ':'
                    let x = self.value();
                    v.push((k, x));
                    self.ws();
                    if self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b']' {
                        self.i += 1;
                        return J::Arr(v);
                    }
                    v.push(self.value());
                    self.ws();
                    if self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut out: Vec<u8> = Vec::new();
                while self.s[self.i] != b'"' {
                    if self.s[self.i] == b'\\' {
                        self.i += 1;
                        let c = self.s[self.i];
                        self.i += 1;
                        match c {
                            b'n' => out.push(b'\n'),
                            b't' => out.push(b'\t'),
                            b'r' => out.push(b'\r'),
                            b'b' => out.push(8),
                            b'f' => out.push(12),
                            b'u' => {
                                let h = std::str::from_utf8(&self.s[self.i..self.i + 4]).unwrap();
                                self.i += 4;
                                let ch = char::from_u32(u32::from_str_radix(h, 16).unwrap()).unwrap_or('?');
                                out.extend_from_slice(ch.to_string().as_bytes());
                            }
                            c => out.push(c),
                        }
                    } else {
                        out.push(self.s[self.i]);
                        self.i += 1;
                    }
                }
                self.i += 1;
                J::Str(String::from_utf8(out).unwrap())
            }
            b't' | b'f' | b'n' => {
                while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_alphabetic() {
                    self.i += 1;
                }
                J::Other
            }
            _ => {
                let st = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                J::Num(std::str::from_utf8(&self.s[st..self.i]).unwrap().parse().unwrap())
            }
        }
    }
}

fn opt_field<'a>(o: &'a J, k: &str) -> Option<&'a J> {
    match o {
        J::Obj(v) => v.iter().find(|(n, _)| n == k).map(|x| &x.1),
        _ => None,
    }
}

fn field<'a>(o: &'a J, k: &str) -> &'a J {
    match o {
        J::Obj(v) => &v.iter().find(|(n, _)| n == k).unwrap_or_else(|| panic!("no field {}", k)).1,
        _ => panic!("not an object"),
    }
}
fn arr(j: &J) -> &Vec<J> {
    match j {
        J::Arr(v) => v,
        _ => panic!("not an array"),
    }
}
fn num(j: &J) -> f64 {
    match j {
        J::Num(n) => *n,
        _ => 0.0,
    }
}

fn save_ppm(path: &std::path::Path, px: &[u8], w: usize, h: usize) {
    let mut out = format!("P6\n{} {}\n255\n", w, h).into_bytes();
    for y in (0..h).rev() {
        for x in 0..w {
            let p = (y * w + x) * 4;
            out.extend_from_slice(&px[p..p + 3]);
        }
    }
    std::fs::write(path, out).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(args.iter().find(|a| !a.starts_with("--")).map(|s| s.as_str()).unwrap_or("../tmp/gpu-dump"));
    let save = args.iter().any(|a| a == "--save");
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("dump dir").filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "json")).collect();
    files.sort();
    let (mut total, mut ok) = (0, 0);
    for desktop in [false, true] {
        for file in &files {
            let j = Parser { s: &std::fs::read(file).unwrap(), i: 0 }.value();
            let name = match field(&j, "name") {
                J::Str(s) => s.clone(),
                _ => String::new(),
            };
            let (w, h) = (num(field(&j, "w")) as i32, num(field(&j, "h")) as i32);
            let ints: Vec<i64> = arr(field(&j, "ints")).iter().map(|v| num(v) as i64).collect();
            let floats: Vec<f32> = arr(field(&j, "floats")).iter().map(|v| num(v) as f32).collect();
            let texts: Vec<String> = arr(field(&j, "texts")).iter().map(|v| if let J::Str(s) = v { s.clone() } else { String::new() }).collect();
            let bytes: Vec<u8> = opt_field(&j, "bytes").map(|b| arr(b).iter().map(|v| num(v) as u8).collect()).unwrap_or_default();
            let want = std::fs::read(file.with_extension("rgba")).unwrap();
            let ctx = match Headless::new(w, h, desktop) {
                Ok(c) => c,
                Err(e) => {
                    println!("no {} context: {}", if desktop { "GL 3.3" } else { "GLES 3" }, e);
                    break;
                }
            };
            let mut player = Player::new(ctx.load_gl().unwrap(), desktop);
            player.screen_fb = ctx.fb;
            if total == 0 || file == &files[0] {
                println!("{}: {}", if desktop { "OpenGL 3.3 core" } else { "OpenGL ES 3" }, player.describe());
            }
            total += 1;
            let t0 = Instant::now();
            if let Err(e) = player.run(&ints, &floats, &texts, &bytes) {
                println!("FAIL  {}: {}", name, e);
                continue;
            }
            let got = read_rgba(&player.gl, w, h);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let (mut sum, mut max, mut far) = (0u64, 0u8, 0usize);
            for (a, b) in got.iter().zip(want.iter()) {
                let d = a.abs_diff(*b);
                sum += d as u64;
                max = max.max(d);
                if d > 3 {
                    far += 1;
                }
            }
            let mean = sum as f64 / got.len() as f64;
            let farp = far as f64 / got.len() as f64;
            let pass = mean <= 0.5 && farp <= 0.001;
            if pass {
                ok += 1;
            }
            println!("{}  {}: mean {:.3} max {} far {:.2} % | first frame {:.1} ms", if pass { "PASS" } else { "DIFF" }, name, mean, max, farp * 100.0, ms);
            if save {
                let stem = file.file_stem().unwrap().to_string_lossy().to_string();
                save_ppm(&dir.join(format!("{}.{}.ppm", stem, if desktop { "gl" } else { "es" })), &got, w as usize, h as usize);
            }
            drop(player);
        }
    }
    println!("{}/{} native frames match WebGL2", ok, total);
    std::process::exit(if ok == total { 0 } else { 1 });
}
