//! Original instructions own the expected flag bytes, including inherited bits.
use super::{Source, OBJECT, TARGET, WRAM};
use sf_game::Game;
use sf_oracle::SnesBus;
use sf_strat::bosses::{chicken_wings_istrat, seamon_strat};
use sf_strat::enemy_a::ship1aexp_istrat;
use sf_strat::theend::{theend_flyaway_istrat, theend_flyaway_strat};

#[test]
fn mixed_flag_initializers_preserve_all_four_original_bytes() {
    let source = Source::load();
    for (name, run) in [
        ("WINGS_ISTRAT", chicken_wings_istrat as fn(&mut Game, u16)),
        ("THEEND_FLYAWAY_ISTRAT", theend_flyaway_istrat),
        ("SHIP1AEXP_ISTRAT", ship1aexp_istrat),
    ] {
        for inherited in 0..=u8::MAX {
            let mut game = Game::new();
            let id = game.objs.alloc().unwrap();
            let continuation = game.world.register_strategy(theend_flyaway_strat);
            game.vars.pviewvelz = -27;
            let object = &mut game.objs.aliens[id as usize];
            object.sflags = inherited;
            object.sflags2 = inherited ^ 0xA5;
            object.sflags3 = inherited.rotate_left(1);
            object.sflags4 = inherited.rotate_right(1);
            object.type_ = inherited;
            object.stratptr = Some(continuation);
            object.hp = inherited;
            object.ap = inherited;
            [object.rotx, object.roty, object.rotz] = [253, 254, 255];
            [object.worldx, object.worldy, object.worldz] = [32760, -32760, -32760];
            let object = *object;
            let mut bus = SnesBus::new(source.rom.clone());
            source.seed(&mut bus, &object, &game);
            // Exercise the real tail dispatch, not a patched return or stub.
            let address = WRAM | (OBJECT + source.symbol("AL_STRATPTR"));
            let continuation = source.symbol("THEEND_FLYAWAY_STRAT");
            for offset in 0..3 {
                bus.write8(address + offset, (continuation >> (offset * 8)) as u8);
            }
            source.run(&mut bus, name);
            run(&mut game, id);
            let actual = &game.objs.aliens[id as usize];
            source.compare(&mut bus, actual, &format!("{name} inherited={inherited}"));
            if name == "WINGS_ISTRAT" {
                assert_eq!(
                    actual.animframe,
                    bus.read8(WRAM | (OBJECT + source.symbol("ALX_ANIMFRAME")))
                );
            }
        }
    }
}

#[test]
fn seamon_swim_shape_tests_the_complete_original_second_flag_byte() {
    let source = Source::load();
    for flags in 0..=u8::MAX {
        let mut game = Game::new();
        let player = game.objs.alloc().unwrap();
        game.vars.internal_playpt = player as i16;
        let id = game.objs.alloc().unwrap();
        let object = &mut game.objs.aliens[id as usize];
        object.shape = 31;
        object.sflags = flags;
        object.sflags2 = flags;
        object.sflags3 = flags;
        object.sflags4 = flags;
        object.sbyte1 = 1;
        object.sbyte2 = flags;
        object.sbyte4 = 2; // Do not enter the unrelated jump/splash branch.
        let object = *object;
        let mut bus = SnesBus::new(source.rom.clone());
        source.seed(&mut bus, &object, &game);
        source.byte(&mut bus, OBJECT, "AL_SBYTE4", object.sbyte4);
        source.word(&mut bus, 0, "PLAYPT", TARGET as i16);
        source.run(&mut bus, "SEAMON_STRAT");
        seamon_strat(&mut game, id);
        let mut translated = game.objs.aliens[id as usize];
        // Only decode source shape identities at the oracle boundary.
        assert!(matches!(translated.shape, 31 | 258));
        let address = WRAM | (OBJECT + source.symbol("AL_SHAPE"));
        let header = u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
        assert_eq!(translated.shape, sf_core::shape::resolve_shape_word(header));
        translated.shape = header;
        source.compare(
            &mut bus,
            &translated,
            &format!("SEAMON_STRAT flags={flags}"),
        );
        assert_eq!(
            translated.sbyte4,
            bus.read8(WRAM | (OBJECT + source.symbol("AL_SBYTE4")))
        );
    }
}
