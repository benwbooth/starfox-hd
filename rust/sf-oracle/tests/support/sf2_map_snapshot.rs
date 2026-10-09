#![allow(dead_code)]
//! Decoding of the strategic map screen's and map program's retail state
//! for the oracles.

use sf2_game::strategic_director::{MapDirector, MapTally};
use sf2_game::strategic_screen::{MapCampaign, MapPad, MapScreen, MapShip, MapWingmate, PlaceInfo, Pulse, ScreenLinks, ScriptSubject, ShipFrame};
use sf2_game::strategic_sim::UNIT_CAPACITY;
use sf_oracle::RetailMachine;

use super::strategic_snapshot::{self, Snapshot};

pub(crate) const FRAME: u32 = 0x04B5DA;
pub(crate) const FRAME_RETURN: u32 = 0x04B5FA;
pub(crate) const START: u16 = 0x1000;
pub(crate) const B: u16 = 0x8000;
pub(crate) const X: u16 = 0x0040;
pub(crate) const A: u16 = 0x0080;
pub(crate) const RIGHT: u16 = 0x0100;
pub(crate) const LEFT: u16 = 0x0200;
pub(crate) const DOWN: u16 = 0x0400;
pub(crate) const UP: u16 = 0x0800;
/// The cue ring (`$7F:6E09`) and its write index.
pub(crate) const CUE_RING: u16 = 0x1CF6;
pub(crate) const CUE_INDEX: u16 = 0x1D16;
/// The ship frames' table (`$04:D6DB`) and its bases.
pub(crate) const FRAME_TABLE: u16 = 0x6A61;

pub(crate) fn rom() -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"))
        .expect("user-owned SF2 ROM required")
}

pub(crate) fn navigate_to_map(m: &mut RetailMachine) {
    m.tick_video_frames(0, 600).unwrap();
    for _ in 0..20 {
        m.tick_video_frames(START, 6).unwrap();
        m.tick_video_frames(0, 194).unwrap();
    }
    m.tick_video_frames(0, 60).unwrap();
}

pub(crate) fn pulse(s: &Snapshot, base: u16) -> Pulse {
    let triple = |a: u16| u32::from(s.word(a)) | u32::from(s.byte(a + 2)) << 16;
    Pulse { x: triple(base), y: triple(base + 3), dx: triple(base + 6), dy: triple(base + 9), flags: s.word(base + 12) }
}

