//! Offscreen wgpu runtime tests: a headless wgpu device rendering into a
//! texture (no window/display needed), full Renderer pass pipeline.
//!
//! One #[test] runs all checks sequentially against a single headless
//! renderer.
//!
//! Checks:
//!  (a) Arwing (SHAPE_MYSHIP_4) rendered with a known camera: readback
//!      contains the canopy blue family (NIGHT.COL CA_2 cycle -> palette 8
//!      at col_frame 0) and a COLLITE hull grey shade.
//!  (b) bg_1_1c playing-state frame: screen-top row is the composed sky
//!      blue RGB(49, 90, 148) after the calcbgscroll_l coupling at rx=0.
//!  (c) Title frame vs the exact source-asset CPU composition (8x8 region
//!      averages; GPU scaling may shift each average by one channel value).
//!  (d) An SF2 campaign missile samples the retail SF2 descriptor table and
//!      packed-nibble texture bank without falling back to debug magenta.
//!  (e) The SF1 end-level tally uses its native graph, teammate portraits,
//!      and shield bars instead of falling through to the planet map.

use std::path::PathBuf;
use std::sync::Mutex;

use sf_render::draw_list::{DrawListEntry, DL_FLAG_VISIBLE};
use sf_render::gpu::{Gpu, Vertex3};
use sf_render::renderer::{
    config_from_repo_root, EndingReplayBackdrop, EndingReplayInputs, FrameInputs, GameState,
    Renderer, RendererConfig, Sf2AudioOutput, Sf2Difficulty, Sf2EndingPhase, Sf2FlightControlStyle,
    Sf2FrameInputs, Sf2GameOverChoice, Sf2GameOverPhase, Sf2MissionBackdrop, Sf2MissionMessage,
    Sf2MissionMessageInputs, Sf2MissionMessagePhase, Sf2Mode, Sf2Pilot, Sf2PilotSelectionCursor,
    Sf2PilotSelectionPhase, Sf2StrategicActor, Sf2StrategicActorAppearance, Sf2StrategicActorKind,
    Sf2TitleMenuItem, Sf2TitlePage, SF2_RADAR_CONTACT_CAPACITY,
};
use sf_render::shape_data::SHAPE_EXT_ASTEROID1;
use sf_render::shapes::{self, SHAPE_ELASER2, SHAPE_MYSHIP_4};

mod common;
use common::{grid_8x8, SOURCE_TITLE_COMPOSITE_GRID};

const W: u32 = 1280;
const H: u32 = 720;
const SF1_CLEAR_RGB: [u8; 3] = [0, 0, 0];
const CORNERIA_SKY_RGB: [u8; 3] = [49, 90, 148];
static GPU_TEST_LOCK: Mutex<()> = Mutex::new(());
const SF2_TEST_MISSION_TIME_TENTHS: u16 = 11;
const SF2_MAP_WIDTH: i32 = 256;
const SF2_MAP_HEIGHT: i32 = 224;
const SF2_TITLE_MISSION_FNV1A: u32 = 0xA8CAEC13;
const SF2_INTRO_TITLE_SPLASH_FNV1A: u32 = 0xE00700F2;
const SF2_INTRO_TITLE_RESPONSE_FNV1A: u32 = 0x7B2084FE;
const SF2_END_SCREEN_FNV1A: u32 = 0x17E2AE06;
const SF2_END_START_RESPONSE_FNV1A: u32 = 0x6D7C2E93;
const SF2_BRIEFING_MESSAGE_FNV1A: u32 = 0x86663F26;
const SF2_OPENING_REPORT_FNV1A: u32 = 0x8D1501C7;
const SF2_PILOT_SELECTION_FOX_FNV1A: u32 = 0xC2AC3D23;
const DITHER_TEST_WIDTH: u32 = 512;
const DITHER_TEST_HEIGHT: u32 = 448;
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
const FNV_PRIME: u32 = 0x01000193;
const FIRST_RETURN_FNV1A: u32 = 0x92CFDF43;
const SECOND_RETURN_FNV1A: u32 = 0xC250FC31;
const POST_INTERCEPTION_RETURN_FNV1A: u32 = 0x21EA4E82;
const POST_FIGHTER_INTERCEPT_RETURN_FNV1A: u32 = 0x55FCE0B4;
const POST_PIGMA_RETURN_FNV1A: u32 = 0x82C530D7;
const POST_ELADARD_RETURN_FNV1A: u32 = 0xB49B8694;
const POST_CARRIER_RETURN_FNV1A: u32 = 0x7FC7CEFE;
const POST_LEON_RETURN_FNV1A: u32 = 0xB902871D;
const POST_MIRAGE_RETURN_FNV1A: u32 = 0x98A81655;

