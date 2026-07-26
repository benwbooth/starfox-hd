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
mod sf1_briefing;
mod sf1_planets;
mod sf2_aim_sight;
mod sf2_backdrop;
mod sf2_briefing;
mod sf2_carrier_backdrop;
mod sf2_eladard_interior_backdrop;
mod sf2_eladard_surface_backdrop;
mod sf2_ending;
mod sf2_fortuna_backdrop;
mod sf2_game_over;
mod sf2_intro;
mod sf2_macbeth_backdrop;
mod sf2_opening_overview;
mod sf2_pilot_selection;
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
mod sf2_meteor_backdrop;
mod sf2_mission_hud;
mod sf2_mission_message_panel;
mod sf2_mission_message_portraits;
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
mod sf2_title;
mod sf2_venom_backdrop;
pub mod shapes_gl;
pub mod sprites;
pub mod text3d;
pub mod transform;
pub mod ui;

#[cfg(test)]
mod campaign_world_backdrop_tests {
    const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
    const FNV_PRIME: u32 = 0x01000193;
    const FIXTURE: &str =
        include_str!("../../../tools/sf2/fixtures/campaign_world_backgrounds.trace");

    fn fixture_hash(world: &str) -> u32 {
        let line = FIXTURE
            .lines()
            .find(|line| line.starts_with(&format!("world name={world} ")))
            .unwrap_or_else(|| panic!("missing {world} campaign-world backdrop fixture"));
        let encoded = line
            .split_whitespace()
            .find_map(|field| field.strip_prefix("rgba_fnv1a="))
            .unwrap_or_else(|| panic!("missing {world} RGBA hash"));
        u32::from_str_radix(encoded, 16).expect("fixture hash must be hexadecimal")
    }

    fn rgba_hash(rgba: Vec<u8>) -> u32 {
        rgba.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
            (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
        })
    }

    #[test]
    fn all_missing_campaign_world_backdrops_match_retail_captures() {
        let worlds = [
            ("venom", super::sf2_venom_backdrop::decode_rgba()),
            ("macbeth", super::sf2_macbeth_backdrop::decode_rgba()),
            ("meteor", super::sf2_meteor_backdrop::decode_rgba()),
            ("fortuna", super::sf2_fortuna_backdrop::decode_rgba()),
        ];
        for (world, rgba) in worlds {
            assert_eq!(rgba_hash(rgba), fixture_hash(world), "{world}");
        }
    }
}
