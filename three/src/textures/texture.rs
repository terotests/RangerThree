//! An image and how it is sampled (three.js `Texture`). The constants are
//! three.js's numeric values, so a scene description can pass them as is.

use ranger::cell::RefCell;
use ranger::prelude::*;
use std::rc::Rc;

use crate::maths::color::ColorSpace;
use crate::maths::matrix3::Matrix3;
use crate::maths::vector2::Vector2;

pub const UV_MAPPING: i64 = 300;
pub const CUBE_REFLECTION_MAPPING: i64 = 301;
pub const CUBE_REFRACTION_MAPPING: i64 = 302;
pub const EQUIRECTANGULAR_REFLECTION_MAPPING: i64 = 303;
pub const EQUIRECTANGULAR_REFRACTION_MAPPING: i64 = 304;
pub const CUBE_UV_REFLECTION_MAPPING: i64 = 306;
pub const REPEAT_WRAPPING: i64 = 1000;
pub const CLAMP_TO_EDGE_WRAPPING: i64 = 1001;
pub const MIRRORED_REPEAT_WRAPPING: i64 = 1002;
pub const NEAREST_FILTER: i64 = 1003;
pub const NEAREST_MIPMAP_NEAREST_FILTER: i64 = 1004;
pub const NEAREST_MIPMAP_LINEAR_FILTER: i64 = 1005;
pub const LINEAR_FILTER: i64 = 1006;
pub const LINEAR_MIPMAP_NEAREST_FILTER: i64 = 1007;
pub const LINEAR_MIPMAP_LINEAR_FILTER: i64 = 1008;
pub const UNSIGNED_BYTE_TYPE: i64 = 1009;
pub const FLOAT_TYPE: i64 = 1015;
pub const UNSIGNED_INT_TYPE: i64 = 1014;
pub const RGBA_FORMAT: i64 = 1023;
pub const DEPTH_FORMAT: i64 = 1026;

pub type TextureRef = Rc<RefCell<Texture>>;
pub type ImageRef = Rc<RefCell<Image>>;

/// RGBA pixels, 8 bits per channel, rows from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: i64,
    pub height: i64,
    pub depth: i64,
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(width: i64, height: i64, data: Vec<u8>) -> Image {
        Image { width, height, depth: 1, data }
    }

    pub fn shared(self) -> ImageRef {
        Rc::new(RefCell::new(self))
    }

    /// The pixel at (x, y) as 0..1 floats, coordinates clamped.
    pub fn texel(&self, x: i64, y: i64) -> Vec<f64> {
        let cx = x.max(0).min(self.width - 1);
        let cy = y.max(0).min(self.height - 1);
        let i = ((cy * self.width + cx) * 4) as usize;
        if i + 3 >= self.data.len() {
            return vec![1.0, 1.0, 1.0, 1.0];
        }
        vec![
            self.data[i] as f64 / 255.0,
            self.data[i + 1] as f64 / 255.0,
            self.data[i + 2] as f64 / 255.0,
            self.data[i + 3] as f64 / 255.0,
        ]
    }
}

#[derive(Clone)]
pub struct Texture {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub image: Option<ImageRef>,
    /// Six faces for a cube texture (+x, -x, +y, -y, +z, -z).
    pub faces: Vec<ImageRef>,
    pub mapping: i64,
    pub wrap_s: i64,
    pub wrap_t: i64,
    pub wrap_r: i64,
    pub mag_filter: i64,
    pub min_filter: i64,
    pub anisotropy: f64,
    pub format: i64,
    pub data_type: i64,
    pub offset: Vector2,
    pub repeat: Vector2,
    pub center: Vector2,
    pub rotation: f64,
    pub matrix_auto_update: bool,
    pub matrix: Matrix3,
    pub generate_mipmaps: bool,
    pub premultiply_alpha: bool,
    pub flip_y: bool,
    pub unpack_alignment: i64,
    pub color_space: ColorSpace,
    pub version: i64,
    pub is_cube: bool,
    pub is_data: bool,
}

impl Default for Texture {
    fn default() -> Texture {
        Texture::new()
    }
}

impl Texture {
    pub fn new() -> Texture {
        Texture {
            id: 0,
            uuid: String::new(),
            name: String::new(),
            image: None,
            faces: Vec::new(),
            mapping: UV_MAPPING,
            wrap_s: CLAMP_TO_EDGE_WRAPPING,
            wrap_t: CLAMP_TO_EDGE_WRAPPING,
            wrap_r: CLAMP_TO_EDGE_WRAPPING,
            mag_filter: LINEAR_FILTER,
            min_filter: LINEAR_MIPMAP_LINEAR_FILTER,
            anisotropy: 1.0,
            format: RGBA_FORMAT,
            data_type: UNSIGNED_BYTE_TYPE,
            offset: Vector2::new(0.0, 0.0),
            repeat: Vector2::new(1.0, 1.0),
            center: Vector2::new(0.0, 0.0),
            rotation: 0.0,
            matrix_auto_update: true,
            matrix: Matrix3::identity(),
            generate_mipmaps: true,
            premultiply_alpha: false,
            flip_y: true,
            unpack_alignment: 4,
            color_space: ColorSpace::Linear,
            version: 0,
            is_cube: false,
            is_data: false,
        }
    }

    /// A texture over raw RGBA pixels (three.js `DataTexture`: nearest
    /// filtering, no mipmaps, not flipped).
    pub fn data(image: Image) -> Texture {
        let mut t = Texture::new();
        t.image = Some(image.shared());
        t.mag_filter = NEAREST_FILTER;
        t.min_filter = NEAREST_FILTER;
        t.generate_mipmaps = false;
        t.flip_y = false;
        t.unpack_alignment = 1;
        t.is_data = true;
        t
    }

    pub fn cube(faces: Vec<ImageRef>) -> Texture {
        let mut t = Texture::new();
        t.faces = faces;
        t.mapping = CUBE_REFLECTION_MAPPING;
        t.flip_y = false;
        t.is_cube = true;
        t
    }

    pub fn shared(self) -> TextureRef {
        Rc::new(RefCell::new(self))
    }

    pub fn needs_update(&mut self) {
        self.version += 1;
    }

    /// `matrix` from offset, repeat, rotation and center.
    pub fn update_matrix(&mut self) {
        self.matrix.set_uv_transform(
            self.offset.x,
            self.offset.y,
            self.repeat.x,
            self.repeat.y,
            self.rotation,
            self.center.x,
            self.center.y,
        );
    }

    /// `uv` through the texture's transform and wrapping, as three.js's
    /// `transformUv` (for the software renderer and raycasting).
    pub fn transform_uv(&self, uv: &Vector2) -> Vector2 {
        let mut out = *uv;
        if self.mapping != UV_MAPPING {
            return out;
        }
        out.apply_matrix3(&self.matrix);
        out.x = wrap(out.x, self.wrap_s);
        out.y = wrap(out.y, self.wrap_t);
        if self.flip_y {
            out.y = 1.0 - out.y;
        }
        out
    }
}

fn wrap(v: f64, mode: i64) -> f64 {
    if v >= 0.0 && v <= 1.0 {
        return v;
    }
    if mode == REPEAT_WRAPPING {
        return v - v.floor();
    }
    if mode == MIRRORED_REPEAT_WRAPPING {
        let f = v.floor();
        let r = v - f;
        if (f.abs() as i64) % 2 == 1 {
            return 1.0 - r;
        }
        return r;
    }
    if v < 0.0 {
        0.0
    } else {
        1.0
    }
}