#[derive(Clone, Copy)]
enum StrategicReturn {
    First,
    Second,
    PostInterception,
    PostFighterIntercept,
    PostPigma,
    PostEladard,
    PostCarrier,
    PostLeon,
    PostMirage,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn color_near(px: &[u8], want: [u8; 3], tol: i32) -> bool {
    (0..3).all(|c| (px[c] as i32 - want[c] as i32).abs() <= tol)
}

fn expected_rgb8(color: [f32; 4]) -> [u8; 3] {
    [
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
    ]
}

#[test]
fn gl_runtime_suite() {
    let _gpu_test_guard = GPU_TEST_LOCK.lock().expect("GPU test lock poisoned");
    check_palette_pair_dither();

    let config = config_from_repo_root(&repo_root());
    let mut renderer = match Renderer::new_headless(W as i32, H as i32, &config) {
        Ok(r) => r,
        // No usable GPU adapter in this environment (e.g. CI without a
        // software rasterizer) — skip rather than fail.
        Err(e) => {
            eprintln!("skipping gl_runtime_suite: no wgpu adapter ({e})");
            return;
        }
    };

    check_title_golden(&mut renderer);
    check_bg_1_1c_sky(&mut renderer);
    check_sf1_tally(&mut renderer);
    check_arwing(&mut renderer);
    check_sf2_shape(&mut renderer);
    check_sf2_texture_face(&mut renderer);
    check_sf2_native_frontend(&mut renderer);
    check_sf2_mission_backdrops(&mut renderer);
    check_sf2_mission_message(&mut renderer);
    check_superfx_texture_face(&mut renderer);
    check_player_laser(&mut renderer);

    renderer.shutdown();
    check_sf2_intro_exact(&config);
    check_sf2_ending_exact(&config);
    check_sf1_ending_recap(&config);
    check_sf2_title_exact(&config);
    check_sf2_briefing_exact(&config);
    check_sf2_opening_overview_exact(&config);
    check_sf2_pilot_selection_exact(&config);
    check_sf2_strategic_returns_exact(&config);
}

fn check_sf2_ending_exact(config: &RendererConfig) {
    const END_SCREEN_PRESENTATION_TICK: u32 = 2_000;

    let mut renderer = match Renderer::new_headless(SF2_MAP_WIDTH, SF2_MAP_HEIGHT, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping exact SF2 ending check: no wgpu adapter ({error})");
            return;
        }
    };
    let mut inputs = sf2_inputs(Sf2Mode::Ending);
    let sf2 = inputs.sf2.as_mut().expect("SF2 ending inputs");
    sf2.ending_phase = Sf2EndingPhase::EndScreen;
    sf2.ending_presentation_tick = END_SCREEN_PRESENTATION_TICK;
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_END_SCREEN_FNV1A,
        "SF2 end screen drifted from the retail capture"
    );

    let sf2 = inputs.sf2.as_mut().expect("SF2 ending inputs");
    sf2.ending_phase = Sf2EndingPhase::Leaving;
    sf2.ending_transition_retail_frames = 0;
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_END_START_RESPONSE_FNV1A,
        "SF2 accepted-Start ending response drifted from the retail capture"
    );
    renderer.shutdown();
}

fn check_sf2_intro_exact(config: &RendererConfig) {
    const TITLE_SPLASH_PRESENTATION_TICK: u16 = 779;
    const TITLE_RESPONSE_COUNTDOWN: u8 = 5;

    let mut renderer = match Renderer::new_headless(SF2_MAP_WIDTH, SF2_MAP_HEIGHT, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping exact SF2 intro check: no wgpu adapter ({error})");
            return;
        }
    };
    let mut inputs = sf2_inputs(Sf2Mode::Intro);
    let sf2 = inputs.sf2.as_mut().expect("SF2 intro inputs");
    sf2.intro_presentation_tick = TITLE_SPLASH_PRESENTATION_TICK;
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_INTRO_TITLE_SPLASH_FNV1A,
        "SF2 intro title-splash frame drifted from the retail capture"
    );

    inputs
        .sf2
        .as_mut()
        .expect("SF2 intro inputs")
        .intro_title_menu_countdown = Some(TITLE_RESPONSE_COUNTDOWN);
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_INTRO_TITLE_RESPONSE_FNV1A,
        "SF2 accepted-Start response drifted from the retail capture"
    );
    renderer.shutdown();
}

fn check_sf2_opening_overview_exact(config: &RendererConfig) {
    const FIRST_REPORT_PRESENTATION_TICK: u16 = 131;

    let mut renderer = match Renderer::new_headless(SF2_MAP_WIDTH, SF2_MAP_HEIGHT, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping exact SF2 opening-overview check: no wgpu adapter ({error})");
            return;
        }
    };
    let mut inputs = sf2_inputs(Sf2Mode::StrategicMap);
    let sf2 = inputs.sf2.as_mut().expect("SF2 strategic-map inputs");
    sf2.strategic_phase = sf_render::renderer::Sf2StrategicPhase::Overview;
    sf2.strategic_opening_presentation_tick = FIRST_REPORT_PRESENTATION_TICK;
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_OPENING_REPORT_FNV1A,
        "SF2 opening-overview frame drifted from the retail capture"
    );
    renderer.shutdown();
}

fn check_sf2_briefing_exact(config: &RendererConfig) {
    const RETAIL_MESSAGE_FRAME: u32 = 42;

    let mut renderer = match Renderer::new_headless(SF2_MAP_WIDTH, SF2_MAP_HEIGHT, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping exact SF2 briefing check: no wgpu adapter ({error})");
            return;
        }
    };
    let mut inputs = sf2_inputs(Sf2Mode::Briefing);
    inputs.sf2.as_mut().expect("SF2 briefing inputs").mode_frame = RETAIL_MESSAGE_FRAME;
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_BRIEFING_MESSAGE_FNV1A,
        "SF2 briefing frame drifted from the retail capture"
    );
    renderer.shutdown();
}

fn check_sf2_pilot_selection_exact(config: &RendererConfig) {
    let mut renderer = match Renderer::new_headless(SF2_MAP_WIDTH, SF2_MAP_HEIGHT, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping exact SF2 pilot-selection check: no wgpu adapter ({error})");
            return;
        }
    };
    let inputs = sf2_inputs(Sf2Mode::PilotSelection);
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_PILOT_SELECTION_FOX_FNV1A,
        "SF2 pilot-selection frame drifted from the retail capture"
    );
    renderer.shutdown();
}

fn check_sf2_title_exact(config: &RendererConfig) {
    let mut renderer = match Renderer::new_headless(SF2_MAP_WIDTH, SF2_MAP_HEIGHT, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping exact SF2 title check: no wgpu adapter ({error})");
            return;
        }
    };
    let inputs = sf2_inputs(Sf2Mode::Title);
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_TITLE_MISSION_FNV1A,
        "SF2 title frame drifted from the retail capture"
    );
    renderer.shutdown();
}

