//! Tick 134: wire Planets.currentlevel → strat WRAM 0x1F03 (port level+1)
//! and fix boss7 hard-route gate (`s_jmp_ifnotlevel 3` → port `== 3`).

use sf_game::alien::ATZREMOVE;
use sf_game::Game;
use sf_strat::bosses::strat_boss8_init;
use sf_strat::enemy_a::wm;
use sf_strat::enemy_b::strat_boss7_init;

const BOSS7_HP: u8 = 40;
const BOSS8_HP: u8 = 0x20;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn boss8_hp_follows_port_currentlevel() {
    // Port encoding: 1 = easy (ROM raw 0), 2 = medium/hard-not-easy.
    for (lvl, expect) in [(1u8, BOSS8_HP), (2u8, BOSS8_HP * 2)] {
        let mut g = Game::new();
        g.vars.write_ext8(wm::CURRENTLEVEL, lvl);
        let idx = spawn(&mut g);
        strat_boss8_init(&mut g, idx);
        assert_eq!(g.objs.aliens[idx as usize].hp, expect, "level {lvl}");
        assert_eq!(g.vars.bossmaxhp, expect as u16);
    }
}

#[test]
fn boss7_hp_doubles_only_on_hard_route() {
    // ROM `s_jmp_ifnotlevel 3` — only hard route (port WRAM 3) doubles HP.
    // bossmaxhp also accumulates hatch/launcher children, so assert mother HP only.
    for (lvl, expect) in [(1u8, BOSS7_HP), (2u8, BOSS7_HP), (3u8, BOSS7_HP * 2)] {
        let mut g = Game::new();
        g.vars.write_ext8(wm::CURRENTLEVEL, lvl);
        let idx = spawn(&mut g);
        strat_boss7_init(&mut g, idx);
        assert_eq!(g.objs.aliens[idx as usize].hp, expect, "level {lvl}");
        assert_eq!(
            g.objs.aliens[idx as usize].type_ & ATZREMOVE,
            0,
            "Attack Carrier must survive its behind-camera approach"
        );
        assert!(
            g.vars.bossmaxhp >= expect as u16,
            "maxhp level {lvl}: got {}",
            g.vars.bossmaxhp
        );
    }
}
