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
    cam_rotation_prev: [u16; 3],
    cam_rotation_curr: [u16; 3],
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
    cam_render_rotation: [u16; 3],
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

const FULL_TURN_FINE: i32 = 65_536;
const HALF_TURN_FINE: i32 = 32_768;
const CAMERA_CUT_FINE: i32 = 30_720;
const FINE_ANGLE_SCALE: f32 = 256.0;
const FINE_ANGLE_SHIFT: u32 = 8;

/// Shortest-path interpolation that preserves the source's complete turn
/// fraction. The result remains in the renderer's 0-to-256 angle system.
fn lerp_fine_camera_angle(from: u16, to: u16, t: f32, big_jump: &mut bool) -> f32 {
    let mut difference = i32::from(to) - i32::from(from);
    if difference >= HALF_TURN_FINE {
        difference -= FULL_TURN_FINE;
    }
    if difference < -HALF_TURN_FINE {
        difference += FULL_TURN_FINE;
    }
    if !(-CAMERA_CUT_FINE..=CAMERA_CUT_FINE).contains(&difference) {
        *big_jump = true;
    }
    (f32::from(from) + difference as f32 * t)
        .rem_euclid(FULL_TURN_FINE as f32)
        / FINE_ANGLE_SCALE
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
            cam_rotation_prev: [0; 3],
            cam_rotation_curr: [0; 3],
            cam_valid: false,
            cam_render: CameraState::default(),
            cam_render_pitch_f: 0.0,
            cam_render_yaw_f: 0.0,
            cam_render_rotation: [0; 3],
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

    /// Mirror of `Transform_SetProjection`. The ROM (GSU `mdo_project`,
    /// MOBJ.MC:5156) projects `screen = coord*256/z + center` with vertical
    /// center `cscrc = 112` (RAMSTUFF.ASM), i.e. vertical FOV = 2·atan(112/256)
    /// ≈ 47.2°. The port had a guessed 60° which over-widened the view and
    /// floated objects too high; horizontal stays `f/aspect` so widescreen just
    /// reveals more to the sides (the SNES vertical extent is preserved).
    pub fn set_projection(&mut self, width: i32, height: i32) {
        let aspect = width as f32 / height as f32;
        let fov = 2.0 * (112.0f32 / 256.0).atan();
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

    /// Retain the source projection scale while moving its origin to an
    /// authored source-frame point. CONT.ASM uses this to place the live 3D
    /// controller demonstration at (64, 48), rather than the ordinary
    /// centered gameplay vanishing point.
    pub fn set_projection_source_center(
        &mut self,
        width: i32,
        height: i32,
        source_width: f32,
        source_height: f32,
        center_x: f32,
        center_y: f32,
    ) {
        self.set_projection(width, height);
        if width <= 0 || height <= 0 || source_width <= 0.0 || source_height <= 0.0 {
            return;
        }

        // Match Ui::begin_2d: source height fixes the scale and the nominal
        // source width is centered within wider output.
        let scale = height as f32 / source_height;
        let logical_width = width as f32 / scale;
        let source_left = (logical_width - source_width) * 0.5;
        let output_x = (source_left + center_x) * scale;
        let output_y = center_y * scale;
        let center_ndc_x = output_x * 2.0 / width as f32 - 1.0;
        let center_ndc_y = 1.0 - output_y * 2.0 / height as f32;

        // clip.w = -camera_z, so the off-axis perspective terms have the
        // opposite sign of the desired normalized-device coordinate.
        self.projection[8] = -center_ndc_x;
        self.projection[9] = -center_ndc_y;
    }

    pub fn projection(&self) -> &[f32; 16] {
        &self.projection
    }

    pub fn view(&self) -> &[f32; 16] {
        &self.view
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
        self.cam_render_rotation = [crx, cry, crz]
            .map(|angle| (angle.rem_euclid(256.0) * FINE_ANGLE_SCALE).round() as u16);
        // Build view matrix from camera position and SNES rotation angles.
        // SNES -> GL world: negate Y translation, negate X/Z rotation angles.
        let cy = -cy;
        // --- World->camera view rotation, built DIRECTLY (no transpose) ---
        // The ROM rotates world-relative points into view space with
        // wmat = mcrotmatzxy16(viewrot) applied as-is (marioshowview ->
        // mallrotzsort). The camera->world orientation is Ry(ry)*Rx(rx)*Rz(rz)
        // (ZXY, gsu_rotmat.rs), so the view rotation is its inverse
        //   V = Rz(-rz) * Rx(-rx) * Ry(-ry)
        // — a REVERSED product order that cannot be expressed by sign-tweaking
        // the Ry*Rx*Rz expansion (a transpose+yaw-negate hack renders pure yaw
        // and pure pitch correctly but breaks their composition: the opening
        // cinematic's look-back camera, yaw~108 + pitch~-27, projected the
        // whole arwing formation off-frame -> solid green screen).
        let (sx, cx_) = self.sincos_frac(-crx);
        let (sy, cy_) = self.sincos_frac(-cry);
        let (sz, cz_) = self.sincos_frac(-crz);
        let ry_m = [[cy_, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy_]];
        let rx_m = [[1.0, 0.0, 0.0], [0.0, cx_, -sx], [0.0, sx, cx_]];
        let rz_m = [[cz_, -sz, 0.0], [sz, cz_, 0.0], [0.0, 0.0, 1.0]];
        let mul3 = |a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]| -> [[f32; 3]; 3] {
            let mut m = [[0.0f32; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
                }
            }
            m
        };
        let v_snes = mul3(&rz_m, &mul3(&rx_m, &ry_m));

        // SNES->GL basis change. The draw entries reach the renderer with y
        // negated but z UNCHANGED (left-handed "GL world": x right, y up, z
        // forward-in), and GL camera space wants forward -z. So the input
        // scaling is H = diag(1,-1,1) (undo the entry y-flip into SNES space)
        // and the output scaling is G = diag(1,-1,-1) (camera y up, forward
        // -z): V_gl[i][j] = G[i] * H[j] * V[i][j].
        const G: [f32; 3] = [1.0, -1.0, -1.0];
        const H: [f32; 3] = [1.0, -1.0, 1.0];

        // Translation (negate for view matrix)
        let tx = -fp16_to_float(cx);
        let ty = -fp16_to_float(cy);
        let tz = -fp16_to_float(cz);

        identity(&mut self.view);
        for row in 0..3 {
            for col in 0..3 {
                self.view[col * 4 + row] = G[row] * H[col] * v_snes[row][col];
            }
        }

        // Apply translation in rotated space
        self.view[12] = self.view[0] * tx + self.view[4] * ty + self.view[8] * tz;
        self.view[13] = self.view[1] * tx + self.view[5] * ty + self.view[9] * tz;
        self.view[14] = self.view[2] * tx + self.view[6] * ty + self.view[10] * tz;
    }

    /// Mirror of `Transform_SetCamera`: advance the interpolation history
    /// (called exactly once per game tick).
    pub fn set_camera(&mut self, cx: i32, cy: i32, cz: i32, crx: i16, cry: i16, crz: i16) {
        self.set_camera_fine(
            cx,
            cy,
            cz,
            [
                (crx as u16) << FINE_ANGLE_SHIFT,
                (cry as u16) << FINE_ANGLE_SHIFT,
                (crz as u16) << FINE_ANGLE_SHIFT,
            ],
        );
    }

    /// Set the camera with the source's complete 16-bit turn fractions.
    /// Whole angle units remain available through [`CameraState`], while the
    /// view matrix and interpolation retain the authored low-byte precision.
    pub fn set_camera_fine(&mut self, cx: i32, cy: i32, cz: i32, rotation: [u16; 3]) {
        if self.cam_valid {
            self.cam_prev = self.cam_curr;
            self.cam_rotation_prev = self.cam_rotation_curr;
        }
        self.cam_curr = CameraState {
            x: cx,
            y: cy,
            z: cz,
            rx: (rotation[0] >> FINE_ANGLE_SHIFT) as u8 as i8 as i16,
            ry: (rotation[1] >> FINE_ANGLE_SHIFT) as u8 as i8 as i16,
            rz: (rotation[2] >> FINE_ANGLE_SHIFT) as u8 as i8 as i16,
        };
        self.cam_rotation_curr = rotation;
        if !self.cam_valid {
            self.cam_prev = self.cam_curr;
            self.cam_rotation_prev = self.cam_rotation_curr;
            self.cam_valid = true;
        }
        self.build_view_matrix_f(
            cx,
            cy,
            cz,
            f32::from(rotation[0]) / FINE_ANGLE_SCALE,
            f32::from(rotation[1]) / FINE_ANGLE_SCALE,
            f32::from(rotation[2]) / FINE_ANGLE_SCALE,
        );
    }

    /// Camera cut (viewtype change, fixed-position jump, level load).
    pub fn snap_camera(&mut self) {
        self.cam_prev = self.cam_curr;
        self.cam_rotation_prev = self.cam_rotation_curr;
    }

    /// Mirror of `Transform_SetViewLerp`: rebuild the view matrix from
    /// lerp(prev, curr, alpha), with camera-cut (big jump) detection.
    pub fn set_view_lerp(&mut self, alpha: f32) {
        if !self.cam_valid {
            return;
        }
        let alpha = alpha.clamp(0.0, 1.0);

        let mut big_jump = false;
        let rx = lerp_fine_camera_angle(
            self.cam_rotation_prev[0],
            self.cam_rotation_curr[0],
            alpha,
            &mut big_jump,
        );
        let ry = lerp_fine_camera_angle(
            self.cam_rotation_prev[1],
            self.cam_rotation_curr[1],
            alpha,
            &mut big_jump,
        );
        let rz = lerp_fine_camera_angle(
            self.cam_rotation_prev[2],
            self.cam_rotation_curr[2],
            alpha,
            &mut big_jump,
        );

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
            self.cam_rotation_prev = self.cam_rotation_curr;
            let c = self.cam_curr;
            self.build_view_matrix_f(
                c.x,
                c.y,
                c.z,
                f32::from(self.cam_rotation_curr[0]) / FINE_ANGLE_SCALE,
                f32::from(self.cam_rotation_curr[1]) / FINE_ANGLE_SCALE,
                f32::from(self.cam_rotation_curr[2]) / FINE_ANGLE_SCALE,
            );
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

    /// Camera position and complete authored turn fractions used by the most
    /// recent view build. Fixed-update source projection consumes this typed
    /// snapshot without reconstructing angles from a floating-point matrix.
    pub fn source_camera(&self) -> (CameraState, [u16; 3]) {
        (self.cam_render, self.cam_render_rotation)
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
        // MOBJ.MC converts each byte angle to a negated 16-bit angle before
        // calling `mcrotmatzxy16`, transposes that matrix, and finally lets
        // the point path consume its columns. The two transposes cancel:
        // object points are transformed by the direct Z-X-Y matrix of the
        // negated angles. The generated meshes already negate source Y, so
        // conjugate the source matrix by that Y reflection before storing it
        // in the column-major GPU matrix.
        let (sx, cx_) = self.sincos_frac(-rx);
        let (sy, cy_) = self.sincos_frac(-ry);
        let (sz, cz_) = self.sincos_frac(-rz);

        identity(out);

        // ROM `mcrotmatzxy16` (MWCROT.MC): source row-major Z-X-Y matrix.
        let source = [
            [
                cy_ * cz_ + sy * sx * sz,
                -cy_ * sz + sy * sx * cz_,
                sy * cx_,
            ],
            [cx_ * sz, cx_ * cz_, -sx],
            [
                -sy * cz_ + cy_ * sx * sz,
                sy * sz + cy_ * sx * cz_,
                cy_ * cx_,
            ],
        ];
        const SOURCE_TO_RENDER_Y: [f32; 3] = [1.0, -1.0, 1.0];
        for row in 0..3 {
            for col in 0..3 {
                out[col * 4 + row] =
                    SOURCE_TO_RENDER_Y[row] * source[row][col] * SOURCE_TO_RENDER_Y[col];
            }
        }

        // Translation
        out[12] = fp16_to_float(x);
        out[13] = fp16_to_float(y);
        out[14] = fp16_to_float(z);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_WIDTH: f32 = 256.0;
    const SOURCE_HEIGHT: f32 = 224.0;

    fn transform_direction(matrix: &[f32; 16], direction: [f32; 3]) -> [f32; 3] {
        [
            matrix[0] * direction[0] + matrix[4] * direction[1] + matrix[8] * direction[2],
            matrix[1] * direction[0] + matrix[5] * direction[1] + matrix[9] * direction[2],
            matrix[2] * direction[0] + matrix[6] * direction[1] + matrix[10] * direction[2],
        ]
    }

    #[test]
    fn combined_object_angles_follow_the_source_point_pipeline() {
        use sf_core::snes_trig::zxy_matrix_q15;

        const PITCH: u8 = 239;
        const YAW: u8 = 96;
        const ROLL: u8 = 54;
        const SOURCE_POINT: [i16; 3] = [70, 28, -100];

        let source_matrix = zxy_matrix_q15(
            PITCH.wrapping_neg(),
            YAW.wrapping_neg(),
            ROLL.wrapping_neg(),
        );
        let source_output = std::array::from_fn::<_, 3, _>(|row| {
            (i32::from(source_matrix[row][0]) * i32::from(SOURCE_POINT[0])
                + i32::from(source_matrix[row][1]) * i32::from(SOURCE_POINT[1])
                + i32::from(source_matrix[row][2]) * i32::from(SOURCE_POINT[2])) as f32
                / 32_768.0
        });

        let transform = Transform::new();
        let mut model = [0.0; 16];
        transform.build_model_matrix(
            &mut model,
            0,
            0,
            0,
            i16::from(PITCH),
            i16::from(YAW),
            i16::from(ROLL),
        );
        let render_input = [
            f32::from(SOURCE_POINT[0]),
            -f32::from(SOURCE_POINT[1]),
            f32::from(SOURCE_POINT[2]),
        ];
        let render_output = transform_direction(&model, render_input);
        let expected = [source_output[0], -source_output[1], source_output[2]];
        for axis in 0..3 {
            assert!(
                (render_output[axis] - expected[axis]).abs() < 0.02,
                "axis {axis}: render={} source={}",
                render_output[axis],
                expected[axis]
            );
        }
    }

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

    #[test]
    fn fine_camera_rotation_preserves_the_authored_low_byte() {
        const HALF_ANGLE_UNIT: u16 = 128;

        let mut transform = Transform::new();
        let identity_view = *transform.view();
        transform.set_camera_fine(0, 0, 0, [0, HALF_ANGLE_UNIT, 0]);

        let (_, yaw) = transform.render_camera_angles_f();
        assert!((yaw - 0.5).abs() < 1e-6);
        assert_ne!(*transform.view(), identity_view);
        assert_eq!(transform.render_camera().ry, 1);
    }

    #[test]
    fn source_projection_center_maps_to_the_authored_widescreen_point() {
        const OUTPUT_WIDTH: i32 = 1280;
        const OUTPUT_HEIGHT: i32 = 720;
        const CENTER_X: f32 = 64.0;
        const CENTER_Y: f32 = 48.0;

        let mut transform = Transform::new();
        transform.set_projection_source_center(
            OUTPUT_WIDTH,
            OUTPUT_HEIGHT,
            SOURCE_WIDTH,
            SOURCE_HEIGHT,
            CENTER_X,
            CENTER_Y,
        );

        let scale = OUTPUT_HEIGHT as f32 / SOURCE_HEIGHT;
        let source_left = (OUTPUT_WIDTH as f32 / scale - SOURCE_WIDTH) * 0.5;
        let expected_x = ((source_left + CENTER_X) * scale * 2.0 / OUTPUT_WIDTH as f32) - 1.0;
        let expected_y = 1.0 - CENTER_Y * scale * 2.0 / OUTPUT_HEIGHT as f32;
        assert!((-transform.projection()[8] - expected_x).abs() < 0.0001);
        assert!((-transform.projection()[9] - expected_y).abs() < 0.0001);
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
