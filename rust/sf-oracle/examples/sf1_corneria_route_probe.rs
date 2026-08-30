//! Search deterministic controller-only flight patterns for a Corneria trace.
//!
//! This is a diagnostic, not an accuracy oracle: it never changes game state.
//! A successful pattern can therefore be replayed unchanged against the retail
//! cartridge and the native port by the paired trace harness.

mod support;

use sf_core::pad;
use sf_game::shell::{GameState, GameplayEntryPhase, Shell};
use sf_game::vars::{GF_PLAYERDEAD, GF_PLAYERDYING, PSF2_PLAYERHP0};
use sf_oracle::sf1_input::{
    PilotAction, CORNERIA_ATTACK_CARRIER_SHAPE, CORNERIA_INPUT_SEGMENT_FRAMES,
};
use std::cmp::Ordering;

const SEARCH_TICKS: u32 = 8_000;
const SEARCH_SEGMENTS: usize = 120;
const SEARCH_GENERATIONS: u32 = 160;
const SEARCH_POPULATION: usize = 48;
const SEARCH_WORKERS: usize = 16;
const MUTATION_LOOKBACK_SEGMENTS: usize = 14;
const MUTATION_LOOKAHEAD_SEGMENTS: usize = 5;
const MAX_MUTATIONS: usize = 6;

#[derive(Clone, Copy, Debug)]
enum FlightPattern {
    Center,
    HorizontalSweep,
    VerticalSweep,
    Box,
    UpperSweep,
    LowerSweep,
    Circle,
    RollSweep,
    BoxRoll,
    BoxBomb,
    BoxRollBomb,
    LissajousA,
    LissajousB,
    LissajousC,
    LissajousD,
    IrregularA,
    IrregularB,
    IrregularC,
}

impl FlightPattern {
    const ALL: [Self; 18] = [
        Self::Center,
        Self::HorizontalSweep,
        Self::VerticalSweep,
        Self::Box,
        Self::UpperSweep,
        Self::LowerSweep,
        Self::Circle,
        Self::RollSweep,
        Self::BoxRoll,
        Self::BoxBomb,
        Self::BoxRollBomb,
        Self::LissajousA,
        Self::LissajousB,
        Self::LissajousC,
        Self::LissajousD,
        Self::IrregularA,
        Self::IrregularB,
        Self::IrregularC,
    ];

    fn steering(self, game_frame: u16) -> u16 {
        let frame = u32::from(game_frame);
        match self {
            Self::Center => 0,
            Self::HorizontalSweep => {
                if frame / 120 & 1 == 0 {
                    pad::LEFT
                } else {
                    pad::RIGHT
                }
            }
            Self::VerticalSweep => {
                if frame / 120 & 1 == 0 {
                    pad::UP
                } else {
                    pad::DOWN
                }
            }
            Self::Box | Self::BoxRoll | Self::BoxBomb | Self::BoxRollBomb => {
                let roll = if matches!(self, Self::BoxRoll | Self::BoxRollBomb)
                    && (frame % 32 == 0 || frame % 32 == 3)
                {
                    if frame / 32 & 1 == 0 {
                        pad::TLEFT
                    } else {
                        pad::TRIGHT
                    }
                } else {
                    0
                };
                let bomb = if matches!(self, Self::BoxBomb | Self::BoxRollBomb)
                    && matches!(game_frame, 690 | 900 | 1_250)
                {
                    pad::A
                } else {
                    0
                };
                (match frame / 90 & 3 {
                    0 => pad::UP | pad::LEFT,
                    1 => pad::UP | pad::RIGHT,
                    2 => pad::DOWN | pad::RIGHT,
                    _ => pad::DOWN | pad::LEFT,
                }) | roll
                    | bomb
            }
            Self::UpperSweep => {
                pad::UP
                    | if frame / 120 & 1 == 0 {
                        pad::LEFT
                    } else {
                        pad::RIGHT
                    }
            }
            Self::LowerSweep => {
                pad::DOWN
                    | if frame / 120 & 1 == 0 {
                        pad::LEFT
                    } else {
                        pad::RIGHT
                    }
            }
            Self::Circle => match frame / 60 & 3 {
                0 => pad::UP,
                1 => pad::RIGHT,
                2 => pad::DOWN,
                _ => pad::LEFT,
            },
            Self::RollSweep => {
                let steer = if frame / 120 & 1 == 0 {
                    pad::LEFT
                } else {
                    pad::RIGHT
                };
                let roll = if frame % 120 == 0 || frame % 120 == 3 {
                    if steer == pad::LEFT {
                        pad::TLEFT
                    } else {
                        pad::TRIGHT
                    }
                } else {
                    0
                };
                steer | roll
            }
            Self::LissajousA | Self::LissajousB | Self::LissajousC | Self::LissajousD => {
                let (horizontal_period, vertical_period) = match self {
                    Self::LissajousA => (73, 109),
                    Self::LissajousB => (97, 149),
                    Self::LissajousC => (131, 181),
                    Self::LissajousD => (47, 83),
                    _ => unreachable!(),
                };
                let horizontal = if frame / horizontal_period & 1 == 0 {
                    pad::LEFT
                } else {
                    pad::RIGHT
                };
                let vertical = if frame / vertical_period & 1 == 0 {
                    pad::UP
                } else {
                    pad::DOWN
                };
                horizontal | vertical
            }
            Self::IrregularA | Self::IrregularB | Self::IrregularC => {
                let seed = match self {
                    Self::IrregularA => 17u32,
                    Self::IrregularB => 73u32,
                    Self::IrregularC => 151u32,
                    _ => unreachable!(),
                };
                let mut value = (frame / 47).wrapping_add(seed);
                value ^= value << 13;
                value ^= value >> 17;
                value ^= value << 5;
                match value % 9 {
                    0 => 0,
                    1 => pad::UP,
                    2 => pad::DOWN,
                    3 => pad::LEFT,
                    4 => pad::RIGHT,
                    5 => pad::UP | pad::LEFT,
                    6 => pad::UP | pad::RIGHT,
                    7 => pad::DOWN | pad::LEFT,
                    _ => pad::DOWN | pad::RIGHT,
                }
            }
        }
    }
}

