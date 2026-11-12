//! Which of 32 layers an object belongs to (three.js `Layers`); a camera
//! sees an object when their masks share a layer.

use ranger::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layers {
    pub mask: i64,
}

impl Default for Layers {
    fn default() -> Layers {
        Layers { mask: 1 }
    }
}

/// `x` as a signed 32-bit number, as JavaScript's bitwise operators leave
/// it (three.js's masks are those: all layers is -1).
fn wrap32(x: i64) -> i64 {
    let v = x & 0xffffffff;
    if v >= 0x80000000 {
        v - 0x100000000
    } else {
        v
    }
}

fn bit(layer: i64) -> i64 {
    wrap32(1 << (layer & 31))
}

impl Layers {
    pub fn new() -> Layers {
        Layers { mask: 1 }
    }

    pub fn set(&mut self, layer: i64) {
        self.mask = bit(layer);
    }

    pub fn enable(&mut self, layer: i64) {
        self.mask = wrap32(self.mask | bit(layer));
    }

    pub fn enable_all(&mut self) {
        self.mask = -1;
    }

    pub fn toggle(&mut self, layer: i64) {
        self.mask = wrap32(self.mask ^ bit(layer));
    }

    pub fn disable(&mut self, layer: i64) {
        self.mask = wrap32(self.mask & !bit(layer));
    }

    pub fn disable_all(&mut self) {
        self.mask = 0;
    }

    pub fn test(&self, layers: &Layers) -> bool {
        (self.mask & layers.mask) != 0
    }

    pub fn is_enabled(&self, layer: i64) -> bool {
        (self.mask & bit(layer)) != 0
    }
}
