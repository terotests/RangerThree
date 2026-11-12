//! How a surface looks (three.js's materials). One struct carries the
//! properties of every material type; `kind` says which shading model
//! uses them, and a property the kind does not read is simply unused.

use ranger::cell::RefCell;
use ranger::prelude::*;
use std::rc::Rc;

use crate::maths::color::Color;
use crate::maths::math_utils::INFINITY;
use crate::maths::vector2::Vector2;
use crate::textures::texture::TextureRef;

pub type MaterialRef = Rc<RefCell<Material>>;

/// three.js's material classes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MaterialKind {
    MeshBasic,
    MeshLambert,
    MeshPhong,
    MeshStandard,
    MeshPhysical,
    MeshToon,
    MeshNormal,
    MeshMatcap,
    MeshDepth,
    MeshDistance,
    Shadow,
    LineBasic,
    LineDashed,
    Points,
    Sprite,
    Shader,
    RawShader,
}

impl MaterialKind {
    /// three.js's `type` string.
    pub fn type_name(&self) -> String {
        match self {
            MaterialKind::MeshBasic => "MeshBasicMaterial".to_string(),
            MaterialKind::MeshLambert => "MeshLambertMaterial".to_string(),
            MaterialKind::MeshPhong => "MeshPhongMaterial".to_string(),
            MaterialKind::MeshStandard => "MeshStandardMaterial".to_string(),
            MaterialKind::MeshPhysical => "MeshPhysicalMaterial".to_string(),
            MaterialKind::MeshToon => "MeshToonMaterial".to_string(),
            MaterialKind::MeshNormal => "MeshNormalMaterial".to_string(),
            MaterialKind::MeshMatcap => "MeshMatcapMaterial".to_string(),
            MaterialKind::MeshDepth => "MeshDepthMaterial".to_string(),
            MaterialKind::MeshDistance => "MeshDistanceMaterial".to_string(),
            MaterialKind::Shadow => "ShadowMaterial".to_string(),
            MaterialKind::LineBasic => "LineBasicMaterial".to_string(),
            MaterialKind::LineDashed => "LineDashedMaterial".to_string(),
            MaterialKind::Points => "PointsMaterial".to_string(),
            MaterialKind::Sprite => "SpriteMaterial".to_string(),
            MaterialKind::Shader => "ShaderMaterial".to_string(),
            MaterialKind::RawShader => "RawShaderMaterial".to_string(),
        }
    }

    /// Whether lights shade it.
    pub fn is_lit(&self) -> bool {
        match self {
            MaterialKind::MeshLambert
            | MaterialKind::MeshPhong
            | MaterialKind::MeshStandard
            | MaterialKind::MeshPhysical
            | MaterialKind::MeshToon
            | MaterialKind::Shadow => true,
            _ => false,
        }
    }
}

/// Which faces are drawn (values as three.js's constants).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Front,
    Back,
    Double,
}

/// How a fragment combines with what is drawn (three.js's values 0..5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Blending {
    None,
    Normal,
    Additive,
    Subtractive,
    Multiply,
    Custom,
}

#[derive(Clone)]
pub struct Material {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub kind: MaterialKind,
    pub version: i64,
    // -- every material
    pub side: Side,
    pub shadow_side: Option<Side>,
    pub opacity: f64,
    pub transparent: bool,
    pub alpha_test: f64,
    pub alpha_hash: bool,
    pub blending: Blending,
    pub depth_test: bool,
    pub depth_write: bool,
    pub color_write: bool,
    pub polygon_offset: bool,
    pub polygon_offset_factor: f64,
    pub polygon_offset_units: f64,
    pub dithering: bool,
    pub premultiplied_alpha: bool,
    pub tone_mapped: bool,
    pub visible: bool,
    pub vertex_colors: bool,
    pub fog: bool,
    pub wireframe: bool,
    pub wireframe_linewidth: f64,
    pub flat_shading: bool,
    // -- colours and maps
    pub color: Color,
    pub map: Option<TextureRef>,
    pub alpha_map: Option<TextureRef>,
    pub ao_map: Option<TextureRef>,
    pub ao_map_intensity: f64,
    pub light_map: Option<TextureRef>,
    pub light_map_intensity: f64,
    pub emissive: Color,
    pub emissive_intensity: f64,
    pub emissive_map: Option<TextureRef>,
    pub bump_map: Option<TextureRef>,
    pub bump_scale: f64,
    pub normal_map: Option<TextureRef>,
    pub normal_scale: Vector2,
    pub normal_map_type: i64,
    pub displacement_map: Option<TextureRef>,
    pub displacement_scale: f64,
    pub displacement_bias: f64,
    pub specular_map: Option<TextureRef>,
    pub env_map: Option<TextureRef>,
    pub env_map_intensity: f64,
    pub reflectivity: f64,
    pub refraction_ratio: f64,
    pub combine: i64,
    // -- Phong
    pub specular: Color,
    pub shininess: f64,
    // -- Standard / Physical
    pub roughness: f64,
    pub metalness: f64,
    pub roughness_map: Option<TextureRef>,
    pub metalness_map: Option<TextureRef>,
    pub clearcoat: f64,
    pub clearcoat_roughness: f64,
    pub ior: f64,
    pub transmission: f64,
    pub thickness: f64,
    pub attenuation_distance: f64,
    pub attenuation_color: Color,
    pub sheen: f64,
    pub sheen_roughness: f64,
    pub sheen_color: Color,
    pub iridescence: f64,
    pub iridescence_ior: f64,
    pub anisotropy: f64,
    pub specular_intensity: f64,
    pub specular_color: Color,
    pub dispersion: f64,
    // -- Toon / Matcap / Depth
    pub gradient_map: Option<TextureRef>,
    pub matcap: Option<TextureRef>,
    pub depth_packing: i64,
    // -- Points / Sprite / Lines
    pub size: f64,
    pub size_attenuation: bool,
    pub rotation: f64,
    pub linewidth: f64,
    pub linecap: String,
    pub linejoin: String,
    pub scale: f64,
    pub dash_size: f64,
    pub gap_size: f64,
}

