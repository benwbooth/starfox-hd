//! Star Fox HD — Rust application shell (the RIIR integration binary).
//!
//! Port (C oracle): `src/main.c` — SDL init, GL 3.3 core context, fixed
//! 50 ms (20 Hz) tick accumulator with render interpolation, SF_STATE_DUMP.
//! Game-side state machine and frame-input producers live in
//! `sf_game::shell`; rendering in `sf_render`; audio in `sf_audio`.
//!
//! Extra env knobs (not in the C build, used by the smoke tests):
//! - `SF_MAX_TICKS=<n>`  exit after n game ticks
//! - `SF_HIDDEN=1`       create the window hidden (headless-ish CI runs)
//! - `SF_FAST_FORWARD=1` run fixed ticks without wall-clock pacing (tests)
//! - `SF2_AUTOPLAY_PAUSE_AFTER_SORTIES=<n>` hold a reached map for capture
//! - `SF_DUMP_PPM=<path>` write one RGB PPM frame readback (at tick
//!   `SF_DUMP_PPM_TICK`, default 220).

mod audio;
mod config;
mod input;
mod statedump;

use std::path::{Path, PathBuf};
use std::time::Instant;

use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Keycode;
use sdl3::video::FullscreenType;

use sf_core::{DrawListEntry as CoreEntry, GAME_TICK_MS, MAX_DRAW_LIST};
use sf_game::shell::Shell;
use sf_render::draw_list::{
    DrawListEntry as RenderEntry, DL_FLAG_HIGHLIGHT, DL_FLAG_SHADOW, DL_FLAG_VISIBLE,
};
use sf_render::renderer::{
    EndingReplayBackdrop as RenderEndingReplayBackdrop, EndingReplayInputs, FrameInputs,
    GameState as RenderState, Renderer, RendererConfig, Sf2AudioOutput, Sf2Difficulty,
    Sf2EndingPhase, Sf2FlightControlStyle, Sf2FrameInputs, Sf2GameOverChoice, Sf2GameOverPhase,
    Sf2MapPoint, Sf2MissionBackdrop, Sf2MissionMessage, Sf2MissionMessageInputs,
    Sf2MissionMessageIrisFrame, Sf2MissionMessagePhase, Sf2Mode, Sf2Pilot, Sf2PilotSelectionCursor,
    Sf2PilotSelectionPhase, Sf2RadarContact, Sf2ResultsChoice, Sf2ResultsPhase, Sf2StrategicActor,
    Sf2StrategicActorAppearance, Sf2StrategicActorKind, Sf2StrategicPhase, Sf2TitleMenuItem,
    Sf2TitlePage, WindowState, SF2_RADAR_CONTACT_CAPACITY, WINDOWARRAY_SIZE,
};
use sf_render::shapes::Sf2PolygonPalette;

use crate::audio::AudioSys;
use crate::config::Config;
use crate::input::Input;
use crate::statedump::StateDump;

const STAR_FOX_2_ROM_SIZE: usize = 1_048_576;
const STAR_FOX_2_TITLE_OFFSET: usize = 32_704;
const STAR_FOX_2_TITLE: &[u8] = b"STARFOX2";
const STAR_FOX_2_SAVE_MAGIC: &[u8; 5] = b"SF2HD";
const STAR_FOX_2_SAVE_VERSION: u8 = 1;
const STAR_FOX_2_SAVE_EXPERT_UNLOCKED_FLAG: u8 = 0x10;
const STAR_FOX_2_SAVE_LENGTH: usize = STAR_FOX_2_SAVE_MAGIC.len() + 2;
const DEFAULT_STAR_FOX_2_SAVE_PATH: &str = "starfox2.save";
const WORLD_TO_RENDER_FRACTIONAL_BITS: u32 = 16;
const STAR_FOX_2_TICKS_PER_SECOND: f64 = 15.0;
const SF2_RETAIL_PRESENTATION_FRAMES_PER_TICK: u32 = 4;

/// Stable diagnostic codes for native SF2 state dumps. These describe port
/// modes, not source-machine execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Sf2DumpMode {
    Intro = 1,
    Title = 2,
    Records = 3,
    Briefing = 4,
    StrategicMap = 5,
    PilotSelection = 6,
    Mission = 7,
    Results = 8,
    Ending = 9,
    GameOver = 10,
}

impl From<sf2_game::GameMode> for Sf2DumpMode {
    fn from(mode: sf2_game::GameMode) -> Self {
        match mode {
            sf2_game::GameMode::Intro(_) => Self::Intro,
            sf2_game::GameMode::Title => Self::Title,
            sf2_game::GameMode::Records => Self::Records,
            sf2_game::GameMode::Briefing => Self::Briefing,
            sf2_game::GameMode::StrategicMap => Self::StrategicMap,
            sf2_game::GameMode::PilotSelection => Self::PilotSelection,
            sf2_game::GameMode::Mission => Self::Mission,
            sf2_game::GameMode::GameOver => Self::GameOver,
            sf2_game::GameMode::Results => Self::Results,
            sf2_game::GameMode::Ending => Self::Ending,
        }
    }
}

/// sf_core entry -> sf_render entry (field-identical structs; the render
/// crate keeps its own copy so it does not depend on game types).
fn to_render_entry(e: &CoreEntry) -> RenderEntry {
    RenderEntry {
        x: e.x,
        y: e.y,
        z: e.z,
        rx: e.rx,
        ry: e.ry,
        rz: e.rz,
        shape_id: e.shape_id,
        color_table: e.color_table,
        sort_z: e.sort_z,
        sflags: e.sflags,
        explosion_cnt: e.explosion_cnt,
        anim_frame: e.anim_frame,
        col_frame: e.col_frame,
        depth_offset: e.depth_offset,
        flags: e.flags,
        shad_x: e.shad_x,
        shad_y: e.shad_y,
        shad_z: e.shad_z,
        tscroll_x: e.tscroll_x,
        tscroll_y: e.tscroll_y,
        obj_id: e.obj_id,
    }
}

fn sf2_world_to_render(value: i16) -> i32 {
    i32::from(value) << WORLD_TO_RENDER_FRACTIONAL_BITS
}

fn sf2_depth_to_render(value: i16) -> i32 {
    sf2_world_to_render(value.wrapping_neg())
}

fn to_sf2_render_entry(object: &sf2_game::RenderObject) -> RenderEntry {
    object
        .shape
        .catalog_entry()
        .expect("native SF2 objects carry a validated catalog shape");
    let mut flags = 0;
    if object.flags.visible {
        flags |= DL_FLAG_VISIBLE;
    }
    if object.flags.casts_shadow {
        flags |= DL_FLAG_SHADOW;
    }
    if object.flags.highlighted {
        flags |= DL_FLAG_HIGHLIGHT;
    }
    RenderEntry {
        x: sf2_world_to_render(object.position.x),
        y: sf2_world_to_render(object.position.y),
        z: sf2_depth_to_render(object.position.z),
        rx: i16::from(object.rotation.pitch.units()),
        ry: i16::from(object.rotation.yaw.units()),
        rz: i16::from(object.rotation.roll.units()),
        shape_id: object.shape.flat_render_id(),
        color_table: object.material_set.catalog_token(),
        sort_z: object.sort_depth,
        sflags: 0,
        explosion_cnt: object.animation.explosion_frame,
        anim_frame: object.animation.shape_frame,
        col_frame: object.animation.color_frame,
        depth_offset: object.depth_offset,
        flags,
        shad_x: 0,
        shad_y: 0,
        shad_z: 0,
        tscroll_x: object.texture_scroll_x,
        tscroll_y: object.texture_scroll_y,
        obj_id: object.object.stable_render_id(),
    }
}