fn check_sf1_ending_recap(config: &RendererConfig) {
    const WIDTH: usize = 256;
    const HEIGHT: usize = 224;
    const PANEL_LEFT: usize = 112;
    const PANEL_TOP: usize = 16;
    const PANEL_WIDTH: usize = 128;
    const PANEL_HEIGHT: usize = 136;
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;
    const EXPECTED: [(EndingReplayBackdrop, u64); 2] = [
        (EndingReplayBackdrop::RisingGradient, 0x764b0b891087caa5),
        (EndingReplayBackdrop::SplitGradient, 0xc85ebbfcf15eca25),
    ];

    let mut renderer = match Renderer::new_headless(WIDTH as i32, HEIGHT as i32, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping ending recap check: no wgpu adapter ({error})");
            return;
        }
    };
    for (backdrop, expected_panel_hash) in EXPECTED {
        let inputs = FrameInputs {
            game_state: GameState::Ending,
            ending_replay: Some(EndingReplayInputs {
                backdrop,
                title: "LEVEL 1",
                subtitle: None,
                location: Some("CORNERIA"),
                location_second_line: None,
                details: [
                    "NAME   - ATTACK CARRIER",
                    "WEAPON - MISSILE BLASTER",
                    "SIZE   - H70*W100*D150",
                ],
                detail_characters_visible: 1,
            }),
            ..Default::default()
        };
        renderer.begin_frame();
        renderer.submit(&[], &[], 1.0, &inputs);
        renderer.end_frame();
        let pixels = renderer.read_pixels_rgb();
        let mut panel_hash = FNV_OFFSET;
        for row in PANEL_TOP..PANEL_TOP + PANEL_HEIGHT {
            let start = (row * WIDTH + PANEL_LEFT) * 3;
            let end = start + PANEL_WIDTH * 3;
            for &byte in &pixels[start..end] {
                panel_hash = (panel_hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
            }
        }
        assert_eq!(panel_hash, expected_panel_hash, "{backdrop:?} panel");

        let header = &pixels[(24 * WIDTH + 16) * 3..(48 * WIDTH) * 3];
        assert!(
            header.chunks_exact(3).any(|pixel| pixel == [74, 239, 255]),
            "retail cyan recap glyph layer missing"
        );
        let first_detail = &pixels[(168 * WIDTH + 16) * 3..(176 * WIDTH + 24) * 3];
        assert!(
            first_detail
                .chunks_exact(3)
                .any(|pixel| pixel == [255, 255, 255]),
            "first gradually revealed detail glyph missing"
        );
    }
    renderer.shutdown();
}

fn check_sf1_tally(renderer: &mut Renderer) {
    let inputs = FrameInputs {
        game_state: GameState::Tally,
        newmap: 1,
        meters: 1,
        score: 0,
        tally_active: true,
        tally_stage_perc: 65,
        tally_current_perc: 30,
        tally_teammate_shields: [40, 20, 0],
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();

    let pixels = renderer.read_pixels_rgb();
    if let Some(path) = std::env::var_os("SF1_TALLY_DUMP_PPM") {
        let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
        ppm.extend_from_slice(&pixels);
        std::fs::write(path, ppm).expect("write requested SF1 tally dump");
    }

    let cyan = pixels
        .chunks_exact(3)
        .filter(|pixel| color_near(pixel, [104, 216, 248], 3))
        .count();
    let pink = pixels
        .chunks_exact(3)
        .filter(|pixel| color_near(pixel, [240, 88, 104], 3))
        .count();
    let distinct = pixels
        .chunks_exact(3)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        cyan > 500,
        "tally percentage graph is absent ({cyan} pixels)"
    );
    assert!(
        pink > 1_000,
        "tally teammate bars are absent ({pink} pixels)"
    );
    assert!(
        distinct.len() > 30,
        "tally portraits collapsed to {} framebuffer colors",
        distinct.len()
    );
}

fn check_palette_pair_dither() {
    let mut gpu = match Gpu::new_headless(DITHER_TEST_WIDTH, DITHER_TEST_HEIGHT) {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("skipping palette-pair dither check: no wgpu adapter ({e})");
            return;
        }
    };
    let identity = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let quad = [
        Vertex3 {
            pos: [-1.0, -1.0, 0.0],
        },
        Vertex3 {
            pos: [1.0, -1.0, 0.0],
        },
        Vertex3 {
            pos: [1.0, 1.0, 0.0],
        },
        Vertex3 {
            pos: [-1.0, -1.0, 0.0],
        },
        Vertex3 {
            pos: [1.0, 1.0, 0.0],
        },
        Vertex3 {
            pos: [-1.0, 1.0, 0.0],
        },
    ];
    let mut palette = [[0.0; 4]; 16];
    palette[1] = [1.0, 0.0, 0.0, 1.0];
    palette[2] = [0.0, 1.0, 0.0, 1.0];

    gpu.begin_frame();
    gpu.set_clear_color(0.0, 0.0, 1.0, 1.0);
    gpu.push_palette_pair_tris(&quad, &identity, &identity, &identity, &palette, [1, 2]);
    gpu.end_frame();
    let (width, _, pixels) = gpu.read_pixels().expect("headless dither readback");
    let pixel = |x: usize, y: usize| {
        let offset = (y * width as usize + x) * 4;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
    };

    // The 2x presentation scale must preserve each source pixel as a 2x2
    // block: equal X/Y parity selects low red, differing parity high green.
    assert_eq!(pixel(0, 0), [255, 0, 0]);
    assert_eq!(pixel(1, 0), [255, 0, 0]);
    assert_eq!(pixel(2, 0), [0, 255, 0]);
    assert_eq!(pixel(3, 0), [0, 255, 0]);
    assert_eq!(pixel(0, 2), [0, 255, 0]);
    assert_eq!(pixel(2, 2), [255, 0, 0]);

    // Palette entry zero remains transparent and does not cover the clear.
    gpu.begin_frame();
    gpu.push_palette_pair_tris(&quad, &identity, &identity, &identity, &palette, [0, 2]);
    gpu.end_frame();
    let (width, _, pixels) = gpu.read_pixels().expect("transparent dither readback");
    let pixel = |x: usize, y: usize| {
        let offset = (y * width as usize + x) * 4;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
    };
    assert_eq!(pixel(0, 0), [0, 0, 255]);
    assert_eq!(pixel(2, 0), [0, 255, 0]);
}

const fn strategic_actor(
    kind: Sf2StrategicActorKind,
    appearance: Sf2StrategicActorAppearance,
    x: i16,
    y: i16,
) -> Option<Sf2StrategicActor> {
    Some(Sf2StrategicActor {
        kind,
        appearance,
        position: sf_render::renderer::Sf2MapPoint { x, y },
    })
}

