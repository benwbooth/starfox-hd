//! Common strategy routines used by all object behaviors.
//!
//! Port (C oracle): `src/strat/strat_common.c/h` (STRATROU.ASM -> C):
//! velocity generation (alvelvecs, alvel3vecs), position updates
//! (addvecs, addalvecs), chase functions (Fchase, Achase, speedto),
//! percentage scaling (perc75/62/87/...), distance/angle helpers, object
//! management (makeobj, removeobj) and the shared lightweight projectile.
//!
//! Integer discipline matches C exactly: 16-bit wrapping adds, arithmetic
//! shifts on signed values, float->int truncation where the C build casts,
//! and the same f32 trig constants (`2.0f * 3.14159265f / 256.0f`).
//!
//! Strategy globals live in the typed records owned by
//! [`sf_game::vars::GameVars`]. [`sv`] contains semantic field identifiers;
//! it is not an address namespace.

use sf_game::alien::{
    Alien, ObjectVisualKind, StratId, ACF_FIRSTFRAME, ACF_WEAPON, AFONFIRE, ASF2_COLLDISABLE,
    ASF3_REALOBJ, ASF4_INVISIBLE, ATLASER, ATZREMOVE, NUMBER_AL,
};
// NUMBER_AL used by updateengine_srou bounds check.
use sf_game::vars::GameVars;
use sf_game::Game;

// ============================================================
// Typed strategy-variable identifiers
// ============================================================
pub use sf_game::vars::StrategyVariable as sv;

/// Typed WRAM-mirror access for the [`sv`] block (little-endian, matching
/// C `world_read_ext16`).
pub trait StratRam {
    fn sv_u8(&self, variable: sv) -> u8;
    fn sv_i8(&self, variable: sv) -> i8;
    fn sv_u16(&self, variable: sv) -> u16;
    fn sv_i16(&self, variable: sv) -> i16;
    fn set_sv_u8(&mut self, variable: sv, value: u8);
    fn set_sv_i8(&mut self, variable: sv, value: i8);
    fn set_sv_u16(&mut self, variable: sv, value: u16);
    fn set_sv_i16(&mut self, variable: sv, value: i16);
}

impl StratRam for GameVars {
    fn sv_u8(&self, variable: sv) -> u8 {
        self.sv_u16(variable) as u8
    }
    fn sv_i8(&self, variable: sv) -> i8 {
        self.sv_u16(variable) as u8 as i8
    }
    fn sv_u16(&self, variable: sv) -> u16 {
        let state = &self.strategy;
        let bindings = &self.strategy_bindings;
        match variable {
            sv::RandomSeed => state.random_seed,
            sv::ProjectileStrategy => bindings.projectile,
            sv::PlayerStrategyBase => bindings.player_base,
            sv::PlayerRotationX => state.player_rotation[0] as u16,
            sv::PlayerRotationY => state.player_rotation[1] as u16,
            sv::PlayerRotationZ => state.player_rotation[2] as u16,
            sv::PlayerSpeed => state.player_speed as u16,
            sv::PlayerTargetSpeed => state.player_target_speed.into(),
            sv::PlayerMediumSpeed => state.player_medium_speed.into(),
            sv::PlayerTurnRotation => state.player_turn_rotation as u16,
            sv::PlayerDepthShake => state.player_depth_shake as u16,
            sv::PlayerDepthShakeVelocity => state.player_depth_shake_velocity as u16,
            sv::PlayerDepthTilt => state.player_depth_tilt as u8 as u16,
            sv::PlayerDepthStrategyOffset => state.player_depth_strategy_offset.into(),
            sv::PlayerRollVelocity => state.player_roll_velocity as u8 as u16,
            sv::PlayerRollOffset => state.player_roll_offset as u8 as u16,
            sv::PlayerRollDelay => state.player_roll_delay.into(),
            sv::PlayerControlDelay => state.player_control_delay.into(),
            sv::PlayerRollFloatCursor => state.player_roll_float_cursor,
            sv::PlayerRollFloat => state.player_roll_float as u8 as u16,
            sv::ViewShakeX => state.view_shake[0].into(),
            sv::ViewShakeY => state.view_shake[1].into(),
            sv::ViewShakeZ => state.view_shake[2].into(),
            sv::ScreenFlashCount => state.screen_flash_count.into(),
            sv::ScreenFlashKind => state.screen_flash_kind.into(),
            sv::PlayerHitCount => state.player_hit_count.into(),
            sv::Lives => state.lives.into(),
            sv::StayBlack => state.stay_black as u8 as u16,
            sv::WipeActive => state.wipe_active.into(),
            sv::ArrowFlags => state.arrow_flags.into(),
            sv::ViewCenterY => state.view_center_y as u16,
            sv::BoostDepthOffset => state.boost_depth_offset as u8 as u16,
            sv::PlayerMinX => state.player_min_x as u16,
            sv::PlayerMaxX => state.player_max_x as u16,
            sv::PlayerMaxY => state.player_max_y as u16,
            sv::MouseMinX => state.mouse_min_x as u16,
            sv::MouseMaxX => state.mouse_max_x as u16,
            sv::MouseMaxY => state.mouse_max_y as u16,
            sv::WaterPlayerMinY => state.water_player_min_y as u16,
            sv::WaterPlayerMaxY => state.water_player_max_y as u16,
            sv::PlayerMoveLimit => state.player_move_limit.into(),
            sv::PlayerMoveLimitMask => state.player_move_limit_mask.into(),
            sv::MissileBoundaryFlags => state.missile_boundary_flags.into(),
            sv::BoostCount => state.boost_count.into(),
            sv::BoostObject => state.boost_object as u16,
            sv::PlayerViewX => state.player_view_position[0] as u16,
            sv::PlayerViewY => state.player_view_position[1] as u16,
            sv::PlayerViewZ => state.player_view_position[2] as u16,
            sv::BackgroundScrollZ => state.background_scroll_z as u16,
            sv::HudRotation => state.hud_rotation as u16,
            sv::ViewPitch => state.view_pitch as u16,
            sv::ViewYaw => state.view_yaw as u16,
            sv::ViewDistance => state.view_distance as u16,
            sv::ViewRoll => state.view_roll as u16,
            sv::ViewKind => state.view_kind.into(),
            sv::FadeDirection => state.fade_direction as u8 as u16,
            sv::ViewTargetObject => state.view_target_object as u16,
            sv::FixedViewX => state.fixed_view_position[0] as u16,
            sv::FixedViewY => state.fixed_view_position[1] as u16,
            sv::FixedViewZ => state.fixed_view_position[2] as u16,
            sv::PlayerByte1 => state.player_bytes[0].into(),
            sv::PlayerByte2 => state.player_bytes[1].into(),
            sv::PlayerByte3 => state.player_bytes[2].into(),
            sv::NoMaximumBackgroundY => state.no_maximum_background_y.into(),
            sv::BackgroundY => state.background_y as u16,
            sv::StrategyWord1 => state.strategy_words[0] as u16,
            sv::StrategyWord2 => state.strategy_words[1] as u16,
            sv::StrategyWord3 => state.strategy_words[2] as u16,
            sv::PlayerCollisionBody => state.player_collision_objects[0] as u16,
            sv::PlayerCollisionLeftWing => state.player_collision_objects[1] as u16,
            sv::PlayerCollisionRightWing => state.player_collision_objects[2] as u16,
            sv::PlayerShapeIntact => state.player_shapes[0],
            sv::PlayerShapeNoLeftWing => state.player_shapes[1],
            sv::PlayerShapeNoRightWing => state.player_shapes[2],
            sv::PlayerShapeNoWings => state.player_shapes[3],
            sv::FireCount => state.fire_count.into(),
            sv::FireDelay => state.fire_delay.into(),
            sv::SpecialDelay => state.special_delay.into(),
            sv::SpecialWeaponCount => state.special_weapon_count,
            sv::MissileTopLeft => state.missile_bounds[0] as u16,
            sv::MissileBottomLeft => state.missile_bounds[1] as u16,
            sv::MissileTopRight => state.missile_bounds[2] as u16,
            sv::ViewXOffset => state.fixed_view_offset[0] as u16,
            sv::ViewYOffset => state.fixed_view_offset[1] as u16,
            sv::SmokeVariable => state.smoke_variable,
            sv::FireSmokeStrategyBase => bindings.fire_smoke_base,
            sv::PuffStrategy => bindings.puff,
            sv::SparkyStrategy => bindings.sparky,
            sv::CircleObject => state.circle_object as u16,
            sv::PlayerLaserCount => state.player_laser_count.into(),
        }
    }
    fn sv_i16(&self, variable: sv) -> i16 {
        self.sv_u16(variable) as i16
    }
    fn set_sv_u8(&mut self, variable: sv, value: u8) {
        self.set_sv_u16(variable, value.into());
    }
    fn set_sv_i8(&mut self, variable: sv, value: i8) {
        self.set_sv_u16(variable, value as u8 as u16);
    }
    fn set_sv_u16(&mut self, variable: sv, value: u16) {
        let state = &mut self.strategy;
        let bindings = &mut self.strategy_bindings;
        match variable {
            sv::RandomSeed => state.random_seed = value,
            sv::ProjectileStrategy => bindings.projectile = value,
            sv::PlayerStrategyBase => bindings.player_base = value,
            sv::PlayerRotationX => state.player_rotation[0] = value as i16,
            sv::PlayerRotationY => state.player_rotation[1] = value as i16,
            sv::PlayerRotationZ => state.player_rotation[2] = value as i16,
            sv::PlayerSpeed => state.player_speed = value as i16,
            sv::PlayerTargetSpeed => state.player_target_speed = value as u8,
            sv::PlayerMediumSpeed => state.player_medium_speed = value as u8,
            sv::PlayerTurnRotation => state.player_turn_rotation = value as i16,
            sv::PlayerDepthShake => state.player_depth_shake = value as i16,
            sv::PlayerDepthShakeVelocity => state.player_depth_shake_velocity = value as i16,
            sv::PlayerDepthTilt => state.player_depth_tilt = value as u8 as i8,
            sv::PlayerDepthStrategyOffset => state.player_depth_strategy_offset = value as u8,
            sv::PlayerRollVelocity => state.player_roll_velocity = value as u8 as i8,
            sv::PlayerRollOffset => state.player_roll_offset = value as u8 as i8,
            sv::PlayerRollDelay => state.player_roll_delay = value as u8,
            sv::PlayerControlDelay => state.player_control_delay = value as u8,
            sv::PlayerRollFloatCursor => state.player_roll_float_cursor = value,
            sv::PlayerRollFloat => state.player_roll_float = value as u8 as i8,
            sv::ViewShakeX => state.view_shake[0] = value as u8,
            sv::ViewShakeY => state.view_shake[1] = value as u8,
            sv::ViewShakeZ => state.view_shake[2] = value as u8,
            sv::ScreenFlashCount => state.screen_flash_count = value as u8,
            sv::ScreenFlashKind => state.screen_flash_kind = value as u8,
            sv::PlayerHitCount => state.player_hit_count = value as u8,
            sv::Lives => state.lives = value as u8,
            sv::StayBlack => state.stay_black = value as u8 as i8,
            sv::WipeActive => state.wipe_active = value as u8,
            sv::ArrowFlags => state.arrow_flags = value as u8,
            sv::ViewCenterY => state.view_center_y = value as i16,
            sv::BoostDepthOffset => state.boost_depth_offset = value as u8 as i8,
            sv::PlayerMinX => state.player_min_x = value as i16,
            sv::PlayerMaxX => state.player_max_x = value as i16,
            sv::PlayerMaxY => state.player_max_y = value as i16,
            sv::MouseMinX => state.mouse_min_x = value as i16,
            sv::MouseMaxX => state.mouse_max_x = value as i16,
            sv::MouseMaxY => state.mouse_max_y = value as i16,
            sv::WaterPlayerMinY => state.water_player_min_y = value as i16,
            sv::WaterPlayerMaxY => state.water_player_max_y = value as i16,
            sv::PlayerMoveLimit => state.player_move_limit = value as u8,
            sv::PlayerMoveLimitMask => state.player_move_limit_mask = value as u8,
            sv::MissileBoundaryFlags => state.missile_boundary_flags = value as u8,
            sv::BoostCount => state.boost_count = value as u8,
            sv::BoostObject => state.boost_object = value as i16,
            sv::PlayerViewX => state.player_view_position[0] = value as i16,
            sv::PlayerViewY => state.player_view_position[1] = value as i16,
            sv::PlayerViewZ => state.player_view_position[2] = value as i16,
            sv::BackgroundScrollZ => state.background_scroll_z = value as i16,
            sv::HudRotation => state.hud_rotation = value as i16,
            sv::ViewPitch => state.view_pitch = value as i16,
            sv::ViewYaw => state.view_yaw = value as i16,
            sv::ViewDistance => state.view_distance = value as i16,
            sv::ViewRoll => state.view_roll = value as i16,
            sv::ViewKind => state.view_kind = value as u8,
            sv::FadeDirection => state.fade_direction = value as u8 as i8,
            sv::ViewTargetObject => state.view_target_object = value as i16,
            sv::FixedViewX => state.fixed_view_position[0] = value as i16,
            sv::FixedViewY => state.fixed_view_position[1] = value as i16,
            sv::FixedViewZ => state.fixed_view_position[2] = value as i16,
            sv::PlayerByte1 => state.player_bytes[0] = value as u8,
            sv::PlayerByte2 => state.player_bytes[1] = value as u8,
            sv::PlayerByte3 => state.player_bytes[2] = value as u8,
            sv::NoMaximumBackgroundY => state.no_maximum_background_y = value as u8,
            sv::BackgroundY => state.background_y = value as i16,
            sv::StrategyWord1 => state.strategy_words[0] = value as i16,
            sv::StrategyWord2 => state.strategy_words[1] = value as i16,
            sv::StrategyWord3 => state.strategy_words[2] = value as i16,
            sv::PlayerCollisionBody => state.player_collision_objects[0] = value as i16,
            sv::PlayerCollisionLeftWing => state.player_collision_objects[1] = value as i16,
            sv::PlayerCollisionRightWing => state.player_collision_objects[2] = value as i16,
            sv::PlayerShapeIntact => state.player_shapes[0] = value,
            sv::PlayerShapeNoLeftWing => state.player_shapes[1] = value,
            sv::PlayerShapeNoRightWing => state.player_shapes[2] = value,
            sv::PlayerShapeNoWings => state.player_shapes[3] = value,
            sv::FireCount => state.fire_count = value as u8,
            sv::FireDelay => state.fire_delay = value as u8,
            sv::SpecialDelay => state.special_delay = value as u8,
            sv::SpecialWeaponCount => state.special_weapon_count = value,
            sv::MissileTopLeft => state.missile_bounds[0] = value as i16,
            sv::MissileBottomLeft => state.missile_bounds[1] = value as i16,
            sv::MissileTopRight => state.missile_bounds[2] = value as i16,
            sv::ViewXOffset => state.fixed_view_offset[0] = value as i16,
            sv::ViewYOffset => state.fixed_view_offset[1] = value as i16,
            sv::SmokeVariable => state.smoke_variable = value,
            sv::FireSmokeStrategyBase => bindings.fire_smoke_base = value,
            sv::PuffStrategy => bindings.puff = value,
            sv::SparkyStrategy => bindings.sparky = value,
            sv::CircleObject => state.circle_object = value as i16,
            sv::PlayerLaserCount => state.player_laser_count = value as u8,
        }
    }
    fn set_sv_i16(&mut self, variable: sv, value: i16) {
        self.set_sv_u16(variable, value as u16);
    }
}

