//! The playground's scenes built with the crate's Rust API and drawn by the
//! software renderer, frame by frame, as bench/make-bench.mjs draws them
//! from JavaScript. Native: `cargo run --release --example soft_bench`;
//! WebAssembly: build for wasm32-wasip1 and run with bench/run-wasm.mjs.
//!
//!   soft_bench <scene> [W H FRAMES]    scene: torus | primitives | instancing
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use rangerthree::core::bounds::set_instance_matrix_at;
use rangerthree::core::object3d::{add, ensure_instance_colors, look_at, Background, Object3D, ObjectRef};
use rangerthree::geometries::polyhedron::icosahedron_geometry;
use rangerthree::geometries::primitives::*;
use rangerthree::materials::material::{Material, MaterialKind, MaterialRef};
use rangerthree::maths::color::{Color, ColorSpace};
use rangerthree::maths::euler::{Euler, EulerOrder};
use rangerthree::maths::vector3::Vector3;
use rangerthree::renderers::soft::SoftRenderer;
use rangerthree::scenes::fog::Fog;

type Frame = Box<dyn FnMut(f64)>;

fn material(kind: MaterialKind, hex: i64) -> MaterialRef {
    let mut m = Material::new(kind);
    m.color = Color::from_hex(hex);
    m.shared()
}

fn scene_with(bg: i64) -> ObjectRef {
    let s = Object3D::scene().shared();
    s.borrow_mut().scene_data_mut().unwrap().background = Background::Color(Color::from_hex(bg));
    s
}

fn camera(fov: f64, x: f64, y: f64, z: f64) -> ObjectRef {
    let c = Object3D::perspective_camera(fov, 4.0 / 3.0, 0.1, 100.0).shared();
    c.borrow_mut().position.set(x, y, z);
    look_at(&c, &Vector3::new(0.0, 0.0, 0.0));
    c
}

fn rot(o: &ObjectRef, x: f64, y: f64, z: f64) {
    o.borrow_mut().set_rotation_from_euler(&Euler::new(x, y, z, EulerOrder::Xyz));
}

fn torus(r: Rc<RefCell<SoftRenderer>>) -> Frame {
    let scene = scene_with(0x1b1e26);
    let cam = camera(50.0, 0.0, 1.5, 5.0);
    let knot = Object3D::mesh(torus_knot_geometry(1.0, 0.32, 96.0, 10.0, 2.0, 3.0).shared(), material(MaterialKind::MeshStandard, 0xff8a3d)).shared();
    add(&scene, &knot);
    add(&scene, &Object3D::ambient_light(Color::from_hex(0xffffff), 0.25).shared());
    let sun = Object3D::directional_light(Color::from_hex(0xffffff), 2.5).shared();
    sun.borrow_mut().position.set(3.0, 4.0, 2.0);
    add(&scene, &sun);
    Box::new(move |t: f64| {
        rot(&knot, t / 2000.0, t / 1300.0, 0.0);
        r.borrow_mut().render(&scene, &cam);
    })
}

fn primitives(r: Rc<RefCell<SoftRenderer>>) -> Frame {
    let scene = scene_with(0x222831);
    let cam = camera(55.0, 0.0, 4.0, 9.0);
    let floor = Object3D::mesh(plane_geometry(14.0, 14.0, 1.0, 1.0).shared(), material(MaterialKind::MeshLambert, 0x8892a0)).shared();
    rot(&floor, -std::f64::consts::PI / 2.0, 0.0, 0.0);
    floor.borrow_mut().position.y = -1.0;
    add(&scene, &floor);
    let pi = std::f64::consts::PI;
    let shapes = vec![
        box_geometry(1.4, 1.4, 1.4, 1.0, 1.0, 1.0),
        sphere_geometry(0.9, 32.0, 16.0, 0.0, pi * 2.0, 0.0, pi),
        cone_geometry(0.9, 1.8, 24.0, 1.0, false, 0.0, pi * 2.0),
        icosahedron_geometry(0.9, 0.0),
        torus_geometry(0.7, 0.28, 12.0, 32.0, pi * 2.0, 0.0, pi * 2.0),
    ];
    let colors = [0x4cc9f0, 0xf72585, 0xffd166, 0x06d6a0, 0xf8961e];
    let group = Object3D::group().shared();
    let n = shapes.len();
    let mut meshes: Vec<ObjectRef> = Vec::new();
    for (i, g) in shapes.into_iter().enumerate() {
        let m = Object3D::mesh(g.shared(), material(MaterialKind::MeshStandard, colors[i])).shared();
        let a = (i as f64 / n as f64) * pi * 2.0;
        m.borrow_mut().position.set(a.cos() * 3.0, 0.0, a.sin() * 3.0);
        add(&group, &m);
        meshes.push(m);
    }
    add(&scene, &group);
    add(&scene, &Object3D::ambient_light(Color::from_hex(0xffffff), 0.2).shared());
    let lamp = Object3D::point_light(Color::from_hex(0xffffff), 3.0, 0.0, 2.0).shared();
    lamp.borrow_mut().position.set(0.0, 3.0, 0.0);
    add(&scene, &lamp);
    Box::new(move |t: f64| {
        rot(&group, 0.0, t / 3000.0, 0.0);
        for (i, m) in meshes.iter().enumerate() {
            rot(m, t / (900.0 + i as f64 * 200.0), t / 1100.0, 0.0);
        }
        r.borrow_mut().render(&scene, &cam);
    })
}

