//! The scene graph (three.js `Object3D` and the classes built on it).
//!
//! An object is shared (`ObjectRef = Rc<RefCell<Object3D>>`): a parent holds
//! its children strongly and a child its parent weakly. What kind of object
//! it is (mesh, camera, light...) and that kind's data is `kind`.
//!
//! The rotation is kept as `quaternion` (with `rotation_order` for reading
//! it back as Euler angles); `rotation()` and `set_rotation()` convert.
//! Operations that need the object's place in the graph (add, remove,
//! world matrices, lookAt) are free functions taking the `ObjectRef`.

use ranger::cell::RefCell;
use ranger::prelude::*;
use std::rc::{Rc, Weak};

use super::buffer_attribute::BufferAttribute;
use super::buffer_geometry::{AttributeRef, BufferGeometry, GeometryRef};
use super::layers::Layers;
use crate::materials::material::{Material, MaterialKind, MaterialRef};
use crate::maths::color::Color;
use crate::maths::euler::{Euler, EulerOrder};
use crate::maths::math_utils::{ArrayKind, DEG2RAD, RAD2DEG};
use crate::maths::matrix4::Matrix4;
use crate::maths::quaternion::Quaternion;
use crate::maths::vector2::Vector2;
use crate::maths::vector3::Vector3;
use crate::scenes::fog::Fog;
use crate::textures::texture::TextureRef;

pub type ObjectRef = Rc<RefCell<Object3D>>;

/// What a perspective or orthographic camera projects (three.js's camera
/// properties; the ones the other projection does not use stay at their
/// defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewOffset {
    pub enabled: bool,
    pub full_width: f64,
    pub full_height: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone)]
pub struct CameraData {
    pub fov: f64,
    pub aspect: f64,
    pub near: f64,
    pub far: f64,
    pub zoom: f64,
    pub focus: f64,
    pub film_gauge: f64,
    pub film_offset: f64,
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub view: Option<ViewOffset>,
    pub projection_matrix: Matrix4,
    pub projection_matrix_inverse: Matrix4,
    pub matrix_world_inverse: Matrix4,
}

impl CameraData {
    pub fn perspective(fov: f64, aspect: f64, near: f64, far: f64) -> CameraData {
        CameraData {
            fov,
            aspect,
            near,
            far,
            zoom: 1.0,
            focus: 10.0,
            film_gauge: 35.0,
            film_offset: 0.0,
            left: -1.0,
            right: 1.0,
            top: 1.0,
            bottom: -1.0,
            view: None,
            projection_matrix: Matrix4::identity(),
            projection_matrix_inverse: Matrix4::identity(),
            matrix_world_inverse: Matrix4::identity(),
        }
    }

    pub fn orthographic(left: f64, right: f64, top: f64, bottom: f64, near: f64, far: f64) -> CameraData {
        let mut c = CameraData::perspective(50.0, 1.0, near, far);
        c.left = left;
        c.right = right;
        c.top = top;
        c.bottom = bottom;
        c
    }

    pub fn film_width(&self) -> f64 {
        self.film_gauge * self.aspect.min(1.0)
    }

    pub fn film_height(&self) -> f64 {
        self.film_gauge / self.aspect.max(1.0)
    }

    pub fn focal_length(&self) -> f64 {
        let v_extent_slope = (DEG2RAD * 0.5 * self.fov).tan();
        0.5 * self.film_height() / v_extent_slope
    }

    pub fn set_focal_length(&mut self, focal_length: f64) {
        let v_extent_slope = 0.5 * self.film_height() / focal_length;
        self.fov = RAD2DEG * 2.0 * v_extent_slope.atan();
    }

    pub fn effective_fov(&self) -> f64 {
        RAD2DEG * 2.0 * ((DEG2RAD * 0.5 * self.fov).tan() / self.zoom).atan()
    }

    /// The perspective frustum's half extent at `distance`: (min, max) corners.
    pub fn view_bounds(&self, distance: f64) -> Vec<Vector2> {
        let half_h = distance * (DEG2RAD * 0.5 * self.fov).tan() / self.zoom;
        let half_w = half_h * self.aspect;
        vec![Vector2::new(-half_w, -half_h), Vector2::new(half_w, half_h)]
    }

    pub fn update_perspective(&mut self) {
        let near = self.near;
        let mut top = near * (DEG2RAD * 0.5 * self.fov).tan() / self.zoom;
        let mut height = 2.0 * top;
        let mut width = self.aspect * height;
        let mut left = -0.5 * width;
        if let Some(view) = self.view.filter(|v| v.enabled) {
            left += view.offset_x * width / view.full_width;
            top -= view.offset_y * height / view.full_height;
            width *= view.width / view.full_width;
            height *= view.height / view.full_height;
        }
        let skew = self.film_offset;
        if skew != 0.0 {
            left += near * skew / self.film_width();
        }
        let far = self.far;
        self.projection_matrix.make_perspective(left, left + width, top, top - height, near, far);
        self.projection_matrix_inverse = self.projection_matrix;
        self.projection_matrix_inverse.invert();
    }