/// Source runtime random draw over the typed four-byte stream.
pub fn sf_random(vars: &mut GameVars) -> u16 {
    u16::from(vars.advance_random())
}

const HALF_TURN_ANGLE: u8 = 128;

/// Camera-facing object pitch and yaw used by source `s_rots_flat` effects.
///
/// This is a semantic view transform over the port's typed strategy state;
/// it does not expose source-machine storage. The returned pair is
/// `[pitch, yaw]` in the game's one-byte turn representation.
pub fn flat_billboard_rotation(vars: &GameVars) -> [u8; 2] {
    let pitch = (vars.strategy.view_pitch >> 8) as u8;
    let view_yaw = (vars.strategy.view_yaw >> 8) as u8;
    let player_turn = (vars.strategy.player_turn_rotation >> 8) as u8;
    let yaw = view_yaw
        .wrapping_neg()
        .wrapping_add(HALF_TURN_ANGLE)
        .wrapping_add(player_turn);
    [pitch, yaw]
}

#[cfg(test)]
mod billboard_rotation_tests {
    use super::*;

    #[test]
    fn flat_billboard_faces_the_neutral_training_camera() {
        let vars = GameVars::default();
        assert_eq!(flat_billboard_rotation(&vars), [0, HALF_TURN_ANGLE]);
    }

    #[test]
    fn flat_billboard_uses_view_and_player_turn_high_bytes() {
        let mut vars = GameVars::default();
        vars.strategy.view_pitch = 3 << 8;
        vars.strategy.view_yaw = -5 << 8;
        vars.strategy.player_turn_rotation = 7 << 8;
        assert_eq!(flat_billboard_rotation(&vars), [3, 140]);
    }
}

// ============================================================
// Chase / Transition (C strat_common.c:36-80)
// ============================================================

/// C `Strat_Chase` (Fchase_A, STRATMAC.INC). 16-bit wrapping step.
pub fn strat_chase(mut current: i16, target: i16, rate: i16) -> i16 {
    if current < target {
        current = current.wrapping_add(rate);
        if current > target {
            current = target;
        }
    } else if current > target {
        current = current.wrapping_sub(rate);
        if current < target {
            current = target;
        }
    }
    current
}

/// C `Strat_Chase8`.
pub fn strat_chase8(mut current: u8, target: u8, rate: u8) -> u8 {
    if current < target {
        current = current.wrapping_add(rate);
        if current > target {
            current = target;
        }
    } else if current > target {
        if current - target < rate {
            current = target;
        } else {
            current -= rate;
        }
    }
    current
}

/// C `Strat_ChaseProportional` / ROM `Achase_var2A` → `sr16_achase_alvar*`:
/// `current + adiv2^n(target - current)`, with i16 wrap on the subtract.
///
/// `adiv2` (STRATMAC.INC:712) is a signed halve toward zero, applied `shift`
/// times. A plain arithmetic `>>` rounds toward -inf and breaks asymptotic
/// chases. The ±32768 gap must use i16-wrapped `(target - current)` (not an
/// i32 widen of that subtract) so the sign matches the 65816; only the
/// subsequent `|diff|>>shift` widens so `i16::MIN` does not panic.
pub fn strat_chase_proportional(current: i16, target: i16, shift: u32) -> i16 {
    if current == target {
        return current;
    }
    let diff = target.wrapping_sub(current);
    let mut step = if diff >= 0 {
        diff >> shift
    } else {
        -((-(diff as i32) >> shift) as i16)
    };
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    current.wrapping_add(step)
}

