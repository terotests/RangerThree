//! ShaderMaterial and RawShaderMaterial for the GPU command stream: the
//! program sources three.js would compile (its prefixes, GLSL 1 sources
//! carried to GLSL ES 3.00), with every vertex input at a fixed location so
//! the ATTRIB commands can name it, and the uniform declarations that decide
//! how each material uniform is set.
use crate::materials::material::Material;
use ranger::prelude::*;

/// The first location of a material's own attributes; position, normal,
/// uv, color, instanceMatrix and instanceColor keep the main program's.
pub const CUSTOM_ATTRIB_BASE: i64 = 10;
/// GLES 3 guarantees 16 vertex inputs.
pub const MAX_ATTRIBS: i64 = 16;

/// The uniforms the renderer sets for every shader material.
pub fn builtin_uniforms() -> Vec<String> {
    vec![
        String::from("modelMatrix"),
        String::from("modelViewMatrix"),
        String::from("projectionMatrix"),
        String::from("viewMatrix"),
        String::from("normalMatrix"),
        String::from("cameraPosition"),
        String::from("isOrthographic"),
    ]
}

fn lines_of(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut start: usize = 0;
    let mut i: usize = 0;
    while i < n {
        if bytes[i] == 10 {
            out.push(src[start..i].to_string());
            start = i + 1;
        }
        i += 1;
    }
    if start < n {
        out.push(src[start..n].to_string());
    }
    out
}

/// A declaration line's words without `;`, `,`, precision qualifiers and
/// array sizes: [qualifier, type, name...]; empty when `line` is not one.
fn declaration(line: &str) -> Vec<String> {
    let t = line.trim().to_string();
    let mut code = t.clone();
    if let Some(p) = t.find("//") {
        code = t[0..p].to_string();
    }
    let flat = code.replace(";", " ").replace(",", " ");
    let mut words: Vec<String> = Vec::new();
    for w in flat.split_whitespace() {
        if w == "lowp" || w == "mediump" || w == "highp" || w == "flat" || w == "smooth" {
            continue;
        }
        let mut name = w.to_string();
        if let Some(b) = w.find("[") {
            name = w[0..b].to_string();
        }
        if !name.is_empty() {
            words.push(name);
        }
    }
    if words.len() < 3 {
        return Vec::new();
    }
    let q = words[0].clone();
    if q != "uniform" && q != "attribute" && q != "in" {
        return Vec::new();
    }
    // a function's parameters or a block are not declarations of this kind
    if code.contains("(") || code.contains("{") {
        return Vec::new();
    }
    words
}

/// The `uniform` declarations of `vs` and `fs` as type, name pairs, each name once.
pub fn uniform_declarations(vs: &str, fs: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut src = vs.to_string();
    src.push_str("\n");
    src.push_str(fs);
    for line in lines_of(&src).iter() {
        let d = declaration(line);
        if d.is_empty() || d[0] != "uniform" {
            continue;
        }
        for k in 2..d.len() {
            let mut seen = false;
            let mut j = 1;
            while j < out.len() {
                if out[j] == d[k] {
                    seen = true;
                }
                j += 2;
            }
            if !seen {
                out.push(d[1].clone());
                out.push(d[k].clone());
            }
        }
    }
    out
}

/// The fixed location of a three.js attribute name, -1 for the material's own.
fn fixed_location(name: &str) -> i64 {
    if name == "position" {
        0
    } else if name == "normal" {
        1
    } else if name == "uv" {
        2
    } else if name == "color" {
        3
    } else if name == "instanceMatrix" {
        4
    } else if name == "instanceColor" {
        8
    } else {
        -1
    }
}

/// The vertex source with each `attribute` / `in` declaration given its
/// location (`layout(location = n) in`), followed by the material's own
/// attribute names in location order from CUSTOM_ATTRIB_BASE: element 0 is
/// the source.
pub fn locate_attributes(vs: &str) -> Vec<String> {
    let mut out = String::new();
    let mut names: Vec<String> = Vec::new();
    for line in lines_of(vs).iter() {
        let d = declaration(line);
        if d.is_empty() || d[0] == "uniform" {
            out.push_str(line);
            out.push_str("\n");
            continue;
        }
        for k in 2..d.len() {
            let mut loc = fixed_location(&d[k]);
            if loc < 0 {
                loc = CUSTOM_ATTRIB_BASE + names.len() as i64;
                names.push(d[k].clone());
            }
            out.push_str("layout(location = ");
            out.push_str(&loc.to_string());
            out.push_str(") in ");
            out.push_str(&d[1]);
            out.push_str(" ");
            out.push_str(&d[k]);
            out.push_str(";\n");
        }
    }
    let mut v: Vec<String> = vec![out];
    for n in names.iter() {
        v.push(n.clone());
    }
    v
}

/// `src` without `#version` lines (the prefix gives the version).
fn without_version(src: &str) -> String {
    let mut out = String::new();
    for line in lines_of(src).iter() {
        if line.trim().starts_with("#version") {
            continue;
        }
        out.push_str(line);
        out.push_str("\n");
    }
    out
}