pub(crate) fn screen(s: &Snapshot) -> MapScreen {
    MapScreen {
        cursor_x: s.byte(0xDA91),
        cursor_y: s.byte(0xDA92),
        cursor_velocity_x: s.word(0xD978),
        cursor_velocity_y: s.word(0xD97A),
        cursor_speed: s.word(0xD97C),
        interface: s.word(0xDA8B),
        hover: s.word(0xDA8D),
        countdown: s.word(0xDA8F),
        selected_x: s.byte(0xDA93),
        selected_y: s.byte(0xDA94),
        arrival_x: s.byte(0xDA95),
        arrival_y: s.byte(0xDA96),
        pulses: std::array::from_fn(|k| pulse(s, 0xDA97 + 14 * k as u16)),
        clock_frames: s.word(0xDA5F),
        elapsed_steps: s.word(0xDA5B),
        threat_cue_timer: s.word(0xDA75),
        select_state: s.word(0xDA1B),
        script: s.word(0xDA7F),
        menu_choice: s.word(0xDAE7),
        menu_home: s.word(0xDAE9),
        menu_confirm: s.word(0xDAEF),
        menu_place: s.place_id(s.word(0xDAED)),
        ship: MapShip {
            x: s.word(0xDAF2),
            y: s.word(0xDAF5),
            heading: s.word(0xDAF7),
            sector: s.word(0xDB09),
            speed: s.word(0xDAFB),
            step_x: s.word(0xDAFD),
            step_y: s.word(0xDAFF),
            travel: s.word(0xDB01),
            destination: s.place_id(s.word(0xDB07)),
            span_x: s.byte(0xDB0D),
            span_y: s.byte(0xDB0E),
            error: s.byte(0xDB0F),
            target_x: s.byte(0xDB11),
            target_y: s.byte(0xDB12),
            target_pad: s.byte(0xDB13),
            x_pad: s.byte(0xDAF4),
            flight_steps: s.word(0xDB03),
            flight_countdown: s.word(0xDB05),
        },
        wingmate: MapWingmate {
            x: s.word(0xDB14),
            y: s.word(0xDB17),
            heading: s.word(0xDB19),
            speed: s.word(0xDB1B),
            velocity_x: s.word(0xDB1D),
            velocity_y: s.word(0xDB1F),
            sector: s.word(0xDB21),
            flags: s.word(0xDB23),
        },
        info: PlaceInfo {
            shown: s.word(0xDB2D),
            number: s.word(0xDB2F),
            detail: s.word(0xDB31),
            x: s.word(0xDB3D),
            y: s.word(0xDB3F),
            kind: s.word(0xDB57),
        },
        hovered_place: s.place_id(s.word(0xDB55)),
        ordered_place: s.place_id(s.word(0xDB53)),
        hovered_unit: s.unit_id(s.word(0xE09F)),
        planet_warnings: s.word(0x1C12),
        planet_warning: s.byte(0x1C10),
        warning_flash: [s.byte(0xF4EC), s.byte(0xF4ED), s.byte(0xF4EE), s.byte(0xF4EF)],
        travel_sound: s.byte(0x1CE5),
        script_offset: s.word(0xDA81),
        script_subject: subject(s, s.word(0xDA83)),
        script_countdown: s.word(0xDA85),
        saved_ship: (s.word(0xDA87), s.word(0xDA89)),
        warp_target: [s.byte(0xDB25), s.byte(0xDB26), s.byte(0xDB27)],
        warp_spare: s.byte(0xDB28),
        saved_player: (s.word(0xD99C), s.word(0xD99E)),
        saved_motion: s.word(0xE0A1),
        campaign: MapCampaign {
            marker_count: s.word(0xD99A),
            marker_cursor: s.word(0xDAEB),
            markers_placed: s.word(0xDA2F),
            wave_cursor: s.word(0xDB51),
            wave_ships: s.word(0xD98F),
            wave_escorts: s.word(0xDA41),
            start_cursor: s.word(0xDA3F),
            bases_left: s.word(0xDA59),
            placement: [s.byte(0xD9B6), s.byte(0xD9B7), s.byte(0xD9B8), s.byte(0xD9B9)],
        },
    }
}

/// DA83 holds a unit pointer or, after the guard placement, an index.
pub(crate) fn subject(s: &Snapshot, word: u16) -> ScriptSubject {
    let offset = word.wrapping_sub(strategic_snapshot::UNITS);
    if offset % strategic_snapshot::UNIT_SIZE == 0 && usize::from(offset / strategic_snapshot::UNIT_SIZE) < UNIT_CAPACITY {
        s.unit_id(word).map_or(ScriptSubject::Value(word), ScriptSubject::Unit)
    } else {
        ScriptSubject::Value(word)
    }
}

pub(crate) fn links(s: &Snapshot) -> ScreenLinks {
    ScreenLinks {
        scene: s.links(),
        service: s.word(0x1C08),
        mode: s.word(0x1B84),
        sortie_escort: s.unit_id(s.word(0xE08D)),
        warp_place: s.place_id(s.word(0xDB5D)),
        warp_unit: s.unit_id(s.word(0xDB5F)),
        difficulty: s.word(0xD7F2),
        batch_bonus: s.word(0xDA3B),
        satellite_timing: s.word(0x1BA3),
        display_flags: s.word(0x1B9C),
        stage_flags: s.word(0xD7F8),
        presentation_countdown: s.word(0x1C6E),
        presentation_flags: s.word(0x1C67),
        message: s.word(0xF582),
        text_state: [s.word(0xF576), s.word(0xF578)],
        pilot_shields: [s.byte(0x1DD1), s.byte(0x1DD5), s.byte(0x1DD7), s.byte(0x1DDB)],
        launch_location: s.word(0x1BB5),
        launch_layout: s.word(0x1BA5),
        mission_result: s.word(0xD79D),
        mission_rank: s.word(0xD7F4),
        mission_extra: s.word(0x1BF2),
        random: s.word(0x1C00),
        missile_kinds: [s.byte(0x1C06), s.byte(0x1C07)],
        hud_mode: s.byte(0x1B9E),
        planet_health: s.word(0xDB47),
        planet_damage_percent: s.word(0xDB49),
        pilots: [s.byte(0x1E14), s.byte(0x1E15)],
        planet_place: s.place_id(s.word(0xDB4D)),
        station_place: s.place_id(s.word(0xE07B)),
        pursuit_unit: s.unit_id(s.word(0xDB63)),
        menu_pad: [s.word(0x1C1F), s.word(0x1C21)],
        arrival_word: s.word(0xDA7D),
        final_stage: s.word(0xDB29),
        timeline: s.word(0xD9FD),
    }
}

