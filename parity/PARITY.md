# Three parity

What "Three works in M-Files" means, as a list of items that each pass or
fail by a test, and the meter that runs them. The implementation is
web/three (rangerthree: Ranger's game_engine/three ported to Ranger-Rust,
its React Three Fiber reader fiber_tree, the GPU path in the window page and
the software rasteriser) and the .tsx kit (desktop/uix/js/tsx.js, the
`THREE` and the JSX a document sees, run by CErXes). The reference is
three.js r185 and @react-three/fiber 9, pinned in tools/fetch-three.sh and
used only to make goldens and reference pictures: nothing of it is shipped
or imported by M-Files.

    web/three/parity/tools/fetch-three.sh     the reference into tmp/three-ref
    node web/three/parity/run.mjs             measure every item (~5 min)
    node web/three/parity/run.mjs --only=value | --only=render | --grep=texture.
    node web/three/parity/run.mjs --png=tmp/parity-png   pictures to look at

Results: results/three-parity.txt (one line per item) and .json.

## The items

items.json lists 1223 items on five axes. An item is done when its test
passes; nothing is counted half.

| axis | items | what passes |
| --- | ---: | --- |
| value | 872 | a `THREE.*` call answers what three.js answers |
| render | 213 | a scene looks like three.js's picture of it |
| r3f | 80 | React Three Fiber's rules hold (42 as pictures, 38 in the kit) |
| example | 50 | a three.js example runs as a .tsx document |
| perf | 8 | the GPU path keeps up with three.js |

**value** (value-probes.txt): every public method of the math classes
(Vector2/3/4, Matrix3/4, Quaternion, Euler, Color, Box2/3, Sphere, Plane,
Ray, Line3, Triangle, Spherical, Cylindrical, Frustum, MathUtils), of
Object3D, BufferAttribute, BufferGeometry, Raycaster, Layers,
EventDispatcher, Timer, Clock, the curves, cameras and Mesh, and the
constructors and defaults of every material, light, texture, object and
loader that has a value without a GPU, and animation (tracks, mixer,
loops, cross-fades). A probe is either `Class.method TOKENS` (a fixed
receiver from probe-prelude.js gets the method with fixed arguments; the
answer is the result and the receiver after the call) or an expression.
tools/gen-goldens.mjs runs each on three.js in Node into goldens/value.json;
the meter runs the same probe on the kit's `THREE` in CErXes, serialised
the same way (probe-prelude.js `ser`), and compares numbers within 1e-5
relative. The harness is checked by running three.js itself in CErXes
(`THREE_PARITY_SELF=1`): 871 of 872 pass, the one left (FileLoader) needs
`AbortController`, which CErXes does not have.