fn strategic_return_inputs(checkpoint: StrategicReturn) -> FrameInputs<'static> {
    use Sf2StrategicActorAppearance::{
        EscalatedAssault, OpeningAssault, PostCarrier, PostEladard, PostFighterIntercept,
        PostInterception, PostLeon, PostMirage, PostPigma,
    };
    use Sf2StrategicActorKind::{
        AttackingFighter, DefensePlatform, EasternInterceptor, EnemyCarrier, EnemyFormation,
        FighterProjectile, Missile, MissileTrail, NorthernInstallation, PatrolShip, RivalFighter,
        SouthernInstallation, UnknownSignal,
    };

    let (actors, completed_sorties) = match checkpoint {
        StrategicReturn::First => (
            [
                strategic_actor(NorthernInstallation, OpeningAssault, 16, 14),
                strategic_actor(SouthernInstallation, OpeningAssault, 208, 110),
                strategic_actor(EnemyCarrier, OpeningAssault, 220, 7),
                strategic_actor(EnemyFormation, OpeningAssault, 62, 40),
                strategic_actor(EasternInterceptor, OpeningAssault, 203, 88),
                strategic_actor(PatrolShip, OpeningAssault, 12, 150),
                strategic_actor(MissileTrail, OpeningAssault, 100, 132),
                strategic_actor(Missile, OpeningAssault, 180, 117),
                None,
                None,
            ],
            1,
        ),
        StrategicReturn::Second => (
            [
                strategic_actor(NorthernInstallation, EscalatedAssault, 16, 14),
                strategic_actor(SouthernInstallation, EscalatedAssault, 208, 110),
                strategic_actor(EnemyCarrier, EscalatedAssault, 220, 7),
                strategic_actor(EnemyFormation, EscalatedAssault, 45, 45),
                strategic_actor(EasternInterceptor, EscalatedAssault, 198, 89),
                strategic_actor(PatrolShip, EscalatedAssault, 12, 150),
                strategic_actor(Missile, EscalatedAssault, 147, 125),
                None,
                None,
                None,
            ],
            2,
        ),
        StrategicReturn::PostInterception => (
            [
                strategic_actor(NorthernInstallation, PostInterception, 16, 14),
                strategic_actor(SouthernInstallation, PostInterception, 208, 110),
                strategic_actor(EnemyCarrier, PostInterception, 220, 7),
                strategic_actor(EnemyFormation, PostInterception, 47, 66),
                strategic_actor(EasternInterceptor, PostInterception, 172, 94),
                strategic_actor(PatrolShip, PostInterception, 12, 150),
                strategic_actor(AttackingFighter, PostInterception, 132, 119),
                None,
                None,
                None,
            ],
            3,
        ),
        StrategicReturn::PostFighterIntercept => (
            [
                strategic_actor(NorthernInstallation, PostFighterIntercept, 16, 14),
                strategic_actor(SouthernInstallation, PostFighterIntercept, 208, 110),
                strategic_actor(EnemyCarrier, PostFighterIntercept, 220, 7),
                strategic_actor(EnemyFormation, PostFighterIntercept, 46, 64),
                strategic_actor(EasternInterceptor, PostFighterIntercept, 170, 95),
                strategic_actor(PatrolShip, PostFighterIntercept, 12, 150),
                strategic_actor(AttackingFighter, PostFighterIntercept, 135, 119),
                strategic_actor(FighterProjectile, PostFighterIntercept, 86, 136),
                None,
                None,
            ],
            4,
        ),
        StrategicReturn::PostPigma => (
            [
                strategic_actor(NorthernInstallation, PostPigma, 16, 14),
                strategic_actor(SouthernInstallation, PostPigma, 208, 110),
                strategic_actor(EnemyCarrier, PostPigma, 220, 7),
                strategic_actor(EnemyFormation, PostPigma, 44, 71),
                strategic_actor(EasternInterceptor, PostPigma, 140, 95),
                strategic_actor(PatrolShip, PostPigma, 12, 145),
                strategic_actor(RivalFighter, PostPigma, 211, 120),
                strategic_actor(FighterProjectile, PostPigma, 115, 132),
                None,
                None,
            ],
            5,
        ),
        StrategicReturn::PostEladard => (
            [
                strategic_actor(NorthernInstallation, PostEladard, 16, 12),
                strategic_actor(SouthernInstallation, PostEladard, 208, 110),
                strategic_actor(EnemyCarrier, PostEladard, 220, 7),
                strategic_actor(EnemyFormation, PostEladard, 41, 75),
                strategic_actor(EasternInterceptor, PostEladard, 161, 96),
                strategic_actor(PatrolShip, PostEladard, 12, 150),
                strategic_actor(AttackingFighter, PostEladard, 192, 122),
                strategic_actor(UnknownSignal, PostEladard, 45, 101),
                strategic_actor(FighterProjectile, PostEladard, 86, 139),
                None,
            ],
            6,
        ),
        StrategicReturn::PostCarrier => (
            [
                strategic_actor(NorthernInstallation, PostCarrier, 16, 14),
                strategic_actor(SouthernInstallation, PostCarrier, 208, 110),
                strategic_actor(EnemyCarrier, PostCarrier, 220, 7),
                strategic_actor(EnemyFormation, PostCarrier, 25, 78),
                strategic_actor(AttackingFighter, PostCarrier, 125, 112),
                strategic_actor(PatrolShip, PostCarrier, 12, 150),
                strategic_actor(UnknownSignal, PostCarrier, 54, 123),
                strategic_actor(MissileTrail, PostCarrier, 8, 80),
                strategic_actor(Missile, PostCarrier, 24, 111),
                strategic_actor(FighterProjectile, PostCarrier, 49, 148),
            ],
            7,
        ),
        StrategicReturn::PostLeon => (
            [
                strategic_actor(NorthernInstallation, PostLeon, 16, 14),
                strategic_actor(SouthernInstallation, PostLeon, 208, 110),
                strategic_actor(EnemyCarrier, PostLeon, 220, 7),
                strategic_actor(EnemyFormation, PostLeon, 25, 78),
                strategic_actor(AttackingFighter, PostLeon, 125, 112),
                strategic_actor(PatrolShip, PostLeon, 12, 150),
                strategic_actor(UnknownSignal, PostLeon, 54, 123),
                strategic_actor(MissileTrail, PostLeon, 8, 80),
                strategic_actor(Missile, PostLeon, 24, 111),
                strategic_actor(FighterProjectile, PostLeon, 49, 148),
            ],
            8,
        ),
        StrategicReturn::PostMirage => (
            [
                strategic_actor(NorthernInstallation, PostMirage, 16, 14),
                strategic_actor(SouthernInstallation, PostMirage, 208, 110),
                strategic_actor(EnemyCarrier, PostMirage, 220, 7),
                strategic_actor(EnemyFormation, PostMirage, 25, 78),
                strategic_actor(AttackingFighter, PostMirage, 14, 120),
                strategic_actor(PatrolShip, PostMirage, 12, 150),
                strategic_actor(DefensePlatform, PostMirage, 72, 102),
                strategic_actor(UnknownSignal, PostMirage, 39, 143),
                strategic_actor(FighterProjectile, PostMirage, 104, 113),
                None,
            ],
            9,
        ),
    };
    let mut inputs = sf2_inputs(Sf2Mode::StrategicMap);
    let sf2 = inputs.sf2.as_mut().expect("SF2 fixture");
    sf2.primary_pilot = Some(Sf2Pilot::Fox);
    sf2.wingmate = Some(Sf2Pilot::Slippy);
    sf2.item_count = 3;
    sf2.campaign_sorties_completed = completed_sorties;
    sf2.strategic_player = sf_render::renderer::Sf2MapPoint { x: 72, y: 102 };
    sf2.strategic_actors = actors;
    match checkpoint {
        StrategicReturn::First => {
            sf2.primary_shield = 32;
            sf2.wingmate_shield = 16;
            sf2.elapsed_campaign_frames = 12 * 15;
        }
        StrategicReturn::Second => {
            sf2.primary_shield = 8;
            sf2.corneria_damage_percent = 10;
            sf2.score = 100;
            sf2.elapsed_campaign_frames = 19 * 15;
        }
        StrategicReturn::PostInterception => {
            sf2.primary_shield = 8;
            sf2.corneria_damage_percent = 74;
            sf2.score = 100;
            sf2.elapsed_campaign_frames = 55 * 15;
        }
        StrategicReturn::PostFighterIntercept => {
            sf2.primary_shield = 8;
            sf2.corneria_damage_percent = 74;
            sf2.score = 300;
            sf2.elapsed_campaign_frames = 51 * 15;
        }
        StrategicReturn::PostPigma => {
            sf2.primary_shield = 8;
            sf2.corneria_damage_percent = 74;
            sf2.item_count = 2;
            sf2.score = 1_300;
            sf2.elapsed_campaign_frames = 61 * 15;
        }
        StrategicReturn::PostEladard => {
            sf2.primary_shield = 40;
            sf2.wingmate_shield = 40;
            sf2.corneria_damage_percent = 89;
            sf2.score = 2_251;
            sf2.elapsed_campaign_frames = 68 * 15;
        }
        StrategicReturn::PostCarrier => {
            sf2.primary_shield = 34;
            sf2.wingmate_shield = 13;
            sf2.corneria_damage_percent = 89;
            sf2.score = 3_003;
            sf2.elapsed_campaign_frames = 75 * 15;
        }
        StrategicReturn::PostLeon => {
            sf2.primary_shield = 40;
            sf2.wingmate_shield = 40;
            sf2.corneria_damage_percent = 89;
            sf2.score = 3_403;
            sf2.elapsed_campaign_frames = 76 * 15;
        }
        StrategicReturn::PostMirage => {
            sf2.primary_shield = 100;
            sf2.wingmate_shield = 100;
            sf2.item_count = 3;
            sf2.score = 3_903;
            sf2.elapsed_campaign_frames = 80 * 15;
        }
    }
    inputs
}