/// C `Strat_SpeedTo` (sr_speedto, STRATROU.ASM:2707-2733). Returns true
/// when the target speed is reached.
pub fn strat_speed_to(al: &mut Alien, target_speed: u8, rate: u8) -> bool {
    // SR_SPEEDTO is byte arithmetic throughout. Its subtraction and `bpl`
    // interpret (vel-target) as signed i8, so gaps >=128 deliberately chase the
    // long way around the byte circle. Carry is set only when the target was
    // already reached on entry; a move or snap always returns clear.
    let diff = al.vel.wrapping_sub(target_speed) as i8;
    if diff == 0 {
        return true;
    }

    let magnitude = if diff < 0 {
        (0u8).wrapping_sub(diff as u8)
    } else {
        diff as u8
    };
    if magnitude < rate {
        al.vel = target_speed;
    } else if diff < 0 {
        al.vel = al.vel.wrapping_add(rate);
    } else {
        al.vel = al.vel.wrapping_sub(rate);
    }
    false
}

// ============================================================
// Percentage scaling (C strat_common.c:84-107, STRATROU.ASM perc*)
// ============================================================

/// C `Strat_Perc75`: val * 3/4 = val/2 + val/4 (arithmetic shifts).
pub fn strat_perc75(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 2)
}

/// C `Strat_Perc87`: val * 7/8.
pub fn strat_perc87(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 2).wrapping_add(val >> 3)
}

/// C `Strat_Perc62`: val * 5/8.
pub fn strat_perc62(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 3)
}

/// C `Strat_Perc56`: val * 9/16.
pub fn strat_perc56(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 4)
}

/// C `Strat_Perc93`: val * 15/16. ROM `perc93a_l` sums the shifted halves
/// (val>>1 + val>>2 + val>>3 + val>>4), NOT val - val>>4 — they differ by the
/// truncation (perc93(100)=93 not 94; perc93(-1)=-4 not 0). Oracle-verified.
pub fn strat_perc93(val: i16) -> i16 {
    (val >> 1)
        .wrapping_add(val >> 2)
        .wrapping_add(val >> 3)
        .wrapping_add(val >> 4)
}

// ============================================================
// Velocity vector generation (C strat_common.c:111-161)
// ============================================================

/// C `Strat_GenVecs2D` (alvelvecs_l, STRATROU.ASM:100-148): XZ velocity
/// from Y rotation + speed; vy = 0.
pub fn strat_gen_vecs_2d(al: &mut Alien) {
    use crate::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    let angle = al.roty as usize;
    let vel = al.vel as i8;
    al.vx = mulslog_mac8(vel, SINTAB[angle]) as i16;
    al.vy = 0;
    al.vz = mulslog_mac8(vel, COSTAB[angle]) as i16;
}

/// ROM `nvecs_l` (STRATROU.ASM:162-216): XZ from angle+speed for `s_gen_vecs`.
/// Table index is `-angle + 1` (nega then `inx`); does **not** write vy.
/// `vel` is the raw `al_vel` byte (signed speed, sign-extended).
pub fn strat_nvecs(angle: u8, vel: u8) -> (i16, i16) {
    use crate::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    let idx = angle.wrapping_neg().wrapping_add(1) as usize;
    let v = vel as i8;
    (
        mulslog_mac8(v, SINTAB[idx]) as i16,
        mulslog_mac8(v, COSTAB[idx]) as i16,
    )
}

/// Apply `nvecs_l` into `al_vx`/`al_vz` from `al_roty`/`al_vel` (vy untouched).
pub fn strat_gen_vecs_nvecs(al: &mut Alien) {
    let (vx, vz) = strat_nvecs(al.roty, al.vel);
    al.vx = vx;
    al.vz = vz;
}

/// C `Strat_GenVecs3D` (alvel3vecs_l, STRATROU.ASM:221-283): ROM-exact 3D
/// velocity from pitch and yaw.
pub fn strat_gen_vecs_3d(al: &mut Alien) {
    // `n3dvecs_l` negates YAW but does not negate pitch. Keeping a former
    // renderer-oriented pitch negation here made scripted vertical arcs run
    // backward in world simulation (notably DM_LB1's two-door entrance). Any
    // display-axis conversion belongs in the renderer, not world physics.
    use crate::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    let yaw = (al.roty as i8).wrapping_neg() as u8 as usize;
    let pitch = al.rotx as usize;
    let vel = al.vel as i8;
    let cosx = COSTAB[pitch];

    al.vx = mulslog_mac8(mulslog_mac8(vel, SINTAB[yaw]), cosx) as i16;
    al.vy = mulslog_mac8(vel, SINTAB[pitch]) as i16;
    al.vz = mulslog_mac8(mulslog_mac8(vel, COSTAB[yaw]), cosx) as i16;
}

/// ROM `sr_gen_3dvecs` / `sr_gen_3dvecs1..3` (STRATROU.ASM:2624):
/// `n3dvecs_l` then `al_vx/vy/vz = (x1/y1/z1) << shift`.
/// `shift` 0..=3 maps to the four ROM entry points.
pub fn strat_gen_vecs_3d_scaled(al: &mut Alien, shift: u32) {
    strat_gen_vecs_3d(al);
    debug_assert!(shift <= 3);
    al.vx = al.vx.wrapping_shl(shift);
    al.vy = al.vy.wrapping_shl(shift);
    al.vz = al.vz.wrapping_shl(shift);
}

/// C `Strat_GenSideVecs` (sidevecs_l): sideways vector, angle rotated 90
/// degrees (64 - roty).
pub fn strat_gen_side_vecs(al: &Alien) -> (i16, i16) {
    use crate::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    // ROM sidevecs_l: angle = (64 - roty) + 1 (an `inx` before the table
    // lookup; the port omitted the +1). Verified by sf-oracle.
    let angle = 65u8.wrapping_sub(al.roty) as usize;
    let vel = al.vel as i8;
    (
        mulslog_mac8(vel, SINTAB[angle]) as i16,
        mulslog_mac8(vel, COSTAB[angle]) as i16,
    )
}

/// C `Strat_GenFrontVecs` (frontvecs_l).
pub fn strat_gen_front_vecs(al: &Alien) -> (i16, i16) {
    use crate::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    // ROM frontvecs_l: angle = roty + 1 (an `inx` before the table lookup; the
    // port omitted the +1). Verified by sf-oracle.
    let angle = al.roty.wrapping_add(1) as usize;
    let vel = al.vel as i8;
    (
        mulslog_mac8(vel, SINTAB[angle]) as i16,
        mulslog_mac8(vel, COSTAB[angle]) as i16,
    )
}

// ============================================================
// Position update (C strat_common.c:165-177)
// ============================================================

/// C `Strat_ApplyVelocity` (addalvecs_l, STRATROU.ASM:518-535).
pub fn strat_apply_velocity(al: &mut Alien) {
    al.worldx = al.worldx.wrapping_add(al.vx);
    al.worldy = al.worldy.wrapping_add(al.vy);
    al.worldz = al.worldz.wrapping_add(al.vz);
}

/// C `Strat_AddToPos` (addvecs_l, STRATROU.ASM:497).
pub fn strat_add_to_pos(al: &mut Alien, dx: i16, dy: i16, dz: i16) {
    al.worldx = al.worldx.wrapping_add(dx);
    al.worldy = al.worldy.wrapping_add(dy);
    al.worldz = al.worldz.wrapping_add(dz);
}

// ============================================================
// Distance / angle (C strat_common.c:181-198)
// ============================================================

/// C `Strat_DistXZ` (xzdiffs_l, STRATROU.ASM:1796): a scaled Euclidean-ish
/// magnitude, NOT Manhattan. Proven bit-exact vs `xzdiffs_l` (sf-oracle
/// audit_coldet.rs).
pub fn strat_dist_xz(a: &Alien, b: &Alien) -> i16 {
    sf_core::aim_angle::xzdiffs(
        b.worldx.wrapping_sub(a.worldx),
        b.worldz.wrapping_sub(a.worldz),
    )
}

/// ROM `xydiffs_abs_l` (STRATROU.ASM:1532): Manhattan `|dx| + |dy|` into
/// `rangexz` (also used by `bossBrange_srou`). Distinct from
/// [`strat_dist_xz`]'s scaled-Euclidean XZ metric.
pub fn xy_diffs_abs(worldx: i16, worldy: i16, tx: i16, ty: i16) -> i16 {
    let mut dx = tx.wrapping_sub(worldx);
    if dx < 0 {
        dx = dx.wrapping_neg();
    }
    let mut dy = ty.wrapping_sub(worldy);
    if dy < 0 {
        dy = dy.wrapping_neg();
    }
    dx.wrapping_add(dy)
}

/// ROM `xydiffs_l` (STRATROU.ASM:1865): Manhattan `|obj2.x-obj1.x| +
/// |obj2.y-obj1.y|` into `rangexy` (s_jmp_XYdist* macros).
pub fn xy_diffs(a: &Alien, b: &Alien) -> i16 {
    xy_diffs_abs(a.worldx, a.worldy, b.worldx, b.worldy)
}

/// ROM `xzdiffs_abs_l` (STRATROU.ASM:1488): Manhattan `|tx-worldx| +
/// |tz-worldz|` into `rangexz`. Distinct from [`strat_dist_xz`] (scaled
/// Euclidean via `xzdiffs_l` / `xzdiffs_diffabs_l`).
pub fn xz_diffs_abs(worldx: i16, worldz: i16, tx: i16, tz: i16) -> i16 {
    let mut dx = tx.wrapping_sub(worldx);
    if dx < 0 {
        dx = dx.wrapping_neg();
    }
    let mut dz = tz.wrapping_sub(worldz);
    if dz < 0 {
        dz = dz.wrapping_neg();
    }
    dx.wrapping_add(dz)
}

/// ROM `xzdiffs_off_l` (STRATROU.ASM:1439): Manhattan from obj1 to
/// `(obj2.x+ox, obj2.z+oz)`.
pub fn xz_diffs_off(a: &Alien, b: &Alien, ox: i16, oz: i16) -> i16 {
    xz_diffs_abs(
        a.worldx,
        a.worldz,
        b.worldx.wrapping_add(ox),
        b.worldz.wrapping_add(oz),
    )
}