    pub fn update_orthographic(&mut self) {
        let dx = (self.right - self.left) / (2.0 * self.zoom);
        let dy = (self.top - self.bottom) / (2.0 * self.zoom);
        let cx = (self.right + self.left) / 2.0;
        let cy = (self.top + self.bottom) / 2.0;
        let mut left = cx - dx;
        let mut right = cx + dx;
        let mut top = cy + dy;
        let mut bottom = cy - dy;
        if let Some(view) = self.view.filter(|v| v.enabled) {
            let scale_w = (self.right - self.left) / view.full_width / self.zoom;
            let scale_h = (self.top - self.bottom) / view.full_height / self.zoom;
            left += scale_w * view.offset_x;
            right = left + scale_w * view.width;
            top -= scale_h * view.offset_y;
            bottom = top - scale_h * view.height;
        }
        self.projection_matrix.make_orthographic(left, right, top, bottom, self.near, self.far);
        self.projection_matrix_inverse = self.projection_matrix;
        self.projection_matrix_inverse.invert();
    }
}

/// A light's shadow: the camera it renders depth from and its filtering.
#[derive(Clone)]
pub struct LightShadow {
    pub camera: ObjectRef,
    pub bias: f64,
    pub normal_bias: f64,
    pub radius: f64,
    pub blur_samples: i64,
    pub intensity: f64,
    pub map_size: Vector2,
    pub auto_update: bool,
    pub needs_update: bool,
}

impl LightShadow {
    pub fn new(camera: Object3D) -> LightShadow {
        LightShadow {
            camera: camera.shared(),
            bias: 0.0,
            normal_bias: 0.0,
            radius: 1.0,
            blur_samples: 8,
            intensity: 1.0,
            map_size: Vector2::new(512.0, 512.0),
            auto_update: true,
            needs_update: false,
        }
    }
}

/// Every light's properties; each light kind reads its own.
#[derive(Clone)]
pub struct LightData {
    pub color: Color,
    pub intensity: f64,
    pub ground_color: Color,
    pub distance: f64,
    pub decay: f64,
    pub angle: f64,
    pub penumbra: f64,
    pub width: f64,
    pub height: f64,
    pub target: Option<ObjectRef>,
    pub shadow: Option<LightShadow>,
    pub map: Option<TextureRef>,
}

impl LightData {
    pub fn new(color: Color, intensity: f64) -> LightData {
        LightData {
            color,
            intensity,
            ground_color: Color::new(1.0, 1.0, 1.0),
            distance: 0.0,
            decay: 2.0,
            angle: std::f64::consts::PI / 3.0,
            penumbra: 0.0,
            width: 10.0,
            height: 10.0,
            target: None,
            shadow: None,
            map: None,
        }
    }
}

/// A mesh's (or points', or lines') geometry and materials.
#[derive(Clone)]
pub struct MeshData {
    pub geometry: GeometryRef,
    pub materials: Vec<MaterialRef>,
    /// three.js was given an array of materials (one per geometry group).
    pub material_is_array: bool,
    pub morph_target_influences: Vec<f64>,
    pub morph_target_dictionary: Map<String, i64>,
    /// InstancedMesh: one 4x4 matrix per instance (16 floats each).
    pub instance_matrix: Option<AttributeRef>,
    pub instance_color: Option<AttributeRef>,
    pub count: i64,
}

impl MeshData {
    pub fn new(geometry: GeometryRef, material: MaterialRef) -> MeshData {
        MeshData {
            geometry,
            materials: vec![material],
            material_is_array: false,
            morph_target_influences: Vec::new(),
            morph_target_dictionary: Map::new(),
            instance_matrix: None,
            instance_color: None,
            count: 1,
        }
    }

    pub fn material(&self) -> MaterialRef {
        self.materials[0].clone()
    }
}

/// What a scene draws behind everything.
#[derive(Clone)]
pub enum Background {
    None,
    Color(Color),
    Texture(TextureRef),
}

#[derive(Clone)]
pub struct SceneData {
    pub background: Background,
    pub background_blurriness: f64,
    pub background_intensity: f64,
    pub environment: Option<TextureRef>,
    pub environment_intensity: f64,
    pub fog: Option<Fog>,
    pub override_material: Option<MaterialRef>,
}

impl SceneData {
    pub fn new() -> SceneData {
        SceneData {
            background: Background::None,
            background_blurriness: 0.0,
            background_intensity: 1.0,
            environment: None,
            environment_intensity: 1.0,
            fog: None,
            override_material: None,
        }
    }
}

#[derive(Clone)]
pub struct LodLevel {
    pub object: ObjectRef,
    pub distance: f64,
    pub hysteresis: f64,
}

/// A level-of-detail switch: the levels by distance, the one shown last.
#[derive(Clone)]
pub struct LodData {
    pub levels: Vec<LodLevel>,
    pub current_level: i64,
    pub auto_update: bool,
}

#[derive(Clone)]
pub struct SpriteData {
    pub geometry: GeometryRef,
    pub material: MaterialRef,
    pub center: Vector2,
}

#[derive(Clone)]
pub enum ObjectKind {
    Object3D,
    Group,
    Bone,
    Scene(SceneData),
    Mesh(MeshData),
    InstancedMesh(MeshData),
    Points(MeshData),
    Line(MeshData),
    LineSegments(MeshData),
    LineLoop(MeshData),
    Sprite(SpriteData),
    PerspectiveCamera(CameraData),
    OrthographicCamera(CameraData),
    AmbientLight(LightData),
    HemisphereLight(LightData),
    DirectionalLight(LightData),
    PointLight(LightData),
    SpotLight(LightData),
    RectAreaLight(LightData),
    Lod(LodData),
}

