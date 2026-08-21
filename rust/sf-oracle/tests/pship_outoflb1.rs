//! Frame differential for the DM_END base-escape ship.

use sf_game::vars::GF_STRATDONE1;
use sf_game::Game;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, pshipoutoflb1_istrat, pshipoutoflb1_strat, viewoutoflb1_istrat,
    viewoutoflb1_strat,
};

const OBJECT_BLOCK: u32 = 0x0500;
const CAMERA_BLOCK: u32 = 0x0580;
const OLD_TARGET_BLOCK: u32 = 0x0600;
const OBJECT_NEXT: u32 = 0x00;
const OBJECT_ATTACHED_VALUE: u32 = 0x06;
const OBJECT_WORLD_X: u32 = 0x0C;
const OBJECT_WORLD_Y: u32 = 0x0E;
const OBJECT_WORLD_Z: u32 = 0x10;
const OBJECT_ROTATION_X: u32 = 0x12;
const OBJECT_ROTATION_Y: u32 = 0x13;
const OBJECT_ROTATION_Z: u32 = 0x14;
const OBJECT_SPEED: u32 = 0x15;
const OBJECT_STRATEGY_FLAGS_2: u32 = 0x1E;
const OBJECT_STRATEGY_BYTE_1: u32 = 0x22;
const OBJECT_STRATEGY_WORD_1: u32 = 0x26;
const OBJECT_STRATEGY_WORD_2: u32 = 0x28;
const OBJECT_VELOCITY_X: u32 = 0x2F;
const OBJECT_VELOCITY_Y: u32 = 0x31;
const OBJECT_VELOCITY_Z: u32 = 0x33;
const ACTIVE_LIST_HEAD: u32 = 0x12AD;
const GAME_FLAGS: u32 = 0x155B;
const GAME_FRAME: u32 = 0x1640;
const VIEW_TARGET_OBJECT: u32 = 0x1628;
const EXTENDED_STRATEGY_STATE_BASE: u32 = 0x7E_1CDA;
const INITIAL_POSITION: [i16; 3] = [0, 0, 1000];
const FRAME_LIMIT: u16 = 400;
const ESCAPE_SHIP_INITIAL_POSITION: [i16; 3] = [0, 0, -5258];
const ESCAPE_CAMERA_INITIAL_POSITION: [i16; 3] = [-50, -1500, -5158];
const OLD_TARGET_INITIAL_POSITION: [i16; 3] = [-13, -60, -6258];
const ESCAPE_VIEW_DISTANCE: i16 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EscapeShipState {
    position: [i16; 3],
    rotation: [u8; 3],
    velocity: [i16; 3],
    speed: u8,
    strategy_state: u8,
    strategy_byte_1: u8,
    done: bool,
}

fn source_state(bus: &SnesBus) -> EscapeShipState {
    EscapeShipState {
        position: [
            bus.read16(OBJECT_BLOCK + OBJECT_WORLD_X) as i16,
            bus.read16(OBJECT_BLOCK + OBJECT_WORLD_Y) as i16,
            bus.read16(OBJECT_BLOCK + OBJECT_WORLD_Z) as i16,
        ],
        rotation: [
            bus.read8(OBJECT_BLOCK + OBJECT_ROTATION_X),
            bus.read8(OBJECT_BLOCK + OBJECT_ROTATION_Y),
            bus.read8(OBJECT_BLOCK + OBJECT_ROTATION_Z),
        ],
        velocity: [
            bus.read16(OBJECT_BLOCK + OBJECT_VELOCITY_X) as i16,
            bus.read16(OBJECT_BLOCK + OBJECT_VELOCITY_Y) as i16,
            bus.read16(OBJECT_BLOCK + OBJECT_VELOCITY_Z) as i16,
        ],
        speed: bus.read8(OBJECT_BLOCK + OBJECT_SPEED),
        strategy_state: bus.read8(EXTENDED_STRATEGY_STATE_BASE + OBJECT_BLOCK),
        strategy_byte_1: bus.read8(OBJECT_BLOCK + OBJECT_STRATEGY_BYTE_1),
        done: bus.read8(GAME_FLAGS) & GF_STRATDONE1 != 0,
    }
}