/// The `#include <chunk>` names in `src` (three.js shader chunks, which this
/// path does not have).
pub fn includes(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines_of(src).iter() {
        let t = line.trim().to_string();
        if t.starts_with("#include") {
            out.push(t[8..].trim().to_string());
        }
    }
    out
}

fn defines(m: &Material) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i + 1 < m.shader_defines.len() {
        out.push_str("#define ");
        out.push_str(&m.shader_defines[i]);
        out.push_str(" ");
        out.push_str(&m.shader_defines[i + 1]);
        out.push_str("\n");
        i += 2;
    }
    out
}

const PRECISION: &str = "precision highp float;\nprecision highp int;\nprecision highp sampler2D;\nprecision highp samplerCube;\n";

/// The program's sources as three.js builds them for WebGL2: [vertex,
/// fragment, own attribute names...]. `raw` is a RawShaderMaterial (no
/// three.js prefix); GLSL 1 sources get the GLSL ES 3.00 renames either way.
pub fn shader_sources(m: &Material, raw: bool, instanced: bool, colors: bool) -> Vec<String> {
    let glsl1 = !m.glsl3;
    let mut vs = String::from("#version 300 es\n");
    let mut fs = String::from("#version 300 es\n");
    if glsl1 {
        vs.push_str("#define attribute in\n#define varying out\n#define texture2D texture\n");
        fs.push_str("#define varying in\nlayout(location = 0) out highp vec4 pc_fragColor;\n#define gl_FragColor pc_fragColor\n");
        fs.push_str("#define gl_FragDepthEXT gl_FragDepth\n#define texture2D texture\n#define textureCube texture\n#define texture2DProj textureProj\n");
        fs.push_str("#define texture2DLodEXT textureLod\n#define texture2DProjLodEXT textureProjLod\n#define textureCubeLodEXT textureLod\n");
        fs.push_str("#define texture2DGradEXT textureGrad\n#define texture2DProjGradEXT textureProjGrad\n#define textureCubeGradEXT textureGrad\n");
    }
    let name = if raw { "RawShaderMaterial" } else { "ShaderMaterial" };
    let mut common = String::from("#define SHADER_TYPE ");
    common.push_str(name);
    common.push_str("\n#define SHADER_NAME ");
    common.push_str(name);
    common.push_str("\n");
    common.push_str(&defines(m));
    if !raw {
        vs.push_str(PRECISION);
        fs.push_str(PRECISION);
        if instanced {
            common.push_str("#define USE_INSTANCING\n");
        }
        if colors {
            common.push_str("#define USE_COLOR\n");
        }
    }
    vs.push_str(&common);
    fs.push_str(&common);
    if !raw {
        vs.push_str("uniform mat4 modelMatrix;\nuniform mat4 modelViewMatrix;\nuniform mat4 projectionMatrix;\nuniform mat4 viewMatrix;\n");
        vs.push_str("uniform mat3 normalMatrix;\nuniform vec3 cameraPosition;\nuniform bool isOrthographic;\n");
        vs.push_str("#ifdef USE_INSTANCING\nlayout(location = 4) in mat4 instanceMatrix;\n#endif\n");
        vs.push_str("layout(location = 0) in vec3 position;\nlayout(location = 1) in vec3 normal;\nlayout(location = 2) in vec2 uv;\n");
        vs.push_str("#ifdef USE_COLOR\nlayout(location = 3) in vec3 color;\n#endif\n");
        fs.push_str("uniform mat4 viewMatrix;\nuniform vec3 cameraPosition;\nuniform bool isOrthographic;\n");
    }
    let located = locate_attributes(&without_version(&m.vertex_shader));
    vs.push_str(&located[0]);
    fs.push_str(&without_version(&m.fragment_shader));
    let mut out: Vec<String> = vec![vs, fs];
    for k in 1..located.len() {
        out.push(located[k].clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::material::MaterialKind;

    #[test]
    fn declarations_and_locations() {
        let u = uniform_declarations("uniform float time;\nuniform highp vec3 a, b[2]; // c\nvoid f(in float x) {}", "uniform sampler2D map;\nuniform float time;");
        assert_eq!(u, vec!["float", "time", "vec3", "a", "vec3", "b", "sampler2D", "map"]);
        let l = locate_attributes("attribute float offset;\nin vec3 position;\nattribute vec2 a2;\nvoid main() {}");
        assert_eq!(l[1], "offset");
        assert_eq!(l[2], "a2");
        assert!(l[0].contains("layout(location = 10) in float offset;"));
        assert!(l[0].contains("layout(location = 0) in vec3 position;"));
        assert!(l[0].contains("layout(location = 11) in vec2 a2;"));
        let mut m = Material::new(MaterialKind::Shader);
        m.vertex_shader = String::from("void main() { gl_Position = vec4(position, 1.0); }");
        m.fragment_shader = String::from("#include <common>\nvoid main() { gl_FragColor = vec4(1.0); }");
        let s = shader_sources(&m, false, false, false);
        assert!(s[0].starts_with("#version 300 es\n"));
        assert!(s[1].contains("#define gl_FragColor pc_fragColor"));
        assert_eq!(includes(&m.fragment_shader), vec!["<common>"]);
    }
}