impl ObjectKind {
    /// three.js's `type` string.
    pub fn type_name(&self) -> String {
        match self {
            ObjectKind::Object3D => "Object3D".to_string(),
            ObjectKind::Group => "Group".to_string(),
            ObjectKind::Bone => "Bone".to_string(),
            ObjectKind::Scene(_) => "Scene".to_string(),
            ObjectKind::Mesh(_) => "Mesh".to_string(),
            ObjectKind::InstancedMesh(_) => "InstancedMesh".to_string(),
            ObjectKind::Points(_) => "Points".to_string(),
            ObjectKind::Line(_) => "Line".to_string(),
            ObjectKind::LineSegments(_) => "LineSegments".to_string(),
            ObjectKind::LineLoop(_) => "LineLoop".to_string(),
            ObjectKind::Sprite(_) => "Sprite".to_string(),
            ObjectKind::PerspectiveCamera(_) => "PerspectiveCamera".to_string(),
            ObjectKind::OrthographicCamera(_) => "OrthographicCamera".to_string(),
            ObjectKind::AmbientLight(_) => "AmbientLight".to_string(),
            ObjectKind::HemisphereLight(_) => "HemisphereLight".to_string(),
            ObjectKind::DirectionalLight(_) => "DirectionalLight".to_string(),
            ObjectKind::PointLight(_) => "PointLight".to_string(),
            ObjectKind::SpotLight(_) => "SpotLight".to_string(),
            ObjectKind::RectAreaLight(_) => "RectAreaLight".to_string(),
            ObjectKind::Lod(_) => "LOD".to_string(),
        }
    }
}

pub struct Object3D {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub kind: ObjectKind,
    pub parent: Option<Weak<RefCell<Object3D>>>,
    pub children: Vec<ObjectRef>,
    pub up: Vector3,
    pub position: Vector3,
    pub quaternion: Quaternion,
    pub rotation_order: EulerOrder,
    pub scale: Vector3,
    pub matrix: Matrix4,
    pub matrix_world: Matrix4,
    pub matrix_auto_update: bool,
    pub matrix_world_auto_update: bool,
    pub matrix_world_needs_update: bool,
    pub layers: Layers,
    pub visible: bool,
    pub cast_shadow: bool,
    pub receive_shadow: bool,
    pub frustum_culled: bool,
    pub render_order: f64,
}

impl Object3D {
    pub fn new(kind: ObjectKind) -> Object3D {
        Object3D {
            id: 0,
            uuid: String::new(),
            name: String::new(),
            kind,
            parent: None,
            children: Vec::new(),
            up: Vector3::new(0.0, 1.0, 0.0),
            position: Vector3::zero(),
            quaternion: Quaternion::identity(),
            rotation_order: EulerOrder::Xyz,
            scale: Vector3::one(),
            matrix: Matrix4::identity(),
            matrix_world: Matrix4::identity(),
            matrix_auto_update: true,
            matrix_world_auto_update: true,
            matrix_world_needs_update: false,
            layers: Layers::new(),
            visible: true,
            cast_shadow: false,
            receive_shadow: false,
            frustum_culled: true,
            render_order: 0.0,
        }
    }

    pub fn shared(self) -> ObjectRef {
        Rc::new(RefCell::new(self))
    }

    pub fn group() -> Object3D {
        Object3D::new(ObjectKind::Group)
    }

    pub fn scene() -> Object3D {
        Object3D::new(ObjectKind::Scene(SceneData::new()))
    }

    pub fn mesh(geometry: GeometryRef, material: MaterialRef) -> Object3D {
        Object3D::new(ObjectKind::Mesh(MeshData::new(geometry, material)))
    }

    /// A mesh drawn `count` times, each with its own matrix (identity) and
    /// no per-instance colour yet.
    pub fn instanced_mesh(geometry: GeometryRef, material: MaterialRef, count: i64) -> Object3D {
        let mut data = MeshData::new(geometry, material);
        let mut array: Vec<f64> = Vec::new();
        for _ in 0..count {
            for k in 0..16 {
                array.push(if k % 5 == 0 { 1.0 } else { 0.0 });
            }
        }
        data.instance_matrix = Some(BufferAttribute::float32(array, 16).shared());
        data.count = count;
        Object3D::new(ObjectKind::InstancedMesh(data))
    }

    pub fn points(geometry: GeometryRef, material: MaterialRef) -> Object3D {
        Object3D::new(ObjectKind::Points(MeshData::new(geometry, material)))
    }

    pub fn line(geometry: GeometryRef, material: MaterialRef) -> Object3D {
        Object3D::new(ObjectKind::Line(MeshData::new(geometry, material)))
    }

    pub fn line_segments(geometry: GeometryRef, material: MaterialRef) -> Object3D {
        Object3D::new(ObjectKind::LineSegments(MeshData::new(geometry, material)))
    }

    pub fn line_loop(geometry: GeometryRef, material: MaterialRef) -> Object3D {
        Object3D::new(ObjectKind::LineLoop(MeshData::new(geometry, material)))
    }