**render** (scenes.mjs): one scene per feature, written as the {t, a, c}
tree a `<scene3d>` gives (React Three Fiber's names): 23 geometries with
Three's defaults and with their own arguments plus extrude, shape, tube,
buffer, indexed, edges and wireframe geometry; 9 material types on a sphere
and a box; 24 material props (wireframe, flatShading, side, emissive,
opacity, blending modes, depthTest/Write, colorWrite, toneMapped, fog,
vertexColors, alphaTest, polygonOffset...); a PBR roughness/metalness
grid, Phong shininess, physical transmission, sheen and iridescence, the
shadow material; every light type with its options (decay, distance,
angle, penumbra, target, four and eight point lights); shadows of each
light and shadow map type, bias, radius, self-shadowing; textures (filters,
mipmaps, anisotropy, wrapping, repeat, offset, rotation, flipY, colour
space, alpha/emissive/normal/bump/roughness/metalness/ao/displacement/
light/specular/gradient maps, matcap, environment and background textures,
a texture given again); fog, fogExp2, every tone mapping, exposure,
`<Canvas flat>` and `linear`, the default camera, environment intensity,
background blurriness, override material, a transparent canvas; sprites,
points, lines, dashed lines, groups, renderOrder, transparency sorting,
visibility, morph targets, frustum culling, negative scale; cameras (fov,
near, far, zoom, up, rotation, orthographic).

**r3f**: the same kind of pictures for React Three Fiber's own rules
(positions and pierced props such as `position-x`, `rotation` with an
order, scalar `scale`, colours as names, numbers, arrays, `hsl()` and
`rgb()`, `material-color` on a mesh, `attach` for materials, geometries,
`material-0`..., background, fog, maps and buffer attributes, keys,
`makeDefault`, `<Canvas camera>`, `dpr`, `shadows="soft"`, `gl` props,
constants by name), and 38 kit items for behaviour that is not a picture:
hooks (useFrame with priority, useThree and its selector, useLoader for
textures and glTF, useGraph, refs), pointer events (onClick, over/out,
move, down/up, double click, context menu, wheel, stopPropagation,
onPointerMissed, eventObject, instanceId), invalidate, advance, clock,
viewport, pointer, raycaster, extend, createPortal, dispose, Suspense,
onUpdate, args changes, texture props, manual cameras, resize, and the
objects a .tsx makes in code and mounts (instancedMesh matrices and
colours, a skinned mesh, LOD, `<primitive>`, setViewOffset).

**example**: 50 three.js examples (webgl_*), each passing when it runs as
a .tsx document and its picture matches the example's at a fixed time.

**perf**: frame time within 1.5x of three.js in the same browser for 1000
meshes, 100k triangles, 10k instances, shadows and 32 textures; a frame
that moves 100 objects sends a diff; memory stable over 1000 frames; the
first frame within 300 ms once the module is in.

## How a picture is compared

The reference: tools/r3f-ref.js builds the tree with three.js by React
Three Fiber's rules (applyProps: colours set, math objects copied, arrays
through fromArray/set, numbers through setScalar, pierced props, `attach`,
geometries and materials attached by default, colour maps made sRGB) and
`<scene3d>` is R3F's `<Canvas>`: ACES Filmic tone mapping (none with
`flat`), sRGB output (linear with `linear`), PCF soft shadows with
`shadows`, the camera at [0, 0, 5] with fov 75 when the scene has none.
One difference, documented: the scene's first camera (or the one with
`makeDefault`) is the camera drawn with, as drei's `makeDefault` cameras,
which is how .tsx documents (Vault Breach) are written. A second one:
shadows are on when a light casts them and the scene does not say
`shadows={false}` (R3F's Canvas has them off unless `shadows`), as
.tsx documents written before the meter expect; shadow.off says false.

Ours: the same tree to rangerthree's WASM build on WebGL2
(three_scene/three_draw, the window page's mfd-three.wasm) and to
Pres3DStill (examples/parity_soft.rs, the software picture
`mfiles:fiber@1` answers without a GPU). Both run in Chromium with
SwiftShader, 160 x 120, no antialiasing. Textures are made in code
(scenes.mjs `TEXTURES`) and given to both sides as the same bytes; a prop
`"name@1"` is that texture, as a .tsx's `map={dataTexture}` becomes.

A picture matches when it looks the same at 8 bits a channel where the
scene is (the pixels that differ from the reference's background in
either picture): the mean channel difference is at most 6, and at most 3 %
of those pixels are off by more than 40 in a channel, not counting pixels
on an edge of the reference. The GPU picture decides the item; the
software picture is reported beside it and not counted.

## Status (2026-11-11, main 3a8e714)

See results/three-parity.txt. Before any implementation work:

- value: 1 of 872. The kit's `THREE` has only DataTexture and a few
  constants; documents get Three's objects only as JSX.
- render and r3f: the GPU path passes a handful. The big gaps, by how
  many items they hold back: lighting (MeshStandardMaterial is drawn as
  Phong with no physical light units, no ACES tone mapping), material
  types (toon, normal, depth, matcap, physical drawn wrong or white),
  textures beyond map (normal, bump, roughness, ao... maps), point and
  spot lights, points, lines, multi-material meshes, fog, orthographic
  cameras.
- kit, example, perf: not measured yet (open), each needs its pass built.

## Order of work

One item at a time, the ones that hold back most items first:

1. the lighting model: physical light units, MeshStandardMaterial's GGX,
   ACES tone mapping and sRGB output as R3F sets them (moves most render
   items at once)
2. the kit's `THREE`: the math classes from rangerthree's own Vector3/
   Matrix4/Quaternion/Euler/Box3 (Ranger's game_engine/three), then
   Object3D, BufferGeometry and the rest of the value axis
3. transparency, blending modes and LinearFilter (started: patch in the
   project files, three-parity/engine-blend-wip.patch)
4. material types, maps, lights, shadows, objects, cameras by item
5. the kit pass (hooks, events), then examples and perf