fn to_sf2_pilot(pilot: sf2_game::Pilot) -> Sf2Pilot {
    match pilot {
        sf2_game::Pilot::Fox => Sf2Pilot::Fox,
        sf2_game::Pilot::Falco => Sf2Pilot::Falco,
        sf2_game::Pilot::Peppy => Sf2Pilot::Peppy,
        sf2_game::Pilot::Slippy => Sf2Pilot::Slippy,
        sf2_game::Pilot::Miyu => Sf2Pilot::Miyu,
        sf2_game::Pilot::Fay => Sf2Pilot::Fay,
    }
}

fn to_sf2_mission_message(
    message: sf2_game::MissionMessageState,
) -> Option<Sf2MissionMessageInputs> {
    let message_kind = match message.message? {
        sf2_game::MissionMessage::FlyFasterByPressingYButton => {
            Sf2MissionMessage::FlyFasterByPressingYButton
        }
    };
    let iris_frame = |frame| match frame {
        sf2_game::MissionMessageIrisFrame::ThinLine => Sf2MissionMessageIrisFrame::ThinLine,
        sf2_game::MissionMessageIrisFrame::EmptyPanel => Sf2MissionMessageIrisFrame::EmptyPanel,
        sf2_game::MissionMessageIrisFrame::SparseInterference => {
            Sf2MissionMessageIrisFrame::SparseInterference
        }
        sf2_game::MissionMessageIrisFrame::DenseInterference => {
            Sf2MissionMessageIrisFrame::DenseInterference
        }
        sf2_game::MissionMessageIrisFrame::FullInterference => {
            Sf2MissionMessageIrisFrame::FullInterference
        }
    };
    let phase = match message.phase {
        sf2_game::MissionMessagePhase::Hidden => return None,
        sf2_game::MissionMessagePhase::Opening(frame) => {
            Sf2MissionMessagePhase::Opening(iris_frame(frame))
        }
        sf2_game::MissionMessagePhase::Open => Sf2MissionMessagePhase::Open {
            portrait_talking: message.portrait_talking,
        },
        sf2_game::MissionMessagePhase::Closing(frame) => {
            Sf2MissionMessagePhase::Closing(iris_frame(frame))
        }
    };
    Some(Sf2MissionMessageInputs {
        message: message_kind,
        phase,
    })
}

fn to_sf2_strategic_actor(actor: sf2_game::StrategicMapActor) -> Sf2StrategicActor {
    Sf2StrategicActor {
        kind: match actor.kind {
            sf2_game::StrategicMapActorKind::NorthernInstallation => {
                Sf2StrategicActorKind::NorthernInstallation
            }
            sf2_game::StrategicMapActorKind::SouthernInstallation => {
                Sf2StrategicActorKind::SouthernInstallation
            }
            sf2_game::StrategicMapActorKind::EnemyCarrier => Sf2StrategicActorKind::EnemyCarrier,
            sf2_game::StrategicMapActorKind::EnemyFormation => {
                Sf2StrategicActorKind::EnemyFormation
            }
            sf2_game::StrategicMapActorKind::EasternInterceptor => {
                Sf2StrategicActorKind::EasternInterceptor
            }
            sf2_game::StrategicMapActorKind::PatrolShip => Sf2StrategicActorKind::PatrolShip,
            sf2_game::StrategicMapActorKind::MissileTrail => Sf2StrategicActorKind::MissileTrail,
            sf2_game::StrategicMapActorKind::Missile => Sf2StrategicActorKind::Missile,
            sf2_game::StrategicMapActorKind::AttackingFighter => {
                Sf2StrategicActorKind::AttackingFighter
            }
            sf2_game::StrategicMapActorKind::RivalFighter => Sf2StrategicActorKind::RivalFighter,
            sf2_game::StrategicMapActorKind::FighterProjectile => {
                Sf2StrategicActorKind::FighterProjectile
            }
            sf2_game::StrategicMapActorKind::UnknownSignal => Sf2StrategicActorKind::UnknownSignal,
            sf2_game::StrategicMapActorKind::DefensePlatform => {
                Sf2StrategicActorKind::DefensePlatform
            }
        },
        appearance: match actor.appearance {
            sf2_game::StrategicMapAppearance::OpeningAssault => {
                Sf2StrategicActorAppearance::OpeningAssault
            }
            sf2_game::StrategicMapAppearance::EscalatedAssault => {
                Sf2StrategicActorAppearance::EscalatedAssault
            }
            sf2_game::StrategicMapAppearance::PostInterception => {
                Sf2StrategicActorAppearance::PostInterception
            }
            sf2_game::StrategicMapAppearance::PostFighterIntercept => {
                Sf2StrategicActorAppearance::PostFighterIntercept
            }
            sf2_game::StrategicMapAppearance::PostPigma => Sf2StrategicActorAppearance::PostPigma,
            sf2_game::StrategicMapAppearance::PostEladard => {
                Sf2StrategicActorAppearance::PostEladard
            }
            sf2_game::StrategicMapAppearance::PostCarrier => {
                Sf2StrategicActorAppearance::PostCarrier
            }
            sf2_game::StrategicMapAppearance::PostLeon => Sf2StrategicActorAppearance::PostLeon,
            sf2_game::StrategicMapAppearance::PostMirage => Sf2StrategicActorAppearance::PostMirage,
        },
        position: Sf2MapPoint {
            x: actor.position.x,
            y: actor.position.y,
        },
    }
}

fn to_sf2_polygon_palette(mission: &sf2_game::MissionState) -> Sf2PolygonPalette {
    use sf2_game::{AstropolisPhase, EladardPhase, MissionVisit};

    match mission.visit {
        MissionVisit::EladardBase
            if matches!(
                mission.eladard.phase,
                EladardPhase::SurfaceApproach
                    | EladardPhase::SurfaceBarriers
                    | EladardPhase::BaseEntrance
            ) =>
        {
            Sf2PolygonPalette::EladardSurface
        }
        MissionVisit::AstropolisAssault
            if matches!(
                mission.astropolis.phase,
                AstropolisPhase::ExteriorApproach | AstropolisPhase::BaseEntry
            ) =>
        {
            Sf2PolygonPalette::AstropolisExterior
        }
        MissionVisit::OpeningEngagement
        | MissionVisit::Reengagement
        | MissionVisit::MissileInterception
        | MissionVisit::FighterIntercept
        | MissionVisit::PigmaDuel
        | MissionVisit::EladardBase
        | MissionVisit::TitaniaBase
        | MissionVisit::MacbethBase
        | MissionVisit::MeteorBase
        | MissionVisit::FortunaBase
        | MissionVisit::VenomBase
        | MissionVisit::FirstBattleCarrier
        | MissionVisit::SecondBattleCarrier
        | MissionVisit::LeonDuel
        | MissionVisit::MirageDragon
        | MissionVisit::RecurringAttackers
        | MissionVisit::LeonPressure
        | MissionVisit::FinalPursuer
        | MissionVisit::WolfBlockade
        | MissionVisit::AstropolisAssault => Sf2PolygonPalette::Standard,
    }
}