fn port_state(game: &Game, object: u16) -> EscapeShipState {
    let ship = &game.objs.aliens[object as usize];
    EscapeShipState {
        position: [ship.worldx, ship.worldy, ship.worldz],
        rotation: [ship.rotx, ship.roty, ship.rotz],
        velocity: [ship.vx, ship.vy, ship.vz],
        speed: ship.vel,
        strategy_state: ship.stratstate,
        strategy_byte_1: ship.sbyte1,
        done: game.vars.gameflags & GF_STRATDONE1 != 0,
    }
}

fn call_source(bus: &mut SnesBus, address: u32) {
    call(
        bus,
        address,
        &Entry {
            x: OBJECT_BLOCK as u16,
            dbr: 0x7E,
            p: 0x20,
            ..Default::default()
        },
    );
}

#[test]
fn base_escape_ship_matches_the_assembled_strategy_until_completion() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: assembled SF1 ROM unavailable");
        return;
    };
    let symbols = load_symbols();
    let initializer = symbols["PSHIPOUTOFLB1_ISTRAT"];
    let strategy = symbols["PSHIPOUTOFLB1_STRAT"];

    let mut source = SnesBus::new(rom);
    source.write16(ACTIVE_LIST_HEAD, OBJECT_BLOCK as u16);
    source.write16(OBJECT_BLOCK + OBJECT_WORLD_X, INITIAL_POSITION[0] as u16);
    source.write16(OBJECT_BLOCK + OBJECT_WORLD_Y, INITIAL_POSITION[1] as u16);
    source.write16(OBJECT_BLOCK + OBJECT_WORLD_Z, INITIAL_POSITION[2] as u16);

    let mut port = Game::new();
    let object = port.objs.alloc().expect("escape ship object");
    {
        let ship = &mut port.objs.aliens[object as usize];
        ship.worldx = INITIAL_POSITION[0];
        ship.worldy = INITIAL_POSITION[1];
        ship.worldz = INITIAL_POSITION[2];
    }

    for frame in 1..=FRAME_LIMIT {
        source.write16(GAME_FRAME, frame);
        port.vars.gameframe = frame;
        if frame == 1 {
            call_source(&mut source, initializer);
            pshipoutoflb1_istrat(&mut port, object);
        } else {
            call_source(&mut source, strategy);
            pshipoutoflb1_strat(&mut port, object);
        }

        let expected = source_state(&source);
        let actual = port_state(&port, object);
        assert_eq!(
            actual, expected,
            "base-escape ship diverged on frame {frame}"
        );
        if expected.done {
            return;
        }
    }

    panic!("base-escape ship did not complete within {FRAME_LIMIT} frames");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EscapeCameraState {
    position: [i16; 3],
    speed: u8,
    strategy_flags_2: u8,
    offsets: [i16; 3],
}