    /// A sprite: a unit quad facing the camera.
    pub fn sprite(material: MaterialRef) -> Object3D {
        let mut g = BufferGeometry::new();
        g.set_attribute(
            "position",
            BufferAttribute::float32(vec![-0.5, -0.5, 0.0, 0.5, -0.5, 0.0, 0.5, 0.5, 0.0, -0.5, 0.5, 0.0], 3).shared(),
        );
        g.set_attribute("uv", BufferAttribute::float32(vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0], 2).shared());
        g.set_index(vec![0.0, 1.0, 2.0, 0.0, 2.0, 3.0]);
        Object3D::new(ObjectKind::Sprite(SpriteData {
            geometry: g.shared(),
            material,
            center: Vector2::new(0.5, 0.5),
        }))
    }

    pub fn perspective_camera(fov: f64, aspect: f64, near: f64, far: f64) -> Object3D {
        let mut c = CameraData::perspective(fov, aspect, near, far);
        c.update_perspective();
        Object3D::new(ObjectKind::PerspectiveCamera(c))
    }

    pub fn orthographic_camera(left: f64, right: f64, top: f64, bottom: f64, near: f64, far: f64) -> Object3D {
        let mut c = CameraData::orthographic(left, right, top, bottom, near, far);
        c.update_orthographic();
        Object3D::new(ObjectKind::OrthographicCamera(c))
    }

    pub fn ambient_light(color: Color, intensity: f64) -> Object3D {
        Object3D::new(ObjectKind::AmbientLight(LightData::new(color, intensity)))
    }

    pub fn hemisphere_light(sky: Color, ground: Color, intensity: f64) -> Object3D {
        let mut d = LightData::new(sky, intensity);
        d.ground_color = ground;
        let mut o = Object3D::new(ObjectKind::HemisphereLight(d));
        o.position.set(0.0, 1.0, 0.0);
        o
    }

    pub fn directional_light(color: Color, intensity: f64) -> Object3D {
        let mut d = LightData::new(color, intensity);
        d.target = Some(Object3D::new(ObjectKind::Object3D).shared());
        d.shadow = Some(LightShadow::new(Object3D::orthographic_camera(-5.0, 5.0, 5.0, -5.0, 0.5, 500.0)));
        let mut o = Object3D::new(ObjectKind::DirectionalLight(d));
        o.position.set(0.0, 1.0, 0.0);
        o
    }

    pub fn point_light(color: Color, intensity: f64, distance: f64, decay: f64) -> Object3D {
        let mut d = LightData::new(color, intensity);
        d.distance = distance;
        d.decay = decay;
        d.shadow = Some(LightShadow::new(Object3D::perspective_camera(90.0, 1.0, 0.5, 500.0)));
        Object3D::new(ObjectKind::PointLight(d))
    }

    pub fn spot_light(color: Color, intensity: f64, distance: f64, angle: f64, penumbra: f64, decay: f64) -> Object3D {
        let mut d = LightData::new(color, intensity);
        d.distance = distance;
        d.angle = angle;
        d.penumbra = penumbra;
        d.decay = decay;
        d.target = Some(Object3D::new(ObjectKind::Object3D).shared());
        d.shadow = Some(LightShadow::new(Object3D::perspective_camera(50.0, 1.0, 0.5, 500.0)));
        let mut o = Object3D::new(ObjectKind::SpotLight(d));
        o.position.set(0.0, 1.0, 0.0);
        o
    }

    pub fn rect_area_light(color: Color, intensity: f64, width: f64, height: f64) -> Object3D {
        let mut d = LightData::new(color, intensity);
        d.width = width;
        d.height = height;
        Object3D::new(ObjectKind::RectAreaLight(d))
    }

    pub fn lod() -> Object3D {
        Object3D::new(ObjectKind::Lod(LodData { levels: Vec::new(), current_level: 0, auto_update: true }))
    }

    pub fn type_name(&self) -> String {
        self.kind.type_name()
    }

    pub fn is_camera(&self) -> bool {
        match &self.kind {
            ObjectKind::PerspectiveCamera(_) => true,
            ObjectKind::OrthographicCamera(_) => true,
            _ => false,
        }
    }

    pub fn is_light(&self) -> bool {
        match &self.kind {
            ObjectKind::AmbientLight(_) => true,
            ObjectKind::HemisphereLight(_) => true,
            ObjectKind::DirectionalLight(_) => true,
            ObjectKind::PointLight(_) => true,
            ObjectKind::SpotLight(_) => true,
            ObjectKind::RectAreaLight(_) => true,
            _ => false,
        }
    }

    pub fn camera(&self) -> Option<&CameraData> {
        match &self.kind {
            ObjectKind::PerspectiveCamera(c) => Some(c),
            ObjectKind::OrthographicCamera(c) => Some(c),
            _ => None,
        }
    }

    pub fn camera_mut(&mut self) -> Option<&mut CameraData> {
        match &mut self.kind {
            ObjectKind::PerspectiveCamera(c) => Some(c),
            ObjectKind::OrthographicCamera(c) => Some(c),
            _ => None,
        }
    }

    pub fn light(&self) -> Option<&LightData> {
        match &self.kind {
            ObjectKind::AmbientLight(l) => Some(l),
            ObjectKind::HemisphereLight(l) => Some(l),
            ObjectKind::DirectionalLight(l) => Some(l),
            ObjectKind::PointLight(l) => Some(l),
            ObjectKind::SpotLight(l) => Some(l),
            ObjectKind::RectAreaLight(l) => Some(l),
            _ => None,
        }
    }

    pub fn light_mut(&mut self) -> Option<&mut LightData> {
        match &mut self.kind {
            ObjectKind::AmbientLight(l) => Some(l),
            ObjectKind::HemisphereLight(l) => Some(l),
            ObjectKind::DirectionalLight(l) => Some(l),
            ObjectKind::PointLight(l) => Some(l),
            ObjectKind::SpotLight(l) => Some(l),
            ObjectKind::RectAreaLight(l) => Some(l),
            _ => None,
        }
    }

    /// The geometry and materials of a mesh, instanced mesh, points or line.
    pub fn mesh_data(&self) -> Option<&MeshData> {
        match &self.kind {
            ObjectKind::Mesh(m) => Some(m),
            ObjectKind::InstancedMesh(m) => Some(m),
            ObjectKind::Points(m) => Some(m),
            ObjectKind::Line(m) => Some(m),
            ObjectKind::LineSegments(m) => Some(m),
            ObjectKind::LineLoop(m) => Some(m),
            _ => None,
        }
    }

    pub fn mesh_data_mut(&mut self) -> Option<&mut MeshData> {
        match &mut self.kind {
            ObjectKind::Mesh(m) => Some(m),
            ObjectKind::InstancedMesh(m) => Some(m),
            ObjectKind::Points(m) => Some(m),
            ObjectKind::Line(m) => Some(m),
            ObjectKind::LineSegments(m) => Some(m),
            ObjectKind::LineLoop(m) => Some(m),
            _ => None,
        }
    }

    pub fn scene_data(&self) -> Option<&SceneData> {
        match &self.kind {
            ObjectKind::Scene(s) => Some(s),
            _ => None,
        }
    }

    pub fn scene_data_mut(&mut self) -> Option<&mut SceneData> {
        match &mut self.kind {
            ObjectKind::Scene(s) => Some(s),
            _ => None,
        }
    }

    /// The rotation as Euler angles in `rotation_order`.
    pub fn rotation_euler(&self) -> Euler {
        let mut e = Euler::default();
        e.set_from_quaternion(&self.quaternion, self.rotation_order);
        e
    }

    pub fn set_rotation(&mut self, e: &Euler) {
        self.rotation_order = e.order;
        self.quaternion.set_from_euler(e);
    }

    pub fn set_rotation_from_axis_angle(&mut self, axis: &Vector3, angle: f64) {
        self.quaternion.set_from_axis_angle(axis, angle);
    }

    pub fn set_rotation_from_euler(&mut self, e: &Euler) {
        self.quaternion.set_from_euler(e);
    }

    /// The rotation of `m`'s upper 3x3, which must be unscaled.
    pub fn set_rotation_from_matrix(&mut self, m: &Matrix4) {
        self.quaternion.set_from_rotation_matrix(m);
    }

    pub fn set_rotation_from_quaternion(&mut self, q: &Quaternion) {
        self.quaternion = *q;
    }

    pub fn apply_matrix4(&mut self, m: &Matrix4) {
        if self.matrix_auto_update {
            self.update_matrix();
        }
        self.matrix.premultiply(m);
        let t = self.matrix.decompose();
        self.position = t.position;
        self.quaternion = t.quaternion;
        self.scale = t.scale;
    }

    pub fn apply_quaternion(&mut self, q: &Quaternion) {
        self.quaternion.premultiply(q);
    }

    /// Rotates around `axis` (unit, local space).
    pub fn rotate_on_axis(&mut self, axis: &Vector3, angle: f64) {
        let q = Quaternion::from_axis_angle(axis, angle);
        self.quaternion.multiply(&q);
    }

    /// Rotates around `axis` (unit, world space; assumes no rotated parent).
    pub fn rotate_on_world_axis(&mut self, axis: &Vector3, angle: f64) {
        let q = Quaternion::from_axis_angle(axis, angle);
        self.quaternion.premultiply(&q);
    }

    pub fn rotate_x(&mut self, angle: f64) {
        self.rotate_on_axis(&Vector3::new(1.0, 0.0, 0.0), angle);
    }

    pub fn rotate_y(&mut self, angle: f64) {
        self.rotate_on_axis(&Vector3::new(0.0, 1.0, 0.0), angle);
    }

    pub fn rotate_z(&mut self, angle: f64) {
        self.rotate_on_axis(&Vector3::new(0.0, 0.0, 1.0), angle);
    }

    /// Moves `distance` along `axis` (unit, local space).
    pub fn translate_on_axis(&mut self, axis: &Vector3, distance: f64) {
        let mut v = *axis;
        v.apply_quaternion(&self.quaternion);
        self.position.add_scaled_vector(&v, distance);
    }

    pub fn translate_x(&mut self, distance: f64) {
        self.translate_on_axis(&Vector3::new(1.0, 0.0, 0.0), distance);
    }

    pub fn translate_y(&mut self, distance: f64) {
        self.translate_on_axis(&Vector3::new(0.0, 1.0, 0.0), distance);
    }

    pub fn translate_z(&mut self, distance: f64) {
        self.translate_on_axis(&Vector3::new(0.0, 0.0, 1.0), distance);
    }

    pub fn local_to_world(&self, v: &Vector3) -> Vector3 {
        let mut out = *v;
        out.apply_matrix4(&self.matrix_world);
        out
    }

    pub fn world_to_local(&self, v: &Vector3) -> Vector3 {
        let mut inv = self.matrix_world;
        inv.invert();
        let mut out = *v;
        out.apply_matrix4(&inv);
        out
    }

    pub fn update_matrix(&mut self) {
        self.matrix.compose(&self.position, &self.quaternion, &self.scale);
        self.matrix_world_needs_update = true;
    }

    /// The direction the object's +z (a camera's or light's -z) points in
    /// world space, from `matrix_world` as it is.
    pub fn world_direction(&self) -> Vector3 {
        let e = self.matrix_world.elements.clone();
        let mut d = Vector3::new(e[8], e[9], e[10]);
        if self.is_camera() {
            d.negate();
        }
        d.normalize();
        d
    }

    pub fn world_position(&self) -> Vector3 {
        let mut v = Vector3::zero();
        v.set_from_matrix_position(&self.matrix_world);
        v
    }

    pub fn world_quaternion(&self) -> Quaternion {
        self.matrix_world.decompose().quaternion
    }

    pub fn world_scale(&self) -> Vector3 {
        self.matrix_world.decompose().scale
    }

    pub fn update_projection_matrix(&mut self) {
        match &mut self.kind {
            ObjectKind::PerspectiveCamera(c) => c.update_perspective(),
            ObjectKind::OrthographicCamera(c) => c.update_orthographic(),
            _ => {}
        }
    }

    pub fn set_view_offset(&mut self, full_width: f64, full_height: f64, x: f64, y: f64, width: f64, height: f64) {
        let perspective = matches!(self.kind, ObjectKind::PerspectiveCamera(_));
        if let Some(c) = self.camera_mut() {
            if perspective {
                c.aspect = full_width / full_height;
            }
            c.view = Some(ViewOffset {
                enabled: true,
                full_width,
                full_height,
                offset_x: x,
                offset_y: y,
                width,
                height,
            });
        }
        self.update_projection_matrix();
    }

    pub fn clear_view_offset(&mut self) {
        if let Some(c) = self.camera_mut() {
            if let Some(mut v) = c.view {
                v.enabled = false;
                c.view = Some(v);
            }
        }
        self.update_projection_matrix();
    }

    /// A copy of the object's own properties (not its children or parent).
    /// Geometry and materials are shared with the original, as three.js does.
    pub fn copy_props(&self) -> Object3D {
        let mut o = Object3D::new(self.kind.clone());
        o.name = self.name.clone();
        o.up = self.up;
        o.position = self.position;
        o.quaternion = self.quaternion;
        o.rotation_order = self.rotation_order;
        o.scale = self.scale;
        o.matrix = self.matrix;
        o.matrix_world = self.matrix_world;
        o.matrix_auto_update = self.matrix_auto_update;
        o.matrix_world_auto_update = self.matrix_world_auto_update;
        o.matrix_world_needs_update = self.matrix_world_needs_update;
        o.layers = self.layers;
        o.visible = self.visible;
        o.cast_shadow = self.cast_shadow;
        o.receive_shadow = self.receive_shadow;
        o.frustum_culled = self.frustum_culled;
        o.render_order = self.render_order;
        o
    }
}