fn to_sf2_mission_backdrop(mission: &sf2_game::MissionState) -> Sf2MissionBackdrop {
    use sf2_game::{CarrierAssaultPhase, EladardPhase, MissionVisit};

    match mission.visit {
        MissionVisit::EladardBase => match mission.eladard.phase {
            EladardPhase::SurfaceApproach
            | EladardPhase::SurfaceBarriers
            | EladardPhase::BaseEntrance
            | EladardPhase::ReturnFlight => Sf2MissionBackdrop::EladardSurface,
            EladardPhase::InteriorPassage
            | EladardPhase::GeneratorRoom
            | EladardPhase::BaseDestruction => Sf2MissionBackdrop::EladardInterior,
        },
        MissionVisit::TitaniaBase => Sf2MissionBackdrop::TitaniaBase,
        MissionVisit::MacbethBase => Sf2MissionBackdrop::MacbethSurface,
        MissionVisit::MeteorBase => Sf2MissionBackdrop::MeteorSurface,
        MissionVisit::FortunaBase => Sf2MissionBackdrop::FortunaSurface,
        MissionVisit::VenomBase => Sf2MissionBackdrop::VenomSurface,
        MissionVisit::FirstBattleCarrier | MissionVisit::SecondBattleCarrier => {
            match mission.carrier_assault.phase {
                CarrierAssaultPhase::ExteriorApproach | CarrierAssaultPhase::ReturnFlight => {
                    Sf2MissionBackdrop::DeepSpace
                }
                CarrierAssaultPhase::InteriorCorridor
                | CarrierAssaultPhase::ReactorApproach
                | CarrierAssaultPhase::ReactorCombat
                | CarrierAssaultPhase::CoreDestruction => Sf2MissionBackdrop::CarrierInterior,
            }
        }
        MissionVisit::AstropolisAssault => Sf2MissionBackdrop::AstropolisVoid,
        MissionVisit::OpeningEngagement
        | MissionVisit::Reengagement
        | MissionVisit::MissileInterception
        | MissionVisit::FighterIntercept
        | MissionVisit::PigmaDuel
        | MissionVisit::LeonDuel
        | MissionVisit::MirageDragon
        | MissionVisit::RecurringAttackers
        | MissionVisit::LeonPressure
        | MissionVisit::FinalPursuer
        | MissionVisit::WolfBlockade => Sf2MissionBackdrop::DeepSpace,
    }
}

