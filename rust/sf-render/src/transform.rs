//! View/projection matrices and SNES->GL coordinate conversion.
//!
//! Port (C oracle): `src/renderer/transform.c`, statics folded into the
//! [`Transform`] struct. All semantics — including the diag(1,-1,-1)
//! SNES->GL conversion, the ZYX rotation order, `Transform_SetViewLerp`'s
//! big-jump camera-cut detection, and the render-camera mirror consumed by
//! the 2D background layer — are transcribed 1:1.

/// Camera state (SNES-convention inputs) kept for per-render-frame
/// interpolation: positions are fixed 16.16 (+Y down), rotations SNES
/// 0-255 angle units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CameraState {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub rx: i16,
    pub ry: i16,
    pub rz: i16,
}

pub struct Transform {
    projection: [f32; 16],
    view: [f32; 16],
    sin_table: [f32; 256],
    cos_table: [f32; 256],
    cam_prev: CameraState,
    cam_curr: CameraState,
    cam_valid: bool,
    /// Camera state actually used for the most recent view-matrix build
    /// (render-frame interpolated values, SNES conventions). Read-only
    /// mirror for the 2D background layer (SNES calcbgscroll_l coupling).
    cam_render: CameraState,
    /// Fractional (sub-angle-unit) signed render-camera pitch/yaw in SNES
    /// units, kept alongside `cam_render` so the 2D background horizon
    /// scroll glides with the interpolated 3D view instead of stepping once
    /// per 20 Hz tick (see `set_view_lerp`).
    cam_render_pitch_f: f32,
    cam_render_yaw_f: f32,
}

pub fn identity(m: &mut [f32; 16]) {
    *m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
}

pub fn multiply(out: &mut [f32; 16], a: &[f32; 16], b: &[f32; 16]) {
    let mut tmp = [0.0f32; 16];
    for i in 0..4 {
        for j in 0..4 {
            let mut acc = 0.0;
            for k in 0..4 {
                acc += a[k * 4 + i] * b[j * 4 + k];
            }
            tmp[j * 4 + i] = acc;
        }
    }
    *out = tmp;
}

pub fn lerp(out: &mut [f32; 16], a: &[f32; 16], b: &[f32; 16], alpha: f32) {
    for i in 0..16 {
        out[i] = a[i] + (b[i] - a[i]) * alpha;
    }
}

/// Convert fixed 16.16 to float.
#[inline]
fn fp16_to_float(val: i32) -> f32 {
    val as f32 / 65536.0
}

// Shortest-path interpolation in the SNES 0-255 angle system, at
// FRACTIONAL precision. The original returned an int16 (`a8 + (int)(diff*t)`,
// transform.c:197), so for the common small per-tick delta (|diff| <= 1 unit
// = 1.4 deg during steady banking/turning) the integer truncation collapsed
// every intra-tick sample back to `a8`: the camera rotation snapped once per
// 20 Hz tick while position glided at the render rate — the residual flight
// "stutter". Returning the un-truncated fractional angle (fed to a
// table-interpolated sin/cos in `sincos_frac`) lets rotation glide too.
// Returns the wrapped angle in [0, 256).
fn lerp_cam_angle_f(from: i16, to: i16, t: f32, big_jump: &mut bool) -> f32 {
    let a8 = (from & 0xFF) as i32;
    let b8 = (to & 0xFF) as i32;
    let mut diff = b8 - a8;
    if diff > 127 {
        diff -= 256;
    }
    if diff < -128 {
        diff += 256;
    }
    // Genuine camera cuts are detected ROM-faithfully upstream (viewtype change
    // -> CameraSnapshot.snap -> snap_camera); this heuristic is only a fallback
    // for a within-viewtype discontinuity. At 48 (~67 deg/tick) it misfired on
    // fast CONTINUOUS rotation (the intro tunnel 360-degree follow), snapping
    // every tick -> the camera "wobbled" at 20 Hz. Only trip near the 180-degree
    // shortest-path ambiguity, where interpolation direction is genuinely
    // undefined; everything below that interpolates smoothly.
    if diff > 120 || diff < -120 {
        *big_jump = true; // ~169 deg in one tick: treat as a flip/cut
    }
    (a8 as f32 + diff as f32 * t).rem_euclid(256.0)
}

