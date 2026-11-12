//! Fog (three.js `Fog` and `FogExp2`).

use ranger::prelude::*;

use crate::maths::color::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FogKind {
    /// Grows linearly from `near` to `far` (three.js `Fog`).
    Linear,
    /// `1 - exp(-(density * depth)^2)` (three.js `FogExp2`).
    Exponential,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub kind: FogKind,
    pub color: Color,
    pub near: f64,
    pub far: f64,
    pub density: f64,
}

impl Fog {
    pub fn linear(color: Color, near: f64, far: f64) -> Fog {
        Fog { kind: FogKind::Linear, color, near, far, density: 0.00025 }
    }

    pub fn exponential(color: Color, density: f64) -> Fog {
        Fog { kind: FogKind::Exponential, color, near: 1.0, far: 1000.0, density }
    }

    /// How much fog covers a point `depth` from the camera, 0..1, as
    /// three.js's fog_fragment: a smoothstep from `near` to `far`, or
    /// `1 - exp(-(density * depth)^2)`.
    pub fn factor(&self, depth: f64) -> f64 {
        match self.kind {
            FogKind::Linear => {
                if self.far == self.near {
                    return if depth >= self.far { 1.0 } else { 0.0 };
                }
                let t = ((depth - self.near) / (self.far - self.near)).max(0.0).min(1.0);
                t * t * (3.0 - 2.0 * t)
            }
            FogKind::Exponential => {
                let d = self.density * depth;
                (1.0 - (-d * d).exp()).max(0.0).min(1.0)
            }
        }
    }
}
