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

pub mod color_data;
pub mod light_data;
mod scene_color_data;
pub mod shape_data;
pub mod shapes;

pub mod bg2d;
pub mod builtin_shapes;
pub mod draw_list;
mod ending;
pub mod font;
pub mod gpu;
pub mod hud;
pub mod particles;
pub mod renderer;
mod sf2_aim_sight;
mod sf2_backdrop;
mod sf2_carrier_backdrop;
mod sf2_eladard_interior_backdrop;
mod sf2_eladard_surface_backdrop;
mod sf2_game_over;
mod sf2_results;
mod sf2_hud_glyphs;
mod sf2_map_damage_glyphs;
mod sf2_map_damage_post_eladard_glyphs;
mod sf2_map_damage_warning_glyphs;
mod sf2_map_glyphs;
mod sf2_map_post_carrier_sprites;
mod sf2_map_post_leon_sprites;
mod sf2_map_post_mirage_sprites;
mod sf2_map_sprites;
mod sf2_mission_hud;
mod sf2_mission_overlay;
mod sf2_strategic_map;
mod sf2_strategic_map_escalated;
mod sf2_strategic_map_post_carrier;
mod sf2_strategic_map_post_eladard;
mod sf2_strategic_map_post_fighter_intercept;
mod sf2_strategic_map_post_interception;
mod sf2_strategic_map_post_leon;
mod sf2_strategic_map_post_mirage;
mod sf2_strategic_map_post_pigma;
mod sf2_titania_backdrop;
pub mod shapes_gl;
pub mod sprites;
pub mod text3d;
pub mod transform;
pub mod ui;