/// ROM `add2posobjyfobjx_l` (STRATROU.ASM:2476): write `dst` world XYZ as
/// `src` world XYZ plus `(ox, oy, oz)` (ROM scratch `x2/y2/z2`).
pub fn add2pos_obj_y_from_obj_x(src: &Alien, dst: &mut Alien, ox: i16, oy: i16, oz: i16) {
    dst.worldx = src.worldx.wrapping_add(ox);
    dst.worldy = src.worldy.wrapping_add(oy);
    dst.worldz = src.worldz.wrapping_add(oz);
}

/// ROM `anglexy_l` / `Yanglexy_l` (STRATROU.ASM:1633): yaw from src→dst in XZ.
/// Uses f32 atan2 exactly like the C build; the final cast truncates
/// through i32 like the x86 float->uint8 conversion.
pub fn strat_angle_xz(src: &Alien, dst: &Alien) -> u8 {
    sf_core::aim_angle::yanglexy(
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    )
}

/// ROM `anglexy_abs_l` (STRATROU.ASM:1405): yaw from `src` to absolute `(x,z)`.
pub fn strat_angle_xz_abs(src: &Alien, x: i16, z: i16) -> u8 {
    sf_core::aim_angle::yanglexy(x.wrapping_sub(src.worldx), z.wrapping_sub(src.worldz))
}

/// ROM `Xanglexy_l` (STRATROU.ASM:1676): elevation from src→dst.
/// Adjacent side is `xzdiffs_l` (scaled Euclidean).
pub fn strat_angle_yz(src: &Alien, dst: &Alien) -> u8 {
    sf_core::aim_angle::xanglexy(
        dst.worldy.wrapping_sub(src.worldy),
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    )
}

/// ROM `Xanglexabs_l` (STRATROU.ASM:1717): elevation from `src` to `(x,y,z)`.
/// Adjacent side is Manhattan from `xzdiffs_abs_l` (not scaled Euclid).
pub fn strat_angle_yz_abs(src: &Alien, x: i16, y: i16, z: i16) -> u8 {
    sf_core::aim_angle::xanglexabs(
        y.wrapping_sub(src.worldy),
        x.wrapping_sub(src.worldx),
        z.wrapping_sub(src.worldz),
    )
}

/// ROM `Yanglexabs_l` (STRATROU.ASM:1762): yaw from `src` to absolute `(x,z)`.
/// Same math as `strat_angle_xz_abs` (alias kept for ROM name parity).
#[inline]
pub fn strat_angle_y_abs(src: &Alien, x: i16, z: i16) -> u8 {
    strat_angle_xz_abs(src, x, z)
}

// ============================================================
// Object management (C strat_common.c:202-242)
// ============================================================

/// Flag the current alien for removal by the dostrats loop.
pub fn strat_remove_obj(g: &mut Game) {
    g.objs.aldead = 1;
}

/// ROM `childremove_Istrat` (GSTRATS.ASM): detach from mother, then remove self.
pub fn child_remove_istrat(g: &mut Game, idx: u16) {
    // divorce_family handles mother unlink when ASF4_CHILDOBJ is set.
    g.objs.divorce_family(idx);
    g.objs.aldead = 1;
}

/// ROM `s_add_playerZ` — scroll object with player view Z velocity.
pub fn add_player_z(g: &mut Game, idx: u16) {
    let v = g.vars.pviewvelz;
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(v);
}

/// ROM `s_init_COLanim x,#v`.
pub fn init_colanim(al: &mut Alien, v: u8) {
    al.colframe = v | 0x80;
}

/// ROM `s_add_COLanim x,#amt,#max` (wrap form).
pub fn add_colanim_wrap(al: &mut Alien, amount: u8, maxframes: u8) {
    let mut f = (al.colframe & 0x7F).wrapping_add(amount);
    if f >= maxframes {
        f = f.wrapping_sub(maxframes);
    }
    al.colframe = 0x80 | f;
}

/// ROM `flash_Istrat` (GSTRATS.ASM) — short colour-flash sprite.
pub fn flash_istrat(g: &mut Game, idx: u16) {
    let tick = crate::enemy_a::sid(g, flash_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        al.tx = 0;
        al.sflags2 |= ASF2_COLLDISABLE;
        init_colanim(al, 0);
    }
    add_player_z(g, idx);
}

/// ROM `flash_strat` — advance colanim; remove when frame reaches 2.
pub fn flash_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    let frame = g.objs.aliens[idx as usize].colframe & 0x7F;
    if frame >= 2 {
        g.objs.aldead = 1;
        return;
    }
    add_colanim_wrap(&mut g.objs.aliens[idx as usize], 1, 4);
}

/// ROM `s_kill_obj` (STRATMAC.INC): hp=0 + colldisable — death sweep runs expstrat.
pub fn kill_obj(al: &mut Alien) {
    al.hp = 0;
    al.sflags2 |= ASF2_COLLDISABLE;
}

/// ROM `kill_Istrat` / `kill_strat` (GSTRATS.ASM:1164) — one-shot kill.
pub fn kill_istrat(g: &mut Game, idx: u16) {
    kill_obj(&mut g.objs.aliens[idx as usize]);
}

/// Alias of [`kill_istrat`] (same ROM entry).
pub fn kill_strat(g: &mut Game, idx: u16) {
    kill_istrat(g, idx);
}

/// ROM `null_strat` (GSTRATS.ASM:1013) — empty end_strat (no-op tick).
pub fn null_strat(_g: &mut Game, _idx: u16) {}

// Stable extended-bank ids generated from the retail ShapeHdr records.
const SH_FIRE: u16 = 357;
// USHAPES.ASM smoke uses smoke_C and extent 40 after coordinate scaling.
// The neighboring fire/burn-mark shape has extent 188 and is not an alias.
const SH_SMOKE: u16 = 358;

/// Source-authored `s_make_smoke` / `s_damagesmoke` cadence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmokeCadence {
    EveryFrame,
    EveryOtherFrame,
    EveryFourthFrame,
    EveryEighthFrame,
}

impl SmokeCadence {
    fn frame_mask(self) -> u16 {
        match self {
            Self::EveryFrame => 0,
            Self::EveryOtherFrame => 1,
            Self::EveryFourthFrame => 3,
            Self::EveryEighthFrame => 7,
        }
    }

    pub fn is_due(self, gameframe: u16) -> bool {
        gameframe & self.frame_mask() == 0
    }
}

/// Live registry ids for fire/smoke strategies.
///
/// The registry is rebuilt on every level load while the WRAM mirror is
/// preserved. Consequently the old SID_* WRAM cache cannot be authoritative:
/// its ids refer to the previous level's registry. Resolve by function
/// identity in the live registry, as every other strategy lane does.
fn fire_smoke_strat_ids(g: &mut Game) -> (StratId, StratId, StratId, StratId) {
    let fire_i = crate::enemy_a::sid(g, fire_istrat);
    let fire_t = crate::enemy_a::sid(g, fire_strat);
    let smoke_i = crate::enemy_a::sid(g, smoke_p_istrat);
    let smoke_t = crate::enemy_a::sid(g, smoke_p_strat);
    g.vars.set_sv_u16(sv::SID_FIRE_SMOKE, fire_i.0 + 1);
    (fire_i, fire_t, smoke_i, smoke_t)
}

/// ROM `makefire_srou_l` (GSTRATS.ASM:1249) — attach a fire child; set `AFONFIRE`.
///
/// Period comes from [`sv::SMVAR_BYTE1`] (set by `s_damagefire` before the JSL).
pub fn makefire_srou(g: &mut Game, parent: u16) -> Option<u16> {
    let period = g.vars.sv_u8(sv::SMVAR_BYTE1);
    let (fire_i, _, _, _) = fire_smoke_strat_ids(g);
    let fire = strat_make_obj(g, SH_FIRE)?;
    // `s_make_obj` links the newborn immediately after the current source
    // object (same-pass first tick), as in make_exp_obj.
    g.objs.active_move_after(fire, parent);
    {
        let al = &mut g.objs.aliens[fire as usize];
        al.sbyte1 = period;
        al.stratptr = Some(fire_i);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags3 &= !ASF3_REALOBJ;
        al.sflags2 |= ASF2_COLLDISABLE;
        al.type_ |= ATZREMOVE;
        // s_rots_flat is cosmetic billboard; HD leaves orientation as-is.
    }
    // s_copy_pos y,x
    {
        let p = g.objs.aliens[parent as usize];
        let al = &mut g.objs.aliens[fire as usize];
        al.worldx = p.worldx;
        al.worldy = p.worldy;
        al.worldz = p.worldz;
    }
    {
        let al = &mut g.objs.aliens[parent as usize];
        al.fireobjptr = fire.wrapping_add(1);
        al.flags |= AFONFIRE;
    }
    Some(fire)
}

/// ROM `makesmoke_srou_l` (GSTRATS.ASM:1265) — spawn a drifting smoke puff.
pub fn makesmoke_srou(g: &mut Game, parent: u16) -> Option<u16> {
    let (_, _, smoke_i, _) = fire_smoke_strat_ids(g);
    let smoke = strat_make_obj(g, SH_SMOKE)?;
    // `s_make_obj` links the newborn immediately after the current source
    // object, so the puff runs its first drift tick on the creation pass.
    g.objs.active_move_after(smoke, parent);
    {
        let al = &mut g.objs.aliens[smoke as usize];
        al.sflags3 &= !ASF3_REALOBJ;
        al.sflags2 |= ASF2_COLLDISABLE;
        al.type_ |= ATZREMOVE;
        al.stratptr = Some(smoke_i);
        al.collstratptr = None;
        al.expstratptr = None;
    }
    {
        let p = g.objs.aliens[parent as usize];
        let al = &mut g.objs.aliens[smoke as usize];
        al.worldx = p.worldx;
        al.worldy = p.worldy;
        al.worldz = p.worldz;
    }
    Some(smoke)
}

