//! Unmodified surface palette entries and the complete render-publication
//! tail. Hardware bus handshakes execute too; the native side retains only
//! domain state. Expected outputs never seed native or subsequent visits.

use super::surface_particle_tests::{Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_data::surface_particles::SurfaceParticlePalette;
use sf2_game::collision_surface::SurfaceMode;
use sf2_game::intro_material::{DepthColorFamily, DepthThresholdTable, SceneLighting};
use sf2_game::player_action::ScenePalette;
use sf2_game::player_mode_selection::PlayerModeSelection;
use sf2_game::player_surface_render::{
    self, AmbientParticleControl, SceneRenderEnvironment, SurfaceViewSide,
};

fn seed_palette(source: &mut Source, native: &mut Native, seed: u16) {
    let palette = ScenePalette {
        colors: std::array::from_fn(|i| seed.wrapping_add((i as u16).wrapping_mul(173))),
        saved_colors: std::array::from_fn(|i| !seed.wrapping_add((i as u16).wrapping_mul(137))),
    };
    for (start, colors) in [(0xEFE5, palette.colors), (0xF2E5, palette.saved_colors)] {
        for (index, color) in colors.into_iter().enumerate() {
            source.bus.write16(WRAM + start + index as u32 * 2, color);
        }
    }
    native.world.palette = Some(palette);
}

fn check_palette(source: &mut Source, native: &Native) {
    let palette = native.world.palette.as_ref().unwrap();
    for (start, colors) in [(0xEFE5, palette.colors), (0xF2E5, palette.saved_colors)] {
        for (index, color) in colors.into_iter().enumerate() {
            assert_eq!(
                color,
                source.bus.read16(WRAM + start + index as u32 * 2),
                "palette {start:04X} color {index}"
            );
        }
    }
    assert_eq!(
        native.world.palette_refresh_requested,
        Some(source.bus.read8(WRAM + 0x1E58) & 0x80 != 0)
    );
}

#[test]
fn original_surface_palette_entries_preserve_unrelated_colors_and_refresh_bits() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source, 1, 0, 0, 0);
    for flag in 0..=255u16 {
        seed_palette(&mut source, &mut native, flag.wrapping_mul(313));
        source.bus.write16(WRAM + 0x1E58, 0xAB00 | flag);
        native.world.palette_refresh_requested = Some(flag & 0x80 != 0);
        // Retain each result for the following entry in both implementations.
        for side in [
            SurfaceViewSide::Nonnegative,
            SurfaceViewSide::Negative,
            SurfaceViewSide::Nonnegative,
        ] {
            source.run(
                match side {
                    SurfaceViewSide::Nonnegative => 0x07EB78,
                    SurfaceViewSide::Negative => 0x07EB94,
                },
                None,
                0,
                OWNER,
                true,
            );
            player_surface_render::replace_polygon_palette(&mut native.world, side).unwrap();
            check_palette(&mut source, &native);
            assert_eq!(
                source.bus.read16(WRAM + 0x1E58) & 0xFF7F,
                0xAB00 | (flag & 0x7F)
            );
        }
    }
}

fn seed_environment(source: &mut Source, native: &mut Native, seed: u16) {
    let palette = if seed & 1 == 0 {
        SurfaceParticlePalette::Negative
    } else {
        SurfaceParticlePalette::Nonnegative
    };
    native.world.render_environment = SceneRenderEnvironment {
        lighting: SceneLighting {
            depth_colors: DepthColorFamily::from_catalog_index(4),
            thresholds: Some(DepthThresholdTable::OPENING),
        },
        plane_height: Some(seed as i16),
        ambient_height_gate: Some(seed.rotate_left(9)),
        ambient_control: Some(AmbientParticleControl::from_bits(seed.rotate_left(5))),
        ambient_palette: Some(palette),
        ambient_size: Some(seed.rotate_left(3)),
    };
    for (address, value) in [
        (0x70004E, 0x8D0C),
        (0x700050, 0x8F40),
        (WRAM + 0x18B9, seed),
        (WRAM + 0x1E11, seed.rotate_left(9)),
        (0x7001BC, seed.rotate_left(5)),
        (0x70285C, if seed & 1 == 0 { 0xFF5D } else { 0xFF6D }),
        (0x70285E, seed.rotate_left(3)),
    ] {
        source.bus.write16(address, value);
    }
}

