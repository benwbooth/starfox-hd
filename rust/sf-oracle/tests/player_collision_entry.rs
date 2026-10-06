//! Original body/wing collision entry points against live native proxy services.
//! The source routines, attachment tails and damage code execute unmodified.

use sf_game::{alien::Alien, Game};
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::{
    common::{sv, StratRam},
    player::{pcbox_attach, pcolb_strat, pcollw_istrat, pcollw_strat, pcolrw_istrat, pcolrw_strat},
};
use std::collections::HashMap;

const ACTOR: u32 = 256;
const PLAYER: u32 = 1280;
const BODY: u32 = 1536;
const OTHER: u32 = 1792;
const EFFECT: u32 = 2048;
const WRAM: u32 = 0x7E_0000;

struct Source {
    rom: Vec<u8>,
    names: HashMap<String, u32>,
}

impl Source {
    fn load() -> Self {
        Self {
            rom: load_built_rom().expect("collision oracle needs source-built ROM"),
            names: load_symbols(),
        }
    }
    fn byte(&self, bus: &mut SnesBus, base: u32, field: &str, value: u8) {
        bus.write8(WRAM | (base + self.names[field]), value);
    }
    fn word(&self, bus: &mut SnesBus, base: u32, field: &str, value: i16) {
        let address = WRAM | (base + self.names[field]);
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            bus.write8(address + offset as u32, byte);
        }
    }
    fn pointer(&self, bus: &mut SnesBus, base: u32, field: &str, target: &str) {
        let address = WRAM | (base + self.names[field]);
        for offset in 0..3 {
            bus.write8(address + offset, (self.names[target] >> (offset * 8)) as u8);
        }
    }
    fn read_word(&self, bus: &mut SnesBus, base: u32, field: &str) -> i16 {
        let address = WRAM | (base + self.names[field]);
        i16::from_le_bytes([bus.read8(address), bus.read8(address + 1)])
    }
    fn actor(&self, bus: &mut SnesBus, base: u32, actor: &Alien) {
        for (field, value) in [
            ("AL_HP", actor.hp),
            ("AL_AP", actor.ap),
            ("AL_COLLCOUNT", actor.collcount),
            ("AL_SFLAGS", actor.sflags),
            ("AL_SFLAGS2", actor.sflags2),
            ("AL_ROTX", actor.rotx),
            ("AL_ROTY", actor.roty),
            ("AL_ROTZ", actor.rotz),
        ] {
            self.byte(bus, base, field, value);
        }
        for (field, value) in [
            ("AL_WORLDX", actor.worldx),
            ("AL_WORLDY", actor.worldy),
            ("AL_WORLDZ", actor.worldz),
        ] {
            self.word(bus, base, field, value);
        }
    }
    fn compare_actor(&self, bus: &mut SnesBus, base: u32, actor: &Alien, context: &str) {
        for (field, value) in [("AL_HP", actor.hp), ("AL_COLLCOUNT", actor.collcount)] {
            assert_eq!(
                value,
                bus.read8(WRAM | (base + self.names[field])),
                "{context}: {field}"
            );
        }
        for (field, value) in [
            ("AL_WORLDX", actor.worldx),
            ("AL_WORLDY", actor.worldy),
            ("AL_WORLDZ", actor.worldz),
        ] {
            assert_eq!(
                value,
                self.read_word(bus, base, field),
                "{context}: {field}"
            );
        }
    }
}