/// ROM `s_make_smoke`: emit at the selected source-authored cadence.
pub fn make_smoke_on_cadence(g: &mut Game, parent: u16, cadence: SmokeCadence) -> Option<u16> {
    if cadence.is_due(g.vars.gameframe) {
        makesmoke_srou(g, parent)
    } else {
        None
    }
}

/// ROM `s_damagesmoke`: arm the damaged-object presentation and emit smoke
/// while durability is at or below the authored threshold.
pub fn damage_smoke_srou(
    g: &mut Game,
    parent: u16,
    durability_threshold: u8,
    cadence: SmokeCadence,
) -> Option<u16> {
    if g.objs.aliens[parent as usize].hp > durability_threshold {
        return None;
    }
    g.objs.aliens[parent as usize].flags |= AFONFIRE;
    make_smoke_on_cadence(g, parent, cadence)
}

// ============================================================
// Splash / engine flame (GSTRATS.ASM:1109-1444)
// ============================================================

const SH_SSPLASH: u16 = 359;
const SH_SPLASH: u16 = 360;
const SH_BOOSTSHAPE: u16 = 362;
/// Default parent shape extents when no catalog is wired (sh_Zmax / sh_Ymax).
const ENGINE_DEFAULT_ZMAX: i16 = 40;
const ENGINE_DEFAULT_YMAX: i16 = 48;

fn splash_strat_ids(g: &mut Game) -> (StratId, StratId) {
    let i = g.world.register_strategy(splash_istrat);
    let t = g.world.register_strategy(splash_strat);
    (i, t)
}

/// Shared body of `makesplash` / `makeSsplash` (GSTRATS.ASM:1114-1133).
fn domakesplash(g: &mut Game, parent: u16, shape: u16) -> Option<u16> {
    let (splash_i, _) = splash_strat_ids(g);
    let splash = strat_make_obj(g, shape)?;
    {
        let al = &mut g.objs.aliens[splash as usize];
        al.sflags3 &= !ASF3_REALOBJ;
        al.sflags2 |= ASF2_COLLDISABLE;
        al.stratptr = Some(splash_i);
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        al.tx = 0;
    }
    {
        let p = g.objs.aliens[parent as usize];
        let al = &mut g.objs.aliens[splash as usize];
        al.worldx = p.worldx;
        al.worldy = p.worldy;
        al.worldz = p.worldz.wrapping_sub(5);
    }
    splash_istrat(g, splash);
    Some(splash)
}

/// ROM `makesplash_srou_l` (GSTRATS.ASM:1112).
pub fn makesplash_srou(g: &mut Game, parent: u16) -> Option<u16> {
    domakesplash(g, parent, SH_SPLASH)
}

/// ROM `makeSsplash_srou_l` (GSTRATS.ASM:1109) — small splash shape.
pub fn makessplash_srou(g: &mut Game, parent: u16) -> Option<u16> {
    domakesplash(g, parent, SH_SSPLASH)
}

/// ROM `splash_Istrat` (GSTRATS.ASM:1135).
pub fn splash_istrat(g: &mut Game, idx: u16) {
    let (_, tick) = splash_strat_ids(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= ASF2_COLLDISABLE;
        init_colanim(al, 0);
        al.stratptr = Some(tick);
    }
    add_player_z(g, idx);
}

/// ROM `splash_strat` (GSTRATS.ASM:1142) — colanim 0..6 then remove.
pub fn splash_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    let f = (g.objs.aliens[idx as usize].colframe & 0x7f).wrapping_add(1);
    if f >= 7 {
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].colframe = 0x80 | f;
}

/// ROM `makeengine_srou_l` (GSTRATS.ASM:1403) — boost/engine flame child.
///
/// Shape extents default when no mesh catalog is attached; `relposz` stores
/// `-sh_Zmax` and the sprite scale is `sh_Ymax - 24`.
pub fn makeengine_srou(g: &mut Game, parent: u16) -> Option<u16> {
    makeengine_srou_with_extents(g, parent, ENGINE_DEFAULT_YMAX, ENGINE_DEFAULT_ZMAX)
}

/// Like [`makeengine_srou`] but with explicit parent shape Y/Z max extents.
pub fn makeengine_srou_with_extents(
    g: &mut Game,
    parent: u16,
    sh_ymax: i16,
    sh_zmax: i16,
) -> Option<u16> {
    let engine = strat_make_obj(g, SH_BOOSTSHAPE)?;
    let rel_z = (-sh_zmax) as i8 as u8;
    let sprite_y = sh_ymax.wrapping_sub(24);
    {
        let al = &mut g.objs.aliens[engine as usize];
        al.sflags2 |= ASF2_COLLDISABLE;
        al.sflags4 |= ASF4_INVISIBLE;
        al.relposz = rel_z;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        // `s_sprite_obj` stores its size operand as a byte.
        al.tx = sprite_y as u8;
    }
    {
        let al = &mut g.objs.aliens[parent as usize];
        al.fireobjptr = engine.wrapping_add(1);
        al.flags |= AFONFIRE;
    }
    updateengine_srou(g, parent);
    // ROM sets invisible AFTER updateengine (flame hidden until first real update).
    g.objs.aliens[engine as usize].sflags4 |= ASF4_INVISIBLE;
    Some(engine)
}

/// ROM `updateengine_srou_l` (GSTRATS.ASM:1438) — place flame at parent + (0,0,relposz).
pub fn updateengine_srou(g: &mut Game, parent: u16) -> bool {
    let raw = g.objs.aliens[parent as usize].fireobjptr;
    if raw == 0 {
        return false;
    }
    let engine = raw.wrapping_sub(1);
    if engine as usize >= NUMBER_AL || !g.objs.aliens[engine as usize].active {
        return false;
    }
    g.objs.aliens[engine as usize].sflags4 &= !ASF4_INVISIBLE;
    let p = g.objs.aliens[parent as usize];
    let oz = g.objs.aliens[engine as usize].relposz as i8;
    // ROM `s_add_Roffs2pos … #0,#0,relposz,1,1,0` — pitch then yaw, no roll.
    let (ox, oy, oz2) = crate::snes_trig::strat_roffs_pitch_yaw(p.rotx, p.roty, 0, 0, oz);
    {
        let al = &mut g.objs.aliens[engine as usize];
        al.worldx = p.worldx.wrapping_add(ox);
        al.worldy = p.worldy.wrapping_add(oy);
        al.worldz = p.worldz.wrapping_add(oz2);
    }
    true
}

/// ROM `boost_Istrat` (GSTRATS.ASM:715) — short-lived boost flame sprite.
pub fn boost_istrat(g: &mut Game, idx: u16) {
    let tick = g.world.register_strategy(boost_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 10; // s_set_lifecnt #10
        al.sflags2 |= ASF2_COLLDISABLE;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.type_ &= !ATZREMOVE; // s_setnoremove_behind
        al.stratptr = Some(tick);
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        // s_sprite_obj x,#0,svar_byte1 — sbyte1 is optional size from boost_sprite.
        al.tx = al.sbyte1;
    }
    // `boost_Istrat` is immediately followed by `boost_strat` in GSTRATS.ASM;
    // initialize the attachment position and consume the first lifetime tick
    // on the creation frame.
    boost_strat(g, idx);
}

/// ROM `boost_strat` (GSTRATS.ASM:723): park on `boostobj` + (0,0,boostZoff)
/// with flags 1,1,0, then `s_dec_lifecnt`.
pub fn boost_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.tx = al.tx.wrapping_sub(1);
    }
    let host = g.vars.sv_i16(sv::BOOSTOBJ);
    if host < 0 || host as usize >= NUMBER_AL || !g.objs.aliens[host as usize].active {
        g.objs.aldead = 1;
        return;
    }
    let host = host as u16;
    let h = g.objs.aliens[host as usize];
    let zoff = g.vars.sv_u8(sv::BOOSTZOFF) as i8;
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_pitch_yaw(h.rotx, h.roty, 0, 0, zoff);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = h.worldx.wrapping_add(rx);
        al.worldy = h.worldy.wrapping_add(ry);
        al.worldz = h.worldz.wrapping_add(rz);
    }
    // s_dec_lifecnt (no kill): DEC count; remove when result is 0.
    let al = &mut g.objs.aliens[idx as usize];
    al.count = al.count.wrapping_sub(1);
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

#[cfg(test)]
mod boost_field_tests {
    use super::*;

    #[test]
    fn boost_uses_typed_sprite_kind_and_second_collision_flag_byte() {
        let mut game = Game::new();
        let host = strat_make_obj(&mut game, 0).expect("boost host");
        let flame = strat_make_obj(&mut game, SH_BOOSTSHAPE).expect("boost flame");
        game.vars.set_sv_i16(sv::BOOSTOBJ, host as i16);

        boost_istrat(&mut game, flame);

        let flame = game.objs.aliens[usize::from(flame)];
        assert_eq!(flame.visual_kind, ObjectVisualKind::ScaledSprite);
        assert_eq!(flame.sflags, 0);
        assert_eq!(flame.sflags2 & ASF2_COLLDISABLE, ASF2_COLLDISABLE);
    }
}

