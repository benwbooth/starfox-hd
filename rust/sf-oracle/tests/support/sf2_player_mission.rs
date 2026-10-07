//! Original mission prefix followed by actual next-visit action dispatch.
use super::{action_tests::Fixture, rom, Source, WRAM};
use sf2_game::path_scene_state::{EncounterCoordination, EncounterObjectiveCounts};
use sf2_game::path_sound::MusicControlRequest;
use sf2_game::player_action::PlayerAction;
use sf2_game::player_camera_auxiliary::{AuxiliaryCameraTask, OrbitStyle};
use sf2_game::player_mission::{self, MissionAdmission};

const OWNER: u16 = 0x0500;
const SLOT: u32 = 64;

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.world.scene.player_configuration = Some(9);
    f.world.scene.encounter_location = Some(11);
    f.world.coordination = Some(EncounterCoordination::default());
    f.world.objective_counts = Some(EncounterObjectiveCounts::default());
    f.world.reticle_inhibited = Some(false);
    f.retained_scene_flags = 0xFECF;
    f.world.cinematic_signals = Some(sf2_game::cinematic_exit::CinematicSignals {
        exit_requested: true, skip_ready: true,
    });
    f
}

fn seed_mission(f: &Fixture, source: &mut Source) {
    let r = f.world.player(&f.objects, f.owner).unwrap();
    source
        .bus
        .write8(WRAM + SLOT + 0x6A71, r.mission.unwrap().flags);
    source
        .bus
        .write8(WRAM + 0xD787, f.world.coordination.unwrap().progress);
    source
        .bus
        .write16(WRAM + 0x1BB5, f.world.scene.encounter_location.unwrap());
    source.bus.write16(
        WRAM + 0xD7F4,
        f.world.objective_counts.unwrap().remaining_word,
    );
    source.bus.write16(
        WRAM + 0x1B96,
        0xFEFF | u16::from(f.world.reticle_inhibited.unwrap()) * 0x100,
    );
}

