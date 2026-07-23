//! MAP_ID_TRAINING — training mode (TRAINING.ASM).
//!
//! C oracle: `src/map/levels.c` `build_training_slice()` +
//! `register_training_inline_callbacks()`.

use super::rc::*;
use super::Route2Level;
use crate::builder::MapBuilder;

/// C `build_training_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.mapwait(2000);

    // ELSEIF block (actual training content):
    // Line 34: pathobj zaco_5,trn_ck
    b.pathobj(0, 0, 0, 3000, SH_ZACO_5, PATH_ID_TRN_CK, 10, 10);

    // Lines 35-36: mapobj BU_8 and BU_1
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_8, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    // Line 38: pilon ground obstacle
    b.mapobj(0, 0, 0x0500, 5000, SH_PILON_PROXY, STRATEGY_GROUNDPILON);

    // Lines 40-45: more building objects
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0x2000, -0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);

    // Line 46 = .et label (eguchifly_goto loop target)
    b.label("training.et");

    // Lines 49-55: training rings and buildings
    b.pathobj(0, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_8, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    b.pathobj(
        0,
        0x0200,
        -150,
        5000,
        SH_NULLSHAPE,
        PATH_ID_TRN_RING2,
        10,
        10,
    );
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_BU_2, IS_HARD180YR);

    b.pathobj(0, 0, -200, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    b.pathobj(
        0,
        -0x0200,
        -150,
        5000,
        SH_NULLSHAPE,
        PATH_ID_TRN_RING2,
        10,
        10,
    );
    b.mapobj(0, 0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0x2000, -0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);

    // Lines 66-76: more rings and buildings
    b.pathobj(0, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    b.mapobj(0, 0x1200, 0, 5000, SH_PILLAR3, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_PILLAR3, IS_HARD180YR);

    b.pathobj(
        0,
        0x0200,
        -200,
        5000,
        SH_NULLSHAPE,
        PATH_ID_TRN_RING2,
        10,
        10,
    );
    b.mapobj(0, 0x1200, 0, 5000, SH_ROBOT_0, IS_HARD180YR);
    b.mapobj(0x1200, -0x1200, 0, 5000, SH_ROBOT_0, IS_HARD180YR);

    b.pathobj(
        0,
        -0x0330,
        -100,
        5000,
        SH_NULLSHAPE,
        PATH_ID_TRN_RING,
        10,
        10,
    );
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0x2000, -0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);

    // Lines 78-87: solo rings
    b.pathobj(1000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);
    b.mapobj(0x0800, 0, 0, 5000, SH_BU_7, IS_HARD180YR);

    b.pathobj(1000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    b.pathobj(
        1000,
        0x0100,
        -100,
        5000,
        SH_NULLSHAPE,
        PATH_ID_TRN_RING2,
        10,
        10,
    );
    b.pathobj(
        800,
        -0x0200,
        -300,
        5000,
        SH_NULLSHAPE,
        PATH_ID_TRN_RING,
        10,
        10,
    );
    b.pathobj(
        800,
        -0x0100,
        -100,
        5000,
        SH_NULLSHAPE,
        PATH_ID_TRN_RING2,
        10,
        10,
    );
    b.pathobj(800, 0, -300, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    b.pathobj(2000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);

    // Line 92: base_1 object
    b.mapobj(0x0300, 0, 0, 5000, SH_BASE_1, IS_BASE_1);

    // Line 94: long ring stretch
    b.pathobj(8000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);

    // Line 95: eguchifly_goto .et.  The inline 65816 block reads the shared
    // 16-bit `eword1` ring counter: values below 15 skip the following GOTO,
    // while 15 or more execute it and repeat the course at `.et`.
    let training_eguchifly_loop_ptr = b.mapcode65816_inline();
    b.mapgoto("training.et");
    b.label("training.eguchifly_continue");

    // Lines 97-99: friend ship pathobjs
    b.pathobj(
        0,
        0,
        -570,
        -100,
        SH_FRIENDSHIP_4,
        PATH_ID_HENTAI_FAL,
        10,
        10,
    );
    b.pathobj(
        0,
        100,
        -470,
        -100,
        SH_FRIENDSHIP_4,
        PATH_ID_HENTAI_FRO,
        10,
        10,
    );
    b.pathobj(
        1000,
        -100,
        -470,
        -100,
        SH_FRIENDSHIP_4,
        PATH_ID_HENTAI_RAB,
        10,
        10,
    );

    // Line 101: mapmsg 123
    b.sendmsg(123);

    // Lines 102-108: .etlop — building loop
    b.label("training.etlop");
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_8, IS_HARD180YR);
    b.mapobj(0x4200, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0x4200, -0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);
    b.maploop("training.etlop", 4);

    // Lines 109-114: pilon obstacles
    b.mapobj(0, -0x0200, -70, 5000, SH_PILON_PROXY, STRATEGY_GROUNDPILON);
    b.mapobj(0, 0, -70, 5000, SH_PILON_PROXY, STRATEGY_GROUNDPILON);
    b.mapobj(
        0x6000,
        0x0200,
        -70,
        5000,
        SH_PILON_PROXY,
        STRATEGY_GROUNDPILON,
    );
    b.mapobj(0, 0, -70, 5000, SH_PILON_PROXY, STRATEGY_GROUNDPILON);
    b.mapobj(0, 0, -140, 5000, SH_PILON_PROXY, STRATEGY_GROUNDPILON);
    b.mapobj(0x8000, 0, -210, 5000, SH_PILON_PROXY, STRATEGY_GROUNDPILON);

    // Line 116: mapgoto .et — loop back to main section
    b.mapgoto("training.et");

    b.resolve();
    let (data, labels) = b.finish();

    // C `register_training_inline_callbacks()`.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![(training_eguchifly_loop_ptr, "training_eguchifly_check")],
    )
}