// ---- the graph -------------------------------------------------------------

pub fn parent_of(object: &ObjectRef) -> Option<ObjectRef> {
    match &object.borrow().parent {
        Some(w) => w.upgrade(),
        None => None,
    }
}

/// Adds `child` to `parent`, taking it from its old parent first.
pub fn add(parent: &ObjectRef, child: &ObjectRef) {
    if Rc::ptr_eq(parent, child) {
        return;
    }
    if let Some(old) = parent_of(child) {
        remove(&old, child);
    }
    child.borrow_mut().parent = Some(Rc::downgrade(parent));
    parent.borrow_mut().children.push(child.clone());
}

/// Removes `child` from `parent` (nothing if it is not a child of it).
pub fn remove(parent: &ObjectRef, child: &ObjectRef) -> bool {
    let mut kept: Vec<ObjectRef> = Vec::new();
    let mut found = false;
    for c in parent.borrow().children.iter() {
        if Rc::ptr_eq(c, child) {
            found = true;
        } else {
            kept.push(c.clone());
        }
    }
    if found {
        parent.borrow_mut().children = kept;
        child.borrow_mut().parent = None;
    }
    found
}

pub fn remove_from_parent(object: &ObjectRef) {
    if let Some(p) = parent_of(object) {
        remove(&p, object);
    }
}

