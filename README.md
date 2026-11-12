# RangerThree

three.js's API written as strict Rust: cargo builds it, and the Ranger
compiler reads the same source, so it also becomes JavaScript (and Ranger's
other targets). Plain three.js code runs on it through a thin JavaScript
facade.

- `three/` is the crate: maths, core (Object3D, BufferGeometry, Raycaster,
  Clock), geometries (including Shape, Extrude, Tube, Edges and Wireframe),
  curves, earcut, materials, textures, and the software renderer.
- `js/` is the facade, which defines `THREE` over `RangerThreeCore`. Load it
  in this order: `three.js`, `three-scene.js`, `three-curves.js`,
  `three-renderer.js`.
- `playground/` is the browser playground that GitHub Pages publishes on
  every push to `main` (`.github/workflows/playground.yml`).
- `parity/` is the value meter (`node parity/value.mjs`), and `tests/` holds
  the comparisons against three.js r185.

## Build

    bash scripts/build-js.sh           # build/rangerthree-core.js
    bash scripts/build-playground.sh   # build/site
    cargo build --workspace

The scripts fetch Ranger at the commit pinned in `deps.json`. To use your
own checkout, set `RANGER_DIR=/path/to/Ranger`.