fn check_sf2_strategic_returns_exact(config: &sf_render::renderer::RendererConfig) {
    let mut renderer = match Renderer::new_headless(SF2_MAP_WIDTH, SF2_MAP_HEIGHT, config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping exact SF2 map checks: no wgpu adapter ({error})");
            return;
        }
    };
    for (checkpoint, expected) in [
        (StrategicReturn::First, FIRST_RETURN_FNV1A),
        (StrategicReturn::Second, SECOND_RETURN_FNV1A),
        (
            StrategicReturn::PostInterception,
            POST_INTERCEPTION_RETURN_FNV1A,
        ),
        (
            StrategicReturn::PostFighterIntercept,
            POST_FIGHTER_INTERCEPT_RETURN_FNV1A,
        ),
        (StrategicReturn::PostPigma, POST_PIGMA_RETURN_FNV1A),
        (StrategicReturn::PostEladard, POST_ELADARD_RETURN_FNV1A),
        (StrategicReturn::PostCarrier, POST_CARRIER_RETURN_FNV1A),
        (StrategicReturn::PostLeon, POST_LEON_RETURN_FNV1A),
        (StrategicReturn::PostMirage, POST_MIRAGE_RETURN_FNV1A),
    ] {
        let inputs = strategic_return_inputs(checkpoint);
        renderer.begin_frame();
        renderer.submit(&[], &[], 1.0, &inputs);
        renderer.end_frame();
        let pixels = renderer.read_pixels_rgb();
        if matches!(checkpoint, StrategicReturn::PostPigma) {
            if let Some(path) = std::env::var_os("SF2_POST_PIGMA_DUMP_PPM") {
                let mut ppm = format!("P6\n{SF2_MAP_WIDTH} {SF2_MAP_HEIGHT}\n255\n").into_bytes();
                ppm.extend_from_slice(&pixels);
                std::fs::write(path, ppm).expect("write requested SF2 Pigma return dump");
            }
        }
        if matches!(checkpoint, StrategicReturn::PostEladard) {
            if let Some(path) = std::env::var_os("SF2_POST_ELADARD_DUMP_PPM") {
                let mut ppm = format!("P6\n{SF2_MAP_WIDTH} {SF2_MAP_HEIGHT}\n255\n").into_bytes();
                ppm.extend_from_slice(&pixels);
                std::fs::write(path, ppm).expect("write requested SF2 Eladard return dump");
            }
        }
        if matches!(checkpoint, StrategicReturn::PostCarrier) {
            if let Some(path) = std::env::var_os("SF2_POST_CARRIER_DUMP_PPM") {
                let mut ppm = format!("P6\n{SF2_MAP_WIDTH} {SF2_MAP_HEIGHT}\n255\n").into_bytes();
                ppm.extend_from_slice(&pixels);
                std::fs::write(path, ppm).expect("write requested SF2 carrier return dump");
            }
        }
        if matches!(checkpoint, StrategicReturn::PostLeon) {
            if let Some(path) = std::env::var_os("SF2_POST_LEON_DUMP_PPM") {
                let mut ppm = format!("P6\n{SF2_MAP_WIDTH} {SF2_MAP_HEIGHT}\n255\n").into_bytes();
                ppm.extend_from_slice(&pixels);
                std::fs::write(path, ppm).expect("write requested SF2 Leon return dump");
            }
        }
        if matches!(checkpoint, StrategicReturn::PostMirage) {
            if let Some(path) = std::env::var_os("SF2_POST_MIRAGE_DUMP_PPM") {
                let mut ppm = format!("P6\n{SF2_MAP_WIDTH} {SF2_MAP_HEIGHT}\n255\n").into_bytes();
                ppm.extend_from_slice(&pixels);
                std::fs::write(path, ppm).expect("write requested SF2 Mirage return dump");
            }
        }
        let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
            (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
        });
        assert_eq!(hash, expected, "SF2 strategic return frame drifted");
    }
    renderer.shutdown();
}

