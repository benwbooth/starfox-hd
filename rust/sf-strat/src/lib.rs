//! Object strategies (enemy/player/boss AI), the istrat table, and
//! collision-facing behavior.
//!
//! Ports (C oracle): `src/strat/strat_*.c`, `src/strat/strat_table.c`,
//! `src/game/obj.c` strategy dispatch, `src/game/coldet.c`.
//! Strategy registration must reproduce the ISTRATS.ASM def_Istrat index
//! order and the synthetic 0x02/0x03/0x05/0x06 address map.
