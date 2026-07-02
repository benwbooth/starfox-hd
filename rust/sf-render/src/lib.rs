//! Renderer: 3D shape pass, 2D background/UI/HUD passes, particles.
//!
//! Ports (C oracle): `src/renderer/*.c` (shapes, draw_list, gl_backend,
//! bg2d, ui, hud, font, sprites, particles, transform) and the generated
//! `shape_data.h` / `light_data.h` tables.
//! Keep SDL2 + OpenGL 3.3 core to match the oracle; HD enhancements
//! (uncapped logic rate, wgpu, etc.) come after parity.

pub mod light_data;
pub mod shape_data;
pub mod shapes;