fn sf2_inputs(mode: Sf2Mode) -> FrameInputs<'static> {
    FrameInputs {
        sf2: Some(Sf2FrameInputs {
            mode,
            intro_presentation_tick: 0,
            intro_title_menu_countdown: None,
            polygon_palette: sf_render::shapes::Sf2PolygonPalette::Standard,
            mission_backdrop: sf_render::renderer::Sf2MissionBackdrop::DeepSpace,
            title_page: Sf2TitlePage::MainMenu,
            title_menu_item: Sf2TitleMenuItem::Mission,
            difficulty: Sf2Difficulty::Normal,
            audio_output: Sf2AudioOutput::Stereo,
            pilot_selection_phase: Sf2PilotSelectionPhase::ChoosingPrimary,
            pilot_selection_cursor: Sf2PilotSelectionCursor::Pilot(Sf2Pilot::Fox),
            flight_control_style: Sf2FlightControlStyle::TypeA,
            primary_pilot: None,
            wingmate: (mode == Sf2Mode::GameOver).then_some(Sf2Pilot::Slippy),
            game_over_phase: Sf2GameOverPhase::Choosing,
            game_over_choice: Sf2GameOverChoice::ContinueWithWingmate,
            game_over_transition_retail_frames: 0,
            results_phase: sf_render::renderer::Sf2ResultsPhase::Revealing,
            results_choice: sf_render::renderer::Sf2ResultsChoice::Retry,
            results_presentation_retail_frames: if mode == Sf2Mode::Results { 652 } else { 0 },
            results_transition_retail_frames: 0,
            ending_phase: Sf2EndingPhase::StaffRoll,
            ending_presentation_tick: 0,
            ending_transition_retail_frames: 0,
            primary_shield: 0,
            wingmate_shield: 0,
            item_count: 0,
            target_count: 0,
            mission_elapsed_time_tenths: SF2_TEST_MISSION_TIME_TENTHS,
            mission_message: None,
            radar_contacts: [None; SF2_RADAR_CONTACT_CAPACITY],
            mode_frame: if mode == Sf2Mode::GameOver { 100 } else { 0 },
            elapsed_campaign_frames: 0,
            corneria_damage_percent: 0,
            score: 0,
            campaign_sorties_completed: 0,
            strategic_opening_presentation_tick: 0,
            strategic_phase: sf_render::renderer::Sf2StrategicPhase::Planning,
            strategic_marker_phase: 0,
            strategic_player: sf_render::renderer::Sf2MapPoint::default(),
            strategic_destination: sf_render::renderer::Sf2MapPoint::default(),
            strategic_actors: [None; sf_render::renderer::SF2_STRATEGIC_MAP_ACTOR_CAPACITY],
        }),
        ..Default::default()
    }
}

fn check_sf2_native_frontend(renderer: &mut Renderer) {
    const MIN_VISIBLE_PIXELS: usize = 10_000;
    const MIN_DISTINCT_COLORS: usize = 8;

    for mode in [
        Sf2Mode::Title,
        Sf2Mode::PilotSelection,
        Sf2Mode::StrategicMap,
        Sf2Mode::Mission,
        Sf2Mode::GameOver,
        Sf2Mode::Results,
    ] {
        let inputs = sf2_inputs(mode);
        renderer.begin_frame();
        renderer.submit(&[], &[], 1.0, &inputs);
        renderer.end_frame();

        let pixels = renderer.read_pixels_rgb();
        let mut colors = std::collections::BTreeSet::new();
        let visible = pixels
            .chunks_exact(3)
            .filter(|pixel| {
                colors.insert([pixel[0], pixel[1], pixel[2]]);
                !color_near(pixel, SF1_CLEAR_RGB, 2)
            })
            .count();
        assert!(
            visible >= MIN_VISIBLE_PIXELS,
            "SF2 {mode:?} frame collapsed to the renderer clear color ({visible} visible pixels)"
        );
        assert!(
            colors.len() >= MIN_DISTINCT_COLORS,
            "SF2 {mode:?} frame collapsed to {} colors",
            colors.len()
        );
    }
}

