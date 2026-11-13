//! Without a browser: scenes built with the crate's Rust API, turned into GPU
//! commands by GpuRenderer and played by gpu-gl in a headless OpenGL ES 3
//! context, frame by frame. This is the path an SDL3 or mobile host takes.
//!
//!   cargo run --release -p gpu-gl --example scenes [W H FRAMES] [--save=dir]
use gpu_gl::{egl::Headless, read_rgba, Player};
use rangerthree::core::bounds::set_instance_matrix_at;
use rangerthree::core::object3d::{add, ensure_instance_colors, look_at, Background, Object3D, ObjectRef};
use rangerthree::geometries::primitives::*;
use rangerthree::materials::material::{Material, MaterialKind};
use rangerthree::maths::color::{Color, ColorSpace};
use rangerthree::maths::euler::{Euler, EulerOrder};
use rangerthree::maths::vector3::Vector3;
use rangerthree::renderers::gpu_frame::{GpuRenderer, GpuSettings};
use std::time::Instant;

fn scene_with(bg: i64) -> ObjectRef {
    let s = Object3D::scene().shared();
    s.borrow_mut().scene_data_mut().unwrap().background = Background::Color(Color::from_hex(bg));
    s
}

fn camera(x: f64, y: f64, z: f64, aspect: f64) -> ObjectRef {
    let c = Object3D::perspective_camera(55.0, aspect, 0.1, 100.0).shared();
    c.borrow_mut().position.set(x, y, z);
    look_at(&c, &Vector3::new(0.0, 0.0, 0.0));
    c
}

fn rot(o: &ObjectRef, x: f64, y: f64, z: f64) {
    o.borrow_mut().set_rotation_from_euler(&Euler::new(x, y, z, EulerOrder::Xyz));
}

type Animate = Box<dyn FnMut(f64)>;

fn torus(aspect: f64) -> (ObjectRef, ObjectRef, Animate) {
    let scene = scene_with(0x1b1e26);
    let mut m = Material::new(MaterialKind::MeshStandard);
    m.color = Color::from_hex(0xff8a3d);
    let knot = Object3D::mesh(torus_knot_geometry(1.0, 0.32, 96.0, 10.0, 2.0, 3.0).shared(), m.shared()).shared();
    add(&scene, &knot);
    add(&scene, &Object3D::ambient_light(Color::from_hex(0xffffff), 0.25).shared());
    let sun = Object3D::directional_light(Color::from_hex(0xffffff), 2.5).shared();
    sun.borrow_mut().position.set(3.0, 4.0, 2.0);
    add(&scene, &sun);
    (scene, camera(0.0, 1.5, 5.0, aspect), Box::new(move |t| rot(&knot, t / 2000.0, t / 1300.0, 0.0)))
}

fn instancing(aspect: f64) -> (ObjectRef, ObjectRef, Animate) {
    const N: usize = 18;
    let scene = scene_with(0x0e1116);
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
    let animate = Box::new(move |t: f64| {
        let mut mb = mesh.borrow_mut();
        let data = mb.mesh_data_mut().unwrap();
        ensure_instance_colors(data);
        let mut i = 0;
        for x in 0..N {
            for z in 0..N {
                let (px, pz) = (x as f64 - N as f64 / 2.0 + 0.5, z as f64 - N as f64 / 2.0 + 0.5);
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
    });
    (scene, camera(0.0, 9.0, 16.0, aspect), animate)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let nums: Vec<i64> = args.iter().filter_map(|a| a.parse().ok()).collect();
    let (w, h, frames) = (*nums.first().unwrap_or(&640), *nums.get(1).unwrap_or(&480), *nums.get(2).unwrap_or(&60) as usize);
    let save = args.iter().find_map(|a| a.strip_prefix("--save=").map(String::from));
    let ctx = Headless::new(w as i32, h as i32, false).expect("headless GLES 3 context");
    let mut player = Player::new(ctx.load_gl().unwrap(), false);
    player.screen_fb = ctx.fb;
    println!("{} | {}x{}, {} frames", player.describe(), w, h, frames);
    let aspect = w as f64 / h as f64;
    let scenes: Vec<(&str, fn(f64) -> (ObjectRef, ObjectRef, Animate))> = vec![("torus", torus), ("instancing", instancing)];
    let mut failed = false;
    // the stream's object ids belong to one GpuRenderer: one renderer per player
    let mut gpu = GpuRenderer::new();
    for (name, build) in scenes {
        let (scene, cam, mut animate) = build(aspect);
        let s = GpuSettings::new(w, h);
        let (mut plan, mut play) = (Vec::new(), Vec::new());
        let mut last = None;
        for i in 0..frames {
            animate(1000.0 + i as f64 * 16.0);
            let t0 = Instant::now();
            let frame = gpu.render(&scene, &cam, &s);
            let t1 = Instant::now();
            if !frame.missing.is_empty() {
                println!("{}: needs {}", name, frame.missing.join(", "));
                failed = true;
                break;
            }
            let floats: Vec<f32> = frame.commands.floats.iter().map(|&v| v as f32).collect();
            if let Err(e) = player.run(&frame.commands.ints, &floats, &frame.commands.texts, &frame.commands.bytes) {
                println!("{}: {}", name, e);
                failed = true;
                break;
            }
            let px = read_rgba(&player.gl, w as i32, h as i32); // waits for the frame
            let t2 = Instant::now();
            plan.push((t1 - t0).as_secs_f64() * 1000.0);
            play.push((t2 - t1).as_secs_f64() * 1000.0);
            last = Some((px, frame.calls, frame.triangles, frame.commands.count));
        }
        let Some((px, calls, tris, count)) = last else { continue };
        let med = |v: &mut Vec<f64>| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() / 2] };
        let bg = [px[0], px[1], px[2]];
        let drawn = px.chunks(4).filter(|p| [p[0], p[1], p[2]] != bg).count();
        println!(
            "{:<11} {} calls, {} triangles, {} commands | crate {:.2} ms + player/GPU {:.2} ms (medians) | {:.1} % of pixels drawn",
            name, calls, tris, count, med(&mut plan), med(&mut play), drawn as f64 * 100.0 / (w * h) as f64
        );
        if drawn == 0 {
            failed = true;
        }
        if let Some(dir) = &save {
            std::fs::create_dir_all(dir).unwrap();
            let mut out = format!("P6\n{} {}\n255\n", w, h).into_bytes();
            for y in (0..h as usize).rev() {
                for x in 0..w as usize {
                    let p = (y * w as usize + x) * 4;
                    out.extend_from_slice(&px[p..p + 3]);
                }
            }
            std::fs::write(format!("{}/{}.ppm", dir, name), out).unwrap();
        }
    }
    std::process::exit(failed as i32);
}
