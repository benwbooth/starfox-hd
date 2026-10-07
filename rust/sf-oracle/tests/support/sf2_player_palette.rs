//! Execute the original primary-player selector and all nested palette loops.
//! Full color words and source flag ownership are independently retained.
use super::{actor, rom, Source, OTHER, OWNER, WRAM};
use sf2_game::path_player_control::PlayerTargetControl;
use sf2_game::player_action::ScenePalette;
use sf2_game::player_consumable::{PlayerConsumableControl, TriggeredUseBlockers};
use sf2_game::player_palette::{self, PlayerPaletteControl};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{ObjectId, ObjectStore, RandomState};

const PRIMARY_SLOT: u32 = 64;
const SECONDARY_SLOT: u32 = 600;

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    primary: ObjectId,
    secondary: ObjectId,
    untouched_bits: u8,
}

impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let primary = actor(&mut objects);
        let secondary = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        for owner in [primary, secondary] {
            world
                .bind_player(
                    &objects,
                    owner,
                    PlayerPathRecords {
                        palette_effects: Some(Default::default()),
                        consumable: Some(Default::default()),
                        target_control: Some(Default::default()),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        world.primary_player = Some(primary);
        world.secondary_player = Some(secondary);
        world.palette_refresh_requested = Some(false);
        world.palette = Some(ScenePalette {
            colors: [0; 128],
            saved_colors: [0; 128],
        });
        Self {
            objects,
            world,
            primary,
            secondary,
            untouched_bits: 7,
        }
    }

    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.primary).unwrap()
    }

    fn configure(&mut self, flags: u8, delay: u8, clock: u8) {
        self.records().palette_effects = Some(PlayerPaletteControl::from_control(flags));
        self.records().consumable = Some(PlayerConsumableControl {
            projectile_blockers: TriggeredUseBlockers::from_control(flags),
            ..Default::default()
        });
        self.records().target_control = Some(PlayerTargetControl {
            transition_delay: delay,
            ..Default::default()
        });
        self.untouched_bits = flags & 7;
        self.world.strategy_clock = u16::from(clock);
        self.world.palette_refresh_requested = Some(false);
    }

    fn seed(&self, source: &mut Source) {
        source.bus.write16(0x12C3, OWNER);
        source
            .bus
            .write16(u32::from(OWNER) + 0x2B, PRIMARY_SLOT as u16);
        source
            .bus
            .write16(u32::from(OTHER) + 0x2B, SECONDARY_SLOT as u16);
        for (slot, owner) in [
            (PRIMARY_SLOT, self.primary),
            (SECONDARY_SLOT, self.secondary),
        ] {
            let r = self.world.player(&self.objects, owner).unwrap();
            source.bus.write8(
                WRAM + slot + 0x6BE9,
                r.palette_effects.unwrap().bits()
                    | r.consumable.unwrap().projectile_blockers.bits()
                    | self.untouched_bits,
            );
            source.bus.write8(
                WRAM + slot + 0x6BEA,
                r.target_control.unwrap().transition_delay,
            );
        }
        source.bus.write8(0xC4, self.world.strategy_clock as u8);
        source.bus.write8(
            0x1E58,
            0x37 | u8::from(self.world.palette_refresh_requested.unwrap()) * 0x80,
        );
        let palette = self.world.palette.as_ref().unwrap();
        for (index, (&color, &saved)) in
            palette.colors.iter().zip(&palette.saved_colors).enumerate()
        {
            source.bus.write16(WRAM + 0xEFE5 + index as u32 * 2, color);
            source.bus.write16(WRAM + 0xF2E5 + index as u32 * 2, saved);
        }
    }

    fn step(&mut self, source: &mut Source) {
        // Enter as the secondary actor: the original must still select primary.
        source.run(0x07EA67, None, 0, OTHER, true);
        player_palette::advance_primary(&self.objects, &mut self.world).unwrap();
        for (slot, owner) in [
            (PRIMARY_SLOT, self.primary),
            (SECONDARY_SLOT, self.secondary),
        ] {
            let r = self.world.player(&self.objects, owner).unwrap();
            assert_eq!(
                source.bus.read8(WRAM + slot + 0x6BE9),
                r.palette_effects.unwrap().bits()
                    | r.consumable.unwrap().projectile_blockers.bits()
                    | self.untouched_bits,
                "palette flags slot={slot}"
            );
            assert_eq!(
                source.bus.read8(WRAM + slot + 0x6BEA),
                r.target_control.unwrap().transition_delay,
                "palette delay slot={slot}"
            );
        }
        assert_eq!(
            source.bus.read8(0x1E58),
            0x37 | u8::from(self.world.palette_refresh_requested.unwrap()) * 0x80
        );
        let palette = self.world.palette.as_ref().unwrap();
        for (index, (&color, &saved)) in
            palette.colors.iter().zip(&palette.saved_colors).enumerate()
        {
            assert_eq!(
                source.bus.read16(WRAM + 0xEFE5 + index as u32 * 2),
                color,
                "live color {index}"
            );
            assert_eq!(
                source.bus.read16(WRAM + 0xF2E5 + index as u32 * 2),
                saved,
                "saved color {index}"
            );
        }
    }
}

#[test]
fn original_primary_palette_all_flag_bytes_timer_boundaries_and_clock_phases() {
    let mut source = Source::new(&rom(), 0xA7);
    let mut f = Fixture::new();
    for flags in 0..=255 {
        for delay in [0, 1, 2, 3, 255, flags] {
            for clock in [0, 7, 8, 16, 24, 31, 32, 255] {
                f.configure(flags, delay, clock);
                let palette = f.world.palette.as_mut().unwrap();
                palette.colors = std::array::from_fn(|index| {
                    (index as u16)
                        .wrapping_mul(991)
                        .wrapping_add(u16::from(flags))
                });
                palette.saved_colors = std::array::from_fn(|index| {
                    (index as u16)
                        .wrapping_mul(37)
                        .wrapping_add(u16::from(delay))
                });
                f.seed(&mut source);
                for _ in 0..3 {
                    f.step(&mut source);
                }
            }
        }
    }
}

#[test]
fn original_palette_all_color_words_flash_pulse_restore_and_full_word_completion() {
    let mut source = Source::new(&rom(), 0x59);
    let mut f = Fixture::new();
    for flags in [8, 0x20, 0x10] {
        // Rotate each block so transparent indices are also exercised as
        // ordinary colors, and both halves participate in every component.
        for block in 0..1024u16 {
            for rotation in [0, 1] {
                f.configure(flags, 0, 0);
                let palette = f.world.palette.as_mut().unwrap();
                palette.colors =
                    std::array::from_fn(|index| block * 64 + ((index + rotation) % 64) as u16);
                palette.saved_colors =
                    std::array::from_fn(|index| palette.colors[index].rotate_left(5));
                f.seed(&mut source);
                f.step(&mut source);
            }
        }
    }
    for flags in [0x10, 0x50, 0xD0, 0x70, 0xF0] {
        for saved in [0, 0x7FFF, 0x8000, 0xFFFF] {
            f.configure(flags, 0, 8);
            let palette = f.world.palette.as_mut().unwrap();
            palette.saved_colors.fill(saved);
            palette.colors.fill(saved ^ 1);
            f.seed(&mut source);
            for _ in 0..4 {
                f.step(&mut source);
            }
        }
    }
}