fn check_sf2_mission_backdrops(renderer: &mut Renderer) {
    let mut hashes = std::collections::BTreeSet::new();
    for backdrop in [
        Sf2MissionBackdrop::DeepSpace,
        Sf2MissionBackdrop::EladardSurface,
        Sf2MissionBackdrop::EladardInterior,
        Sf2MissionBackdrop::TitaniaBase,
        Sf2MissionBackdrop::CarrierInterior,
        Sf2MissionBackdrop::AstropolisVoid,
    ] {
        let mut inputs = sf2_inputs(Sf2Mode::Mission);
        inputs
            .sf2
            .as_mut()
            .expect("SF2 mission inputs")
            .mission_backdrop = backdrop;
        renderer.begin_frame();
        renderer.submit(&[], &[], 1.0, &inputs);
        renderer.end_frame();
        let hash = renderer
            .read_pixels_rgb()
            .into_iter()
            .fold(FNV_OFFSET_BASIS, |value, byte| {
                (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
            });
        assert!(
            hashes.insert(hash),
            "SF2 {backdrop:?} did not select its own retail backdrop texture"
        );
    }
}

fn check_sf2_mission_message(renderer: &mut Renderer) {
    const SF2_MISSION_MESSAGE_FRAME_FNV1A: u32 = 0x5004AF76;

    let mut inputs = sf2_inputs(Sf2Mode::Mission);
    inputs
        .sf2
        .as_mut()
        .expect("SF2 mission inputs")
        .mission_message = Some(Sf2MissionMessageInputs {
        message: Sf2MissionMessage::FlyFasterByPressingYButton,
        phase: Sf2MissionMessagePhase::Open {
            portrait_talking: false,
        },
    });
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    if let Some(path) = std::env::var_os("SF2_MISSION_MESSAGE_DUMP_PPM") {
        let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
        ppm.extend_from_slice(&pixels);
        std::fs::write(path, ppm).expect("write requested SF2 mission message dump");
    }
    let hash = pixels.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    assert_eq!(
        hash, SF2_MISSION_MESSAGE_FRAME_FNV1A,
        "SF2 mission guidance presentation drifted"
    );
}

fn check_sf2_shape(renderer: &mut Renderer) {
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    const ENTRY_FORMATION_CRAFT: u16 = sf_core::shape::sf2_shape_id(415);
    const TEST_DEPTH: i32 = 500;
    let entry = DrawListEntry {
        z: TEST_DEPTH << 16,
        shape_id: ENTRY_FORMATION_CRAFT,
        flags: DL_FLAG_VISIBLE,
        obj_id: 1,
        ..DrawListEntry::default()
    };
    let curr = [entry];
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&curr, &curr, 1.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();
    let visible = px
        .chunks_exact(3)
        .filter(|pixel| !color_near(pixel, SF1_CLEAR_RGB, 2))
        .count();
    assert!(
        visible > 100,
        "SF2 formation craft did not reach the shared 3D pass ({visible} pixels)"
    );
}

fn check_sf2_texture_face(renderer: &mut Renderer) {
    const CAMPAIGN_MISSILE: u16 = sf_core::shape::sf2_shape_id(181);
    const TEST_DEPTH: i32 = 1_600;

    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let base = DrawListEntry {
        shape_id: CAMPAIGN_MISSILE,
        z: TEST_DEPTH << 16,
        flags: DL_FLAG_VISIBLE,
        ..DrawListEntry::default()
    };
    let curr = [
        DrawListEntry {
            x: -280 << 16,
            ry: 32,
            obj_id: 1,
            ..base
        },
        DrawListEntry {
            x: 280 << 16,
            ry: 160,
            obj_id: 2,
            ..base
        },
    ];
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&curr, &curr, 1.0, &inputs);
    renderer.end_frame();

    let pixels = renderer.read_pixels_rgb();
    let mut visible = 0usize;
    let mut magenta = 0usize;
    let mut colors = std::collections::BTreeSet::new();
    for pixel in pixels.chunks_exact(3) {
        if !color_near(pixel, SF1_CLEAR_RGB, 2) {
            visible += 1;
            colors.insert([pixel[0], pixel[1], pixel[2]]);
        }
        if color_near(pixel, [255, 0, 255], 2) {
            magenta += 1;
        }
    }
    assert!(
        visible > 100,
        "textured SF2 missile did not render ({visible} pixels)"
    );
    assert_eq!(magenta, 0, "SF2 COLTEXT fell back to debug magenta");
    assert!(
        colors.len() >= 3,
        "SF2 texture map collapsed to {} flat colors",
        colors.len()
    );
}

fn check_player_laser(renderer: &mut Renderer) {
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let curr = [DrawListEntry {
        shape_id: SHAPE_ELASER2,
        z: 120 << 16,
        rx: 32,
        anim_frame: 4,
        col_frame: 0,
        flags: DL_FLAG_VISIBLE,
        obj_id: 1,
        ..Default::default()
    }];
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        source_resolution: true,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&curr, &curr, 1.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();
    let visible = px
        .chunks_exact(3)
        .filter(|p| !color_near(p, SF1_CLEAR_RGB, 2))
        .count();
    let magenta = px
        .chunks_exact(3)
        .filter(|p| color_near(p, [255, 0, 255], 2))
        .count();
    assert!(
        visible > 20,
        "player laser did not render ({visible} pixels)"
    );
    assert_eq!(magenta, 0, "player laser resolved outside bullet_c");
}

fn check_superfx_texture_face(renderer: &mut Renderer) {
    const TEST_DEPTH: i32 = 500;
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let curr = [DrawListEntry {
        shape_id: SHAPE_EXT_ASTEROID1,
        z: TEST_DEPTH << 16,
        ry: 128,
        flags: DL_FLAG_VISIBLE,
        obj_id: 1,
        ..Default::default()
    }];
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        source_resolution: true,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&curr, &curr, 1.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();
    let mut visible = 0usize;
    let mut magenta = 0usize;
    let mut colors = std::collections::BTreeSet::new();
    for p in px.chunks_exact(3) {
        if !color_near(p, SF1_CLEAR_RGB, 2) {
            visible += 1;
            colors.insert([p[0], p[1], p[2]]);
        }
        if color_near(p, [255, 0, 255], 2) {
            magenta += 1;
        }
    }
    assert!(
        visible > 100,
        "textured asteroid did not render ({visible} pixels)"
    );
    assert_eq!(magenta, 0, "COLTEXT fell back to debug magenta");
    assert!(
        colors.len() >= 3,
        "texture map collapsed to {} flat colors",
        colors.len()
    );
}

