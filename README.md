# RangerThree

three.js's API written as strict Rust: cargo builds it, and the Ranger
compiler reads the same source, so it also becomes JavaScript (and Ranger's
other targets). Plain three.js code runs on it through a thin JavaScript
facade.

- `three/` is the crate: maths, core (Object3D, BufferGeometry, Raycaster,
  Clock), geometries (including Shape, Extrude, Tube, Edges and Wireframe),
  curves, earcut, materials, textures, the software renderer
  (`renderers/soft.rs`) and the frame plan a GPU backend draws from
  (`renderers/plan.rs`).
- `js/` is the facade, which defines `THREE` over `RangerThreeCore`. Load it
  in this order: `three.js`, `three-scene.js`, `three-curves.js`,
  `three-renderer.js`, `three-webgl.js`. `WebGLRenderer` draws with WebGL2
  when the canvas offers it and with the software renderer otherwise, or
  when it is given `{ software: true }`; `renderer.backend` says which.
  Lighting follows three.js's units (Lambert diffuse is albedo / pi), so
  scenes light the same as in three.js.
- `gpu-gl/` plays the GPU command stream (`renderers/gpu.rs`) on GLES 3 /
  GL 3.3 from Rust; `gpu-android/` is the same player in Kotlin
  (`player/GpuPlayer.kt` over a small `Gl` interface), with
  `android/AndroidGl.kt` for android.opengl.GLES30 and a JVM host
  (`jvm/`, JNA over Mesa EGL) for headless tests. Android runs the crate as
  Ranger's Kotlin output plus that player.
- Shaders: `renderers/shaders.rs` writes each GLSL ES 3.00 program variant,
  `renderers/glsl_pre.rs` resolves its `#if`s and macros in the crate, and
  `renderers/wgsl.rs` translates it to WGSL for WebGPU, with the binding
  layout a player needs (`node tests/wgsl-compile.mjs`,
  `node tests/wgsl-draw.mjs` check it in Chromium's WebGPU).
- `playground/` is the browser playground that GitHub Pages publishes on
  every push to `main` (`.github/workflows/playground.yml`).
- `parity/` is the value meter (`node parity/value.mjs`), and `tests/` holds
  the comparisons against three.js r185.

## Build

    bash scripts/build-js.sh           # build/rangerthree-core.js
    bash scripts/build-playground.sh   # build/site
    cargo build --workspace
    bash gpu-android/test.sh           # Kotlin crate + Kotlin player, headless

The scripts fetch Ranger at the commit pinned in `deps.json`. To use your
own checkout, set `RANGER_DIR=/path/to/Ranger`.
