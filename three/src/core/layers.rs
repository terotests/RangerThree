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

impl Layers {
    pub fn new() -> Layers {
        Layers { mask: 1 }
    }

    pub fn set(&mut self, layer: i64) {
        self.mask = (1 << layer) & 0xffffffff;
    }

    pub fn enable(&mut self, layer: i64) {
        self.mask = (self.mask | (1 << layer)) & 0xffffffff;
    }

    pub fn enable_all(&mut self) {
        self.mask = 0xffffffff;
    }

    pub fn toggle(&mut self, layer: i64) {
        self.mask = (self.mask ^ (1 << layer)) & 0xffffffff;
    }

    pub fn disable(&mut self, layer: i64) {
        self.mask = self.mask & !(1 << layer) & 0xffffffff;
    }

    pub fn disable_all(&mut self) {
        self.mask = 0;
    }

    pub fn test(&self, layers: &Layers) -> bool {
        (self.mask & layers.mask) != 0
    }

    pub fn is_enabled(&self, layer: i64) -> bool {
        (self.mask & (1 << layer)) != 0
    }
}
