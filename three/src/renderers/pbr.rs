//! The metal/rough lighting three.js uses for MeshStandardMaterial and
//! MeshPhysicalMaterial: GGX microfacets with Smith height-correlated
//! visibility, Schlick Fresnel and multiple-scattering energy
//! compensation from a DFG table (the split-sum "scale, bias" pairs). The
//! table is integrated here rather than shipped as data.

use ranger::prelude::*;

use crate::maths::color::Color;
use crate::maths::vector3::Vector3;

const RECIPROCAL_PI: f64 = 0.3183098861837907;
const PI: f64 = 3.141592653589793;

/// The DFG table: `size` x `size` texels, row = n·v, column = roughness
/// (texel centres at (i + 0.5) / size), two numbers per texel (the
/// Fresnel scale and bias), integrated with `samples` GGX samples.
pub fn dfg_lut(size: i64, samples: i64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for row in 0..size {
        let nov = ((row as f64 + 0.5) / size as f64).max(0.001);
        for col in 0..size {
            let roughness = (col as f64 + 0.5) / size as f64;
            let ab = integrate_dfg(roughness, nov, samples);
            out.push(ab.x);
            out.push(ab.y);
        }
    }
    out
}

/// Van der Corput radical inverse in base 2 of `i` (no bit operations, so
/// every target computes the same value).
fn radical_inverse(i: i64) -> f64 {
    let mut n = i;
    let mut inv = 0.5;
    let mut r = 0.0;
    while n > 0 {
        if n % 2 == 1 {
            r += inv;
        }
        n /= 2;
        inv *= 0.5;
    }
    r
}

fn integrate_dfg(roughness: f64, nov: f64, samples: i64) -> Vector3 {
    let v = Vector3::new((1.0 - nov * nov).max(0.0).sqrt(), 0.0, nov);
    let a = roughness * roughness;
    let a2 = a * a;
    let mut sa = 0.0;
    let mut sb = 0.0;
    for i in 0..samples {
        let e1 = i as f64 / samples as f64;
        let e2 = radical_inverse(i);
        // GGX importance sample of the half vector around +z
        let phi = 2.0 * PI * e1;
        let cos_t = ((1.0 - e2) / (1.0 + (a2 - 1.0) * e2)).max(0.0).sqrt();
        let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
        let h = Vector3::new(sin_t * phi.cos(), sin_t * phi.sin(), cos_t);
        let voh = v.dot(&h);
        let lz = 2.0 * voh * h.z - v.z;
        let nol = lz.max(0.0);
        let noh = h.z.max(0.0);
        let voh_c = voh.max(0.0);
        if nol > 0.0 {
            let vis = v_ggx_smith_correlated(a, nol, nov);
            let g_vis = vis * 4.0 * nol * voh_c / noh.max(0.000001);
            let fc = (1.0 - voh_c).powf(5.0);
            sa += (1.0 - fc) * g_vis;
            sb += fc * g_vis;
        }
    }
    Vector3::new(sa / samples as f64, sb / samples as f64, 0.0)
}

/// Smith GGX height-correlated visibility (three.js's V_GGX_SmithCorrelated).
pub fn v_ggx_smith_correlated(alpha: f64, nol: f64, nov: f64) -> f64 {
    let a2 = alpha * alpha;
    let gv = nol * (a2 + (1.0 - a2) * nov * nov).sqrt();
    let gl = nov * (a2 + (1.0 - a2) * nol * nol).sqrt();
    0.5 / (gv + gl).max(0.000001)
}

/// GGX normal distribution (three.js's D_GGX).
pub fn d_ggx(alpha: f64, noh: f64) -> f64 {
    let a2 = alpha * alpha;
    let d = noh * noh * (a2 - 1.0) + 1.0;
    RECIPROCAL_PI * a2 / (d * d)
}

/// The DFG pair at (roughness, n·v), bilinear and clamped as a texture
/// lookup with linear filtering would give.
pub fn dfg_at(lut: &Vec<f64>, size: i64, roughness: f64, nov: f64) -> Vector3 {
    let fx = (roughness * size as f64 - 0.5).max(0.0).min(size as f64 - 1.0);
    let fy = (nov * size as f64 - 0.5).max(0.0).min(size as f64 - 1.0);
    let x0 = fx.floor() as i64;
    let y0 = fy.floor() as i64;
    let x1 = (x0 + 1).min(size - 1);
    let y1 = (y0 + 1).min(size - 1);
    let tx = fx - x0 as f64;
    let ty = fy - y0 as f64;
    let mut out = Vector3::zero();
    for c in 0..2 {
        let v00 = lut[((y0 * size + x0) * 2 + c) as usize];
        let v10 = lut[((y0 * size + x1) * 2 + c) as usize];
        let v01 = lut[((y1 * size + x0) * 2 + c) as usize];
        let v11 = lut[((y1 * size + x1) * 2 + c) as usize];
        let v = (v00 * (1.0 - tx) + v10 * tx) * (1.0 - ty) + (v01 * (1.0 - tx) + v11 * tx) * ty;
        if c == 0 {
            out.x = v;
        } else {
            out.y = v;
        }
    }
    out
}

/// three.js's BRDF_GGX_Multiscatter for light `l`, view `v` and normal
/// `n` (unit vectors), specular colour `f0` (f90 = 1) and `roughness`.
pub fn brdf_ggx_multiscatter(l: &Vector3, v: &Vector3, n: &Vector3, f0: &Color, roughness: f64, lut: &Vec<f64>, size: i64) -> Color {
    let alpha = roughness * roughness;
    let mut h = *l;
    h.add(v);
    h.normalize();
    let nol = n.dot(l).max(0.0).min(1.0);
    let nov = n.dot(v).max(0.0).min(1.0);
    let noh = n.dot(&h).max(0.0).min(1.0);
    let voh = v.dot(&h).max(0.0).min(1.0);
    let two: f64 = 2.0;
    let fresnel = two.powf((-5.55473 * voh - 6.98316) * voh);
    let vis = v_ggx_smith_correlated(alpha, nol, nov);
    let d = d_ggx(alpha, noh);
    let k = vis * d;
    let fr = f0.r * (1.0 - fresnel) + fresnel;
    let fg = f0.g * (1.0 - fresnel) + fresnel;
    let fb = f0.b * (1.0 - fresnel) + fresnel;
    let dv = dfg_at(lut, size, roughness, nov);
    let dl = dfg_at(lut, size, roughness, nol);
    let ems_v = 1.0 - (dv.x + dv.y);
    let ems_l = 1.0 - (dl.x + dl.y);
    let comp = ems_v * ems_l;
    let ms_r = multi(f0.r, &dv, &dl, ems_v, ems_l) * comp;
    let ms_g = multi(f0.g, &dv, &dl, ems_v, ems_l) * comp;
    let ms_b = multi(f0.b, &dv, &dl, ems_v, ems_l) * comp;
    Color::new(fr * k + ms_r, fg * k + ms_g, fb * k + ms_b)
}

fn multi(f0: f64, dv: &Vector3, dl: &Vector3, ems_v: f64, ems_l: f64) -> f64 {
    let fss_v = f0 * dv.x + dv.y;
    let fss_l = f0 * dl.x + dl.y;
    let favg = f0 + (1.0 - f0) * 0.047619;
    fss_v * fss_l * favg / (1.0 - ems_v * ems_l * favg + 0.000001)
}