pub(crate) fn pad(s: &Snapshot) -> MapPad {
    MapPad { held: s.word(0x1292), pressed: s.word(0x1296) }
}

/// The cue words retail queued between two snapshots.
pub(crate) fn queued_cues(before: &Snapshot, after: &Snapshot) -> Vec<u16> {
    let mut index = before.byte(CUE_INDEX);
    let end = after.byte(CUE_INDEX);
    let mut cues = Vec::new();
    while index != end {
        cues.push(after.word(CUE_RING + u16::from(index)));
        index = index.wrapping_add(2) & 0x1F;
    }
    cues
}

/// `$04:D6BD`'s frame pointer for a native frame choice.
pub(crate) fn frame_pointer(s: &Snapshot, frame: ShipFrame) -> u16 {
    let offset = u16::from(frame.sector) + 0x10 * u16::from(frame.pilot) + s.word(0xD9CE);
    let entry = u16::from(s.byte(FRAME_TABLE.wrapping_add(offset)));
    (entry << 5).wrapping_add(s.word(0xD9C8))
}

pub(crate) fn director(s: &Snapshot) -> MapDirector {
    MapDirector {
        aftermath: s.word(0xDA0D),
        aftermath_next: s.word(0xDA0F),
        dialog_hold: s.word(0xDA1D),
        message_wait: s.word(0xDA13),
        exit_state: s.word(0xDA21),
        timeline_next: s.word(0xD9FF),
        timeline_delay: s.word(0xDA05),
        schedule_cursor: s.word(0xDA01),
        schedule_steps: s.word(0xDA03),
        handshake: s.word(0xD7FA),
        recapture: s.word(0xDA15),
        ambush: s.word(0xDA17),
        clearing: s.word(0xDA19),
        alert: s.word(0xDA07),
        alert_timer: s.word(0xDA0B),
        threat_delay: s.word(0xDA6D),
        pending_alerts: s.word(0xDB37),
        saved_hold: s.word(0x1B8E),
        choice: s.word(0xD9C2),
        choosing: s.word(0xD9C4),
        sortie_timer: s.word(0xE08B),
        salvo_index: s.word(0xD9B2),
        tally: MapTally {
            goal: s.word(0xDA25),
            enemies_left: s.word(0xDA29),
            marks_left: s.word(0xDA2D),
            marks_cleared: s.word(0xDA33),
            fleets: s.word(0xDA37),
            fleets_cleared: s.word(0xDA3D),
            other_cleared: s.word(0xDA49),
            bases_cleared: s.word(0xDA4F),
        },
    }
}

pub(crate) const PROGRAM: u32 = 0x04B9A3;
pub(crate) const PROGRAM_RETURN: u32 = 0x04AEAB;

pub(crate) const SPRITE_PASS: u32 = 0x048301;
pub(crate) const SPRITE_PASS_RETURNS: [u32; 2] = [0x0483B2, 0x048351];