impl Material {
    /// A material of `kind` with three.js's defaults for that kind.
    pub fn new(kind: MaterialKind) -> Material {
        let mut m = Material {
            id: 0,
            uuid: String::new(),
            name: String::new(),
            kind,
            version: 0,
            side: Side::Front,
            shadow_side: None,
            opacity: 1.0,
            transparent: false,
            alpha_test: 0.0,
            alpha_hash: false,
            blending: Blending::Normal,
            depth_test: true,
            depth_write: true,
            color_write: true,
            polygon_offset: false,
            polygon_offset_factor: 0.0,
            polygon_offset_units: 0.0,
            dithering: false,
            premultiplied_alpha: false,
            tone_mapped: true,
            visible: true,
            vertex_colors: false,
            fog: true,
            wireframe: false,
            wireframe_linewidth: 1.0,
            flat_shading: false,
            color: Color::new(1.0, 1.0, 1.0),
            map: None,
            alpha_map: None,
            ao_map: None,
            ao_map_intensity: 1.0,
            light_map: None,
            light_map_intensity: 1.0,
            emissive: Color::new(0.0, 0.0, 0.0),
            emissive_intensity: 1.0,
            emissive_map: None,
            bump_map: None,
            bump_scale: 1.0,
            normal_map: None,
            normal_scale: Vector2::new(1.0, 1.0),
            normal_map_type: 0,
            displacement_map: None,
            displacement_scale: 1.0,
            displacement_bias: 0.0,
            specular_map: None,
            env_map: None,
            env_map_intensity: 1.0,
            reflectivity: 1.0,
            refraction_ratio: 0.98,
            combine: 0,
            specular: Color::from_hex(0x111111),
            shininess: 30.0,
            roughness: 1.0,
            metalness: 0.0,
            roughness_map: None,
            metalness_map: None,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
            ior: 1.5,
            transmission: 0.0,
            thickness: 0.0,
            attenuation_distance: INFINITY,
            attenuation_color: Color::new(1.0, 1.0, 1.0),
            sheen: 0.0,
            sheen_roughness: 1.0,
            sheen_color: Color::new(0.0, 0.0, 0.0),
            iridescence: 0.0,
            iridescence_ior: 1.3,
            anisotropy: 0.0,
            specular_intensity: 1.0,
            specular_color: Color::new(1.0, 1.0, 1.0),
            dispersion: 0.0,
            gradient_map: None,
            matcap: None,
            depth_packing: 3200,
            size: 1.0,
            size_attenuation: true,
            rotation: 0.0,
            linewidth: 1.0,
            linecap: "round".to_string(),
            linejoin: "round".to_string(),
            scale: 1.0,
            dash_size: 3.0,
            gap_size: 1.0,
        };
        match kind {
            MaterialKind::MeshPhysical => {
                // three.js's physical material derives reflectivity from ior 1.5
                m.reflectivity = 0.5;
            }
            MaterialKind::Shadow => {
                m.color = Color::new(0.0, 0.0, 0.0);
                m.transparent = true;
            }
            MaterialKind::Sprite => {
                m.transparent = true;
            }
            MaterialKind::Points | MaterialKind::LineBasic | MaterialKind::LineDashed => {
                m.fog = true;
            }
            MaterialKind::MeshNormal | MaterialKind::MeshDepth | MaterialKind::MeshDistance => {
                m.fog = false;
            }
            _ => {}
        }
        m
    }

    pub fn shared(self) -> MaterialRef {
        Rc::new(RefCell::new(self))
    }

    pub fn type_name(&self) -> String {
        self.kind.type_name()
    }

    pub fn needs_update(&mut self) {
        self.version += 1;
    }

    /// A copy of `source` that keeps this material's id, uuid and version
    /// (three.js's `copy`; the textures are shared). Use as
    /// `*m = m.copied_from(&source)`.
    pub fn copied_from(&self, source: &Material) -> Material {
        let mut m = source.clone();
        m.id = self.id;
        m.uuid = self.uuid.clone();
        m.version = self.version;
        m
    }
}