/// ROM `boost_sprite` macro (STRATMAC.INC:7725) — spawn `#boostshape` with
/// `boost_Istrat`. Optional `size` → `al_sbyte1`, then the typed sprite scale.
/// The source assigns the initializer and links the new object immediately
/// after the current host; the host finishes its strategy before the child is
/// dispatched later in the same active-list pass.
pub fn boost_sprite(g: &mut Game, size: Option<u8>) -> Option<u16> {
    let flame = strat_make_obj(g, SH_BOOSTSHAPE)?;
    let init = g.world.register_strategy(boost_istrat);
    {
        let al = &mut g.objs.aliens[flame as usize];
        al.stratptr = Some(init);
        al.sflags4 |= ASF4_INVISIBLE; // cleared in boost_Istrat
        if let Some(s) = size {
            al.sbyte1 = s;
        }
    }
    let host = g.vars.sv_i16(sv::BOOSTOBJ);
    if host >= 0 && (host as usize) < NUMBER_AL && g.objs.aliens[host as usize].active {
        g.objs.active_move_after(flame, host as u16);
    }
    Some(flame)
}

/// Set `boostZoff` (signed byte) for subsequent `boost_strat` Roffs.
pub fn set_boost_zoff(g: &mut Game, zoff: i8) {
    g.vars.set_sv_u8(sv::BOOSTZOFF, zoff as u8);
}

/// ROM `fire_Istrat` (GSTRATS.ASM:1226) — arm smoke period counter.
pub fn fire_istrat(g: &mut Game, idx: u16) {
    let (_, fire_t, _, _) = fire_smoke_strat_ids(g);
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte2 = 9;
    al.stratptr = Some(fire_t);
}

/// ROM `fire_strat` (GSTRATS.ASM:1231) — emit smoke on period / mid-period ticks.
pub fn fire_strat(g: &mut Game, idx: u16) {
    // s_rots_flat — cosmetic.
    let sbyte2 = g.objs.aliens[idx as usize].sbyte2;
    let next = sbyte2.wrapping_sub(1);
    if next != 0 {
        g.objs.aliens[idx as usize].sbyte2 = next;
        if next == 8 || next == 16 {
            let _ = makesmoke_srou(g, idx);
        }
        return;
    }
    // Counter hit 0: reload from sbyte1 and emit smoke.
    let period = g.objs.aliens[idx as usize].sbyte1;
    g.objs.aliens[idx as usize].sbyte2 = period;
    let _ = makesmoke_srou(g, idx);
}

/// ROM `smokeP_Istrat` (GSTRATS.ASM:1296).
pub fn smoke_p_istrat(g: &mut Game, idx: u16) {
    let (_, _, _, smoke_t) = fire_smoke_strat_ids(g);
    let al = &mut g.objs.aliens[idx as usize];
    al.visual_kind = ObjectVisualKind::ScaledSprite;
    al.depthoffset = 0;
    al.tx = 0;
    al.stratptr = Some(smoke_t);
    al.collstratptr = None;
    al.expstratptr = None;
    al.sbyte1 = 20;
    al.sword1 = 6;
    init_colanim(al, 0);
    // ASM falls through from the initializer label straight into smokeP_strat,
    // so the creation pass already applies the first drift tick
    // (worldx-1, worldy-sword1) — retail tick-1733 puffs sit at the drifted
    // position in their birth-frame snapshot.
    smoke_p_strat(g, idx);
}

/// ROM `smokeP_strat` (GSTRATS.ASM:1304) — drift up/left, expire on anim wrap or lift.
pub fn smoke_p_strat(g: &mut Game, idx: u16) {
    // s_rots_flat — cosmetic.
    add_colanim_wrap(&mut g.objs.aliens[idx as usize], 1, 8);
    if g.objs.aliens[idx as usize].colframe & 0x7F == 0 {
        g.objs.aldead = 1;
        strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_sub(1);
        al.worldy = al.worldy.wrapping_sub(al.sword1);
        let s1 = al.sbyte1.wrapping_sub(1);
        if s1 != 0 {
            al.sbyte1 = s1;
        } else {
            al.sbyte1 = 20;
            al.sword1 = al.sword1.wrapping_sub(1);
            if al.sword1 == 0 {
                g.objs.aldead = 1;
            }
        }
    }
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `puff_Istrat` (GSTRATS.ASM:1280).
pub fn puff_istrat(g: &mut Game, idx: u16) {
    let tick = crate::enemy_a::sid(g, puff_strat);
    g.vars.set_sv_u16(sv::SID_PUFF, tick.0 + 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags2 |= ASF2_COLLDISABLE;
        init_colanim(al, 0);
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        al.tx = 0;
    }
}

/// ROM `puff_strat` (GSTRATS.ASM:1286).
pub fn puff_strat(g: &mut Game, idx: u16) {
    // s_rots_flat — cosmetic.
    add_colanim_wrap(&mut g.objs.aliens[idx as usize], 1, 9);
    if g.objs.aliens[idx as usize].colframe & 0x7F == 8 {
        g.objs.aldead = 1;
        return;
    }
    add_player_z(g, idx);
}

const SH_PEXPLOD: u16 = 361;

/// ROM `rotsflatstay_Istrat` (GSTRATS.ASM:1200) — billboard, no collide, no strat.
pub fn rotsflatstay_istrat(g: &mut Game, idx: u16) {
    let [pitch, yaw] = flat_billboard_rotation(&g.vars);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotx = pitch;
    al.roty = yaw;
    al.sflags2 |= ASF2_COLLDISABLE;
    al.stratptr = None;
    al.collstratptr = None;
    al.expstratptr = None;
}

fn sparky_strat_id(g: &mut Game) -> StratId {
    let id = crate::enemy_a::sid(g, sparky_strat);
    g.vars.set_sv_u16(sv::SID_SPARKY, id.0 + 1);
    id
}

/// ROM `sparky_Istrat` (GSTRATS.ASM:1207) — short explosion flash, 2-frame life.
pub fn sparky_istrat(g: &mut Game, idx: u16) {
    let sid = sparky_strat_id(g);
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte1 = 2;
    al.shape = SH_PEXPLOD;
    al.sflags2 |= ASF2_COLLDISABLE;
    al.stratptr = Some(sid);
    al.expstratptr = Some(sid);
    al.collstratptr = Some(sid);
    // s_rots_flat — cosmetic.
}

/// ROM `sparky_strat` / `endsparky_strat` (GSTRATS.ASM:1217).
pub fn sparky_strat(g: &mut Game, idx: u16) {
    let s1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    if s1 != 0 {
        g.objs.aliens[idx as usize].sbyte1 = s1;
        return; // endsparky_strat: just end
    }
    g.objs.aldead = 1;
}

/// C `Strat_InitObjVars` (init_objvars_l) — re-exported from sf-game where
/// the map-VM spawn path already carries it.
pub use sf_game::obj::strat_init_obj_vars;

/// C `Strat_MakeObj` (s_make_obj + makeobj_l): alloc + init vars + shape.
pub fn strat_make_obj(g: &mut Game, shape_id: u16) -> Option<u16> {
    let idx = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].shape = shape_id;
    Some(idx)
}

/// C `Strat_CountDown`: decrement `al->count`, true when it has reached 0.
pub fn strat_count_down(al: &mut Alien) -> bool {
    if al.count > 0 {
        al.count -= 1;
        return false;
    }
    true
}

// ============================================================
// Shared lightweight projectile (C strat_common.c:244-319)
// ============================================================

/// C `projectile_mark_dead` (strat_common.c:244).
fn projectile_mark_dead(g: &mut Game, idx: u16) {
    let sbyte6 = g.objs.aliens[idx as usize].sbyte6;
    if sbyte6 & 1 != 0 {
        g.objs.aliens[idx as usize].sbyte6 = sbyte6 & !1u8;
        let n = g.vars.sv_u8(sv::NUMPLASERS);
        if n > 0 {
            g.vars.set_sv_u8(sv::NUMPLASERS, n - 1);
        }
    }
    g.objs.aldead = 1;
}

/// C `Strat_ProjectileOnCollide`.
pub fn strat_projectile_on_collide(g: &mut Game, idx: u16) {
    projectile_mark_dead(g, idx);
}

/// C `Strat_ProjectileTick`.
pub fn strat_projectile_tick(g: &mut Game, idx: u16) {
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);

    let count = g.objs.aliens[idx as usize].count;
    let count = if count > 0 { count - 1 } else { count };
    g.objs.aliens[idx as usize].count = count;
    if count == 0 {
        projectile_mark_dead(g, idx);
        return;
    }

    // Obj_GetPlayer(): slot 0 when active (src/game/obj.c:125).
    if let Some(player) = g.objs.player() {
        let dz = g.objs.aliens[idx as usize]
            .worldz
            .wrapping_sub(player.worldz);
        if !(-12000..=12000).contains(&dz) {
            projectile_mark_dead(g, idx);
        }
    }
}

/// Registry ids of the projectile tick/collide strategies, registering
/// them on first use (C takes the function addresses directly).
pub fn projectile_strat_ids(g: &mut Game) -> (StratId, StratId) {
    let tick = crate::enemy_a::sid(g, strat_projectile_tick);
    let coll = crate::enemy_a::sid(g, strat_projectile_on_collide);
    g.vars.set_sv_u16(sv::SID_PROJ, tick.0 + 1);
    (tick, coll)
}