pub fn clear(object: &ObjectRef) {
    let kids: Vec<ObjectRef> = object.borrow().children.clone();
    for c in kids.iter() {
        c.borrow_mut().parent = None;
    }
    object.borrow_mut().children.clear();
}

/// Moves `child` under `parent` keeping its world transform.
pub fn attach(parent: &ObjectRef, child: &ObjectRef) {
    update_world_matrix(parent, true, false);
    let mut m = parent.borrow().matrix_world;
    m.invert();
    if let Some(old) = parent_of(child) {
        update_world_matrix(&old, true, false);
        let ow = old.borrow().matrix_world;
        m.multiply(&ow);
    }
    child.borrow_mut().apply_matrix4(&m);
    add(parent, child);
    update_world_matrix(child, false, true);
}

/// The object and everything under it, depth first (three.js `traverse` order).
pub fn descendants(object: &ObjectRef) -> Vec<ObjectRef> {
    let mut out: Vec<ObjectRef> = Vec::new();
    collect(object, &mut out, false);
    out
}

/// As `descendants`, skipping invisible objects and what is under them.
pub fn visible_descendants(object: &ObjectRef) -> Vec<ObjectRef> {
    let mut out: Vec<ObjectRef> = Vec::new();
    collect(object, &mut out, true);
    out
}