fn to_sf2_frame_inputs(game: &sf2_game::Game) -> Sf2FrameInputs {
    use sf2_game::{
        AudioOutput, Difficulty, EndingPhase, FlightControlStyle, GameMode, GameOverChoice,
        GameOverDestination, GameOverPhase, ObjectKind, PilotSelectionCursor, PilotSelectionPhase,
        ResultsChoice, ResultsPhase, StrategicMapPhase, TitleMenuItem, TitlePage,
    };

    let state = game.state();
    let mut radar_contacts = [None; SF2_RADAR_CONTACT_CAPACITY];
    let mut radar_contact_count = 0;
    let mut target_count = 0u8;
    if let Some(player) = state
        .mission
        .primary_player
        .and_then(|id| state.objects.get(id))
    {
        for (_, object) in state.objects.active_objects() {
            let friendly = match object.base.kind {
                ObjectKind::Enemy => {
                    target_count = target_count.saturating_add(1);
                    false
                }
                // Retail's active-flight radar shows the local craft at its
                // fixed origin and enemy contacts. The co-pilot craft is not
                // inserted as a second local marker.
                ObjectKind::Wingmate => continue,
                ObjectKind::Player
                | ObjectKind::Projectile
                | ObjectKind::Scenery
                | ObjectKind::Effect => continue,
            };
            if radar_contact_count >= SF2_RADAR_CONTACT_CAPACITY {
                continue;
            }
            let relative_x = object.base.position.x.wrapping_sub(player.base.position.x);
            let relative_z = object.base.position.z.wrapping_sub(player.base.position.z);
            let (lateral, forward) = sf_core::snes_trig::rotate_16xz(
                player.base.yaw.units().wrapping_neg(),
                relative_x,
                relative_z,
            );
            radar_contacts[radar_contact_count] = Some(Sf2RadarContact {
                lateral,
                forward,
                friendly,
            });
            radar_contact_count += 1;
        }
    }
    Sf2FrameInputs {
        mode: match state.mode {
            GameMode::Intro(_) => Sf2Mode::Intro,
            GameMode::Title => Sf2Mode::Title,
            GameMode::Records => Sf2Mode::Records,
            GameMode::Briefing => Sf2Mode::Briefing,
            GameMode::StrategicMap => Sf2Mode::StrategicMap,
            GameMode::PilotSelection => Sf2Mode::PilotSelection,
            GameMode::Mission => Sf2Mode::Mission,
            GameMode::GameOver => Sf2Mode::GameOver,
            GameMode::Results => Sf2Mode::Results,
            GameMode::Ending => Sf2Mode::Ending,
        },
        intro_presentation_tick: state.intro.presentation_tick,
        intro_title_menu_countdown: state.intro.title_menu_countdown,
        polygon_palette: to_sf2_polygon_palette(&state.mission),
        mission_backdrop: to_sf2_mission_backdrop(&state.mission),
        title_page: match state.title.page {
            TitlePage::MainMenu => Sf2TitlePage::MainMenu,
            TitlePage::Difficulty => Sf2TitlePage::Difficulty,
        },
        title_menu_item: match state.title.menu_item {
            TitleMenuItem::Mission => Sf2TitleMenuItem::Mission,
            TitleMenuItem::Records => Sf2TitleMenuItem::Records,
            TitleMenuItem::SoundMode => Sf2TitleMenuItem::SoundMode,
        },
        difficulty: match state.campaign.difficulty {
            Difficulty::Normal => Sf2Difficulty::Normal,
            Difficulty::Hard => Sf2Difficulty::Hard,
            Difficulty::Expert => Sf2Difficulty::Expert,
        },
        audio_output: match state.title.audio_output {
            AudioOutput::Stereo => Sf2AudioOutput::Stereo,
            AudioOutput::Mono => Sf2AudioOutput::Mono,
        },
        pilot_selection_phase: match state.pilot_selection.phase {
            PilotSelectionPhase::Revealing => Sf2PilotSelectionPhase::Revealing,
            PilotSelectionPhase::ChoosingPrimary => Sf2PilotSelectionPhase::ChoosingPrimary,
            PilotSelectionPhase::ChoosingWingmate => Sf2PilotSelectionPhase::ChoosingWingmate,
            PilotSelectionPhase::Ready => Sf2PilotSelectionPhase::Ready,
            PilotSelectionPhase::Launching => Sf2PilotSelectionPhase::Launching,
        },
        pilot_selection_cursor: match state.pilot_selection.cursor {
            PilotSelectionCursor::Pilot(pilot) => {
                Sf2PilotSelectionCursor::Pilot(to_sf2_pilot(pilot))
            }
            PilotSelectionCursor::Control => Sf2PilotSelectionCursor::Control,
        },
        flight_control_style: match state.pilot_selection.control_style {
            FlightControlStyle::TypeA => Sf2FlightControlStyle::TypeA,
            FlightControlStyle::TypeB => Sf2FlightControlStyle::TypeB,
        },
        primary_pilot: state.roster.selected[0].map(to_sf2_pilot),
        wingmate: state.roster.selected[1].map(to_sf2_pilot),
        game_over_phase: match state.game_over.phase {
            GameOverPhase::AndrossTaunt => Sf2GameOverPhase::AndrossTaunt,
            GameOverPhase::Choosing(_) => Sf2GameOverPhase::Choosing,
            GameOverPhase::Leaving { .. } => Sf2GameOverPhase::Leaving,
        },
        game_over_choice: match state.game_over.phase {
            GameOverPhase::Choosing(GameOverChoice::ContinueWithWingmate)
            | GameOverPhase::Leaving {
                destination: GameOverDestination::StrategicMap,
                ..
            } => Sf2GameOverChoice::ContinueWithWingmate,
            GameOverPhase::AndrossTaunt
            | GameOverPhase::Choosing(GameOverChoice::EndCampaign)
            | GameOverPhase::Leaving {
                destination: GameOverDestination::Results,
                ..
            } => Sf2GameOverChoice::EndCampaign,
        },
        game_over_transition_retail_frames: match state.game_over.phase {
            GameOverPhase::Leaving {
                elapsed_retail_frames,
                ..
            } => elapsed_retail_frames,
            GameOverPhase::AndrossTaunt | GameOverPhase::Choosing(_) => 0,
        },
        results_phase: match state.results.phase {
            ResultsPhase::Revealing => Sf2ResultsPhase::Revealing,
            ResultsPhase::OpeningChoices { .. } => Sf2ResultsPhase::OpeningChoices,
            ResultsPhase::Choosing(_) => Sf2ResultsPhase::Choosing,
            ResultsPhase::Leaving { .. } => Sf2ResultsPhase::Leaving,
        },
        results_choice: match state.results.phase {
            ResultsPhase::Revealing
            | ResultsPhase::OpeningChoices { .. }
            | ResultsPhase::Choosing(ResultsChoice::Retry) => Sf2ResultsChoice::Retry,
            ResultsPhase::Choosing(ResultsChoice::Title) => Sf2ResultsChoice::Title,
            ResultsPhase::Leaving { choice, .. } => match choice {
                ResultsChoice::Retry => Sf2ResultsChoice::Retry,
                ResultsChoice::Title => Sf2ResultsChoice::Title,
            },
        },
        results_presentation_retail_frames: match state.results.phase {
            ResultsPhase::Revealing => state
                .mode_frame
                .saturating_mul(SF2_RETAIL_PRESENTATION_FRAMES_PER_TICK),
            ResultsPhase::OpeningChoices { .. } | ResultsPhase::Choosing(_) => {
                u32::from(state.results.choice_presentation_retail_frames)
            }
            ResultsPhase::Leaving {
                elapsed_retail_frames,
                ..
            } => u32::from(elapsed_retail_frames),
        },
        results_transition_retail_frames: match state.results.phase {
            ResultsPhase::Leaving {
                elapsed_retail_frames,
                ..
            } => elapsed_retail_frames,
            ResultsPhase::Revealing
            | ResultsPhase::OpeningChoices { .. }
            | ResultsPhase::Choosing(_) => 0,
        },
        ending_phase: match state.ending.phase {
            EndingPhase::StaffRoll => Sf2EndingPhase::StaffRoll,
            EndingPhase::EndScreen => Sf2EndingPhase::EndScreen,
            EndingPhase::Leaving { .. } => Sf2EndingPhase::Leaving,
        },
        ending_presentation_tick: state.ending.presentation_tick,
        ending_transition_retail_frames: match state.ending.phase {
            EndingPhase::Leaving {
                elapsed_retail_frames,
            } => elapsed_retail_frames,
            EndingPhase::StaffRoll | EndingPhase::EndScreen => 0,
        },
        primary_shield: state
            .mission
            .primary_player
            .and_then(|id| state.objects.get(id))
            .map(|object| object.base.hit_points)
            .unwrap_or_default(),
        wingmate_shield: state
            .mission
            .wingmate
            .and_then(|id| state.objects.get(id))
            .map(|object| object.base.hit_points)
            .unwrap_or_default(),
        item_count: state.mission.item_count,
        target_count,
        mission_elapsed_time_tenths: state.mission.elapsed_time_tenths,
        mission_message: to_sf2_mission_message(state.mission.message),
        radar_contacts,
        mode_frame: state.mode_frame,
        elapsed_campaign_frames: state.campaign.elapsed_frames,
        corneria_damage_percent: state.campaign.corneria_damage_percent,
        score: state.mission.score,
        campaign_sorties_completed: state.campaign.completed_campaign_visits(),
        strategic_opening_presentation_tick: state.strategic_map.opening.presentation_tick,
        strategic_phase: match state.strategic_map.phase {
            StrategicMapPhase::OpeningOverview => Sf2StrategicPhase::Overview,
            StrategicMapPhase::Tutorial(_) => Sf2StrategicPhase::Tutorial,
            StrategicMapPhase::Planning => Sf2StrategicPhase::Planning,
            StrategicMapPhase::Traveling => Sf2StrategicPhase::Traveling,
        },
        strategic_marker_phase: state.strategic_map.marker_phase,
        strategic_player: Sf2MapPoint {
            x: state.strategic_map.player_map_position.x,
            y: state.strategic_map.player_map_position.y,
        },
        strategic_destination: Sf2MapPoint {
            x: state.strategic_map.destination.x,
            y: state.strategic_map.destination.y,
        },
        strategic_actors: state
            .strategic_map
            .actors
            .map(|actor| actor.map(to_sf2_strategic_actor)),
    }
}