fn instancing(r: Rc<RefCell<SoftRenderer>>) -> Frame {
    let scene = scene_with(0x0e1116);
    scene.borrow_mut().scene_data_mut().unwrap().fog = Some(Fog::linear(Color::from_hex(0x0e1116), 14.0, 32.0));
    let cam = camera(55.0, 0.0, 9.0, 16.0);
    const N: usize = 18;
    let mut mat = Material::new(MaterialKind::MeshStandard);
    mat.roughness = 0.4;
    let mesh = Object3D::instanced_mesh(box_geometry(0.8, 0.8, 0.8, 1.0, 1.0, 1.0).shared(), mat.shared(), (N * N) as i64).shared();
    add(&scene, &mesh);
    add(&scene, &Object3D::hemisphere_light(Color::from_hex(0xbfd8ff), Color::from_hex(0x202020), 1.2).shared());
    let sun = Object3D::directional_light(Color::from_hex(0xffffff), 1.6).shared();
    sun.borrow_mut().position.set(5.0, 10.0, 4.0);
    add(&scene, &sun);
    let mut dummy = Object3D::group();
    let mut color = Color::new(1.0, 1.0, 1.0);
    Box::new(move |t: f64| {
        let mut i = 0;
        {
            let mut mb = mesh.borrow_mut();
            let data = mb.mesh_data_mut().unwrap();
            ensure_instance_colors(data);
            for x in 0..N {
                for z in 0..N {
                    let px = x as f64 - N as f64 / 2.0 + 0.5;
                    let pz = z as f64 - N as f64 / 2.0 + 0.5;
                    let d = px.hypot(pz);
                    let h = (d * 0.6 - t / 400.0).sin() * 0.5 + 0.5;
                    dummy.position.set(px, h * 1.5, pz);
                    dummy.set_rotation_from_euler(&Euler::new(0.0, d * 0.2 + t / 2000.0, 0.0, EulerOrder::Xyz));
                    dummy.scale.set(1.0, 0.4 + h * 1.6, 1.0);
                    dummy.update_matrix();
                    set_instance_matrix_at(data, i, &dummy.matrix);
                    color.set_hsl(0.55 + h * 0.35, 0.7, 0.5, ColorSpace::Linear);
                    data.instance_color.as_ref().unwrap().borrow_mut().set_xyz(i, color.r, color.g, color.b);
                    i += 1;
                }
            }
        }
        r.borrow_mut().render(&scene, &cam);
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).map(|s| s.as_str()).unwrap_or("torus").to_string();
    let w: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(320);
    let h: i64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(240);
    let frames: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(30);
    let t0 = Instant::now();
    let r = Rc::new(RefCell::new(SoftRenderer::new(w, h)));
    let mut frame = match name.as_str() {
        "primitives" => primitives(r.clone()),
        "instancing" => instancing(r.clone()),
        _ => torus(r.clone()),
    };
    let setup = t0.elapsed().as_secs_f64() * 1000.0;
    let mut times: Vec<f64> = Vec::new();
    for i in 0..frames {
        let a = Instant::now();
        frame(1000.0 + i as f64 * (1000.0 / 60.0));
        times.push(a.elapsed().as_secs_f64() * 1000.0);
    }
    let rr = r.borrow();
    let mut sum: i64 = 0;
    let mut j = 0;
    while j < rr.pixels.len() {
        sum = (sum * 31 + rr.pixels[j] as i64) % 1000000007;
        j += 7;
    }
    let mut s = times.clone();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean: f64 = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "BENCH {{\"setup\":{:.1},\"first\":{:.2},\"median\":{:.2},\"mean\":{:.2},\"tris\":{},\"checksum\":{}}}",
        setup, times[0], s[s.len() / 2], mean, rr.triangles, sum
    );
}