fn collect(object: &ObjectRef, out: &mut Vec<ObjectRef>, only_visible: bool) {
    if only_visible && !object.borrow().visible {
        return;
    }
    out.push(object.clone());
    let kids: Vec<ObjectRef> = object.borrow().children.clone();
    for c in kids.iter() {
        collect(c, out, only_visible);
    }
}

/// The parent, its parent and so on up to the root.
pub fn ancestors(object: &ObjectRef) -> Vec<ObjectRef> {
    let mut out: Vec<ObjectRef> = Vec::new();
    let mut at = parent_of(object);
    while let Some(p) = at {
        out.push(p.clone());
        at = parent_of(&p);
    }
    out
}

pub fn get_object_by_id(root: &ObjectRef, id: i64) -> Option<ObjectRef> {
    for o in descendants(root).iter() {
        if o.borrow().id == id {
            return Some(o.clone());
        }
    }
    None
}

pub fn get_object_by_name(root: &ObjectRef, name: &str) -> Option<ObjectRef> {
    for o in descendants(root).iter() {
        if o.borrow().name == name {
            return Some(o.clone());
        }
    }
    None
}

/// `matrix_world` of the object and, with `force` or when they changed,
/// of everything under it (three.js `updateMatrixWorld`).
pub fn update_matrix_world(object: &ObjectRef, force: bool) {
    let parent_world = match parent_of(object) {
        Some(p) => Some(p.borrow().matrix_world),
        None => None,
    };
    let mut force_children = force;
    {
        let mut o = object.borrow_mut();
        if o.matrix_auto_update {
            o.update_matrix();
        }
        if o.matrix_world_needs_update || force {
            if o.matrix_world_auto_update {
                match &parent_world {
                    Some(pw) => {
                        let local = o.matrix;
                        o.matrix_world.multiply_matrices(pw, &local);
                    }
                    None => {
                        o.matrix_world = o.matrix;
                    }
                }
            }
            o.matrix_world_needs_update = false;
            force_children = true;
        }
        sync_camera(&mut o);
    }
    let kids: Vec<ObjectRef> = object.borrow().children.clone();
    for c in kids.iter() {
        update_matrix_world(c, force_children);
    }
}

/// `matrix_world` from the parents down (`update_parents`) and/or of the
/// children (three.js `updateWorldMatrix`).
pub fn update_world_matrix(object: &ObjectRef, update_parents: bool, update_children: bool) {
    let parent = parent_of(object);
    if update_parents {
        if let Some(p) = &parent {
            update_world_matrix(p, true, false);
        }
    }
    let parent_world = match &parent {
        Some(p) => Some(p.borrow().matrix_world),
        None => None,
    };
    {
        let mut o = object.borrow_mut();
        if o.matrix_auto_update {
            o.update_matrix();
        }
        if o.matrix_world_auto_update {
            match &parent_world {
                Some(pw) => {
                    let local = o.matrix;
                    o.matrix_world.multiply_matrices(pw, &local);
                }
                None => {
                    o.matrix_world = o.matrix;
                }
            }
        }
        sync_camera(&mut o);
    }
    if update_children {
        let kids: Vec<ObjectRef> = object.borrow().children.clone();
        for c in kids.iter() {
            update_world_matrix(c, false, true);
        }
    }
}

fn sync_camera(o: &mut Object3D) {
    let world = o.matrix_world;
    if let Some(c) = o.camera_mut() {
        c.matrix_world_inverse = world;
        c.matrix_world_inverse.invert();
    }
}

/// Turns the object to face `target` (world space). Cameras and lights
/// point their -z at it, everything else its +z.
pub fn look_at(object: &ObjectRef, target: &Vector3) {
    update_world_matrix(object, true, false);
    let parent = parent_of(object);
    let mut o = object.borrow_mut();
    let mut position = Vector3::zero();
    position.set_from_matrix_position(&o.matrix_world);
    let mut m = Matrix4::identity();
    let up = o.up;
    if o.is_camera() || o.is_light() {
        m.look_at(&position, target, &up);
    } else {
        m.look_at(target, &position, &up);
    }
    o.quaternion.set_from_rotation_matrix(&m);
    if let Some(p) = &parent {
        let mut pm = Matrix4::identity();
        pm.extract_rotation(&p.borrow().matrix_world);
        let mut q = Quaternion::identity();
        q.set_from_rotation_matrix(&pm);
        q.invert();
        o.quaternion.premultiply(&q);
    }
}

/// A copy of `object` and, with `recursive`, of everything under it.
pub fn clone_object(object: &ObjectRef, recursive: bool) -> ObjectRef {
    let copy = object.borrow().copy_props().shared();
    if recursive {
        let kids: Vec<ObjectRef> = object.borrow().children.clone();
        for c in kids.iter() {
            let cc = clone_object(c, true);
            add(&copy, &cc);
        }
    }
    copy
}

