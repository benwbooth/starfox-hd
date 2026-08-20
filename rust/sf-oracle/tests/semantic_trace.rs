//! End-to-end proof that a real retail routine and the flat native port can be
//! compared through the shared, storage-independent semantic trace format.

use sf_difftest::{first_divergence, SemanticFrame, SemanticObject};
use sf_oracle::{
    call, load_retail_rom, snapshot_objects, Entry, SnesBus, AL_VX, AL_VY, AL_VZ, RETAIL_POOL,
    RETAIL_PVIEWVELZ, RETAIL_STRAIGHT_STRAT,
};

const FRAME_COUNT: u64 = 30;
const INITIAL_POSITION_X: i16 = 1_000;
const INITIAL_POSITION_Y: i16 = 500;
const INITIAL_POSITION_Z: i16 = 8_000;
const VELOCITY_X: i16 = 300;
const VELOCITY_Y: i16 = -120;
const VELOCITY_Z: i16 = -50;
const VIEW_FORWARD_VELOCITY: i16 = -200;
const NO_INPUT: u32 = 0;
const PRIMARY_ENEMY: &str = "primary-enemy";

fn trace_frame(
    sequence: u64,
    position: (i16, i16, i16),
    velocity: (i16, i16, i16),
) -> SemanticFrame {
    SemanticFrame::new(sequence, sequence, NO_INPUT)
        .with_field("view.forward_velocity", VIEW_FORWARD_VELOCITY)
        .with_object(
            SemanticObject::new(PRIMARY_ENEMY, "fighter")
                .with_field("position.x", position.0)
                .with_field("position.y", position.1)
                .with_field("position.z", position.2)
                .with_field("velocity.x", velocity.0)
                .with_field("velocity.y", velocity.1)
                .with_field("velocity.z", velocity.2),
        )
}

#[test]
fn retail_straight_motion_matches_native_semantic_trace() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("retail semantic trace skipped: Star Fox retail ROM not found");
        return;
    };

    let mut retail = SnesBus::new(rom);
    let object_block = RETAIL_POOL.base;
    retail.wram_write16(RETAIL_PVIEWVELZ, VIEW_FORWARD_VELOCITY as u16);
    retail.wram_write16(
        object_block + RETAIL_POOL.al_worldx,
        INITIAL_POSITION_X as u16,
    );
    retail.wram_write16(
        object_block + RETAIL_POOL.al_worldy,
        INITIAL_POSITION_Y as u16,
    );
    retail.wram_write16(
        object_block + RETAIL_POOL.al_worldz,
        INITIAL_POSITION_Z as u16,
    );
    retail.wram_write16(object_block + AL_VX, VELOCITY_X as u16);
    retail.wram_write16(object_block + AL_VY, VELOCITY_Y as u16);
    retail.wram_write16(object_block + AL_VZ, VELOCITY_Z as u16);

    let mut native = sf_game::alien::Alien {
        worldx: INITIAL_POSITION_X,
        worldy: INITIAL_POSITION_Y,
        worldz: INITIAL_POSITION_Z,
        vx: VELOCITY_X,
        vy: VELOCITY_Y,
        vz: VELOCITY_Z,
        ..Default::default()
    };

    let mut retail_trace = vec![trace_frame(
        0,
        (INITIAL_POSITION_X, INITIAL_POSITION_Y, INITIAL_POSITION_Z),
        (VELOCITY_X, VELOCITY_Y, VELOCITY_Z),
    )];
    let mut native_trace = retail_trace.clone();

    for sequence in 1..=FRAME_COUNT {
        call(
            &mut retail,
            RETAIL_STRAIGHT_STRAT,
            &Entry {
                x: object_block as u16,
                ..Default::default()
            },
        );
        let retail_object = snapshot_objects(&retail, &RETAIL_POOL)[0];
        retail_trace.push(trace_frame(
            sequence,
            (
                retail_object.worldx,
                retail_object.worldy,
                retail_object.worldz,
            ),
            (VELOCITY_X, VELOCITY_Y, VELOCITY_Z),
        ));

        sf_strat::common::strat_apply_velocity(&mut native);
        native.worldz = native.worldz.wrapping_add(VIEW_FORWARD_VELOCITY);
        native_trace.push(trace_frame(
            sequence,
            (native.worldx, native.worldy, native.worldz),
            (native.vx, native.vy, native.vz),
        ));
    }

    if let Some(divergence) =
        first_divergence(&retail_trace, &native_trace).expect("semantic traces must be valid")
    {
        panic!("retail straight-motion trace diverged: {divergence}");
    }
}