#[test]
fn body_and_wing_power_scaling_match_original_for_every_attack_byte() {
    let source = Source::load();
    for part in 0..3 {
        for wire in [false, true] {
            for ap in 0..=255u8 {
                let mut game = Game::new();
                let player = game.objs.alloc().unwrap();
                game.vars.player_object = player as i16;
                assert!(pcbox_attach(&mut game, player));
                let body = game.coldet.pcbox.body.unwrap();
                let actor = match part {
                    0 => body,
                    1 => game.coldet.pcbox.lwing.unwrap(),
                    _ => game.coldet.pcbox.rwing.unwrap(),
                };
                let other = game.objs.alloc().unwrap();
                let pl = &mut game.objs.aliens[player as usize];
                [pl.worldx, pl.worldy, pl.worldz] = [100, 200, 300];
                // Sign-rich power bytes exercise ASRA's carry input and the
                // wrapped BPL damage clamp, including a wrapped positive result.
                game.objs.aliens[other as usize].ap = ap;
                game.objs.aliens[body as usize].hp = 20;
                game.objs.aliens[body as usize].collcount = 1;
                game.objs.aliens[actor as usize].collcount = 1;
                game.objs.aliens[actor as usize].collobjptr = other;
                game.vars.set_sv_i16(sv::PCOLLOBJ_B, other as i16);
                game.vars.pshipflags2 = if wire { 6 } else { 4 }; // suppress spark RNG, not damage
                let mut bus = SnesBus::new(source.rom.clone());
                let source_body = if part == 0 { ACTOR } else { BODY };
                for (address, id) in [
                    (PLAYER, player),
                    (source_body, body),
                    (ACTOR, actor),
                    (OTHER, other),
                ] {
                    source.actor(&mut bus, address, &game.objs.aliens[id as usize]);
                }
                source.word(&mut bus, 0, "PLAYPT", PLAYER as i16);
                source.word(&mut bus, 0, "PCBOXOBJ_B", source_body as i16);
                source.word(&mut bus, 0, "PCOLLOBJ_B", OTHER as i16);
                source.word(&mut bus, ACTOR, "AL_COLLOBJPTR", OTHER as i16);
                source.byte(&mut bus, 0, "PSHIPFLAGS2", game.vars.pshipflags2);
                let (name, ordinary, run): (_, _, fn(&mut Game, u16)) = match part {
                    0 => ("PCOLB_STRAT", "PBODY_STRAT", pcolb_strat),
                    1 => ("PCOLLW_STRAT", "PLWING_STRAT", pcollw_strat),
                    _ => ("PCOLRW_STRAT", "PRWING_STRAT", pcolrw_strat),
                };
                source.pointer(&mut bus, ACTOR, "AL_STRATPTR", ordinary);
                let exit = call(
                    &mut bus,
                    source.names[name],
                    &Entry {
                        x: ACTOR as u16,
                        p: 0x20,
                        dbr: 0x7E,
                        ..Default::default()
                    },
                );
                assert!(
                    exit.returned,
                    "{name} must execute its complete original tail"
                );
                run(&mut game, actor);
                let context = format!("{name} wire={wire} ap={ap}");
                source.compare_actor(&mut bus, ACTOR, &game.objs.aliens[actor as usize], &context);
                source.compare_actor(
                    &mut bus,
                    source_body,
                    &game.objs.aliens[body as usize],
                    &context,
                );
            }
        }
    }
}