// SNES 0-255 angle -> signed [-128, 128) in the same units (float-safe).
#[inline]
fn signed_angle_f(a: f32) -> f32 {
    let m = a.rem_euclid(256.0);
    if m >= 128.0 {
        m - 256.0
    } else {
        m
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform {
    pub fn new() -> Self {
        // Build sin/cos lookup table for SNES 256-degree angle system.
        let mut sin_table = [0.0f32; 256];
        let mut cos_table = [0.0f32; 256];
        for i in 0..256 {
            // C uses the literal 3.14159265f, which rounds to the same f32
            // as PI, so the table is bit-identical.
            let rad = i as f32 * (2.0 * std::f32::consts::PI / 256.0);
            sin_table[i] = rad.sin();
            cos_table[i] = rad.cos();
        }
        let mut projection = [0.0f32; 16];
        let mut view = [0.0f32; 16];
        identity(&mut projection);
        identity(&mut view);
        Transform {
            projection,
            view,
            sin_table,
            cos_table,
            cam_prev: CameraState::default(),
            cam_curr: CameraState::default(),
            cam_valid: false,
            cam_render: CameraState::default(),
            cam_render_pitch_f: 0.0,
            cam_render_yaw_f: 0.0,
        }
    }

    /// Interpolate sin/cos from the integer SNES tables at a fractional angle
    /// (units in [0, 256)). Linear blend between adjacent 1.4-deg entries —
    /// smooth, and identical to the raw table for whole-unit angles.
    #[inline]
    fn sincos_frac(&self, a: f32) -> (f32, f32) {
        let a = a.rem_euclid(256.0);
        let i0 = a.floor() as usize % 256;
        let i1 = (i0 + 1) % 256;
        let f = a - a.floor();
        let s = self.sin_table[i0] + (self.sin_table[i1] - self.sin_table[i0]) * f;
        let c = self.cos_table[i0] + (self.cos_table[i1] - self.cos_table[i0]) * f;
        (s, c)
    }

    /// Mirror of `Transform_SetProjection` (60 deg FOV, near 1, far 10000).
    pub fn set_projection(&mut self, width: i32, height: i32) {
        let aspect = width as f32 / height as f32;
        let fov = 60.0f32 * (std::f32::consts::PI / 180.0);
        let near = 1.0f32;
        let far = 10000.0f32;
        let f = 1.0 / (fov / 2.0).tan();

        self.projection = [0.0; 16];
        self.projection[0] = f / aspect;
        self.projection[5] = f;
        // wgpu clip space is Z in [0,1], not GL's [-1,1]. Using the GL form
        // (far+near)/(near-far) & 2*far*near/(near-far) clipped the near half
        // of the frustum (objects close to the camera vanished) and compressed
        // depth precision. Use the D3D/wgpu-style depth mapping.
        self.projection[10] = far / (near - far);
        self.projection[11] = -1.0;
        self.projection[14] = (far * near) / (near - far);
    }

    pub fn projection(&self) -> &[f32; 16] {
        &self.projection
    }

    pub fn view(&self) -> &[f32; 16] {
        &self.view
    }

    fn build_view_matrix(&mut self, cx: i32, cy: i32, cz: i32, crx: i16, cry: i16, crz: i16) {
        // Whole-unit angles resolve to exact table entries in the fractional
        // path, so this stays bit-identical to the pre-fractional build.
        self.build_view_matrix_f(cx, cy, cz, crx as f32, cry as f32, crz as f32);
    }

    /// Fractional-angle view build (see `sincos_frac`). `crx/cry/crz` are the
    /// raw SNES camera angles (any range; wrapped internally), pre-negation.
    fn build_view_matrix_f(&mut self, cx: i32, cy: i32, cz: i32, crx: f32, cry: f32, crz: f32) {
        self.cam_render = CameraState {
            x: cx,
            y: cy,
            z: cz,
            rx: crx.round() as i16,
            ry: cry.round() as i16,
            rz: crz.round() as i16,
        };
        self.cam_render_pitch_f = signed_angle_f(crx);
        self.cam_render_yaw_f = signed_angle_f(cry);
        // Build view matrix from camera position and SNES rotation angles.
        // SNES -> GL world: negate Y translation, negate X/Z rotation angles.
        let cy = -cy;
        let (sx, cx_) = self.sincos_frac(crx);
        let (sy, cy_) = self.sincos_frac(cry);
        let (sz, cz_) = self.sincos_frac(crz);

        // Camera rotation = ROM `mcrotmatzxy16` (MWCROT.MC), the GSU routine
        // getview_l feeds the view angles to. ZXY order = Ry·Rx·Rz with per-axis
        // signs Rx=[1,0,0;0,cx,-sx;0,sx,cx], Ry=[cy,0,+sy;0,1,0;-sy,0,cy],
        // Rz=[cz,-sz,0;sz,cz,0;0,0,1]. This expansion reproduces the ROM matrix
        // EXACTLY (Δ=0) — proven in sf-oracle tests/gsu_rotmat.rs. The previous
        // ZYX + flipped-yaw formula matched only pitch/roll; yaw rotated the
        // wrong way and combined rotations were badly off ("entities too high" /
        // horizon disconnect / camera-opposite).
        let mut rotation = [0.0f32; 16];
        identity(&mut rotation);
        rotation[0] = cy_ * cz_ + sy * sx * sz;
        rotation[1] = -cy_ * sz + sy * sx * cz_;
        rotation[2] = sy * cx_;
        rotation[4] = cx_ * sz;
        rotation[5] = cx_ * cz_;
        rotation[6] = -sx;
        rotation[8] = -sy * cz_ + cy_ * sx * sz;
        rotation[9] = sy * sz + cy_ * sx * cz_;
        rotation[10] = cy_ * cx_;

        // Translation (negate for view matrix)
        let tx = -fp16_to_float(cx);
        let ty = -fp16_to_float(cy);
        let tz = -fp16_to_float(cz);

        // View = Rotation^T * Translation
        identity(&mut self.view);
        for i in 0..3 {
            for j in 0..3 {
                self.view[j * 4 + i] = rotation[i * 4 + j];
            }
        }

        // Facing conversion: the SNES camera looks down +Z, the OpenGL
        // camera looks down -Z. Negate the view-space Z row so
        // world-forward (+Z) maps to GL view-forward (-Z).
        self.view[2] = -self.view[2];
        self.view[6] = -self.view[6];
        self.view[10] = -self.view[10];

        // Apply translation in rotated space
        self.view[12] = self.view[0] * tx + self.view[4] * ty + self.view[8] * tz;
        self.view[13] = self.view[1] * tx + self.view[5] * ty + self.view[9] * tz;
        self.view[14] = self.view[2] * tx + self.view[6] * ty + self.view[10] * tz;
    }

    /// Mirror of `Transform_SetCamera`: advance the interpolation history
    /// (called exactly once per game tick).
    pub fn set_camera(&mut self, cx: i32, cy: i32, cz: i32, crx: i16, cry: i16, crz: i16) {
        if self.cam_valid {
            self.cam_prev = self.cam_curr;
        }
        self.cam_curr = CameraState {
            x: cx,
            y: cy,
            z: cz,
            rx: crx,
            ry: cry,
            rz: crz,
        };
        if !self.cam_valid {
            self.cam_prev = self.cam_curr;
            self.cam_valid = true;
        }
        self.build_view_matrix(cx, cy, cz, crx, cry, crz);
    }

    /// Camera cut (viewtype change, fixed-position jump, level load).
    pub fn snap_camera(&mut self) {
        self.cam_prev = self.cam_curr;
    }

    /// Mirror of `Transform_SetViewLerp`: rebuild the view matrix from
    /// lerp(prev, curr, alpha), with camera-cut (big jump) detection.
    pub fn set_view_lerp(&mut self, alpha: f32) {
        if !self.cam_valid {
            return;
        }
        let alpha = alpha.clamp(0.0, 1.0);

        let mut big_jump = false;
        let rx = lerp_cam_angle_f(self.cam_prev.rx, self.cam_curr.rx, alpha, &mut big_jump);
        let ry = lerp_cam_angle_f(self.cam_prev.ry, self.cam_curr.ry, alpha, &mut big_jump);
        let rz = lerp_cam_angle_f(self.cam_prev.rz, self.cam_curr.rz, alpha, &mut big_jump);

        let dx = fp16_to_float(self.cam_curr.x.wrapping_sub(self.cam_prev.x));
        let dy = fp16_to_float(self.cam_curr.y.wrapping_sub(self.cam_prev.y));
        let dz = fp16_to_float(self.cam_curr.z.wrapping_sub(self.cam_prev.z));
        // A displacement this large in one 20Hz tick is a teleport, not motion.
        // Genuine teleports are viewtype-change cuts (cam.snap) handled upstream;
        // this is only a fallback. 600 tripped on fast continuous camera moves
        // (a wide orbital "follow" like the intro), snapping every tick. Raised
        // so only an extreme within-viewtype jump trips it.
        if dx * dx + dy * dy + dz * dz > 2400.0 * 2400.0 {
            big_jump = true;
        }

        if big_jump {
            self.cam_prev = self.cam_curr;
            let c = self.cam_curr;
            self.build_view_matrix(c.x, c.y, c.z, c.rx, c.ry, c.rz);
            return;
        }

        // Wrapping add: the camera position follows the player's i16 world
        // coords scaled to FP16.16, which sit near i32::MAX and wrap at the
        // 16-bit world boundary (SNES modular coords). A plain add overflows
        // in debug when the camera advances near the wrap point.
        let x = self
            .cam_prev
            .x
            .wrapping_add(((self.cam_curr.x.wrapping_sub(self.cam_prev.x)) as f32 * alpha) as i32);
        let y = self
            .cam_prev
            .y
            .wrapping_add(((self.cam_curr.y.wrapping_sub(self.cam_prev.y)) as f32 * alpha) as i32);
        let z = self
            .cam_prev
            .z
            .wrapping_add(((self.cam_curr.z.wrapping_sub(self.cam_prev.z)) as f32 * alpha) as i32);
        self.build_view_matrix_f(x, y, z, rx, ry, rz);
    }

    /// Mirror of `Transform_GetRenderCamera`.
    pub fn render_camera(&self) -> CameraState {
        self.cam_render
    }

    /// Fractional signed render-camera pitch/yaw (SNES units) for the 2D
    /// background horizon coupling — the interpolated angle before it is
    /// rounded into `cam_render`, so the painted horizon glides per render
    /// frame instead of stepping once per tick.
    pub fn render_camera_angles_f(&self) -> (f32, f32) {
        (self.cam_render_pitch_f, self.cam_render_yaw_f)
    }

    /// Mirror of `Transform_BuildModelMatrix`: SNES-convention world
    /// coordinates/angles -> GL-world model matrix.
    pub fn build_model_matrix(
        &self,
        out: &mut [f32; 16],
        x: i32,
        y: i32,
        z: i32,
        rx: i16,
        ry: i16,
        rz: i16,
    ) {
        // Whole-unit angles resolve to exact table entries, so this is
        // bit-identical to the pre-fractional build.
        self.build_model_matrix_f(out, x, y, z, rx as f32, ry as f32, rz as f32);
    }

    /// Fractional-angle model build (see `sincos_frac`): lets an object's
    /// interpolated rotation (e.g. a banking ship) glide per render frame
    /// instead of stepping in whole 1.4-deg SNES units once per tick.
    /// `rx/ry/rz` are raw SNES angles (any range), pre-negation.
    #[allow(clippy::too_many_arguments)]
    pub fn build_model_matrix_f(
        &self,
        out: &mut [f32; 16],
        x: i32,
        y: i32,
        z: i32,
        rx: f32,
        ry: f32,
        rz: f32,
    ) {
        let y = -y;
        let (sx, cx_) = self.sincos_frac(rx);
        let (sy, cy_) = self.sincos_frac(ry);
        let (sz, cz_) = self.sincos_frac(rz);

        identity(out);

        // ROM `mcrotmatzxy16` (MWCROT.MC) — objects use the same GSU matrix as
        // the camera. ZXY order = Ry·Rx·Rz; reproduces the ROM matrix exactly
        // (Δ=0, sf-oracle tests/gsu_rotmat.rs). Was ZYX + flipped yaw.
        out[0] = cy_ * cz_ + sy * sx * sz;
        out[1] = -cy_ * sz + sy * sx * cz_;
        out[2] = sy * cx_;
        out[4] = cx_ * sz;
        out[5] = cx_ * cz_;
        out[6] = -sx;
        out[8] = -sy * cz_ + cy_ * sx * sz;
        out[9] = sy * sz + cy_ * sx * cz_;
        out[10] = cy_ * cx_;

        // Translation
        out[12] = fp16_to_float(x);
        out[13] = fp16_to_float(y);
        out[14] = fp16_to_float(z);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression: a 1-SNES-unit-per-tick camera yaw (the common steady-turn
    // rate) must interpolate at fractional precision across a render frame.
    // The pre-fix integer path (`a8 + (int)(diff*t)`) truncated `1*t` to 0 for
    // all alpha < 1, so the camera rotation froze between ticks and snapped at
    // 20 Hz — the residual flight stutter.
    #[test]
    fn view_lerp_glides_small_camera_yaw() {
        let mut t = Transform::new();
        t.set_projection(1280, 720);
        t.set_camera(0, 0, 0, 0, 20, 0); // tick N-1: yaw 20
        t.set_camera(0, 0, 0, 0, 21, 0); // tick N:   yaw 21 (delta 1)

        t.set_view_lerp(0.0);
        let (_, y0) = t.render_camera_angles_f();
        let view_start = *t.view();
        t.set_view_lerp(0.5);
        let (_, ymid) = t.render_camera_angles_f();
        let view_mid = *t.view();
        t.set_view_lerp(1.0);
        let (_, y1) = t.render_camera_angles_f();

        assert!((y0 - 20.0).abs() < 1e-3, "alpha 0 yaw {y0}");
        assert!((y1 - 21.0).abs() < 1e-3, "alpha 1 yaw {y1}");
        // The key assertion: the mid-frame yaw is ~20.5, NOT stepped to 20/21.
        assert!(
            (ymid - 20.5).abs() < 1e-3,
            "mid yaw {ymid} should glide to ~20.5, not snap"
        );
        // And the mid view matrix must differ from the tick-start view (proves
        // the rendered rotation actually moves between ticks).
        let drift: f32 = view_mid
            .iter()
            .zip(view_start.iter())
            .map(|(m, s)| (m - s).abs())
            .sum();
        assert!(drift > 1e-4, "mid view identical to tick start -> stepping");
    }

    // Whole-unit angles must stay bit-identical through the fractional path
    // (the frac blend collapses to an exact table entry), so non-interpolated
    // objects and the per-tick camera build are unchanged.
    #[test]
    fn integer_angle_build_is_exact() {
        let t = Transform::new();
        for a in [0i16, 1, 45, 64, 127, 200, 255] {
            let (s, c) = t.sincos_frac(a as f32);
            assert_eq!(s, t.sin_table[(a & 0xFF) as usize]);
            assert_eq!(c, t.cos_table[(a & 0xFF) as usize]);
        }
    }
}
