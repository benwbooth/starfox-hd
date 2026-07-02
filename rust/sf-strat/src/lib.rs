//! Object strategies (player/ground/enemy/boss AI) and the istrat table.
//!
//! Ports (C oracle): `src/strat/strat_common.c`, `strat_player.c`,
//! `strat_ground.c`, `strat_enemy.c`, `strat_boss2.c`, `strat_boss_sea.c`,
//! `strat_boss8.c`, `strat_table.c`. Strategies run against `sf-game`'s
//! `Game`/`Hooks` context and register through its strategy registry with
//! the literal ISTRATS.ASM def_Istrat indices plus the synthetic
//! 0x03/0x05/0x06 address map.
//!
//! Lane layout (one porting lane per module group; lanes do not edit this
//! file or each other's modules):
//! - `common`, `player`            -> strat_common.c, strat_player.c
//! - `ground`, `enemy_a`           -> strat_ground.c, strat_enemy.c (front)
//! - `enemy_b`, `bosses`           -> strat_enemy.c (back), strat_boss*.c
//! - `table`                       -> strat_table.c registration

pub mod bosses;
pub mod common;
pub mod enemy_a;
pub mod enemy_b;
pub mod ground;
pub mod player;
pub mod table;