#[test]
fn wing_entries_and_broken_scrapes_match_original_wall_and_object_branches() {
    let source = Source::load();
    for right in [false, true] {
        for broken in [false, true] {
            for wall in [false, true] {
                for initial in [false, true] {
                    for wire in [false, true] {
                        for cooldown in [0, 1, 2, 255] {
                            for roll in [0, 128] {
                                let mut game = Game::new();
                                let player = game.objs.alloc().unwrap();
                                game.vars.player_object = player as i16;
                                assert!(pcbox_attach(&mut game, player));
                                let wing = if right {
                                    game.coldet.pcbox.rwing
                                } else {
                                    game.coldet.pcbox.lwing
                                }
                                .unwrap();
                                let body = game.coldet.pcbox.body.unwrap();
                                let other = game.objs.alloc().unwrap();
                                let effect = game.objs.alloc().unwrap();
                                let pl = &mut game.objs.aliens[player as usize];
                                [pl.worldx, pl.worldy, pl.worldz] = [32760, 200, 300];
                                pl.rotz = roll;
                                game.objs.aliens[wing as usize].collobjptr =
                                    if wall { 0 } else { other };
                                game.objs.aliens[wing as usize].sword1 = effect as i16;
                                game.objs.aliens[wing as usize].collcount = 1;
                                game.objs.aliens[body as usize].collcount = cooldown;
                                game.objs.aliens[body as usize].hp = 40;
                                game.objs.aliens[other as usize].ap = 8;
                                game.vars.pshipflags = if broken {
                                    if right {
                                        16
                                    } else {
                                        8
                                    }
                                } else {
                                    0
                                };
                                game.vars.pshipflags2 = if wire { 6 } else { 4 };
                                game.vars.set_sv_i16(sv::PLROTX, 32760);
                                game.vars.set_sv_i16(sv::PLAYER_ZSHAKE, 1234);
                                let mut bus = SnesBus::new(source.rom.clone());
                                for (address, id) in [
                                    (ACTOR, wing),
                                    (PLAYER, player),
                                    (BODY, body),
                                    (OTHER, other),
                                    (EFFECT, effect),
                                ] {
                                    source.actor(&mut bus, address, &game.objs.aliens[id as usize]);
                                }
                                source.word(&mut bus, 0, "PLAYPT", PLAYER as i16);
                                source.word(&mut bus, 0, "PCBOXOBJ_B", BODY as i16);
                                source.word(
                                    &mut bus,
                                    ACTOR,
                                    "AL_COLLOBJPTR",
                                    if wall { 0 } else { OTHER as i16 },
                                );
                                source.word(&mut bus, ACTOR, "AL_SWORD1", EFFECT as i16);
                                source.word(&mut bus, 0, "PLROTX", 32760);
                                source.word(&mut bus, 0, "PLAYER_ZSHAKE", 1234);
                                source.byte(&mut bus, 0, "PSHIPFLAGS", game.vars.pshipflags);
                                source.byte(&mut bus, 0, "PSHIPFLAGS2", game.vars.pshipflags2);
                                source.pointer(
                                    &mut bus,
                                    ACTOR,
                                    "AL_STRATPTR",
                                    if right {
                                        "PRWING_STRAT"
                                    } else {
                                        "PLWING_STRAT"
                                    },
                                );
                                let (name, run): (_, fn(&mut Game, u16)) = match (right, initial) {
                                    (false, false) => ("PCOLLW_STRAT", pcollw_strat),
                                    (false, true) => ("PCOLLW_ISTRAT", pcollw_istrat),
                                    (true, false) => ("PCOLRW_STRAT", pcolrw_strat),
                                    (true, true) => ("PCOLRW_ISTRAT", pcolrw_istrat),
                                };
                                // Source allocation has no free slot; native
                                // retains the same failure branch without fake constructors.
                                while game.objs.alloc().is_some() {}
                                assert!(
                                    call(
                                        &mut bus,
                                        source.names[name],
                                        &Entry {
                                            x: ACTOR as u16,
                                            p: 0x20,
                                            dbr: 0x7E,
                                            ..Default::default()
                                        }
                                    )
                                    .returned
                                );
                                run(&mut game, wing);
                                let context = format!("{name} broken={broken} wall={wall} wire={wire} cooldown={cooldown} roll={roll}");
                                for (address, id) in [
                                    (ACTOR, wing),
                                    (PLAYER, player),
                                    (BODY, body),
                                    (EFFECT, effect),
                                ] {
                                    source.compare_actor(
                                        &mut bus,
                                        address,
                                        &game.objs.aliens[id as usize],
                                        &context,
                                    );
                                }
                                assert_eq!(
                                    game.vars.sv_i16(sv::PLROTX),
                                    source.read_word(&mut bus, 0, "PLROTX"),
                                    "{context}: pitch"
                                );
                                assert_eq!(
                                    game.vars.sv_i16(sv::PLAYER_ZSHAKE),
                                    source.read_word(&mut bus, 0, "PLAYER_ZSHAKE"),
                                    "{context}: shake"
                                );
                                assert_eq!(
                                    game.vars.pshipflags,
                                    bus.read8(WRAM | source.names["PSHIPFLAGS"]),
                                    "{context}: ship flags"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
