# Benchmarks: the same scenes in every engine

The playground's scenes (Torus knot, Primitives and lights, Instancing)
drawn frame by frame. Every engine draws the same picture: the checksums of
the last frame agree.

## Software renderer (`renderers/soft.rs`)

    cargo build --release --example soft_bench
    cargo build --release --example soft_bench --target wasm32-wasip1
    bash scripts/build-js.sh
    COMPONENTENGINE=../componentengine CERXES_BIN=.../cerxes node bench/run-all.mjs 320 240 20

- Rust native and Rust -> wasm32-wasip1 in Node (`three/examples/soft_bench.rs`,
  `bench/run-wasm.mjs`): the scenes written with the crate's Rust API.
- The Ranger -> JavaScript build with the facade, the playground's own
  scene code (`bench/make-bench.mjs` writes one script): in Node's V8, in the
  CErXes command line (register machine) and in CErXes built for
  WebAssembly with its JIT off and on (`bench/run-cerxes-wasm.mjs`, needs
  componentengine's `playground/cerxes-wasm` built with `--features wjit`
  into `target-jit`).
- `--no-render` rows: the frame's JavaScript alone (`scene.updateMatrixWorld()`
  in place of drawing), what stays in the engine when something else draws.

Results: `bench/results.json`.

## GPU (WebGL2 backend) driven from CErXes

`cerxes-gl/` gives a script in CErXes a WebGL2 context: `gl-client.js`
(inside the engine) records the calls and sends them over `__hostSync` once
per frame; `gl-host.js` (in the page) plays them on the canvas's real
context. Objects are numbered on the engine's side, so only queries
(getParameter, shader and program status, readPixels) wait for the page.
RangerThree's own `js/three-webgl.js` runs unchanged on it.

    (cd cerxes-gl && RUSTFLAGS="-C link-arg=--export-table -C link-arg=--growable-table" \
      cargo build --release --target wasm32-wasip1 --features wjit)
    node bench/gl-bench.mjs 320 240 60

Headless Chromium (SwiftShader). Results: `bench/gl-results.json`.
`cerxes-gl/` links CErXes, so its Rust crate is AGPL-3.0-or-later like
CErXes; the JavaScript is MIT.
