//! RangerThree: a 3D library with three.js's API, written as strict Rust
//! that cargo builds and the Ranger compiler also reads, so the same source
//! becomes JavaScript, Go, Python, C++ and Ranger's other targets.

#![allow(unused_imports)]

pub mod core;
pub mod extras;
pub mod geometries;
pub mod materials;
pub mod maths;
pub mod scenes;
pub mod textures;
