//! Planet/route selection control flow and stage path parsing.
//!
//! C oracle: `src/game/planets.c` (PLANETS.ASM -> C conversion). The C
//! globals it drives — `g_whichroute` / `g_stage` / `g_lives` /
//! `g_routes[4]` / `g_currentplanet` / `g_newmap` / `g_peppermsg` /
//! `g_currentlevel` / `g_nebula_on` / `g_mapmode` (game_vars.c) — live in
//! [`Planets`]; defaults match `GameVars_Init` (game_vars.c:443-458).
//! `g_bg_dmalist` stays in [`crate::vars::GameVars`]; the shell zeroes it at
//! the `Planets_Init` call site (planets.c:306).

use crate::score;
use sf_core::{pad, sf1_planets::Sf1BriefingMessage};

/// C `PLANET_ROUTE_CHOICES` (src/game/planets.c:9).
pub const PLANET_ROUTE_CHOICES: u8 = 3;
/// C `PLANET_ROUTE_SLOTS` (src/game/planets.c:10).
pub const PLANET_ROUTE_SLOTS: usize = 4;

/// C `DEFAULT_LIVES` (src/variables.h:303).
pub const DEFAULT_LIVES: u8 = 3;

/// Result of one first-game route-selection input sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSelectionResult {
    Idle,
    Changed,
    Confirmed,
}

/// C `PlanetPathId` (src/game/planets.c:40-69). PATH_ID_1..=22 are the
/// enum's numeric values; the ends follow.
pub mod path_id {
    pub use sf_core::sf1_planets::route_path::*;
}

/// C `PathNextType` (src/game/planets.c:71-75).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NextType {
    None,
    Direct,
    RouteChoice,
}

/// C `StagePathNode` (src/game/planets.c:77-85). `map_id` values are the
/// planets.c MapId enum, which matches `sf_map::catalog::map_id`.
#[derive(Debug, Clone, Copy)]
struct StagePathNode {
    planet: u8,
    map_id: u32,
    briefing_message: Sf1BriefingMessage,
    currentlevel: u8,
    next_type: NextType,
    choice_slot: u8,
    next_path: u16,
}

const NODE_UNUSED: StagePathNode = StagePathNode {
    planet: 0,
    map_id: 0,
    briefing_message: Sf1BriefingMessage::None,
    currentlevel: 0,
    next_type: NextType::None,
    choice_slot: 0,
    next_path: path_id::INVALID,
};

const fn node(
    planet: u8,
    map_id: u32,
    briefing_message: Sf1BriefingMessage,
    currentlevel: u8,
    next_type: NextType,
    choice_slot: u8,
    next_path: u16,
) -> StagePathNode {
    StagePathNode {
        planet,
        map_id,
        briefing_message,
        currentlevel,
        next_type,
        choice_slot,
        next_path,
    }
}

/// C `kRouteRoots` (src/game/planets.c:87).
const ROUTE_ROOTS: [u16; PLANET_ROUTE_CHOICES as usize] = [path_id::P1, path_id::P6, path_id::P11];

