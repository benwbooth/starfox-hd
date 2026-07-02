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

// Shortest-path interpolation in the SNES 0-255 angle system.
fn lerp_cam_angle(from: i16, to: i16, t: f32, big_jump: &mut bool) -> i16 {
    let a8 = (from & 0xFF) as i32;
    let b8 = (to & 0xFF) as i32;
    let mut diff = b8 - a8;
    if diff > 127 {
        diff -= 256;
    }
    if diff < -128 {
        diff += 256;
    }
    if diff > 48 || diff < -48 {
        *big_jump = true; // > ~67 deg in one tick: cut
    }
    let out = a8 + (diff as f32 * t) as i32;
    (out & 0xFF) as i16
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
        }
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
        self.projection[10] = (far + near) / (near - far);
        self.projection[11] = -1.0;
        self.projection[14] = (2.0 * far * near) / (near - far);
    }

    pub fn projection(&self) -> &[f32; 16] {
        &self.projection
    }

    pub fn view(&self) -> &[f32; 16] {
        &self.view
    }

    fn build_view_matrix(&mut self, cx: i32, cy: i32, cz: i32, crx: i16, cry: i16, crz: i16) {
        self.cam_render = CameraState {
            x: cx,
            y: cy,
            z: cz,
            rx: crx,
            ry: cry,
            rz: crz,
        };
        // Build view matrix from camera position and SNES rotation angles.
        // SNES -> GL world: negate Y translation, negate X/Z rotation angles.
        let cy = -cy;
        let crx = crx.wrapping_neg();
        let crz = crz.wrapping_neg();
        let ax = (crx & 0xFF) as usize;
        let ay = (cry & 0xFF) as usize;
        let az = (crz & 0xFF) as usize;

        let (sx, cx_) = (self.sin_table[ax], self.cos_table[ax]);
        let (sy, cy_) = (self.sin_table[ay], self.cos_table[ay]);
        let (sz, cz_) = (self.sin_table[az], self.cos_table[az]);

        // ZYX rotation order (matching SNES)
        let mut rotation = [0.0f32; 16];
        identity(&mut rotation);
        rotation[0] = cy_ * cz_;
        rotation[1] = cy_ * sz;
        rotation[2] = -sy;
        rotation[4] = sx * sy * cz_ - cx_ * sz;
        rotation[5] = sx * sy * sz + cx_ * cz_;
        rotation[6] = sx * cy_;
        rotation[8] = cx_ * sy * cz_ + sx * sz;
        rotation[9] = cx_ * sy * sz - sx * cz_;
        rotation[10] = cx_ * cy_;

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
        let rx = lerp_cam_angle(self.cam_prev.rx, self.cam_curr.rx, alpha, &mut big_jump);
        let ry = lerp_cam_angle(self.cam_prev.ry, self.cam_curr.ry, alpha, &mut big_jump);
        let rz = lerp_cam_angle(self.cam_prev.rz, self.cam_curr.rz, alpha, &mut big_jump);

        let dx = fp16_to_float(self.cam_curr.x.wrapping_sub(self.cam_prev.x));
        let dy = fp16_to_float(self.cam_curr.y.wrapping_sub(self.cam_prev.y));
        let dz = fp16_to_float(self.cam_curr.z.wrapping_sub(self.cam_prev.z));
        // A displacement this large in one 20Hz tick is a teleport, not motion.
        if dx * dx + dy * dy + dz * dz > 600.0 * 600.0 {
            big_jump = true;
        }

        if big_jump {
            self.cam_prev = self.cam_curr;
            let c = self.cam_curr;
            self.build_view_matrix(c.x, c.y, c.z, c.rx, c.ry, c.rz);
            return;
        }

        let x = self.cam_prev.x
            + ((self.cam_curr.x.wrapping_sub(self.cam_prev.x)) as f32 * alpha) as i32;
        let y = self.cam_prev.y
            + ((self.cam_curr.y.wrapping_sub(self.cam_prev.y)) as f32 * alpha) as i32;
        let z = self.cam_prev.z
            + ((self.cam_curr.z.wrapping_sub(self.cam_prev.z)) as f32 * alpha) as i32;
        self.build_view_matrix(x, y, z, rx, ry, rz);
    }

    /// Mirror of `Transform_GetRenderCamera`.
    pub fn render_camera(&self) -> CameraState {
        self.cam_render
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
        let y = -y;
        let rx = rx.wrapping_neg();
        let rz = rz.wrapping_neg();
        let ax = (rx & 0xFF) as usize;
        let ay = (ry & 0xFF) as usize;
        let az = (rz & 0xFF) as usize;

        let (sx, cx_) = (self.sin_table[ax], self.cos_table[ax]);
        let (sy, cy_) = (self.sin_table[ay], self.cos_table[ay]);
        let (sz, cz_) = (self.sin_table[az], self.cos_table[az]);

        identity(out);

        // ZYX rotation (matching SNES rotation order)
        out[0] = cy_ * cz_;
        out[1] = cy_ * sz;
        out[2] = -sy;
        out[4] = sx * sy * cz_ - cx_ * sz;
        out[5] = sx * sy * sz + cx_ * cz_;
        out[6] = sx * cy_;
        out[8] = cx_ * sy * cz_ + sx * sz;
        out[9] = cx_ * sy * sz - sx * cz_;
        out[10] = cx_ * cy_;

        // Translation
        out[12] = fp16_to_float(x);
        out[13] = fp16_to_float(y);
        out[14] = fp16_to_float(z);
    }
}