#[derive(Debug)]
struct Outcome {
    pattern: FlightPattern,
    firing: bool,
    level_updates: u32,
    level_frame: u16,
    map: u32,
    map_pointer: u16,
    player_position: [i16; 3],
    body_health: Option<u8>,
    boss_seen: bool,
    died: bool,
    state: GameState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fitness {
    boss_seen: bool,
    survived: bool,
    level_frame: u16,
    body_health: u8,
    map_pointer: u16,
}

impl Ord for Fitness {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.boss_seen,
            self.survived,
            self.level_frame,
            self.body_health,
            self.map_pointer,
        )
            .cmp(&(
                other.boss_seen,
                other.survived,
                other.level_frame,
                other.body_health,
                other.map_pointer,
            ))
    }
}

impl PartialOrd for Fitness {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn is_active_level(shell: &Shell) -> bool {
    shell.state() == GameState::Playing
        && shell.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
}

fn run(pattern: FlightPattern, firing: bool) -> Outcome {
    let mut shell = support::configured_shell();
    let mut level_updates = 0;
    let mut boss_seen = false;
    let mut died = false;
    let trace_damage = std::env::var_os("SF1_ROUTE_PROBE_TRACE").is_some()
        && matches!(pattern, FlightPattern::Box)
        && !firing;
    let mut previous_body_health = None;

    for tick in 0..SEARCH_TICKS {
        let input = if is_active_level(&shell) {
            pattern.steering(shell.game.vars.gameframe) | if firing { pad::Y } else { 0 }
        } else {
            support::weapon_input(tick) & !pad::Y
        };
        shell.tick(input);
        if is_active_level(&shell) {
            level_updates += 1;
        }
        let body_health = shell
            .game
            .coldet
            .pcbox
            .body
            .map(|slot| shell.game.objs.aliens[slot as usize].hp);
        if trace_damage && body_health != previous_body_health {
            let player = shell.game.objs.aliens[shell.game.vars.internal_playpt as usize];
            let mut nearby = shell
                .game
                .objs
                .active_indices()
                .into_iter()
                .filter(|slot| Some(*slot) != shell.game.coldet.pcbox.player)
                .map(|slot| {
                    let object = shell.game.objs.aliens[slot as usize];
                    let distance = i32::from(object.worldx.wrapping_sub(player.worldx)).abs()
                        + i32::from(object.worldy.wrapping_sub(player.worldy)).abs()
                        + i32::from(object.worldz.wrapping_sub(player.worldz)).abs();
                    (
                        distance,
                        slot,
                        object.shape,
                        object.hp,
                        [object.worldx, object.worldy, object.worldz],
                    )
                })
                .collect::<Vec<_>>();
            nearby.sort_by_key(|entry| entry.0);
            println!(
                "damage frame={} health={:?}->{:?} player={:?} nearby={:?}",
                shell.game.vars.gameframe,
                previous_body_health,
                body_health,
                [player.worldx, player.worldy, player.worldz],
                nearby.into_iter().take(8).collect::<Vec<_>>(),
            );
        }
        previous_body_health = body_health;
        boss_seen |= shell
            .game
            .objs
            .aliens
            .iter()
            .any(|object| object.active && object.shape == CORNERIA_ATTACK_CARRIER_SHAPE);
        died = shell.game.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0
            || shell.game.vars.pshipflags2 & PSF2_PLAYERHP0 != 0;
        if boss_seen || died || shell.state() == GameState::Tally {
            break;
        }
    }

    let player = shell.game.objs.aliens[shell.game.vars.internal_playpt as usize];
    let body_health = shell
        .game
        .coldet
        .pcbox
        .body
        .map(|slot| shell.game.objs.aliens[slot as usize].hp);
    Outcome {
        pattern,
        firing,
        level_updates,
        level_frame: shell.game.vars.gameframe,
        map: shell.frame().newmap,
        map_pointer: shell.game.vars.mapptr,
        player_position: [player.worldx, player.worldy, player.worldz],
        body_health,
        boss_seen,
        died,
        state: shell.state(),
    }
}

fn run_genome(genome: &[PilotAction]) -> Fitness {
    let mut shell = support::configured_shell();
    let mut boss_seen = false;
    let mut died = false;

    for tick in 0..SEARCH_TICKS {
        let input = if is_active_level(&shell) {
            let segment = usize::from(shell.game.vars.gameframe / CORNERIA_INPUT_SEGMENT_FRAMES);
            genome.get(segment).map_or(0, |action| action.pad_bits())
        } else {
            support::weapon_input(tick) & !pad::Y
        };
        shell.tick(input);
        boss_seen |= shell
            .game
            .objs
            .aliens
            .iter()
            .any(|object| object.active && object.shape == CORNERIA_ATTACK_CARRIER_SHAPE);
        died = shell.game.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0
            || shell.game.vars.pshipflags2 & PSF2_PLAYERHP0 != 0;
        if boss_seen || died || shell.state() == GameState::Tally {
            break;
        }
    }

    let body_health = shell
        .game
        .coldet
        .pcbox
        .body
        .map_or(0, |slot| shell.game.objs.aliens[slot as usize].hp);
    Fitness {
        boss_seen,
        survived: !died,
        level_frame: shell.game.vars.gameframe,
        body_health,
        map_pointer: shell.game.vars.mapptr,
    }
}

fn next_random(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

fn mutate_genome(best: &[PilotAction], best_fitness: Fitness, seed: u32) -> Vec<PilotAction> {
    let mut genome = best.to_vec();
    let death_segment = usize::from(best_fitness.level_frame / CORNERIA_INPUT_SEGMENT_FRAMES);
    let first = death_segment.saturating_sub(MUTATION_LOOKBACK_SEGMENTS);
    let last = (death_segment + MUTATION_LOOKAHEAD_SEGMENTS).min(genome.len() - 1);
    let mut random = seed.max(1);
    let mutations = 1 + next_random(&mut random) as usize % MAX_MUTATIONS;
    for _ in 0..mutations {
        let segment = first + next_random(&mut random) as usize % (last - first + 1);
        genome[segment] =
            PilotAction::ALL[next_random(&mut random) as usize % PilotAction::ALL.len()];
    }
    genome
}

fn box_genome() -> Vec<PilotAction> {
    (0..SEARCH_SEGMENTS)
        .map(|segment| match segment / 3 & 3 {
            0 => PilotAction::UpLeft,
            1 => PilotAction::UpRight,
            2 => PilotAction::DownRight,
            _ => PilotAction::DownLeft,
        })
        .collect()
}

fn search_route() {
    let mut best = box_genome();
    let mut best_fitness = run_genome(&best);
    println!("search_start fitness={best_fitness:?}");

    for generation in 0..SEARCH_GENERATIONS {
        let candidates = (0..SEARCH_POPULATION)
            .map(|candidate| {
                let seed = generation
                    .wrapping_mul(65_537)
                    .wrapping_add(candidate as u32)
                    .wrapping_add(1);
                mutate_genome(&best, best_fitness, seed)
            })
            .collect::<Vec<_>>();
        let mut evaluated = Vec::with_capacity(candidates.len());
        for chunk in candidates.chunks(SEARCH_WORKERS) {
            std::thread::scope(|scope| {
                let handles = chunk
                    .iter()
                    .cloned()
                    .map(|genome| scope.spawn(move || (run_genome(&genome), genome)))
                    .collect::<Vec<_>>();
                for handle in handles {
                    evaluated.push(handle.join().expect("route-search worker"));
                }
            });
        }
        if let Some((fitness, genome)) = evaluated
            .into_iter()
            .max_by_key(|(fitness, _)| *fitness)
            .filter(|(fitness, _)| *fitness > best_fitness)
        {
            best = genome;
            best_fitness = fitness;
            println!(
                "search_improved generation={generation} fitness={best_fitness:?} genome={best:?}"
            );
        } else if generation % 10 == 0 {
            println!("search_progress generation={generation} fitness={best_fitness:?}");
        }
        if best_fitness.boss_seen {
            break;
        }
    }
    println!("search_complete fitness={best_fitness:?} genome={best:?}");
}

fn main() {
    if std::env::var_os("SF1_ROUTE_SEARCH").is_some() {
        search_route();
        return;
    }
    for firing in [false, true] {
        for pattern in FlightPattern::ALL {
            let outcome = run(pattern, firing);
            println!(
                "pattern={:?} firing={} updates={} frame={} map={} pointer={} player={:?} body_health={:?} boss_seen={} died={} state={:?}",
                outcome.pattern,
                outcome.firing,
                outcome.level_updates,
                outcome.level_frame,
                outcome.map,
                outcome.map_pointer,
                outcome.player_position,
                outcome.body_health,
                outcome.boss_seen,
                outcome.died,
                outcome.state,
            );
        }
    }
}