/// C `kStagePaths` (src/game/planets.c:93-120), verbatim.
#[rustfmt::skip]
const STAGE_PATHS: [StagePathNode; path_id::COUNT as usize] = {
    use sf_map::catalog::map_id as m;
    use Sf1BriefingMessage as B;
    use NextType::{Direct, None as NoneT, RouteChoice};
    let mut t = [NODE_UNUSED; path_id::COUNT as usize];
    t[path_id::P1 as usize]  = node( 0, m::M2_1,       B::CounterattackVenom, 1, RouteChoice, 2, path_id::INVALID);
    t[path_id::P2 as usize]  = node( 3, m::M2_2,       B::DestroyRockCrusher, 1, Direct,      0, path_id::P3);
    t[path_id::P3 as usize]  = node( 5, m::M2_3,       B::RetakeWeatherControl, 1, Direct,      0, path_id::P4);
    t[path_id::P4 as usize]  = node( 9, m::M2_4,       B::EscapeAmoeba, 1, Direct,      0, path_id::P5);
    t[path_id::P5 as usize]  = node(15, m::M2_5,       B::VenomBackDoor, 1, Direct,      0, path_id::END2);
    t[path_id::P6 as usize]  = node( 0, m::M1_1,       B::CounterattackVenom, 0, RouteChoice, 1, path_id::INVALID);
    t[path_id::P7 as usize]  = node( 2, m::M1_2,       B::DestroyRockCrusher, 0, Direct,      0, path_id::P8);
    t[path_id::P8 as usize]  = node( 6, m::M1_3,       B::DestroyArmadaCores, 0, Direct,      0, path_id::P9);
    t[path_id::P9 as usize]  = node( 8, m::M1_4,       B::UseRetros, 0, Direct,      0, path_id::P10);
    t[path_id::P10 as usize] = node(15, m::M1_5,       B::DestroyAndrossCore, 0, Direct,      0, path_id::END1);
    t[path_id::P11 as usize] = node( 0, m::M3_1,       B::CourseThree, 2, RouteChoice, 0, path_id::INVALID);
    t[path_id::P12 as usize] = node( 1, m::M3_2,       B::EscapeTractorBeam, 2, Direct,      0, path_id::P13);
    t[path_id::P13 as usize] = node( 4, m::M3_3,       B::FortunaCreatures, 2, Direct,      0, path_id::P14);
    t[path_id::P14 as usize] = node( 7, m::M3_4,       B::ProceedToMacbeth, 2, Direct,      0, path_id::P15);
    t[path_id::P15 as usize] = node(11, m::M3_5,       B::PreventMacbethBase, 2, Direct,      0, path_id::P16);
    t[path_id::P16 as usize] = node(15, m::M3_6,       B::DestroyAndross, 2, Direct,      0, path_id::END3);
    t[path_id::P17 as usize] = node( 3, m::M2_2,       B::DestroyRockCrusher, 1, RouteChoice, 3, path_id::INVALID);
    t[path_id::P18 as usize] = node(10, m::BLACKHOLE,  B::BlackHoleWarning, 0, Direct,      0, path_id::P4);
    t[path_id::P19 as usize] = node(10, m::BLACKHOLE,  B::BlackHoleWarning, 0, Direct,      0, path_id::P10);
    t[path_id::P20 as usize] = node(10, m::BLACKHOLE,  B::BlackHoleWarning, 0, Direct,      0, path_id::P14);
    t[path_id::P21 as usize] = node( 2, m::M1_2,       B::DestroyRockCrusher, 0, RouteChoice, 3, path_id::INVALID);
    t[path_id::P22 as usize] = node( 1, m::M3_2,       B::EscapeTractorBeam, 2, Direct,      0, path_id::OTHEREND);
    t[path_id::END3 as usize]     = node(15, m::M3_7,     B::DestroyAndross, 2, NoneT, 0, path_id::INVALID);
    t[path_id::END2 as usize]     = node(15, m::M2_6,     B::VenomBackDoor, 1, NoneT, 0, path_id::INVALID);
    t[path_id::END1 as usize]     = node(15, m::M1_6,     B::DestroyAndrossCore, 0, NoneT, 0, path_id::INVALID);
    t[path_id::OTHEREND as usize] = node(14, m::SPECIAL,  B::ProtectCorneria, 0, NoneT, 0, path_id::INVALID);
    t
};

/// C `normalize_route_choice` (src/game/planets.c:124).
fn normalize_route_choice(route: u8) -> u8 {
    if route >= PLANET_ROUTE_CHOICES {
        route % PLANET_ROUTE_CHOICES
    } else {
        route
    }
}

/// C `normalize_path_id` (src/game/planets.c:131).
fn normalize_path_id(p: u16) -> u16 {
    if p >= path_id::COUNT {
        path_id::INVALID
    } else {
        p
    }
}