// (c) Full composed title frame vs source-asset region averages.
fn check_title_golden(renderer: &mut Renderer) {
    let inputs = FrameInputs {
        game_state: GameState::Title,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 0.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();
    let grid = grid_8x8(&px, W as usize, H as usize, 3);
    let mut max_delta = 0i32;
    for (i, (got, want)) in grid
        .iter()
        .zip(SOURCE_TITLE_COMPOSITE_GRID.iter())
        .enumerate()
    {
        for c in 0..3 {
            let delta = (got[c] as i32 - want[c] as i32).abs();
            max_delta = max_delta.max(delta);
            assert!(
                delta <= 1,
                "title GL region {i} channel {c}: got {} want {} (delta {delta})",
                got[c],
                want[c]
            );
        }
    }
    println!("title source-asset grid: max region delta {max_delta}");
}

// (b) bg_1_1c: playing state on map 1_1 with a level camera at rx=0. The
// Authored linear camera coupling windows a uniform sky-blue row at the top.
fn check_bg_1_1c_sky(renderer: &mut Renderer) {
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let inputs = FrameInputs {
        game_state: GameState::Playing,
        newmap: 1, // MAP_ID_1_1 -> default bg 4 (bg_1_1c)
        currentbg: 0,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();
    // Sample the top row (a few columns across the screen).
    for x in [10usize, W as usize / 2, W as usize - 10] {
        let p = &px[x * 3..x * 3 + 3];
        assert!(
            color_near(p, CORNERIA_SKY_RGB, 2),
            "bg_1_1c top row at x={x}: got ({}, {}, {}), want sky blue {CORNERIA_SKY_RGB:?}",
            p[0],
            p[1],
            p[2]
        );
    }
}

// (a) Arwing with a known camera: canopy blue + hull grey present.
fn check_arwing(renderer: &mut Renderer) {
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);

    // Two orientations so both canopy side-faces are unambiguously visible:
    // one seen from the front-top, one from the rear-top.
    let base = DrawListEntry {
        shape_id: SHAPE_MYSHIP_4,
        flags: DL_FLAG_VISIBLE,
        ..Default::default()
    };
    let curr = [
        DrawListEntry {
            x: -70 << 16,
            y: 0,
            z: 150 << 16,
            rx: 32, // pitch toward the camera
            ry: 128,
            obj_id: 1,
            ..base
        },
        DrawListEntry {
            x: 70 << 16,
            y: 0,
            z: 150 << 16,
            rx: 224,
            ry: 0,
            obj_id: 2,
            ..base
        },
    ];

    let inputs = FrameInputs {
        game_state: GameState::Boot, // no bg/hud/ui passes; clear color only
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&curr, &curr, 1.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();

    // Expected canopy blue: face color 44 (COLANIM CA_2), col_frame 0 ->
    // COLNORM palette 8 = NIGHT.COL 0x7FB6. Depth bank 0 (150 < 2560).
    let canopy = expected_rgb8(shapes::resolve_face_color(SHAPE_MYSHIP_4, 44, 0, 0, 9, 0));
    // Sanity-pin the family: NIGHT.COL palettes 5-8 decode to the blue ramp;
    // palette 8 (0x7FB6) is (181, 239, 255) in 8-bit.
    assert_eq!(canopy, [181, 239, 255], "canopy material decode changed");

    // Expected hull greys: COLLITE rows 0 and 1 across all shade indices.
    let mut hull_greys: Vec<[u8; 3]> = Vec::new();
    for face_color in [0u8, 1u8] {
        for shade in 0..10 {
            hull_greys.push(expected_rgb8(shapes::resolve_face_color(
                SHAPE_MYSHIP_4,
                face_color,
                0,
                0,
                shade,
                0,
            )));
        }
    }

    let mut non_bg = 0usize;
    let mut canopy_hits = 0usize;
    let mut hull_hits = 0usize;
    for p in px.chunks_exact(3) {
        if !color_near(p, SF1_CLEAR_RGB, 2) {
            non_bg += 1;
        }
        if color_near(p, canopy, 3) {
            canopy_hits += 1;
        }
        if hull_greys.iter().any(|g| color_near(p, *g, 3)) {
            hull_hits += 1;
        }
    }

    println!("arwing: non_bg={non_bg} canopy_hits={canopy_hits} hull_hits={hull_hits}");
    assert!(
        non_bg > 500,
        "arwing barely rendered: {non_bg} non-bg pixels"
    );
    assert!(
        canopy_hits > 10,
        "canopy blue not found ({canopy_hits} hits)"
    );
    assert!(hull_hits > 50, "hull greys not found ({hull_hits} hits)");
}

/// Render-direction guard: an object at +worldx must land on the RIGHT half
/// of the screen. If it lands left, the 3D renderer is mirroring X (the
/// "faces left but moves right" bug).
#[test]
fn positive_worldx_renders_on_right_half() {
    let _gpu_test_guard = GPU_TEST_LOCK.lock().expect("GPU test lock poisoned");
    let config = config_from_repo_root(&repo_root());
    let mut renderer = match Renderer::new_headless(W as i32, H as i32, &config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping render-direction test: no wgpu adapter ({e})");
            return;
        }
    };
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let base = DrawListEntry {
        shape_id: SHAPE_MYSHIP_4,
        flags: DL_FLAG_VISIBLE,
        ..Default::default()
    };
    // +x, ahead in +z, well within the FOV so it's clearly right-of-center.
    let curr = [DrawListEntry {
        x: 150 << 16,
        y: 0,
        z: 600 << 16,
        obj_id: 1,
        ..base
    }];
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&curr, &curr, 1.0, &inputs);
    renderer.end_frame();
    let px = renderer.read_pixels_rgb();
    let (w, h) = (W as usize, H as usize);
    let (mut left, mut right) = (0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            let lum = px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32;
            if lum > 80 {
                if x < w / 2 {
                    left += 1;
                } else {
                    right += 1;
                }
            }
        }
    }
    eprintln!("RENDER-DIR: +worldx object  left_px={left}  right_px={right}");
    assert!(
        right > left,
        "+worldx must render RIGHT (left={left} right={right}); left>right => X is mirrored"
    );
}