pub(crate) fn sprites(s: &Snapshot) -> sf2_game::strategic_sprites::MapSprites {
    let pair = |a: u16| (s.word(a), s.word(a + 2));
    sf2_game::strategic_sprites::MapSprites {
        counter: s.word(0xEFD3),
        phases: [s.word(0xEFD5), s.word(0xEFD7), s.word(0xEFD9), s.word(0xEFDB)],
        wraps: [s.word(0xEFDD), s.word(0xEFDF), s.word(0xEFE1), s.word(0xEFE3)],
        blink: s.word(0xD994),
        flash: s.word(0xD996),
        corner: s.word(0xD991),
        choice_blink: s.word(0x1BEE),
        cursor_frame: s.word(0xD97F),
        target_countdown: s.byte(0xD981),
        target_phase: s.word(0xD982),
        shield_blink: s.word(0x1BF0),
        speed_mark: s.word(0xDA7B),
        view_icon: pair(0xEC7F),
        satellite_icon: pair(0xE07D),
        satellite_beam: pair(0xE081),
        explosion_count: s.word(0xD9D0),
        explosion_place: s.place_id(s.word(0xD9D2)),
        explosions: std::array::from_fn(|k| std::array::from_fn(|b| s.byte(0xD9D4 + 5 * k as u16 + b as u16))),
        panel: std::array::from_fn(|k| s.byte(0xE825 + k as u16)),
        highlight_frame: s.word(0xF558),
        burst_frames: (s.word(0xF56C), s.word(0xF56E)),
        ring_frame: s.word(0xF58A),
        flare_frame: s.word(0xF596),
        marker_tick: s.word(0xF5A0),
        marker_phase: s.word(0xF5A2),
        marker_target: (s.word(0xF5A4), s.word(0xF5A6)),
        highlight_kind: s.word(0xF55A),
        effect_origin: s.word(0xF570),
        ring_position: s.word(0xF588),
        ring_animation: s.word(0xF58C),
        flare_style: s.word(0xF598),
        cursor: s.word(0x1C49),
        count: s.word(0xE834),
        oam: (0..sf2_game::strategic_sprites::OAM_BYTES as u16).map(|k| s.byte(0xE83F + k)).collect(),
    }
}

pub(crate) fn sprite_inputs(s: &Snapshot) -> sf2_game::strategic_sprites::SpriteInputs {
    sf2_game::strategic_sprites::SpriteInputs {
        hud_flags: s.byte(0x1AA6),
        gauge_style: s.word(0x1DD3),
        gauge_row: s.word(0xD812),
        panel_enabled: s.word(0xF556),
    }
}

pub(crate) fn hud(s: &Snapshot) -> sf2_game::strategic_hud::MapHud {
    sf2_game::strategic_hud::MapHud {
        marker_cycle: std::array::from_fn(|k| s.byte(0xF4E5 + k as u16)),
        station_cycle: std::array::from_fn(|k| s.byte(0xF4F3 + k as u16)),
        palette: (0..sf2_game::strategic_hud::PALETTE_BYTES as u16).map(|k| s.byte(0xEFE5 + k)).collect(),
    }
}

pub(crate) fn hud_inputs(s: &Snapshot) -> sf2_game::strategic_hud::HudInputs {
    sf2_game::strategic_hud::HudInputs {
        service_flags: s.byte(0x1AA7),
        layout: s.byte(0x1BA2),
        score: s.word(0xD816),
        lives: s.byte(0x1DD2),
    }
}

pub(crate) fn radio(s: &Snapshot) -> sf2_game::strategic_radio::MapRadio {
    sf2_game::strategic_radio::MapRadio {
        opened_from: s.word(0xF594),
        page_frames: s.word(0xF584),
        accept: s.word(0xF56A),
        shown_place: s.word(0xF59A),
        box_base: s.word(0xF572),
        choice: s.word(0xF574),
        panel: s.word(0xF556),
        radio_event: s.word(0x1E84),
        radio: sf2_game::stage_announcer::StageMessage {
            voice: s.word(0xF566),
            progress: s.word(0xF55E),
            timer: s.word(0xF560),
        },
        tick: s.word(0x1E60),
        cleared_word: s.word(0xDB39),
        layers: s.byte(0x1C51),
    }
}

pub(crate) fn message_box(s: &Snapshot) -> sf2_game::strategic_radio::MessageBox {
    let w = |a: usize| s.gsu_word(a);
    sf2_game::strategic_radio::MessageBox {
        busy: w(0x37C),
        size: w(0x36C),
        style: w(0x378),
        open: w(0x37A),
        left_pilot: w(0x382),
        right_pilot: w(0x386),
        message: w(0x388),
        progress: w(0x38A),
        duration: w(0x390),
        remaining: w(0x380),
        row: w(0x38E),
        portrait: w(0x392),
        height: w(0x356),
    }
}

pub(crate) fn radio_inputs(s: &Snapshot) -> sf2_game::strategic_radio::RadioInputs {
    sf2_game::strategic_radio::RadioInputs { portrait: s.word(0x1E70), held: s.word(0x1292) }
}
