# wgpu backend porting contract

We are replacing the glow (OpenGL 3.3) backend in `sf-render` with wgpu. The
new backend lives in `rust/sf-render/src/gpu.rs` (type `Gpu`) and is already
built and compiling. Your job: convert one pass file from glow calls to the
`Gpu` retained draw API, matching the signatures below.

## The model

The old code was immediate-mode GL: bind a program, set uniforms, bind a VBO,
`draw_arrays`. The new `Gpu` is **retained**: during a frame each pass *pushes*
CPU vertices + per-draw state; `Gpu::end_frame` uploads once and replays all
draws in call order (so 3D still draws before 2D overlays). You never touch
wgpu directly in a pass — only the `Gpu` methods.

## Gpu API (all you may call)

```rust
use crate::gpu::{Gpu, Vertex3, Vertex2, TextureId, WHITE_TEX};

// Vertex3 { pos: [f32;3] }              // 3D (flat pipeline)
// Vertex2 { pos: [f32;2], uv: [f32;2] } // 2D overlay

// --- 3D (was the `flat` shader) ---
gpu.push_flat_tris(&[Vertex3], proj:&[f32;16], view:&[f32;16], model:&[f32;16], color:[f32;4]);
gpu.push_flat_lines(&[Vertex3], proj, view, model, color);  // vertex pairs (GL_LINES)

// --- 2D (was the `hud` shader) ---
// use_texture: 0 = solid uColor, 1 = RGBA texture * uColor (discard a<0.5),
//              2 = palette-indexed R8 texture (discard index 0)
gpu.push_overlay_tris(&[Vertex2], proj:&[f32;16], model:&[f32;16], color:[f32;4],
                      use_texture:u32, palette:Option<&[[f32;4];16]>, texture:TextureId);
gpu.push_overlay_fan(&[Vertex2], proj, model, color, use_texture, palette, texture); // was TRIANGLE_FAN

// --- textures (create once in `new`, keep the TextureId in the pass struct) ---
let id: TextureId = gpu.create_texture_rgba(w, h, &rgba_bytes); // 4 bytes/px
let id: TextureId = gpu.create_texture_r8(w, h, &index_bytes);  // 1 byte/px (fonts/palette)
gpu.update_texture(id, &new_bytes);                             // same dims, re-upload
```

Notes:
- Solid 2D draws (mode 0) pass `texture: WHITE_TEX` and `palette: None`.
- `push_overlay_fan` triangulates a fan `(v0,v1,v2, v0,v2,v3, ...)` — use it
  wherever the old code did `draw_arrays(TRIANGLE_FAN, 0, n)`.
- The old `set_vec4("uColor", r,g,b,a)` → the `color:[r,g,b,a]` argument.
- The old `set_int("uUseTexture", n)` → the `use_texture:n` argument.
- The old `uPalette[16]` (16 `set_vec4`s) → `Some(&[[f32;4];16])`.
- **Do not** set matrices as uniforms; pass `transform.projection()`,
  `transform.view()`, and the built model matrix as arguments per draw.

## Transform API (unchanged, read matrices from it)

```rust
transform.projection() -> &[f32;16]
transform.view() -> &[f32;16]
transform.build_model_matrix(&mut m, x:i32,y:i32,z:i32, rx:i16,ry:i16,rz:i16)
transform.build_model_matrix_f(&mut m, x:f32,y:f32,z:f32, frx:f32,fry:f32,frz:f32)
```

For 2D passes the "projection" is the pass's own ortho matrix (built locally as
today) and "model" is the local model matrix; pass ortho as the `proj` arg and
identity/local model as the `model` arg to `push_overlay_*` (view is ignored by
the overlay pipeline).

## Signature conversion (every pass)

- `Pass::new(gl: &glow::Context, ...)` → `Pass::new(gpu: &mut Gpu, ...)`.
  Move texture creation (`gl.tex_image_2d`) to `gpu.create_texture_*`, storing
  the returned `TextureId`(s) in the struct. Drop all vao/vbo/program fields.
- `pass.render(gl, backend, transform, ...)` →
  `pass.render(gpu: &mut Gpu, transform, ...)`. Drop the `gl` and `backend`
  params. Replace every glow draw with a `gpu.push_*` call.
- `pass.destroy(gl)` → delete it (wgpu frees resources on drop). Remove calls
  from `renderer.rs::shutdown` too (the orchestrator is handled separately).
- Remove `use glow;` and any `gl_backend::` use from the pass.

## ShapeStore (shapes_gl.rs) specifics

`GpuShape` currently builds a `positions: Vec<f32>` (xyz triples: all triangle
verts first, then all line verts) and uploads it to a VBO. Instead:
- Store `tri_verts: Vec<Vertex3>` and `line_verts: Vec<Vertex3>` on `GpuShape`
  (split at the triangle/line boundary the existing code already computes).
- Drop `vao`, `vbo`. `register` takes `&mut Gpu` only if it needs textures (it
  doesn't) — it can drop the `gl` param entirely and just build CPU vectors.
- The shape draw becomes `gpu.push_flat_tris(&shape.tri_verts, proj, view,
  &model, color)` and `gpu.push_flat_lines(&shape.line_verts, ...)`.
- `register_builtins(&mut self)` drops its `gl` param.

## What the orchestrator (renderer.rs) will call — match these exactly

The renderer holds `gpu: Gpu` and calls, in `submit`:
```
self.bg2d.render(&mut self.gpu, &self.transform, inputs, w, h);
self.draw_list.render(&mut self.gpu, &self.shapes, &mut self.transform, prev, curr, alpha);
self.particles.render(&mut self.gpu, &self.transform);
self.hud.render(&mut self.gpu, &mut self.sprites, &mut self.font, inputs, w, h);
self.ui.render(&mut self.gpu, &mut self.font, &self.bg2d, inputs, w, h);
self.ui.render_fade(&mut self.gpu, inputs, w, h);
```
Keep each pass's *other* params (inputs, sizes, sibling passes) exactly as they
are today; only `gl`+`backend` are removed and replaced by `&mut Gpu` as the
first argument. Constructors: `Pass::new(&mut gpu, &asset_root)` (drop `gl`).

## Do not

- Do not add `wgpu` imports to a pass — only `crate::gpu::*`.
- Do not change draw *order* or geometry math — this is a backend swap, not a
  visual change. Byte-for-byte the same vertices/colors.
- Do not touch other pass files or renderer.rs (owned separately).
