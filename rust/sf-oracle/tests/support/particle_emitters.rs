//! Particle producers must publish the right flag byte as well as draw data.
use super::{Source, OBJECT, WRAM};
use sf_game::Game;
use sf_oracle::SnesBus;
use sf_strat::enemy_a::{
    bigparticleexplode_istrat, circ2particleexplode_istrat, fastparticleexplode_istrat,
    particleexplode_istrat, particlefire_istrat, particlefiredown_istrat, particlepollen_istrat,
};

#[test]
fn all_particle_initializers_match_original_inherited_flags_and_payload() {
    let source = Source::load();
    for (name, run, continuation) in [
        (
            "PARTICLEEXPLODE_ISTRAT",
            particleexplode_istrat as fn(&mut Game, u16),
            "PARTICLEEXPLODE_STRAT",
        ),
        (
            "FASTPARTICLEEXPLODE_ISTRAT",
            fastparticleexplode_istrat,
            "FASTPARTICLEEXPLODE_STRAT",
        ),
        (
            "BIGPARTICLEEXPLODE_ISTRAT",
            bigparticleexplode_istrat,
            "BIGPARTICLEEXPLODE_STRAT",
        ),
        (
            "CIRC2PARTICLEEXPLODE_ISTRAT",
            circ2particleexplode_istrat,
            "CIRC2PARTICLEEXPLODE_STRAT",
        ),
        (
            "PARTICLEFIRE_ISTRAT",
            particlefire_istrat,
            "PARTICLEFIRE_STRAT",
        ),
        (
            "PARTICLEFIREDOWN_ISTRAT",
            particlefiredown_istrat,
            "PARTICLEFIRE_STRAT",
        ),
        (
            "PARTICLEPOLLEN_ISTRAT",
            particlepollen_istrat,
            "PARTICLEPOLLEN_STRAT",
        ),
    ] {
        for inherited in 0..=u8::MAX {
            let mut game = Game::new();
            let id = game.objs.alloc().unwrap();
            let object = &mut game.objs.aliens[id as usize];
            object.flags = inherited;
            object.sflags = inherited;
            object.sflags2 = inherited ^ 0xA5;
            object.sflags3 = inherited.rotate_left(1);
            object.sflags4 = inherited.rotate_right(1);
            object.sbyte1 = inherited;
            object.sbyte2 = inherited;
            object.sbyte3 = inherited;
            object.count = inherited;
            let object = *object;
            let mut bus = SnesBus::new(source.rom.clone());
            source.seed(&mut bus, &object, &game);
            source.byte(&mut bus, OBJECT, "AL_FLAGS", inherited);
            source.run(&mut bus, name);
            run(&mut game, id);
            let actual = &game.objs.aliens[id as usize];
            source.compare(&mut bus, actual, &format!("{name} inherited={inherited}"));
            assert_eq!(
                actual.flags,
                bus.read8(WRAM | (OBJECT + source.symbol("AL_FLAGS")))
            );
            assert!(actual.expstratptr.is_some());
            let address = WRAM | (OBJECT + source.symbol("ALX_EXPSTRATPTR"));
            let source_continuation = u32::from(bus.read8(address))
                | (u32::from(bus.read8(address + 1)) << 8)
                | (u32::from(bus.read8(address + 2)) << 16);
            assert_eq!(source_continuation, source.symbol(continuation));
        }
    }
}