/// A default material (three.js's `new Mesh()` has a white MeshBasicMaterial).
pub fn default_material() -> MaterialRef {
    Material::new(MaterialKind::MeshBasic).shared()
}

pub fn default_geometry() -> GeometryRef {
    BufferGeometry::new().shared()
}

/// Sets `count` instance colours to white, so that `set_color_at` can write.
pub fn ensure_instance_colors(data: &mut MeshData) {
    if data.instance_color.is_none() {
        let n = data.count as usize;
        data.instance_color = Some(BufferAttribute::new(vec![1.0; n * 3], 3, ArrayKind::Float32, false).shared());
    }
}

/// A mesh vertex's position with the morph targets applied (three.js
/// `Mesh.getVertexPosition`).
pub fn vertex_position(data: &MeshData, index: usize) -> Vector3 {
    let g = data.geometry.borrow();
    let mut target = match g.get_attribute("position") {
        Some(p) => p.borrow().get_vector3(index),
        None => return Vector3::zero(),
    };
    if let Some(morphs) = g.morph_attributes.get("position") {
        let influences = &data.morph_target_influences;
        let mut morph = Vector3::zero();
        for (i, attr) in morphs.iter().enumerate() {
            if i >= influences.len() {
                break;
            }
            let influence = influences[i];
            if influence == 0.0 {
                continue;
            }
            let mut v = attr.borrow().get_vector3(index);
            if !g.morph_targets_relative {
                v.sub(&target);
            }
            morph.add_scaled_vector(&v, influence);
        }
        target.add(&morph);
    }
    target
}

/// The `lineDistance` attribute a dashed line reads: the distance along the
/// line to each vertex. `segments` pairs vertices as LineSegments does.
pub fn compute_line_distances(geometry: &GeometryRef, segments: bool) {
    let mut g = geometry.borrow_mut();
    if g.index.is_some() {
        return;
    }
    let pos = match g.get_attribute("position") {
        Some(p) => p,
        None => return,
    };
    let p = pos.borrow();
    let count = p.count;
    let mut d: Vec<f64> = vec![0.0; count];
    if segments {
        let mut i = 0;
        while i + 1 < count {
            let start = p.get_vector3(i);
            let end = p.get_vector3(i + 1);
            d[i] = if i == 0 { 0.0 } else { d[i - 1] };
            d[i + 1] = d[i] + start.distance_to(&end);
            i += 2;
        }
    } else {
        for i in 1..count {
            let start = p.get_vector3(i - 1);
            let end = p.get_vector3(i);
            d[i] = d[i - 1] + start.distance_to(&end);
        }
    }
    g.set_attribute("lineDistance", BufferAttribute::new(d, 1, ArrayKind::Float32, false).shared());
}

/// Adds `object` as `lod`'s child shown from `distance` on.
pub fn lod_add_level(lod: &ObjectRef, object: &ObjectRef, distance: f64, hysteresis: f64) {
    let distance = distance.abs();
    {
        let mut o = lod.borrow_mut();
        if let ObjectKind::Lod(d) = &mut o.kind {
            let mut at = d.levels.len();
            for (l, level) in d.levels.iter().enumerate() {
                if distance < level.distance {
                    at = l;
                    break;
                }
            }
            d.levels.insert(at, LodLevel { object: object.clone(), distance, hysteresis });
        }
    }
    add(lod, object);
}

/// The level object for a viewing distance.
pub fn lod_object_for_distance(lod: &ObjectRef, distance: f64) -> Option<ObjectRef> {
    let o = lod.borrow();
    let d = match &o.kind {
        ObjectKind::Lod(d) => d,
        _ => return None,
    };
    if d.levels.is_empty() {
        return None;
    }
    let mut i = 1;
    while i < d.levels.len() {
        let level = &d.levels[i];
        let mut level_distance = level.distance;
        if level.object.borrow().visible {
            level_distance -= level_distance * level.hysteresis;
        }
        if distance < level_distance {
            break;
        }
        i += 1;
    }
    Some(d.levels[i - 1].object.clone())
}

/// Shows the level for `camera`'s distance and hides the others.
pub fn lod_update(lod: &ObjectRef, camera: &ObjectRef) {
    let mut v1 = Vector3::zero();
    let zoom = {
        let c = camera.borrow();
        v1.set_from_matrix_position(&c.matrix_world);
        match c.camera() {
            Some(cd) => cd.zoom,
            None => 1.0,
        }
    };
    let mut o = lod.borrow_mut();
    let mut v2 = Vector3::zero();
    v2.set_from_matrix_position(&o.matrix_world);
    let distance = v1.distance_to(&v2) / zoom;
    if let ObjectKind::Lod(d) = &mut o.kind {
        let n = d.levels.len();
        if n <= 1 {
            return;
        }
        d.levels[0].object.borrow_mut().visible = true;
        let mut i = 1;
        while i < n {
            let mut level_distance = d.levels[i].distance;
            if d.levels[i].object.borrow().visible {
                level_distance -= level_distance * d.levels[i].hysteresis;
            }
            if distance >= level_distance {
                d.levels[i - 1].object.borrow_mut().visible = false;
                d.levels[i].object.borrow_mut().visible = true;
            } else {
                break;
            }
            i += 1;
        }
        d.current_level = (i - 1) as i64;
        while i < n {
            d.levels[i].object.borrow_mut().visible = false;
            i += 1;
        }
    }
}