/// Shell state code -> sf_render GameState (same boot.h ordering).
fn to_render_state(code: u8) -> RenderState {
    match code {
        0 => RenderState::Boot,
        1 => RenderState::Title,
        2 => RenderState::Briefing,
        3 => RenderState::PlanetSelect,
        4 => RenderState::Playing,
        5 => RenderState::Continue,
        6 => RenderState::Ending,
        7 => RenderState::Tally,
        8 => RenderState::AttractIntro,
        _ => RenderState::Boot,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedGame {
    StarFox,
    StarFox2,
}

struct CliArgs {
    config_path: PathBuf,
    asset_root: Option<PathBuf>,
    shader_dir: Option<PathBuf>,
    game: SelectedGame,
    rom_path: Option<PathBuf>,
    save_path: PathBuf,
}

fn parse_args() -> CliArgs {
    let mut args = CliArgs {
        config_path: PathBuf::from("starfox.ini"),
        asset_root: None,
        shader_dir: None,
        game: SelectedGame::StarFox,
        rom_path: None,
        save_path: PathBuf::from(DEFAULT_STAR_FOX_2_SAVE_PATH),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => {
                if let Some(v) = it.next() {
                    args.config_path = PathBuf::from(v);
                }
            }
            "--asset-root" => args.asset_root = it.next().map(PathBuf::from),
            "--shader-dir" => args.shader_dir = it.next().map(PathBuf::from),
            "--game" => match it.next().as_deref() {
                Some("sf1" | "starfox" | "star-fox") => args.game = SelectedGame::StarFox,
                Some("sf2" | "starfox2" | "star-fox-2") => args.game = SelectedGame::StarFox2,
                Some(other) => {
                    eprintln!("Invalid --game value: {other} (expected sf1 or sf2)");
                    std::process::exit(2);
                }
                None => {
                    eprintln!("--game requires sf1 or sf2");
                    std::process::exit(2);
                }
            },
            "--sf2" => args.game = SelectedGame::StarFox2,
            "--rom" => args.rom_path = it.next().map(PathBuf::from),
            "--save" => {
                if let Some(value) = it.next() {
                    args.save_path = PathBuf::from(value);
                }
            }
            "--help" | "-h" => {
                println!(
                    "Star Fox HD\n\n  --game sf1|sf2      select game (default sf1)\n  --sf2               shorthand for --game sf2\n  --rom PATH           SF2 retail ROM path\n  --save PATH          SF2 campaign-progress file\n  --config PATH        configuration file\n  --asset-root PATH    renderer asset root\n  --shader-dir PATH    load shaders from disk"
                );
                std::process::exit(0);
            }
            other => eprintln!("Unknown argument: {other}"),
        }
    }
    args
}

fn write_ppm(path: &str, w: i32, h: i32, rgb: &[u8]) {
    let mut buf = format!("P6\n{w} {h}\n255\n").into_bytes();
    buf.extend_from_slice(rgb);
    if let Err(e) = std::fs::write(path, buf) {
        eprintln!("SF_DUMP_PPM: write {path} failed: {e}");
    } else {
        println!("SF_DUMP_PPM: wrote {path}");
    }
}

fn encode_sf2_progress(progress: sf2_game::CampaignProgress) -> [u8; STAR_FOX_2_SAVE_LENGTH] {
    let mut bytes = [0; STAR_FOX_2_SAVE_LENGTH];
    bytes[..STAR_FOX_2_SAVE_MAGIC.len()].copy_from_slice(STAR_FOX_2_SAVE_MAGIC);
    bytes[STAR_FOX_2_SAVE_MAGIC.len()] = STAR_FOX_2_SAVE_VERSION;
    if progress.expert_unlocked {
        bytes[STAR_FOX_2_SAVE_MAGIC.len() + 1] |= STAR_FOX_2_SAVE_EXPERT_UNLOCKED_FLAG;
    }
    bytes
}

fn decode_sf2_progress(bytes: &[u8]) -> Option<sf2_game::CampaignProgress> {
    if bytes.len() != STAR_FOX_2_SAVE_LENGTH
        || &bytes[..STAR_FOX_2_SAVE_MAGIC.len()] != STAR_FOX_2_SAVE_MAGIC
        || bytes[STAR_FOX_2_SAVE_MAGIC.len()] != STAR_FOX_2_SAVE_VERSION
    {
        return None;
    }
    let flags = bytes[STAR_FOX_2_SAVE_MAGIC.len() + 1];
    Some(sf2_game::CampaignProgress {
        expert_unlocked: flags & STAR_FOX_2_SAVE_EXPERT_UNLOCKED_FLAG != 0,
    })
}

fn load_sf2_progress(path: &Path) -> sf2_game::CampaignProgress {
    match std::fs::read(path) {
        Ok(bytes) => match decode_sf2_progress(&bytes) {
            Some(progress) => {
                println!("Star Fox 2 progress loaded from {}", path.display());
                progress
            }
            None => {
                eprintln!(
                    "Star Fox 2 progress {} has an unsupported format; starting with locked Expert difficulty",
                    path.display()
                );
                sf2_game::CampaignProgress::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            sf2_game::CampaignProgress::default()
        }
        Err(error) => {
            eprintln!(
                "Star Fox 2 progress {} could not be read: {error}; starting with locked Expert difficulty",
                path.display()
            );
            sf2_game::CampaignProgress::default()
        }
    }
}

fn save_sf2_progress(path: &Path, progress: sf2_game::CampaignProgress) -> bool {
    match std::fs::write(path, encode_sf2_progress(progress)) {
        Ok(()) => {
            println!("Star Fox 2 progress saved to {}", path.display());
            true
        }
        Err(error) => {
            eprintln!(
                "Star Fox 2 progress {} could not be written: {error}",
                path.display()
            );
            false
        }
    }
}

fn load_sf2(cli: &CliArgs) -> sf2_game::Game {
    let rom_path = cli
        .rom_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("Star Fox 2 (USA, Europe).sfc"));
    let rom = std::fs::read(&rom_path).unwrap_or_else(|error| {
        eprintln!(
            "Star Fox 2 ROM {} could not be read: {error}",
            rom_path.display()
        );
        std::process::exit(1);
    });
    if rom.len() != STAR_FOX_2_ROM_SIZE {
        eprintln!(
            "Star Fox 2 ROM {} has {} bytes; expected the 1,048,576-byte headerless retail ROM",
            rom_path.display(),
            rom.len()
        );
        std::process::exit(1);
    }
    let title_end = STAR_FOX_2_TITLE_OFFSET + STAR_FOX_2_TITLE.len();
    if rom.get(STAR_FOX_2_TITLE_OFFSET..title_end) != Some(STAR_FOX_2_TITLE) {
        eprintln!(
            "Star Fox 2 ROM {} does not contain the expected internal title",
            rom_path.display()
        );
        std::process::exit(1);
    }
    drop(rom);
    println!(
        "Star Fox 2 native runtime loaded from {}",
        rom_path.display()
    );
    sf2_game::Game::new_with_progress(load_sf2_progress(&cli.save_path))
}

fn main() {
    let cli = parse_args();
    let cfg = Config::load(&cli.config_path);
    let mut sf2 = (cli.game == SelectedGame::StarFox2).then(|| load_sf2(&cli));
    let mut saved_sf2_progress = sf2.as_ref().map(|game| game.state().progress);

    // --- SDL init (main.c Init) ---
    let sdl = match sdl3::init() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SDL_Init failed: {e}");
            std::process::exit(1);
        }
    };
    let video = sdl.video().expect("SDL video subsystem");

    let hidden = std::env::var("SF_HIDDEN")
        .map(|v| v == "1")
        .unwrap_or(false);
    let mut builder = video.window(
        if sf2.is_some() {
            "Star Fox 2 HD"
        } else {
            "Star Fox HD"
        },
        cfg.window_width as u32,
        cfg.window_height as u32,
    );
    builder.resizable().position_centered();
    if hidden {
        builder.hidden();
    }
    if cfg.fullscreen != 0 {
        // SDL3 collapsed exclusive/desktop fullscreen; both config values
        // map to fullscreen (main.c:43-44).
        builder.fullscreen();
    }
    let mut window = match builder.build() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("SDL_CreateWindow failed: {e}");
            std::process::exit(1);
        }
    };

    // wgpu renders into the SDL3 window's surface via raw-window-handle.
    // SAFETY: `window` outlives `gpu` (drop order is reverse of declaration),
    // so the 'static surface never dangles.
    // Frame dumping (SF_DUMP_PPM) needs read_pixels_rgb, which only works on a
    // headless Gpu — so route the whole app through the offscreen target when
    // dumping. The SDL window still exists for the event loop; it just isn't
    // presented to. Normal runs use the windowed surface.
    let offscreen = std::env::var_os("SF_DUMP_PPM").is_some();
    let gpu = if offscreen {
        match sf_render::gpu::Gpu::new_headless(cfg.window_width as u32, cfg.window_height as u32) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("wgpu headless init failed: {e}");
                std::process::exit(1);
            }
        }
    } else {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = match unsafe {
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(&window)
                    .expect("SDL3 window handle -> wgpu surface target"),
            )
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("wgpu create_surface failed: {e}");
                std::process::exit(1);
            }
        };
        match sf_render::gpu::Gpu::new_for_surface(
            instance,
            surface,
            cfg.window_width as u32,
            cfg.window_height as u32,
        ) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("wgpu init failed: {e}");
                std::process::exit(1);
            }
        }
    };

    // --- Renderer / audio / input / game shell ---
    let render_cfg = RendererConfig {
        shader_dir: cli.shader_dir.clone(),
        asset_root: cli.asset_root.clone().unwrap_or_else(|| cfg.asset_root()),
    };
    let mut renderer = match Renderer::new(gpu, cfg.window_width, cfg.window_height, &render_cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Renderer init failed: {e}");
            std::process::exit(1);
        }
    };

    let mut audio = if sf2.is_some() {
        match AudioSys::new_sf2(&sdl, cfg.audio_asset_dir()) {
            Ok(audio) => audio,
            Err(error) => {
                eprintln!("Star Fox 2 audio asset validation failed: {error}");
                std::process::exit(1);
            }
        }
    } else {
        match AudioSys::new(&sdl, cfg.audio_asset_dir()) {
            Ok(audio) => audio,
            Err(error) => {
                eprintln!("Audio asset validation failed: {error}");
                std::process::exit(1);
            }
        }
    };

    let gamepad_subsystem = sdl.gamepad().ok();
    let mut input = Input::new(sf2.is_some());
    if let Some(gp) = &gamepad_subsystem {
        input.open_all_gamepads(gp);
    }

    let mut shell = Shell::new();
    // C boot.c: Strat_RegisterAll() after World_Init — populate g_istrats[]
    // and the strategy address map so map objects get their per-frame AI and
    // the player alien exists.
    // Install the strategy registration hook: runs now and re-runs after
    // every World::init() reset in the shell (C re-runs Strat_RegisterAll on
    // each level load; a startup-only call gets wiped by the first reset).
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    // The source creates the base ship before its transfer-bound level setup,
    // then installs the map-specific opening strategy at the handoff boundary.
    shell.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    shell.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    shell.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
    shell.set_ending_score_part(Box::new(sf_strat::endscore::spawn_final_score_part));
    shell.set_ending_boss_replay(Box::new(sf_strat::endseq::spawn_replay_boss));
    // Wire real per-shape collision half-extents (C load_collision_extents)
    // from the renderer's shape meshes into the collision system. The shape
    // store is fully populated by Renderer::new (register_builtins).
    shell.set_shape_extents(renderer.shapes.all_shape_half_extents());

    let mut dump = StateDump::from_env();
    let max_ticks: Option<u64> = std::env::var("SF_MAX_TICKS")
        .ok()
        .and_then(|v| v.parse().ok());
    let ppm_path = std::env::var("SF_DUMP_PPM").ok().filter(|p| !p.is_empty());
    let ppm_tick: u64 = std::env::var("SF_DUMP_PPM_TICK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(220);
    let mut ppm_pending = ppm_path.is_some();
    // SF_NOINTERP=1: render the tick-exact game state (no render-frame
    // interpolation). Every frame then shows a true 20 Hz game state that
    // matches the ROM's per-tick output — authentic (choppier) and the
    // correct basis for parity checking, since interpolation invents
    // between-tick states the ROM never computes.
    let no_interp = std::env::var("SF_NOINTERP")
        .map(|v| v == "1")
        .unwrap_or(false);
    let fast_forward = std::env::var("SF_FAST_FORWARD")
        .map(|value| value == "1")
        .unwrap_or(false);

    // --- Fixed timestep game loop with interpolation (main.c:153-201) ---
    let tick_duration = if sf2.is_some() {
        1.0 / STAR_FOX_2_TICKS_PER_SECOND
    } else {
        GAME_TICK_MS as f64 / 1_000.0
    };
    let mut prev_time = Instant::now();
    let mut accumulator = 0.0f64;

    let mut prev_list: Vec<RenderEntry> = Vec::with_capacity(MAX_DRAW_LIST);
    let mut curr_list: Vec<RenderEntry> = Vec::with_capacity(MAX_DRAW_LIST);
    let mut total_ticks: u64 = 0;
    let mut running = true;
    let (mut fb_w, mut fb_h) = (cfg.window_width, cfg.window_height);

    let mut event_pump = sdl.event_pump().expect("event pump");

    while running {
        let now = Instant::now();
        let mut dt = if fast_forward {
            tick_duration
        } else {
            now.duration_since(prev_time).as_secs_f64()
        };
        prev_time = now;
        // Cap dt to prevent spiral of death (main.c:171).
        if dt > 0.25 {
            dt = 0.25;
        }
        accumulator += dt;

        // --- HandleEvents (main.c:86-111) ---
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    eprintln!("[exit] SDL Event::Quit received -> quitting");
                    running = false;
                }
                Event::Window {
                    win_event: WindowEvent::Resized(w, h),
                    ..
                }
                | Event::Window {
                    win_event: WindowEvent::PixelSizeChanged(w, h),
                    ..
                } => {
                    renderer.resize(w, h);
                    fb_w = w;
                    fb_h = h;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    eprintln!("[exit] Escape key -> quitting");
                    running = false;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F11),
                    ..
                } => {
                    let fs = window.fullscreen_state() != FullscreenType::Off;
                    let _ = window.set_fullscreen(!fs);
                }
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Some(gp) = &gamepad_subsystem {
                        input.add_gamepad(gp, sdl3::sys::joystick::SDL_JoystickID(which));
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    eprintln!("[input] controller {which} removed");
                    input.remove_gamepad(sdl3::sys::joystick::SDL_JoystickID(which));
                }
                _ => {}
            }
        }

        // --- Fixed timestep game ticks (main.c:177-189) ---
        let mut frame = shell.frame();
        while accumulator >= tick_duration {
            prev_list.clear();
            prev_list.extend_from_slice(&curr_list);

            // SfRtl_BeginFrame -> selected game tick -> draw-list bridge.
            input.begin_frame(&event_pump.keyboard_state(), sf2.as_ref());
            curr_list.clear();
            if let Some(game) = sf2.as_mut() {
                let previous_mode = game.mode();
                if let Err(error) = game.tick(input.pad1) {
                    eprintln!(
                        "Star Fox 2 runtime failed on frame {} in {:?}: {error:?}",
                        game.frame(),
                        game.mode(),
                    );
                    running = false;
                    accumulator = 0.0;
                    break;
                }
                if game.mode() != previous_mode {
                    println!(
                        "[sf2-state] {previous_mode:?} -> {:?} (frame {})",
                        game.mode(),
                        game.frame()
                    );
                }
                audio.tick_sf2(game);
                let progress = game.state().progress;
                if saved_sf2_progress != Some(progress)
                    && save_sf2_progress(&cli.save_path, progress)
                {
                    saved_sf2_progress = Some(progress);
                }
                curr_list.extend(
                    game.render_objects()
                        .iter()
                        .take(MAX_DRAW_LIST)
                        .map(to_sf2_render_entry),
                );

                if let Some(d) = dump.as_mut() {
                    d.tick_render(input.pad1, Sf2DumpMode::from(game.mode()) as u8, &curr_list);
                }

                // SF2 object/camera coordinates are signed world units with
                // forward toward decreasing depth. The shared renderer uses
                // the opposite world-depth basis before its view transform.
                let cam = game.camera();
                renderer.transform.set_camera(
                    sf2_world_to_render(cam.position.x),
                    sf2_world_to_render(cam.position.y),
                    sf2_depth_to_render(cam.position.z),
                    i16::from(cam.rotation.pitch.units()),
                    i16::from(cam.rotation.yaw.units()),
                    i16::from(cam.rotation.roll.units()),
                );
            } else {
                shell.tick(input.pad1);
                curr_list.extend(
                    shell
                        .draw_list()
                        .iter()
                        .take(MAX_DRAW_LIST)
                        .map(to_render_entry),
                );

                frame = shell.frame();
                audio.tick(&mut shell, &frame);

                if let Some(d) = dump.as_mut() {
                    let list = shell.draw_list();
                    d.tick(
                        input.pad1,
                        frame.game_state_code,
                        &list[..list.len().min(MAX_DRAW_LIST)],
                    );
                }

                // Camera for the render pass (nmi.c GameCamera_Update ->
                // Transform_SetCamera / Transform_SnapCamera).
                let cam = frame.camera;
                renderer
                    .transform
                    .set_camera_fine(cam.x, cam.y, cam.z, cam.rotation);
                if cam.snap {
                    renderer.transform.snap_camera();
                }
            }

            accumulator -= tick_duration;
            total_ticks += 1;
            if let Some(max) = max_ticks {
                if total_ticks >= max {
                    running = false;
                    accumulator = 0.0;
                    break;
                }
            }
        }

        // Interpolation alpha for smooth rendering (main.c:192). alpha=1
        // shows the latest tick verbatim (tick-exact / ROM-accurate).
        let alpha = if no_interp {
            1.0
        } else {
            (accumulator / tick_duration) as f32
        };

        // Assemble the shared render inputs from the selected game's native
        // state.
        let inputs = if let Some(game) = sf2.as_ref() {
            let mut inputs = FrameInputs::default();
            inputs.sf2 = Some(to_sf2_frame_inputs(game));
            inputs.game_state = RenderState::Boot;
            inputs.gameframe = game.frame() as u16;
            inputs.stage = u16::from(game.mission().is_some());
            inputs
        } else {
            let mut windows = [WindowState::default(); WINDOWARRAY_SIZE];
            for (dst, src) in windows.iter_mut().zip(frame.windows.iter()) {
                *dst = WindowState {
                    mode: src.mode,
                    wm_val: src.wm_val,
                    stayblack: src.stayblack,
                };
            }
            FrameInputs {
                sf2: None,
                game_state: to_render_state(frame.game_state_code),
                currentbg: frame.currentbg,
                newmap: frame.newmap,
                bgflags: frame.bgflags,
                bg2_xscroll: frame.bg2_xscroll,
                nomax_bg2_yscroll: frame.nomax_bg2_yscroll,
                scene_style: frame.scene_style,
                pal_target: frame.pal_target,
                palfade_num: frame.palfade_num,
                windowmode: frame.windowmode,
                windows,
                screen_wipe: frame.screen_wipe,
                screen_fill_circle: frame.screen_fill_circle,
                meters: frame.meters,
                stayblack: frame.stayblack,
                gameflags: frame.gameflags,
                gameframe: frame.gameframe,
                briefing_phase: frame.briefing_phase,
                briefing_choice: frame.briefing_choice,
                control_type: frame.control_type,
                boostcnt: frame.boostcnt,
                arrows: frame.arrows,
                player_view_mode: frame.player_view_mode,
                stage: frame.stage,
                shield_cur: frame.shield_cur,
                shield_max: frame.shield_max,
                boss_hp_cur: frame.boss_hp_cur,
                boss_hp_max: frame.boss_hp_max,
                lives: frame.lives,
                bombs: frame.bombs,
                specflash: frame.specflash,
                shieldup: frame.shieldup,
                msg_count1: frame.msg_count1,
                msg_count2: frame.msg_count2,
                whichfriend: frame.whichfriend,
                friends_meter: frame.friends_meter,
                message_text: frame.message_text.as_deref(),
                whichroute: frame.whichroute,
                currentplanet: frame.currentplanet,
                nebula_on: frame.nebula_on,
                route_path_ids: &frame.route_path_ids,
                planet_presentation: frame.planet_presentation,
                score: frame.score_total,
                credits: frame.credits,
                tally_active: frame.tally_active,
                tally_stage_perc: frame.tally_stage_perc,
                tally_current_perc: frame.tally_current_perc,
                tally_teammate_shields: frame.tally_teammate_shields,
                tally_bonus_visible: frame.tally_bonus_visible,
                ending_replay: frame.ending_replay.map(|replay| EndingReplayInputs {
                    backdrop: match replay.backdrop {
                        sf_game::shell::EndingReplayBackdrop::RisingGradient => {
                            RenderEndingReplayBackdrop::RisingGradient
                        }
                        sf_game::shell::EndingReplayBackdrop::SplitGradient => {
                            RenderEndingReplayBackdrop::SplitGradient
                        }
                    },
                    title: replay.text.title,
                    subtitle: replay.text.subtitle,
                    location: replay.text.location,
                    location_second_line: replay.text.location_second_line,
                    details: replay.text.details,
                    detail_characters_visible: replay.detail_characters_visible,
                }),
            }
        };

        // Render interpolated frame (main.c:195-200).
        renderer.begin_frame();
        renderer.submit(&prev_list, &curr_list, alpha, &inputs);
        // HUD do_arrows wrap queues SE $8A into Hud::pending_sounds — drain
        // into the audio ring (SPRITES.ASM:872 trigse $8a).
        for id in renderer.take_pending_hud_sounds() {
            if sf2.is_none() {
                audio.play_hud_se(&shell, id);
            }
        }
        renderer.end_frame(); // uploads geometry + presents the wgpu surface

        if ppm_pending && total_ticks >= ppm_tick {
            if let Some(path) = &ppm_path {
                let rgb = renderer.read_pixels_rgb();
                write_ppm(path, fb_w, fb_h, &rgb);
            }
            ppm_pending = false;
        }
    }

    // Final PPM if the run ended before the requested tick.
    if ppm_pending {
        if let Some(path) = &ppm_path {
            let rgb = renderer.read_pixels_rgb();
            write_ppm(path, fb_w, fb_h, &rgb);
        }
    }

    if let Some(d) = dump.as_mut() {
        d.flush();
    }
    audio.shutdown();
    renderer.shutdown();
    println!("Shutdown after {total_ticks} ticks");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_progress_round_trips_typed_expert_unlock() {
        let locked = sf2_game::CampaignProgress::default();
        assert_eq!(
            decode_sf2_progress(&encode_sf2_progress(locked)),
            Some(locked)
        );

        let unlocked = sf2_game::CampaignProgress {
            expert_unlocked: true,
        };
        let encoded = encode_sf2_progress(unlocked);
        assert_eq!(
            encoded[STAR_FOX_2_SAVE_MAGIC.len() + 1],
            STAR_FOX_2_SAVE_EXPERT_UNLOCKED_FLAG
        );
        assert_eq!(decode_sf2_progress(&encoded), Some(unlocked));
        assert_eq!(decode_sf2_progress(b"unsupported"), None);
    }

    #[test]
    fn sf1_tally_has_a_dedicated_render_state() {
        assert_eq!(to_render_state(7), RenderState::Tally);
    }

    #[test]
    fn sf2_game_over_has_a_stable_diagnostic_mode() {
        assert_eq!(
            Sf2DumpMode::from(sf2_game::GameMode::GameOver),
            Sf2DumpMode::GameOver
        );
        assert_eq!(Sf2DumpMode::GameOver as u8, 10);
    }

    #[test]
    fn sf2_bridge_converts_decreasing_world_depth_to_renderer_forward() {
        const WORLD_DEPTH: i16 = -2_435;
        assert_eq!(
            sf2_depth_to_render(WORLD_DEPTH),
            2_435i32 << WORLD_TO_RENDER_FRACTIONAL_BITS
        );
    }

    #[test]
    fn sf2_bridge_selects_polygon_palettes_from_typed_mission_phases() {
        use sf2_game::{AstropolisPhase, EladardPhase, MissionState, MissionVisit};

        let mut mission = MissionState {
            visit: MissionVisit::EladardBase,
            ..MissionState::default()
        };
        for phase in [
            EladardPhase::SurfaceApproach,
            EladardPhase::SurfaceBarriers,
            EladardPhase::BaseEntrance,
        ] {
            mission.eladard.phase = phase;
            assert_eq!(
                to_sf2_polygon_palette(&mission),
                Sf2PolygonPalette::EladardSurface
            );
        }
        for phase in [
            EladardPhase::InteriorPassage,
            EladardPhase::GeneratorRoom,
            EladardPhase::BaseDestruction,
            EladardPhase::ReturnFlight,
        ] {
            mission.eladard.phase = phase;
            assert_eq!(
                to_sf2_polygon_palette(&mission),
                Sf2PolygonPalette::Standard
            );
        }

        mission.visit = MissionVisit::AstropolisAssault;
        for phase in [
            AstropolisPhase::ExteriorApproach,
            AstropolisPhase::BaseEntry,
        ] {
            mission.astropolis.phase = phase;
            assert_eq!(
                to_sf2_polygon_palette(&mission),
                Sf2PolygonPalette::AstropolisExterior
            );
        }
        for phase in [
            AstropolisPhase::InteriorCorridor,
            AstropolisPhase::SecurityTurret,
            AstropolisPhase::BranchCorridor,
            AstropolisPhase::CoreSpikes,
            AstropolisPhase::ExposedCube,
            AstropolisPhase::AndrossMask,
            AstropolisPhase::FinalCore,
            AstropolisPhase::MaskReforming,
            AstropolisPhase::CoreDestruction,
            AstropolisPhase::Escape,
        ] {
            mission.astropolis.phase = phase;
            assert_eq!(
                to_sf2_polygon_palette(&mission),
                Sf2PolygonPalette::Standard
            );
        }

        mission.visit = MissionVisit::TitaniaBase;
        assert_eq!(
            to_sf2_polygon_palette(&mission),
            Sf2PolygonPalette::Standard
        );
        mission.visit = MissionVisit::MacbethBase;
        assert_eq!(
            to_sf2_polygon_palette(&mission),
            Sf2PolygonPalette::Standard
        );
        mission.visit = MissionVisit::SecondBattleCarrier;
        assert_eq!(
            to_sf2_polygon_palette(&mission),
            Sf2PolygonPalette::Standard
        );
    }

    #[test]
    fn sf2_bridge_selects_oracle_backdrops_from_typed_mission_phases() {
        use sf2_game::{
            AstropolisPhase, CarrierAssaultPhase, EladardPhase, MissionState, MissionVisit,
            TitaniaPhase,
        };

        let mut mission = MissionState {
            visit: MissionVisit::EladardBase,
            ..MissionState::default()
        };
        for phase in [
            EladardPhase::SurfaceApproach,
            EladardPhase::SurfaceBarriers,
            EladardPhase::BaseEntrance,
            EladardPhase::ReturnFlight,
        ] {
            mission.eladard.phase = phase;
            assert_eq!(
                to_sf2_mission_backdrop(&mission),
                Sf2MissionBackdrop::EladardSurface
            );
        }
        for phase in [
            EladardPhase::InteriorPassage,
            EladardPhase::GeneratorRoom,
            EladardPhase::BaseDestruction,
        ] {
            mission.eladard.phase = phase;
            assert_eq!(
                to_sf2_mission_backdrop(&mission),
                Sf2MissionBackdrop::EladardInterior
            );
        }

        mission.visit = MissionVisit::TitaniaBase;
        for phase in [
            TitaniaPhase::SurfaceApproach,
            TitaniaPhase::FirstSwitch,
            TitaniaPhase::SurfaceTransit,
            TitaniaPhase::SecondSwitch,
            TitaniaPhase::BaseOpening,
            TitaniaPhase::BaseEntry,
            TitaniaPhase::Interior,
            TitaniaPhase::FinalSwitch,
            TitaniaPhase::BaseEscape,
            TitaniaPhase::ReturnFlight,
        ] {
            mission.titania.phase = phase;
            assert_eq!(
                to_sf2_mission_backdrop(&mission),
                Sf2MissionBackdrop::TitaniaBase
            );
        }

        mission.visit = MissionVisit::MacbethBase;
        assert_eq!(
            to_sf2_mission_backdrop(&mission),
            Sf2MissionBackdrop::MacbethSurface
        );

        for visit in [
            MissionVisit::FirstBattleCarrier,
            MissionVisit::SecondBattleCarrier,
        ] {
            mission.visit = visit;
            for phase in [
                CarrierAssaultPhase::ExteriorApproach,
                CarrierAssaultPhase::ReturnFlight,
            ] {
                mission.carrier_assault.phase = phase;
                assert_eq!(
                    to_sf2_mission_backdrop(&mission),
                    Sf2MissionBackdrop::DeepSpace
                );
            }
            for phase in [
                CarrierAssaultPhase::InteriorCorridor,
                CarrierAssaultPhase::ReactorApproach,
                CarrierAssaultPhase::ReactorCombat,
                CarrierAssaultPhase::CoreDestruction,
            ] {
                mission.carrier_assault.phase = phase;
                assert_eq!(
                    to_sf2_mission_backdrop(&mission),
                    Sf2MissionBackdrop::CarrierInterior
                );
            }
        }

        mission.visit = MissionVisit::AstropolisAssault;
        for phase in [
            AstropolisPhase::ExteriorApproach,
            AstropolisPhase::BaseEntry,
            AstropolisPhase::InteriorCorridor,
            AstropolisPhase::SecurityTurret,
            AstropolisPhase::BranchCorridor,
            AstropolisPhase::CoreSpikes,
            AstropolisPhase::ExposedCube,
            AstropolisPhase::AndrossMask,
            AstropolisPhase::FinalCore,
            AstropolisPhase::MaskReforming,
            AstropolisPhase::CoreDestruction,
            AstropolisPhase::Escape,
        ] {
            mission.astropolis.phase = phase;
            assert_eq!(
                to_sf2_mission_backdrop(&mission),
                Sf2MissionBackdrop::AstropolisVoid
            );
        }
    }
}