/// Route/stage progression state.
#[derive(Debug, Clone)]
pub struct Planets {
    /// C `g_whichroute`.
    pub whichroute: u8,
    /// C `g_stage`.
    pub stage: u16,
    /// C `g_lives`.
    pub lives: u8,
    /// C `g_currentlevel`.
    pub currentlevel: u8,
    /// C `g_routes[4]`.
    pub routes: [u16; PLANET_ROUTE_SLOTS],
    /// C `g_currentplanet`.
    pub currentplanet: i16,
    /// C `g_mapmode`.
    pub mapmode: u16,
    /// C `g_newmap`.
    pub newmap: u32,
    /// Source `peppermsg`, represented by its mission-domain identity.
    pub briefing_message: Sf1BriefingMessage,
    /// C `g_nebula_on`.
    pub nebula_on: u16,
    /// C `s_route_converted` (planets.c:122).
    route_converted: bool,

    /// C `credits` (CONFIG/GAME.INC:33) — bonus continue credits awarded by
    /// the end-of-level tally (checkbonus). New game inits to 0
    /// (PLANETS.ASM:211); the tally increments on `bonertab` threshold
    /// crossings (MAIN.ASM:1138). Persists across stages.
    pub credits: u8,
    /// C `specbuf`/`specptr` (MAIN.ASM:1213-1217) — the recorded per-stage hit
    /// percentages, one entry appended per completed stage (101 =
    /// [`score::STAGE_SKIPPED`]). `calctotalscore` sums these; see
    /// [`Planets::total_score`].
    pub stage_scores: Vec<u8>,
}

impl Default for Planets {
    /// GameVars_Init defaults (game_vars.c:443-458).
    fn default() -> Self {
        Planets {
            whichroute: 0,
            stage: 0,
            lives: DEFAULT_LIVES,
            currentlevel: 0,
            routes: [0; PLANET_ROUTE_SLOTS],
            currentplanet: -1,
            mapmode: 0,
            newmap: 0,
            briefing_message: Sf1BriefingMessage::None,
            nebula_on: 0,
            route_converted: false,
            credits: 0,
            stage_scores: Vec::new(),
        }
    }
}

impl Planets {
    pub fn new() -> Self {
        Self::default()
    }

    /// C `Planets_Init()` (src/game/planets.c:297) — initplanets_l
    /// defaults. Caller must also zero `GameVars::bg_dmalist`
    /// (planets.c:306).
    pub fn init(&mut self) {
        self.lives = DEFAULT_LIVES;
        self.routes[0] = path_id::P12;
        self.routes[1] = path_id::P7;
        self.routes[2] = path_id::P2;
        self.routes[3] = path_id::P19;
        self.stage = 0;
        self.mapmode = 0;
        self.route_converted = false;
        // New game: credits=0 (PLANETS.ASM:211), specptr=0 (MAIN.ASM:41 —
        // done at game start; the tally screen appends per stage).
        self.credits = 0;
        self.stage_scores.clear();
    }

    /// ROM `calctotalscore` (MAIN.ASM:780-800): running total across all
    /// recorded stages. Also the value drawn on the map screen by
    /// `drawroutename` (PLANETS.ASM:1547) and compared by `checkbonus`.
    pub fn total_score(&self) -> u16 {
        score::calc_total_score(&self.stage_scores)
    }

    /// ROM `maketotalscore` / `maketotalscore2` average percentage
    /// (MAIN.ASM:649-658): `floor(total / played stages)`.
    pub fn average_score(&self) -> u16 {
        score::calc_average_score(&self.stage_scores)
    }

    /// Append a completed stage's percentage to `specbuf` and run
    /// `checkbonus` (MAIN.ASM:1213-1219): returns `true` if the new running
    /// total crossed a fresh `bonertab` threshold, in which case a credit is
    /// awarded (`inc credits`, MAIN.ASM:1138) and the caller should play the
    /// bonus SFX ([`score::SE_BONUS`], `trigse $1a`, MAIN.ASM:1149).
    pub fn record_stage_score(&mut self, stage_perc: u8) -> bool {
        let crossed = self.append_stage_score(stage_perc);
        if crossed {
            self.award_bonus_credit();
        }
        crossed
    }