fn compare_environment(source: &mut Source, native: &Native, mode: u8, flags: u8) {
    let state = native.world.render_environment;
    for (name, actual, address) in [
        (
            "depth family",
            0x8B0C + state.lighting.depth_colors.unwrap().catalog_index() as u16 * 128,
            0x70004E,
        ),
        (
            "thresholds",
            0x8F1C + state.lighting.thresholds.unwrap().catalog_index() as u16 * 4,
            0x700050,
        ),
        (
            "render plane",
            state.plane_height.unwrap() as u16,
            WRAM + 0x18B9,
        ),
        (
            "height gate",
            state.ambient_height_gate.unwrap(),
            WRAM + 0x1E11,
        ),
        (
            "ambient control",
            state.ambient_control.unwrap().bits(),
            0x7001BC,
        ),
        (
            "ambient palette",
            match state.ambient_palette.unwrap() {
                SurfaceParticlePalette::Negative => 0xFF5D,
                SurfaceParticlePalette::Nonnegative => 0xFF6D,
            },
            0x70285C,
        ),
        ("ambient size", state.ambient_size.unwrap(), 0x70285E),
    ] {
        assert_eq!(
            actual,
            source.bus.read16(address),
            "{name} mode={mode:02X} flags={flags:02X}"
        );
    }
}

fn step(source: &mut Source, native: &mut Native, mode: u8, flags: u8, plane: i16) {
    native.world.surface_mode = Some(SurfaceMode { flags: mode });
    native.world.environment_plane_height = Some(plane);
    native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap()
        .mode_selection = Some(PlayerModeSelection {
        surface_control: flags,
        ..Default::default()
    });
    source.bus.write16(WRAM + 0x1B4D, 0xAA00 | u16::from(mode));
    source.bus.write16(WRAM + 0x1E0F, plane as u16);
    source.bus.write8(WRAM + SLOT + 0x6B64, flags);
    source.run_with_y(0x07C355, Some(0x07C440), 0, OWNER, true, Some(SLOT as u16));
    player_surface_render::publish_environment(&native.objects, &mut native.world, native.owner)
        .unwrap();
    compare_environment(source, native, mode, flags);
    assert_eq!(source.bus.read16(WRAM + 0x1B4D), 0xAA00 | u16::from(mode));
    assert_eq!(source.bus.read8(WRAM + SLOT + 0x6B64), flags);
}

#[test]
fn original_surface_render_tail_matches_every_mode_and_independent_flag_byte() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source, 1, 0, 0, 0);
    seed_palette(&mut source, &mut native, 0x2ABC);
    source.bus.write16(WRAM + 0x1E58, 0x53);
    native.world.palette_refresh_requested = Some(false);
    let random = native.world.random.bytes();
    for mode in 0..=255u8 {
        seed_environment(&mut source, &mut native, u16::from(mode).wrapping_mul(521));
        for flags in 0..=255u8 {
            // Crossings retain prior published values, including transitions
            // between both flag branches. Ineligible modes retain all fields.
            step(
                &mut source,
                &mut native,
                mode,
                flags,
                (u16::from(mode).wrapping_mul(263) ^ u16::from(flags).wrapping_mul(521)) as i16,
            );
        }
        check_palette(&mut source, &native);
        assert_eq!(native.world.random.bytes(), random);
    }
}

#[test]
fn original_positive_height_tail_preserves_every_inherited_ambient_control_word() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source, 1, 0, 0, 0);
    seed_environment(&mut source, &mut native, 0x7531);
    for bits in 0..=u16::MAX {
        source.bus.write16(0x7001BC, bits);
        native.world.render_environment.ambient_control =
            Some(AmbientParticleControl::from_bits(bits));
        step(
            &mut source,
            &mut native,
            0xFA,
            4 | (bits as u8 & 2),
            bits as i16,
        );
    }
}

#[test]
fn original_render_and_palette_sequence_retains_state_across_scene_mode_changes() {
    let mut source = Source::new(&rom(), 0);
    let mut native = Native::new(&mut source, 1, 0, 0, 0);
    seed_environment(&mut source, &mut native, 0xA731);
    seed_palette(&mut source, &mut native, 0x7421);
    source.bus.write16(WRAM + 0x1E58, 0x1253);
    native.world.palette_refresh_requested = Some(false);
    for visit in 0..4096u16 {
        let mode = [2, 0, 2, 4, 0xAA, 7, 2, 0xFF][usize::from(visit & 7)];
        let flags = visit.rotate_left(3).wrapping_add(visit) as u8;
        if visit % 11 == 0 {
            let side = if flags & 4 == 0 {
                SurfaceViewSide::Negative
            } else {
                SurfaceViewSide::Nonnegative
            };
            source.run(
                if side == SurfaceViewSide::Negative {
                    0x07EB94
                } else {
                    0x07EB78
                },
                None,
                0,
                OWNER,
                true,
            );
            player_surface_render::replace_polygon_palette(&mut native.world, side).unwrap();
        }
        step(
            &mut source,
            &mut native,
            mode,
            flags,
            visit.wrapping_mul(571) as i16,
        );
        check_palette(&mut source, &native);
    }
}