#[test]
fn state_three_camera_behavior_matches_the_assembled_strategy() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: assembled SF1 ROM unavailable");
        return;
    };
    let symbols = load_symbols();
    let source_strategy = symbols["VIEWOUTOFLB1_STRAT"];
    let ship_position = [100i16, 200, 250];
    let camera_position = [300i16, 400, 500];

    let mut source = SnesBus::new(rom);
    for (offset, value) in [
        (OBJECT_WORLD_X, ship_position[0]),
        (OBJECT_WORLD_Y, ship_position[1]),
        (OBJECT_WORLD_Z, ship_position[2]),
    ] {
        source.write16(OBJECT_BLOCK + offset, value as u16);
    }
    for (offset, value) in [
        (OBJECT_WORLD_X, camera_position[0]),
        (OBJECT_WORLD_Y, camera_position[1]),
        (OBJECT_WORLD_Z, camera_position[2]),
    ] {
        source.write16(CAMERA_BLOCK + offset, value as u16);
    }
    source.write8(EXTENDED_STRATEGY_STATE_BASE + OBJECT_BLOCK, 3);
    source.write16(VIEW_TARGET_OBJECT, OBJECT_BLOCK as u16);
    call(
        &mut source,
        source_strategy,
        &Entry {
            x: CAMERA_BLOCK as u16,
            dbr: 0x7E,
            p: 0x20,
            ..Default::default()
        },
    );
    let expected = EscapeCameraState {
        position: [
            source.read16(CAMERA_BLOCK + OBJECT_WORLD_X) as i16,
            source.read16(CAMERA_BLOCK + OBJECT_WORLD_Y) as i16,
            source.read16(CAMERA_BLOCK + OBJECT_WORLD_Z) as i16,
        ],
        speed: source.read8(CAMERA_BLOCK + OBJECT_SPEED),
        strategy_flags_2: source.read8(CAMERA_BLOCK + OBJECT_STRATEGY_FLAGS_2),
        offsets: [
            source.read16(CAMERA_BLOCK + OBJECT_STRATEGY_WORD_1) as i16,
            source.read16(CAMERA_BLOCK + OBJECT_STRATEGY_WORD_2) as i16,
            source.read16(CAMERA_BLOCK + OBJECT_ATTACHED_VALUE) as i16,
        ],
    };

    let mut port = Game::new();
    let ship = port.objs.alloc().expect("escape ship");
    let camera = port.objs.alloc().expect("escape camera");
    {
        let object = &mut port.objs.aliens[ship as usize];
        object.worldx = ship_position[0];
        object.worldy = ship_position[1];
        object.worldz = ship_position[2];
        object.stratstate = 3;
    }
    {
        let object = &mut port.objs.aliens[camera as usize];
        object.worldx = camera_position[0];
        object.worldy = camera_position[1];
        object.worldz = camera_position[2];
    }
    port.vars.set_sv_i16(sv::VIEWTOOBJ, ship as i16);
    viewoutoflb1_strat(&mut port, camera);
    let object = &port.objs.aliens[camera as usize];
    let actual = EscapeCameraState {
        position: [object.worldx, object.worldy, object.worldz],
        speed: object.vel,
        strategy_flags_2: object.sflags2,
        offsets: [object.sword1, object.sword2, object.ptr as i16],
    };

    assert_eq!(actual, expected);
}