    /// Append the newly counted stage percentage at the retail graph-commit
    /// boundary. Returns whether `checkbonus` armed a delayed bonus; unlike
    /// [`Self::record_stage_score`], this does not award the credit yet.
    pub fn append_stage_score(&mut self, stage_perc: u8) -> bool {
        // checkbonus compares against clbm = total BEFORE this stage was added
        // (MAIN.ASM:1082-1084).
        let old_total = self.total_score();
        self.stage_scores.push(stage_perc);
        score::crossed_bonus_threshold(self.total_score(), old_total)
    }

    /// Retail `inc credits` after the ten-frame bonus sprite countdown.
    pub fn award_bonus_credit(&mut self) {
        self.credits = self.credits.saturating_add(1);
    }

    /// C `convertroute()` (src/game/planets.c:136).
    pub fn convertroute(&mut self) {
        if self.whichroute == 0 {
            self.whichroute = 1;
        } else if self.whichroute == 1 {
            self.whichroute = 0;
        }
    }

    /// Enter the first-game route-selection loop. The map sequence keeps the
    /// converted route value until its final gameplay handoff, matching
    /// `planetseq_l`.
    pub fn begin_route_selection(&mut self) {
        if !self.route_converted {
            self.convertroute();
            self.route_converted = true;
        }
        self.stage = 0;
        let _ = self.drawplanetlines();
    }

    /// Enter a later-stage map walk and return the source stage-path identity
    /// traversed from the previous planet. The converted route remains active
    /// through confirmation and the General Pepper sequence.
    pub fn begin_stage_map_walk(&mut self) -> Option<u16> {
        if !self.route_converted {
            self.convertroute();
            self.route_converted = true;
        }
        let travel_index = usize::from(self.stage.saturating_sub(1));
        let travel_path = self
            .route_path_ids(self.whichroute)
            .get(travel_index)
            .copied();
        if self.drawplanetlines() {
            travel_path
        } else {
            None
        }
    }

    /// Sample the source route-selection controls without skipping the
    /// presentation that follows confirmation.
    pub fn route_selection_input(&mut self, press: u16) -> RouteSelectionResult {
        if press & (pad::LEFT | pad::SELECT | pad::UP) != 0 {
            self.whichroute = normalize_route_choice(self.whichroute);
            self.whichroute = if self.whichroute == 0 {
                PLANET_ROUTE_CHOICES - 1
            } else {
                self.whichroute - 1
            };
            let _ = self.drawplanetlines();
            return RouteSelectionResult::Changed;
        }

        if press & (pad::RIGHT | pad::DOWN) != 0 {
            self.whichroute = normalize_route_choice(self.whichroute);
            self.whichroute += 1;
            if self.whichroute >= PLANET_ROUTE_CHOICES {
                self.whichroute = 0;
            }
            let _ = self.drawplanetlines();
            return RouteSelectionResult::Changed;
        }

        if press & (pad::START | pad::A | pad::B) != 0 {
            self.stage = 0;
            self.currentplanet = 0;
            let _ = self.drawplanetlines();
            return RouteSelectionResult::Confirmed;
        }

        RouteSelectionResult::Idle
    }

    /// Restore the gameplay route encoding at the source's final planet-map
    /// handoff.
    pub fn finish_map_sequence(&mut self) {
        if self.route_converted {
            self.convertroute();
            self.route_converted = false;
        }
    }

    // C routechange* map callbacks (src/game/planets.c:144-167). Not yet
    // reachable — the map lane has not registered ROUTECHANGE native
    // callbacks in sf-map; exposed for that wiring.
    pub fn routechange1(&mut self) {
        self.routes[0] = path_id::P22;
        self.nebula_on = path_id::P22;
    }
    pub fn routechange2(&mut self) {
        self.routes[1] = path_id::P21;
    }
    pub fn routechange3(&mut self) {
        self.routes[2] = path_id::P17;
    }
    pub fn routechangebhole1(&mut self) {
        self.routes[3] = path_id::P19;
    }
    pub fn routechangebhole2(&mut self) {
        self.routes[3] = path_id::P18;
    }
    pub fn routechangebhole3(&mut self) {
        self.routes[3] = path_id::P20;
    }

