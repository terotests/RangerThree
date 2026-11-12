//! Bounds of objects in world space: Box3.setFromObject / expandByObject,
//! the spheres Frustum.intersectsObject tests, and an InstancedMesh's
//! bounds over all its instances.

use ranger::prelude::*;

use super::object3d::{update_world_matrix, vertex_position, MeshData, ObjectKind, ObjectRef};
use crate::maths::box3::Box3;
use crate::maths::matrix4::Matrix4;
use crate::maths::sphere::Sphere;
use crate::maths::vector3::Vector3;

/// The mesh data of a mesh, points, line or instanced mesh.
fn mesh_data_of(kind: &ObjectKind) -> Option<MeshData> {
    match kind {
        ObjectKind::Mesh(d) => Some(d.clone()),
        ObjectKind::InstancedMesh(d) => Some(d.clone()),
        ObjectKind::Points(d) => Some(d.clone()),
        ObjectKind::Line(d) => Some(d.clone()),
        ObjectKind::LineSegments(d) => Some(d.clone()),
        ObjectKind::LineLoop(d) => Some(d.clone()),
        _ => None,
    }
}

/// Instance `i`'s matrix.
pub fn instance_matrix_at(data: &MeshData, i: usize) -> Matrix4 {
    let mut m = Matrix4::identity();
    if let Some(a) = &data.instance_matrix {
        let arr = a.borrow();
        for k in 0..16 {
            m.elements[k] = arr.array[i * 16 + k];
        }
    }
    m
}

pub fn set_instance_matrix_at(data: &MeshData, i: usize, m: &Matrix4) {
    if let Some(a) = &data.instance_matrix {
        let mut arr = a.borrow_mut();
        for k in 0..16 {
            arr.set_component(i, k, m.elements[k]);
        }
    }
}

/// The union of the geometry's box placed by every instance matrix.
pub fn instance_bounding_box(data: &MeshData) -> Box3 {
    let gb = {
        let mut g = data.geometry.borrow_mut();
        if g.bounding_box.is_none() {
            g.compute_bounding_box();
        }
        g.bounding_box.unwrap_or(Box3::empty())
    };
    let mut out = Box3::empty();
    for i in 0..(data.count as usize) {
        let mut b = gb;
        b.apply_matrix4(&instance_matrix_at(data, i));
        out.union(&b);
    }
    out
}

pub fn instance_bounding_sphere(data: &MeshData) -> Sphere {
    let gs = {
        let mut g = data.geometry.borrow_mut();
        if g.bounding_sphere.is_none() {
            g.compute_bounding_sphere();
        }
        g.bounding_sphere.unwrap_or(Sphere::empty())
    };
    let mut out = Sphere::empty();
    for i in 0..(data.count as usize) {
        let mut s = gs;
        s.apply_matrix4(&instance_matrix_at(data, i));
        out.union(&s);
    }
    out
}

/// Grows `b` by `object` and everything under it, in world space.
/// `precise` walks every vertex (morphs applied) instead of the boxes.
pub fn expand_by_object(b: &mut Box3, object: &ObjectRef, precise: bool) {
    update_world_matrix(object, false, false);
    let o = object.borrow();
    if let Some(data) = mesh_data_of(&o.kind) {
        let instanced = matches!(o.kind, ObjectKind::InstancedMesh(_));
        let is_mesh = matches!(o.kind, ObjectKind::Mesh(_));
        let pos = data.geometry.borrow().get_attribute("position");
        if precise && pos.is_some() && !instanced {
            let count = pos.unwrap().borrow().count;
            for i in 0..count {
                let mut v = if is_mesh {
                    vertex_position(&data, i)
                } else {
                    data.geometry.borrow().get_attribute("position").unwrap().borrow().get_vector3(i)
                };
                v.apply_matrix4(&o.matrix_world);
                b.expand_by_point(&v);
            }
        } else {
            let mut gb = if instanced {
                instance_bounding_box(&data)
            } else {
                let mut g = data.geometry.borrow_mut();
                if g.bounding_box.is_none() {
                    g.compute_bounding_box();
                }
                g.bounding_box.unwrap_or(Box3::empty())
            };
            gb.apply_matrix4(&o.matrix_world);
            b.union(&gb);
        }
    }
    let kids: Vec<ObjectRef> = o.children.clone();
    drop(o);
    for c in kids.iter() {
        expand_by_object(b, c, precise);
    }
}

/// The box around `object` and its descendants in world space.
pub fn box_from_object(object: &ObjectRef, precise: bool) -> Box3 {
    let mut b = Box3::empty();
    expand_by_object(&mut b, object, precise);
    b
}

/// The sphere a frustum tests for `object`: its geometry's (or a sprite's
/// unit quad's) bounding sphere in world space; None for other objects.
pub fn world_bounding_sphere(object: &ObjectRef) -> Option<Sphere> {
    let o = object.borrow();
    if let ObjectKind::Sprite(_) = &o.kind {
        let mut s = Sphere::new(Vector3::zero(), 0.7071067811865476);
        s.apply_matrix4(&o.matrix_world);
        return Some(s);
    }
    let data = match mesh_data_of(&o.kind) {
        Some(d) => d,
        None => return None,
    };
    let mut s = if matches!(o.kind, ObjectKind::InstancedMesh(_)) {
        instance_bounding_sphere(&data)
    } else {
        let mut g = data.geometry.borrow_mut();
        if g.bounding_sphere.is_none() {
            g.compute_bounding_sphere();
        }
        g.bounding_sphere.unwrap_or(Sphere::empty())
    };
    s.apply_matrix4(&o.matrix_world);
    Some(s)
}
