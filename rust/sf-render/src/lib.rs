//! Renderer: 3D shape pass, 2D background/UI/HUD passes, particles.
//!
//! Ports (C oracle): `src/renderer/*.c` (shapes, draw_list, gl_backend,
//! bg2d, ui, hud, font, sprites, particles, transform) and the generated
//! `shape_data.h` / `light_data.h` tables.
//!
//! Data layer (pure, no GL): [`shape_data`], [`light_data`], [`shapes`].
//! Runtime layer (GL 3.3 core via `glow`): [`renderer::Renderer`] mirrors
//! the C pass structure exactly — Bg2d -> DrawList 3D (+ shadows) ->
//! particles -> HUD -> UI -> fade. Game-state inputs arrive through the
//! plain [`renderer::FrameInputs`] struct; this crate does not depend on
//! sf-game.

// Pass entry points mirror the C function signatures; parameter-count style
// lints would force artificial context structs on a 1:1 transcription.
#![allow(clippy::too_many_arguments)]

pub mod light_data;
pub mod shape_data;
pub mod shapes;

pub mod bg2d;
pub mod builtin_shapes;
pub mod draw_list;
pub mod gpu;
pub mod font;
pub mod hud;
pub mod particles;
pub mod renderer;
pub mod shapes_gl;
pub mod sprites;
pub mod transform;
pub mod ui;