/// C `Strat_SpawnProjectile` (strat_common.c:278). `owner` is an alien
/// slot index (C passes the pointer). Returns the new slot.
#[allow(clippy::too_many_arguments)]
pub fn strat_spawn_projectile(
    g: &mut Game,
    owner: Option<u16>,
    off_x: i16,
    off_y: i16,
    off_z: i16,
    rot_x: u8,
    rot_y: u8,
    speed: u8,
    lifetime: u8,
    ap: u8,
    coll_type_bit: u8,
) -> Option<u16> {
    let (tick_id, coll_id) = projectile_strat_ids(g);
    let idx = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    if let Some(anchor) = owner {
        // Weapon construction occurs with the firer as the current object,
        // so the new projectile must execute immediately after its owner.
        g.objs.active_move_after(idx, anchor);
    }

    let owner_al = owner
        .filter(|&o| (o as usize) < NUMBER_AL)
        .map(|o| g.objs.aliens[o as usize]);

    let al = &mut g.objs.aliens[idx as usize];
    // This shared helper backs the path and legacy enemy laser lanes. Their
    // default projectile is the retail elaser2 needle; specialized plasma
    // callers overwrite it after allocation.
    al.shape = 511;
    al.sflags4 &= !ASF4_INVISIBLE;
    al.type_ |= ATLASER | ATZREMOVE;

    al.stratptr = Some(tick_id);
    al.collstratptr = Some(coll_id);
    al.expstratptr = Some(coll_id);

    al.count = if lifetime != 0 { lifetime } else { 40 };
    al.hp = 1;
    al.ap = ap;
    al.vel = speed;
    al.rotx = rot_x;
    al.roty = rot_y;

    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | coll_type_bit;

    match (owner, owner_al) {
        (Some(o), Some(own)) => {
            al.worldx = own.worldx.wrapping_add(off_x);
            al.worldy = own.worldy.wrapping_add(off_y);
            al.worldz = own.worldz.wrapping_add(off_z);
            // C: al->immuneptr = (uint16)(owner - g_aliens) — raw index.
            al.immuneptr = o;
        }
        _ => {
            al.worldx = off_x;
            al.worldy = off_y;
            al.worldz = off_z;
        }
    }

    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    Some(idx)
}

// ============================================================
// Sound (C strat_common.c:323)
// ============================================================

/// C `Strat_TrigSE` (trigse macro, MACROS.INC).
pub fn strat_trig_se(g: &mut Game, sound_id: u8) {
    g.hooks.play_se(sound_id);
}

// ============================================================
// End-seq boss mark + game-strat init (ENDSEQ.ASM / GSTRATS.ASM)
// ============================================================

/// Append a semantic encounter to the end-sequence replay list unless it
/// duplicates the previous entry.
pub fn setboss_l(g: &mut Game, encounter: sf_game::vars::BossEncounter) {
    g.vars.mark_boss_encounter(encounter);
}

/// ROM `find_object_l` / `findtarget_l` (STRATROU.ASM:555).
///
/// Walk the active list starting at `search_from` (ROM `fobj`). When
/// `shape == 0`, return the next `ASF3_REALOBJ` after `search_from`, skipping
/// `self_idx`. Otherwise return the next object whose `al_shape` matches,
/// also skipping `self_idx`. Updates `*search_from` to the found object's
/// successor (ROM `fobj = _next`) so callers can iterate.
pub fn find_object(
    g: &Game,
    shape: u16,
    self_idx: u16,
    search_from: &mut Option<u16>,
) -> Option<u16> {
    use sf_game::alien::ASF3_REALOBJ;

    let Some(mut cur) = *search_from else {
        return None;
    };
    if shape == 0 {
        // get_anyobj: start at _next of fobj, skip self, require realobj.
        loop {
            let Some(next) = g.objs.aliens[cur as usize].next else {
                *search_from = None;
                return None;
            };
            cur = next;
            if cur == self_idx {
                continue;
            }
            if g.objs.aliens[cur as usize].sflags3 & ASF3_REALOBJ == 0 {
                continue;
            }
            *search_from = Some(cur);
            return Some(cur);
        }
    }
    // Shape match: walk from fobj inclusive, skip self on match.
    loop {
        if g.objs.aliens[cur as usize].shape == shape && cur != self_idx {
            *search_from = g.objs.aliens[cur as usize].next;
            return Some(cur);
        }
        match g.objs.aliens[cur as usize].next {
            Some(n) => cur = n,
            None => {
                *search_from = None;
                return None;
            }
        }
    }
}

fn xz_range(g: &Game, a: u16, b: u16) -> i16 {
    let aa = &g.objs.aliens[a as usize];
    let bb = &g.objs.aliens[b as usize];
    // ROM find_nearobject_l → jsl xzdiffs_l (scaled Euclidean), not Manhattan.
    strat_dist_xz(aa, bb)
}

/// ROM `find_nearobject_l` / `findntarget_l` (STRATROU.ASM:697).
///
/// Nearest object of `shape` with `min_r <= rangexz < max_r` where `rangexz`
/// is [`strat_dist_xz`] (`xzdiffs_l`). When `shape == 0`, delegates to
/// [`find_any_near_object`]. Updates `search_from` to the successor of the
/// walk head (ROM `fobj = _next` of last).
pub fn find_near_object(
    g: &Game,
    shape: u16,
    self_idx: u16,
    min_r: i16,
    max_r: i16,
    search_from: &mut Option<u16>,
) -> Option<u16> {
    if shape == 0 {
        return find_any_near_object(g, self_idx, min_r, max_r, search_from);
    }
    let Some(start) = *search_from else {
        return None;
    };
    let mut best: Option<u16> = None;
    let mut best_r = max_r;
    let mut cur = Some(start);
    while let Some(c) = cur {
        if c != self_idx && g.objs.aliens[c as usize].shape == shape {
            let r = xz_range(g, self_idx, c);
            if r < best_r && r >= min_r {
                best_r = r;
                best = Some(c);
            }
        }
        cur = g.objs.aliens[c as usize].next;
    }
    // ROM sets fobj to _next of the last scanned node (end of list → 0).
    *search_from = None;
    best
}

/// ROM `find_anynearobject_l` (STRATROU.ASM:761) — nearest `ASF3_REALOBJ`.
pub fn find_any_near_object(
    g: &Game,
    self_idx: u16,
    min_r: i16,
    max_r: i16,
    search_from: &mut Option<u16>,
) -> Option<u16> {
    use sf_game::alien::ASF3_REALOBJ;
    let Some(start) = *search_from else {
        return None;
    };
    let mut best: Option<u16> = None;
    let mut best_r = max_r;
    let mut cur = Some(start);
    while let Some(c) = cur {
        if c != self_idx && g.objs.aliens[c as usize].sflags3 & ASF3_REALOBJ != 0 {
            let r = xz_range(g, self_idx, c);
            if r < best_r && r >= min_r {
                best_r = r;
                best = Some(c);
            }
        }
        cur = g.objs.aliens[c as usize].next;
    }
    *search_from = None;
    best
}

/// ROM `find_radiusobject_l` (STRATROU.ASM:815) — first shape match in
/// `[min_r, max_r)` (list order, not nearest).
pub fn find_radius_object(
    g: &Game,
    shape: u16,
    self_idx: u16,
    min_r: i16,
    max_r: i16,
    search_from: &mut Option<u16>,
) -> Option<u16> {
    if shape == 0 {
        return find_any_radius_object(g, self_idx, min_r, max_r, search_from);
    }
    let Some(start) = *search_from else {
        return None;
    };
    let mut cur = Some(start);
    while let Some(c) = cur {
        if c != self_idx && g.objs.aliens[c as usize].shape == shape {
            let r = xz_range(g, self_idx, c);
            if r < max_r && r >= min_r {
                *search_from = g.objs.aliens[c as usize].next;
                return Some(c);
            }
        }
        cur = g.objs.aliens[c as usize].next;
    }
    *search_from = None;
    None
}

/// ROM `find_anyradiusobject_l` (STRATROU.ASM:874).
pub fn find_any_radius_object(
    g: &Game,
    self_idx: u16,
    min_r: i16,
    max_r: i16,
    search_from: &mut Option<u16>,
) -> Option<u16> {
    use sf_game::alien::ASF3_REALOBJ;
    let Some(start) = *search_from else {
        return None;
    };
    let mut cur = Some(start);
    while let Some(c) = cur {
        if c != self_idx && g.objs.aliens[c as usize].sflags3 & ASF3_REALOBJ != 0 {
            let r = xz_range(g, self_idx, c);
            if r < max_r && r >= min_r {
                *search_from = g.objs.aliens[c as usize].next;
                return Some(c);
            }
        }
        cur = g.objs.aliens[c as usize].next;
    }
    *search_from = None;
    None
}

/// ROM `find_Mobject_l` (STRATROU.ASM:619) — next shape (or any) with
/// `(sflags & mask) != 0`.
pub fn find_mobject(
    g: &Game,
    shape: u16,
    self_idx: u16,
    sflags_mask: u8,
    search_from: &mut Option<u16>,
) -> Option<u16> {
    let Some(start) = *search_from else {
        return None;
    };
    let mut cur = Some(start);
    while let Some(c) = cur {
        if c != self_idx {
            let al = &g.objs.aliens[c as usize];
            let shape_ok = shape == 0 || al.shape == shape;
            if shape_ok && al.sflags & sflags_mask != 0 {
                *search_from = al.next;
                return Some(c);
            }
        }
        cur = g.objs.aliens[c as usize].next;
    }
    *search_from = None;
    None
}

/// ROM `find_sword1_l` (DSTRATS.ASM:3608) — first alien whose `sword1` equals
/// `target` (usually the caller's index).
pub fn find_sword1(g: &Game, target: i16) -> Option<u16> {
    for idx in g.objs.active_indices() {
        if g.objs.aliens[idx as usize].sword1 == target {
            return Some(idx);
        }
    }
    None
}

/// Decode a mother/child `sword1`/`ptr` link (index+1, 0 = end).
fn child_link_index(raw: i16) -> Option<u16> {
    if raw == 0 {
        return None;
    }
    let idx = (raw as u16).wrapping_sub(1);
    if (idx as usize) >= NUMBER_AL {
        return None;
    }
    Some(idx)
}