    /// C `drawplanetlines_l()` (src/game/planets.c:176). Walks the stage
    /// graph for the current route/stage and resolves
    /// `newmap`/`currentlevel`/`currentplanet`/`peppermsg`. Returns false
    /// when the route is exhausted (route complete -> ending).
    pub fn drawplanetlines(&mut self) -> bool {
        self.whichroute = normalize_route_choice(self.whichroute);
        let mut path = ROUTE_ROOTS[self.whichroute as usize];
        let mut stage_remaining = (self.stage as u8) as u16;

        for _ in 0..64 {
            path = normalize_path_id(path);
            if path == path_id::INVALID {
                break;
            }
            let n = &STAGE_PATHS[path as usize];

            self.currentplanet = n.planet as i16;
            self.newmap = n.map_id;
            self.briefing_message = n.briefing_message;
            self.currentlevel = n.currentlevel;

            if stage_remaining == 0 {
                return true;
            }
            stage_remaining -= 1;

            match n.next_type {
                NextType::Direct => path = n.next_path,
                NextType::RouteChoice => {
                    if n.choice_slot as usize >= PLANET_ROUTE_SLOTS {
                        break;
                    }
                    path = self.routes[n.choice_slot as usize];
                }
                NextType::None => break,
            }
        }

        self.currentplanet = -1;
        self.newmap = sf_map::catalog::map_id::NONE;
        self.briefing_message = Sf1BriefingMessage::None;
        false
    }

    /// C `Planets_GetRouteNodes()` (src/game/planets.c:226) — read-only
    /// walk reporting the planet id at each stage.
    pub fn route_nodes(&self, route: u8) -> Vec<u8> {
        let mut out = Vec::new();
        let mut path = ROUTE_ROOTS[normalize_route_choice(route) as usize];
        for _ in 0..64 {
            path = normalize_path_id(path);
            if path == path_id::INVALID {
                break;
            }
            let n = &STAGE_PATHS[path as usize];
            out.push(n.planet);
            match n.next_type {
                NextType::Direct => path = n.next_path,
                NextType::RouteChoice => {
                    if n.choice_slot as usize >= PLANET_ROUTE_SLOTS {
                        break;
                    }
                    path = self.routes[n.choice_slot as usize];
                }
                NextType::None => break,
            }
        }
        out
    }