fn visit(f: &mut Fixture, source: &mut Source) -> MissionAdmission {
    // Choose only the comparison boundary, from the test's independent inputs.
    // Every decision and write inside the prefix runs from the original image.
    let expected = if f.world.objective_counts.unwrap().remaining_word as u8 != 0
        && f.world.reticle_inhibited.unwrap()
        && f.records().action.unwrap().action.is_none()
    {
        MissionAdmission::ForcedRetreatInstalled
    } else {
        MissionAdmission::ContinueExitControl
    };
    let boundary = match expected {
        MissionAdmission::ForcedRetreatInstalled => 0x06A316,
        MissionAdmission::ContinueExitControl => 0x06A045,
    };
    source.run(0x069FAD, Some(boundary), 0, OWNER, true);
    let outcome = player_mission::advance_admission(&f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(outcome, expected);
    assert_eq!(
        source.bus.read8(WRAM + SLOT + 0x6A71),
        f.records().mission.unwrap().flags
    );
    assert_eq!(
        source.bus.read16(WRAM + 0x1BB5),
        f.world.scene.encounter_location.unwrap()
    );
    assert_eq!(
        source.bus.read8(WRAM + 0xD787),
        f.world.coordination.unwrap().progress
    );
    assert_eq!(
        source.bus.read16(WRAM + 0xD7F4),
        f.world.objective_counts.unwrap().remaining_word
    );
    assert_eq!(
        source.bus.read16(WRAM + 0x1B96),
        0xFEFF | u16::from(f.world.reticle_inhibited.unwrap()) * 0x100
    );
    f.compare(source);
    outcome
}

#[test]
fn original_mission_progress_matches_every_configuration_progress_and_latch_flag() {
    let mut source = Source::new(&rom(), 0);
    let mut f = fixture();
    for word in 0..=u16::MAX {
        f.prepare(None, word, word.rotate_left(5));
        f.world.scene.player_configuration = Some(word as u8);
        f.world.coordination.as_mut().unwrap().progress = (word >> 8) as u8;
        f.world.scene.encounter_location = Some(if word & 2 == 0 { 11 } else { 8 });
        f.records().mission.as_mut().unwrap().flags = word.rotate_left(3) as u8;
        f.seed(&mut source, false);
        seed_mission(&f, &mut source);
        visit(&mut f, &mut source);
    }
    f.world.scene.player_configuration = Some(9);
    f.world.scene.encounter_location = Some(8);
    for flag in 0..=255u8 {
        for progress in [254, 255] {
            f.prepare(None, 99, 111);
            f.records().mission.as_mut().unwrap().flags = flag;
            f.world.coordination.as_mut().unwrap().progress = progress;
            f.seed(&mut source, false);
            seed_mission(&f, &mut source);
            visit(&mut f, &mut source);
        }
    }
}

#[test]
fn original_mission_progress_compares_every_full_location_word() {
    let mut source = Source::new(&rom(), 0);
    let mut f = fixture();
    f.world.coordination.as_mut().unwrap().progress = 254;
    for location in 0..=u16::MAX {
        f.prepare(None, 99, location);
        f.records().mission.as_mut().unwrap().flags = 0x3F;
        f.world.scene.encounter_location = Some(location);
        f.seed(&mut source, false);
        seed_mission(&f, &mut source);
        visit(&mut f, &mut source);
    }
}

#[test]
fn original_retreat_admission_matches_every_objective_word_and_existing_action_gate() {
    let mut source = Source::new(&rom(), 0);
    let mut f = fixture();
    f.world.scene.player_configuration = Some(0);
    for word in 0..=u16::MAX {
        for variant in 0..4 {
            f.prepare(
                (variant & 2 != 0).then_some(PlayerAction::TriggeredProjectile),
                word,
                word.rotate_left(7),
            );
            f.world.objective_counts.as_mut().unwrap().remaining_word = word;
            f.world.reticle_inhibited = Some(variant & 1 != 0);
            f.seed(&mut source, false);
            seed_mission(&f, &mut source);
            visit(&mut f, &mut source);
        }
    }
}

#[test]
fn original_mission_installs_retreat_for_next_visit_and_shared_music_observes_last_writer() {
    let mut source = Source::new(&rom(), 0);
    let mut f = fixture();
    f.prepare(None, 531, 0xFFFF);
    f.records().mission.as_mut().unwrap().flags = 0x3F;
    f.world.scene.encounter_location = Some(8);
    f.world.coordination.as_mut().unwrap().progress = 254;
    f.world.objective_counts.as_mut().unwrap().remaining_word = 1;
    f.world.reticle_inhibited = Some(true);
    f.seed(&mut source, false);
    seed_mission(&f, &mut source);
    for frame in 0..48 {
        f.visit(&mut source, false);
        if frame == 2 {
            f.world.coordination.as_mut().unwrap().progress = 255;
            source.bus.write8(WRAM + 0xD787, 255);
        }
        assert_eq!(
            visit(&mut f, &mut source),
            if frame == 0 {
                MissionAdmission::ForcedRetreatInstalled
            } else {
                MissionAdmission::ContinueExitControl
            }
        );
        assert_eq!(f.records().action.unwrap().elapsed, frame);
        assert_eq!(
            f.world.audio.pending_music_control(),
            Some(match frame {
                0 => MusicControlRequest::EncounterProgressTransition,
                1 => MusicControlRequest::ForcedRetreat,
                _ => MusicControlRequest::EncounterProgressComplete,
            })
        );
        assert_eq!(
            f.records().camera_auxiliary.unwrap().task,
            if frame >= 9 {
                AuxiliaryCameraTask::Initialize(OrbitStyle::Retreat)
            } else {
                AuxiliaryCameraTask::Handoff
            }
        );
    }
}