/// ROM `setobjtobechildyx_srou` (STRATROU.ASM:3054) — walk mother `x`'s
/// child list (`sword1` chain, index+1 links) for the child whose
/// `sbyte1 == child_num`. Returns that child index, or `None` if the chain
/// ends without a match (ROM leaves Y=0).
pub fn set_obj_to_be_child_yx(g: &Game, mother: u16, child_num: u8) -> Option<u16> {
    let mut cur = g.objs.aliens[mother as usize].sword1;
    while let Some(idx) = child_link_index(cur) {
        if !g.objs.aliens[idx as usize].active {
            break;
        }
        if g.objs.aliens[idx as usize].sbyte1 == child_num {
            return Some(idx);
        }
        cur = g.objs.aliens[idx as usize].sword1;
    }
    None
}

/// ROM `setobjtobechildxy_srou` (STRATROU.ASM:3074) — same walk starting from
/// mother in `y` (HD: same as [`set_obj_to_be_child_yx`]).
pub fn set_obj_to_be_child_xy(g: &Game, mother: u16, child_num: u8) -> Option<u16> {
    set_obj_to_be_child_yx(g, mother, child_num)
}

/// ROM `modechangeset_l` (DSTRATS.ASM:4719).
pub fn modechange_set(g: &mut Game, idx: u16, state: u8) {
    g.objs.aliens[idx as usize].stratstate = state;
}

/// ROM `modechangeadd_l` (DSTRATS.ASM:4711).
pub fn modechange_add(g: &mut Game, idx: u16, delta: u8) {
    let al = &mut g.objs.aliens[idx as usize];
    al.stratstate = al.stratstate.wrapping_add(delta);
}

/// ROM `floatvar1` / `floatvar2` WRAM (GILESALC / player float oscillators).
pub const WM_FLOATVAR1: u16 = 0x1569;
pub const WM_FLOATVAR2: u16 = 0x156A;

/// ROM `s_set_var2vartab ...,sintab,scale` — sign-extend `SINTAB[angle]`, then
/// arithmetic shift by `scale` (negative = divide toward zero by 2^|scale|).
fn sintab_scaled(angle: u8, scale: i32) -> i16 {
    use crate::snes_trig::SINTAB;
    let v = SINTAB[angle as usize] as i16;
    if scale > 0 {
        v << scale
    } else if scale < 0 {
        v / (1i16 << (-scale))
    } else {
        v
    }
}

/// ROM `flout_srou` (GSTRATS.ASM:2916) — `x1 = floatvar1 + idx`, `x2 = floatvar2 + idx`.
pub fn flout_srou(g: &Game, idx: u16) -> (u8, u8) {
    let [fv1, fv2] = g.vars.shared.float_variables;
    (fv1.wrapping_add(idx as u8), fv2.wrapping_add(idx as u8))
}

/// ROM `float_cont` (GSTRATS.ASM:2912) — add `(tpx,tpy)` to world x/y.
pub fn float_cont(g: &mut Game, idx: u16, tpx: i16, tpy: i16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_add(tpx);
    al.worldy = al.worldy.wrapping_add(tpy);
}

fn float_srou(g: &mut Game, idx: u16, scale: i32) {
    let (x1, x2) = flout_srou(g, idx);
    let tpx = sintab_scaled(x1, scale);
    let tpy = sintab_scaled(x2, scale);
    float_cont(g, idx, tpx, tpy);
}

/// ROM `float256_srou_l` — sintab scale −1.
pub fn float256_srou(g: &mut Game, idx: u16) {
    float_srou(g, idx, -1);
}

/// ROM `float128_srou_l` — sintab scale −2.
pub fn float128_srou(g: &mut Game, idx: u16) {
    float_srou(g, idx, -2);
}

/// ROM `float64_srou_l` — sintab scale −3 (already True; keep public).
pub fn float64_srou(g: &mut Game, idx: u16) {
    float_srou(g, idx, -3);
}

/// ROM `float32_srou_l` — sintab scale −4.
pub fn float32_srou(g: &mut Game, idx: u16) {
    float_srou(g, idx, -4);
}

/// ROM `count_shapes_l` (STRATROU.ASM:948) — count active aliens with `shape`.
pub fn count_shapes(g: &Game, shape: u16) -> u16 {
    let mut n = 0u16;
    for idx in g.objs.active_indices() {
        if g.objs.aliens[idx as usize].shape == shape {
            n = n.wrapping_add(1);
        }
    }
    n
}

/// ROM `set_0collptrsx_l` / `set_0collptrsy_l` — clear coll/exp strat ptrs.
pub fn set_0_collptrs(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.collstratptr = None;
    al.expstratptr = None;
}

/// ROM `set_normcollptrsx_l` / `set_normcollptrsy_l` — hitflash + explode.
pub fn set_norm_collptrs(g: &mut Game, idx: u16) {
    use crate::enemy_a::{strat_explode, strat_hit_flash};
    let coll = g.world.register_strategy(strat_hit_flash);
    let exp = g.world.register_strategy(strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
}

/// ROM `Xflytopos_l` (GSTRATS.ASM:2937) — bank toward `target_x` via sbyte3
/// accel (±10), add to rotz (±45), then achase worldx.
pub fn x_fly_to_pos(g: &mut Game, idx: u16, target_x: i16) {
    let al = &mut g.objs.aliens[idx as usize];
    let dx = target_x.wrapping_sub(al.worldx);
    let mut sb3 = al.sbyte3 as i8;
    if dx >= 0 {
        sb3 = sb3.wrapping_add(1);
    } else {
        sb3 = sb3.wrapping_sub(1);
    }
    sb3 = sb3.clamp(-10, 10);
    al.sbyte3 = sb3 as u8;
    let mut rotz = al.rotz as i8;
    rotz = rotz.wrapping_add(sb3).clamp(-64, 64); // ±deg45
    al.rotz = rotz as u8;
    al.worldx = al.worldx.wrapping_add(sb3 as i16);
    // s_achase_alvar W worldx target 7
    let mut wx = al.worldx;
    let d = target_x.wrapping_sub(wx);
    wx = wx.wrapping_add(d >> 7);
    al.worldx = wx;
}

/// ROM `jumptostate_l` (DSTRATS.ASM:4723) — index a far state table by
/// `al_stratstate` and return the 24-bit entry (bank:addr) as `(bank, addr)`.
///
/// Table layout: 4 bytes per state — `[bank:u8][addr:u16 LE][pad:u8]`.
/// HD strategies usually dispatch via `stratstate` enums instead; this leaf
/// preserves the ROM table lookup for ports that still carry a state table.
pub fn jump_to_state(stratstate: u8, table: &[[u8; 4]]) -> Option<(u8, u16)> {
    let i = stratstate as usize;
    let entry = table.get(i)?;
    let bank = entry[0];
    let addr = u16::from_le_bytes([entry[1], entry[2]]);
    Some((bank, addr))
}

/// ROM `initgame_strats_l` (GSTRATS.ASM:42) — clear view/boss/game flags at
/// game start. HD skips the live strat sweep + dummyobj spawn (shell owns
/// those); this covers the flag/view reset leaf.
pub fn initgame_strats_l(g: &mut Game) {
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    g.vars.set_sv_i16(sv::OUTVZ, 0);
    g.vars.bossmaxhp = 0;
    g.vars.bosshp = 0;
    g.vars.gameflags = 0;
    // gf2_ingame (GSTRATS.ASM:57).
    const GF2_INGAME: u8 = 1; // bit0 of gameflags2 in ROM usage here
    g.vars.shared.game_flags2 = GF2_INGAME;
    crate::enemy_a::set_bossflags(g, 0);
    g.vars.shared.strategy_flags = 0;
}

// ============================================================
// Unprefixed aliases — the canonical short names the other strat lanes'
// compat modules document (`crate::common::chase` etc.); consolidation
// swaps their private duplicates for these.
// ============================================================
pub use self::strat_add_to_pos as add_to_pos;
pub use self::strat_angle_xz as angle_xz;
pub use self::strat_angle_xz_abs as angle_xz_abs;
pub use self::strat_angle_y_abs as angle_y_abs;
pub use self::strat_angle_yz as angle_yz;
pub use self::strat_angle_yz_abs as angle_yz_abs;
pub use self::strat_apply_velocity as apply_velocity;
pub use self::strat_chase as chase;
pub use self::strat_chase8 as chase8;
pub use self::strat_chase_proportional as chase_proportional;
pub use self::strat_count_down as count_down;
pub use self::strat_dist_xz as dist_xz;
pub use self::strat_gen_front_vecs as gen_front_vecs;
pub use self::strat_gen_side_vecs as gen_side_vecs;
pub use self::strat_gen_vecs_2d as gen_vecs_2d;
pub use self::strat_gen_vecs_3d as gen_vecs_3d;
pub use self::strat_make_obj as make_obj;
pub use self::strat_perc56 as perc56;
pub use self::strat_perc62 as perc62;
pub use self::strat_perc75 as perc75;
pub use self::strat_perc87 as perc87;
pub use self::strat_perc93 as perc93;
pub use self::strat_projectile_on_collide as projectile_on_collide;
pub use self::strat_projectile_tick as projectile_tick;
pub use self::strat_remove_obj as remove_obj;
pub use self::strat_spawn_projectile as spawn_projectile;
pub use self::strat_speed_to as speed_to;
pub use self::strat_trig_se as trig_se;

/// ROM `flashturq_l` (WINDOWS.ASM:104) — full cyan screen hitflash.
pub fn flashturq_l(g: &mut Game) {
    g.hooks.flash_turq();
}

/// ROM `flashturq2_l` (WINDOWS.ASM:125) — dim cyan screen hitflash.
pub fn flashturq2_l(g: &mut Game) {
    g.hooks.flash_turq2();
}

/// ROM `flashred_l` (WINDOWS.ASM:146) — red screen hitflash.
pub fn flashred_l(g: &mut Game) {
    g.hooks.flash_red();
}

/// ROM `dealloc_window hitflash` (GSTRATS.ASM screenflash disable path).
pub fn hitflash_off_l(g: &mut Game) {
    g.hooks.hitflash_off();
}