    /// C `Planets_GetRoutePathIds()` (src/game/planets.c:263) — same walk,
    /// reporting the PATH_ID_* sequence.
    pub fn route_path_ids(&self, route: u8) -> Vec<u16> {
        let mut out = Vec::new();
        let mut path = ROUTE_ROOTS[normalize_route_choice(route) as usize];
        for _ in 0..64 {
            path = normalize_path_id(path);
            if path == path_id::INVALID {
                break;
            }
            out.push(path);
            let n = &STAGE_PATHS[path as usize];
            match n.next_type {
                NextType::Direct => path = n.next_path,
                NextType::RouteChoice => {
                    if n.choice_slot as usize >= PLANET_ROUTE_SLOTS {
                        break;
                    }
                    path = self.routes[n.choice_slot as usize];
                }
                NextType::None => break,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_map::catalog::map_id as m;

    #[test]
    fn route1_walk_matches_c_table() {
        // Route choice 1 (root PATH_ID_6) with Planets_Init defaults:
        // routes[1] = PATH_ID_7 -> 8 -> 9 -> 10 -> END1 (planets.c:99-103).
        let mut p = Planets::new();
        p.init();
        assert_eq!(
            p.route_path_ids(1),
            vec![
                path_id::P6,
                path_id::P7,
                path_id::P8,
                path_id::P9,
                path_id::P10,
                path_id::END1
            ]
        );
        assert_eq!(p.route_nodes(1), vec![0, 2, 6, 8, 15, 15]);

        // Stage 0 resolves MAP_ID_1_1 / level 0 / planet 0.
        p.whichroute = 1;
        p.stage = 0;
        assert!(p.drawplanetlines());
        assert_eq!(p.newmap, m::M1_1);
        assert_eq!(p.currentlevel, 0);
        assert_eq!(p.currentplanet, 0);
        assert_eq!(p.briefing_message, Sf1BriefingMessage::CounterattackVenom);

        // Stage past END1 fails and resets outputs (planets.c:214-217).
        p.stage = 6;
        assert!(!p.drawplanetlines());
        assert_eq!(p.currentplanet, -1);
        assert_eq!(p.newmap, m::NONE);
        assert_eq!(p.briefing_message, Sf1BriefingMessage::None);
    }

    #[test]
    fn black_hole_reroute() {
        let mut p = Planets::new();
        p.init();
        p.routechange2(); // routes[1] = PATH_ID_21 (choice slot 3)
        assert_eq!(
            p.route_path_ids(1),
            vec![
                path_id::P6,
                path_id::P21,
                path_id::P19,
                path_id::P10,
                path_id::END1
            ]
        );
        p.routechangebhole2(); // routes[3] = PATH_ID_18 -> jumps to route-2 spine
        assert_eq!(
            p.route_path_ids(1),
            vec![
                path_id::P6,
                path_id::P21,
                path_id::P18,
                path_id::P4,
                path_id::P5,
                path_id::END2
            ]
        );
    }

    #[test]
    fn route_selection_keeps_the_converted_route_until_the_map_sequence_finishes() {
        let mut p = Planets::new();
        p.init();
        p.begin_route_selection();
        assert_eq!(p.whichroute, 1);
        assert_eq!(p.newmap, m::M1_1);

        assert_eq!(
            p.route_selection_input(sf_core::pad::START),
            RouteSelectionResult::Confirmed
        );
        assert_eq!(p.whichroute, 1);
        assert_eq!(p.newmap, m::M1_1);
        assert_eq!(p.stage, 0);

        p.finish_map_sequence();
        assert_eq!(p.whichroute, 0);
        assert_eq!(p.newmap, m::M1_1);
    }
}

/// ROM `dividebynum` (PLANETS.ASM:1884) — signed 16-bit divide of `delta` by
/// `frames`, folding in fractional `remainder` (0..frames-1).
///
/// Used by `calcdxdy` for planet-select camera scroll. Returns
/// `(quotient, new_remainder)`. Control flow is a literal port of the ASM
/// (positive subtract-loop / negative add-until-carry); verified by
/// `sf-oracle` `fuzz_calcdxdy`.
pub fn divide_by_num(delta: i16, frames: u16, remainder: u16) -> (i16, u16) {
    debug_assert!(frames > 0);
    let frames_i = frames as i16;
    if (delta as u16) >= 0x8000 {
        // .negative: X=1; DEX; CLC; ADC frames; BCC .ndivide
        let mut x: i16 = 1;
        let mut a = delta as u16;
        loop {
            x = x.wrapping_sub(1);
            let (sum, carry) = a.overflowing_add(frames);
            a = sum;
            if carry {
                break;
            }
        }
        // SEC; SBC frames (undo wrap)
        a = a.wrapping_sub(frames);
        // CLC; ADC remainder
        a = a.wrapping_add(remainder);
        if a >= frames {
            a = a.wrapping_sub(frames);
            x = x.wrapping_sub(1);
        }
        (x, a)
    } else {
        // positive: X=-1; INX; SEC; SBC frames; BCS .divide
        let mut x: i16 = -1;
        let mut a = delta;
        loop {
            x = x.wrapping_add(1);
            let (diff, borrow) = (a as u16).overflowing_sub(frames);
            a = diff as i16;
            if borrow {
                break;
            }
        }
        // CLC; ADC frames (undo)
        a = a.wrapping_add(frames_i);
        // CLC; ADC remainder
        let mut au = (a as u16).wrapping_add(remainder);
        if au >= frames {
            au = au.wrapping_sub(frames);
            x = x.wrapping_add(1);
        }
        (x, au)
    }
}

/// ROM `calcdxdy` per-axis step (PLANETS.ASM:1844): zero delta is a no-op
/// (`.nochange`); otherwise `divide_by_num`.
pub fn calc_scroll_step(delta: i16, frames: u16, remainder: u16) -> (i16, u16) {
    if delta == 0 {
        (0, remainder)
    } else {
        divide_by_num(delta, frames, remainder)
    }
}