#[test]
fn base_escape_ship_and_camera_match_with_the_authored_creation_order() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: assembled SF1 ROM unavailable");
        return;
    };
    let symbols = load_symbols();
    let source_ship_initializer = symbols["PSHIPOUTOFLB1_ISTRAT"];
    let source_ship_strategy = symbols["PSHIPOUTOFLB1_STRAT"];
    let source_camera_initializer = symbols["VIEWOUTOFLB1_ISTRAT"];
    let source_camera_strategy = symbols["VIEWOUTOFLB1_STRAT"];
    let source_view_distance = symbols["OUTDIST"];

    let mut source = SnesBus::new(rom);
    source.write16(ACTIVE_LIST_HEAD, CAMERA_BLOCK as u16);
    source.write16(CAMERA_BLOCK + OBJECT_NEXT, OBJECT_BLOCK as u16);
    source.write16(OBJECT_BLOCK + OBJECT_NEXT, OLD_TARGET_BLOCK as u16);
    source.write16(OLD_TARGET_BLOCK + OBJECT_NEXT, 0);
    for (block, position) in [
        (OBJECT_BLOCK, ESCAPE_SHIP_INITIAL_POSITION),
        (CAMERA_BLOCK, ESCAPE_CAMERA_INITIAL_POSITION),
        (OLD_TARGET_BLOCK, OLD_TARGET_INITIAL_POSITION),
    ] {
        source.write16(block + OBJECT_WORLD_X, position[0] as u16);
        source.write16(block + OBJECT_WORLD_Y, position[1] as u16);
        source.write16(block + OBJECT_WORLD_Z, position[2] as u16);
    }
    source.write16(VIEW_TARGET_OBJECT, OLD_TARGET_BLOCK as u16);
    source.write16(source_view_distance, ESCAPE_VIEW_DISTANCE as u16);

    let mut port = Game::new();
    let old_target = port.objs.alloc().expect("old camera target");
    let ship = port.objs.alloc().expect("escape ship");
    let camera = port.objs.alloc().expect("escape camera");
    for (object, position) in [
        (ship, ESCAPE_SHIP_INITIAL_POSITION),
        (camera, ESCAPE_CAMERA_INITIAL_POSITION),
        (old_target, OLD_TARGET_INITIAL_POSITION),
    ] {
        let object = &mut port.objs.aliens[object as usize];
        object.worldx = position[0];
        object.worldy = position[1];
        object.worldz = position[2];
    }
    port.vars.set_sv_i16(sv::VIEWTOOBJ, old_target as i16);
    port.vars.set_sv_i16(sv::OUTDIST, ESCAPE_VIEW_DISTANCE);

    for frame in 1..=FRAME_LIMIT {
        source.write16(GAME_FRAME, frame);
        port.vars.gameframe = frame;
        if frame == 1 {
            call(
                &mut source,
                source_camera_initializer,
                &Entry {
                    x: CAMERA_BLOCK as u16,
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                },
            );
            viewoutoflb1_istrat(&mut port, camera);
            call_source(&mut source, source_ship_initializer);
            pshipoutoflb1_istrat(&mut port, ship);
        } else {
            call(
                &mut source,
                source_camera_strategy,
                &Entry {
                    x: CAMERA_BLOCK as u16,
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                },
            );
            viewoutoflb1_strat(&mut port, camera);
            call_source(&mut source, source_ship_strategy);
            pshipoutoflb1_strat(&mut port, ship);
        }

        assert_eq!(
            port_state(&port, ship),
            source_state(&source),
            "base-escape ship diverged on combined frame {frame}",
        );
        let expected_camera = EscapeCameraState {
            position: [
                source.read16(CAMERA_BLOCK + OBJECT_WORLD_X) as i16,
                source.read16(CAMERA_BLOCK + OBJECT_WORLD_Y) as i16,
                source.read16(CAMERA_BLOCK + OBJECT_WORLD_Z) as i16,
            ],
            speed: source.read8(CAMERA_BLOCK + OBJECT_SPEED),
            strategy_flags_2: source.read8(CAMERA_BLOCK + OBJECT_STRATEGY_FLAGS_2) & 0xF0,
            offsets: [
                source.read16(CAMERA_BLOCK + OBJECT_STRATEGY_WORD_1) as i16,
                source.read16(CAMERA_BLOCK + OBJECT_STRATEGY_WORD_2) as i16,
                source.read16(CAMERA_BLOCK + OBJECT_ATTACHED_VALUE) as i16,
            ],
        };
        let actual_camera_object = &port.objs.aliens[camera as usize];
        let actual_camera = EscapeCameraState {
            position: [
                actual_camera_object.worldx,
                actual_camera_object.worldy,
                actual_camera_object.worldz,
            ],
            speed: actual_camera_object.vel,
            strategy_flags_2: actual_camera_object.sflags2 & 0xF0,
            offsets: [
                actual_camera_object.sword1,
                actual_camera_object.sword2,
                actual_camera_object.ptr as i16,
            ],
        };
        assert_eq!(
            actual_camera, expected_camera,
            "base-escape camera diverged on combined frame {frame}",
        );
        if port.vars.gameflags & GF_STRATDONE1 != 0 {
            return;
        }
    }

    panic!("combined base-escape sequence did not complete within {FRAME_LIMIT} frames");
}
