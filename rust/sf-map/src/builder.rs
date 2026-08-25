//! MapBuilder — the map bytecode assembler.
//!
//! Encoding oracle: `reference/ultrastarfox/SF/INC/MAPMACS.INC`, checked
//! against the decoder in `WORLD.ASM`. The fixture tests retain deterministic
//! source-correct blobs for every ported level.
//!
//! Numeric parameters are `i32` and are truncated internally with `as`
//! casts, reproducing the implicit `int -> int16/uint16/uint8` conversions
//! at the C call sites (e.g. `-27 << MYBASE_SCALE` passed to an `int16`
//! parameter). Shape ids stay `u16`; strategy operands distinguish compact
//! or compatibility encodings from typed native Rust strategies.

use crate::consts::*;

const QUANTIZED_COORDINATE_LIMIT: i32 = 512;
const QUANTIZED_DISTANCE_LIMIT: i32 = 256;
const QUANTIZED_POSITION_SHIFT: u32 = 2;
const QUANTIZED_DEPTH_SHIFT: u32 = 4;

/// A resolved label: name -> byte offset in the emitted blob.
pub type Label = (String, u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarShapeMode {
    /// C `MAP_BARSHAPE_WIRE`
    Wire,
    /// C `MAP_BARSHAPE_SOLID`
    Solid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpacebarShape {
    X,
    Xp,
    Y,
    Z,
    Sx,
    Sxp,
    Sy,
    Sz,
}

/// Port of the C `MapBuilder` struct. The fixed-capacity buffers become
/// `Vec`s; overflow (`mb_fail`) therefore cannot happen. Unresolved labels
/// panic in [`MapBuilder::resolve`] instead of emitting offset 0 with a
/// warning — a porting bug should fail loudly in tests.
pub struct MapBuilder {
    data: Vec<u8>,
    labels: Vec<Label>,
    fixups: Vec<(String, u16)>,
    barshape_mode: BarShapeMode,
    barshape_autowait: bool,
    barshape_swait: i16,
    barshape_pos: i16,
}

impl Default for MapBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MapBuilder {
    pub fn new() -> Self {
        MapBuilder {
            data: Vec::new(),
            labels: Vec::new(),
            fixups: Vec::new(),
            barshape_mode: BarShapeMode::Wire,
            barshape_autowait: false,
            barshape_swait: 0,
            barshape_pos: 0,
        }
    }

    // ---- core emit / label machinery (mb_emit8/16, mb_label, mb_fixup16,
    //      mb_lookup_label, mb_resolve) ----

    pub fn emit8(&mut self, value: u8) {
        self.data.push(value);
    }

    pub fn emit16(&mut self, value: u16) {
        self.emit8((value & 0xFF) as u8);
        self.emit8((value >> 8) as u8);
    }

    pub fn emit16s(&mut self, value: i16) {
        self.emit16(value as u16);
    }

    /// Current emit offset — equals the script ptr of the next byte.
    pub fn len(&self) -> u16 {
        self.data.len() as u16
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn label(&mut self, name: &str) {
        let offset = self.len();
        self.labels.push((name.to_string(), offset));
    }

    pub fn fixup16(&mut self, label: &str) {
        let offset = self.len();
        self.fixups.push((label.to_string(), offset));
        self.emit16(0);
    }

    pub fn lookup_label(&self, label: &str) -> Option<u16> {
        self.labels
            .iter()
            .find(|(name, _)| name == label)
            .map(|&(_, offset)| offset)
    }

    /// Patch all fixups. Panics on an unresolved label (the C version
    /// prints a warning and writes offset 0; all ported levels resolve).
    pub fn resolve(&mut self) {
        for i in 0..self.fixups.len() {
            let (label, offset) = self.fixups[i].clone();
            let target = self
                .lookup_label(&label)
                .unwrap_or_else(|| panic!("Map literals: unresolved label '{label}'"));
            self.data[offset as usize] = (target & 0xFF) as u8;
            self.data[offset as usize + 1] = (target >> 8) as u8;
        }
    }

    /// Consume the builder, returning the blob and the label table.
    pub fn finish(self) -> (Vec<u8>, Vec<Label>) {
        (self.data, self.labels)
    }

    // ---- object spawns ----

    fn emit_full_object(
        &mut self,
        opcode: u8,
        frame: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        strategy_word: u16,
        strategy_tag: u8,
    ) {
        self.emit8(opcode);
        self.emit16(frame as u16);
        self.emit16s(x as i16);
        self.emit16s(y as i16);
        self.emit16s(z as i16);
        self.emit16(shape);
        self.emit16(strategy_word);
        self.emit8(strategy_tag);
    }

    /// Full-width object spawn. Compatibility operands retain the original
    /// `NORMOBJ` layout; typed native strategies use `DIRECTOBJ` with the same
    /// record width so map labels and timing remain stable.
    pub fn mapnobj<S: Into<StrategyRef>>(
        &mut self,
        frame: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        strategy: S,
    ) {
        match strategy.into() {
            StrategyRef::Encoded(value) => self.emit_full_object(
                op::NORMOBJ,
                frame,
                x,
                y,
                z,
                shape,
                value as u16,
                (value >> 16) as u8,
            ),
            StrategyRef::Direct(value) => {
                self.emit_full_object(op::DIRECTOBJ, frame, x, y, z, shape, value.id() as u16, 0)
            }
        }
    }

    /// Source `mapobj`: prefer the compact `mapqobj` representation when
    /// its symbol and coordinate constraints hold, then use compact MAPOBJ or
    /// fall back to the full-width native representation.
    pub fn mapobj<S: Into<StrategyRef>>(
        &mut self,
        frame: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        strategy: S,
    ) {
        let strategy = strategy.into();
        if let StrategyRef::Encoded(value) = strategy {
            if shape <= u8::MAX as u16 && value <= u8::MAX as u32 {
                let compact_frame = frame >> QUANTIZED_DEPTH_SHIFT;
                let compact_depth = z >> QUANTIZED_DEPTH_SHIFT;
                let default_shape = crate::istrat_shapes::ISTRAT_SHAPE_DEFAULTS[value as usize];
                // ArgSFX's assembly-time right shift keeps negative source
                // distances outside the unsigned-byte test. Preserve that
                // effective lower bound explicitly in typed Rust.
                if compact_frame >= 0
                    && compact_frame < QUANTIZED_DISTANCE_LIMIT
                    && x > -QUANTIZED_COORDINATE_LIMIT
                    && x < QUANTIZED_COORDINATE_LIMIT
                    && y > -QUANTIZED_COORDINATE_LIMIT
                    && y < QUANTIZED_COORDINATE_LIMIT
                    && compact_depth >= 0
                    && compact_depth < QUANTIZED_DISTANCE_LIMIT
                {
                    let shape_is_implied = default_shape == shape;
                    self.emit8(if shape_is_implied {
                        op::QOBJ2
                    } else {
                        op::QOBJ
                    });
                    self.emit8(compact_frame as u8);
                    self.emit8((x >> QUANTIZED_POSITION_SHIFT) as u8);
                    self.emit8((y >> QUANTIZED_POSITION_SHIFT) as u8);
                    self.emit8(compact_depth as u8);
                    if !shape_is_implied {
                        self.emit8(shape as u8);
                    }
                    self.emit8(value as u8);
                    return;
                }
                let shape_is_implied = default_shape == shape;
                self.emit8(if shape_is_implied {
                    op::DOBJ
                } else {
                    op::MAPOBJ
                });
                self.emit16(frame as u16);
                self.emit16s(x as i16);
                self.emit16s(y as i16);
                self.emit16s(z as i16);
                if !shape_is_implied {
                    self.emit8(shape as u8);
                }
                self.emit8(value as u8);
                return;
            }
        }
        self.mapnobj(frame, x, y, z, shape, strategy);
    }

    /// `mapobjzrot`: compact object spawn with an initial Z rotation.
    /// The source macro is byte-sized for shape and ISTRAT, exactly like the
    /// WORLD.ASM opcode-38 layout.
    pub fn mapobjzrot(
        &mut self,
        frame: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        strat: u32,
        zrot: i32,
    ) {
        assert!(
            shape <= u8::MAX as u16,
            "mapobjzrot shape must fit one byte"
        );
        assert!(
            strat <= u8::MAX as u32,
            "mapobjzrot ISTRAT must fit one byte"
        );
        self.emit8(op::OBJZROT);
        self.emit16(frame as u16);
        self.emit16s(x as i16);
        self.emit16s(y as i16);
        self.emit16s(z as i16);
        self.emit8(shape as u8);
        self.emit8(strat as u8);
        self.emit8(zrot as u8);
    }

    /// mb_maphardrot: mapobj with IS_HARDROT + sbyte1..3 rotation speeds + wait.
    pub fn maphardrot(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        rx: i32,
        ry: i32,
        rz: i32,
    ) {
        self.mapobj(0, x, y, z, shape, is::HARDROT);
        self.setalvarb(al::SBYTE1, rx);
        self.setalvarb(al::SBYTE2, ry);
        self.setalvarb(al::SBYTE3, rz);
        self.mapwait(wait);
    }

    // ---- flow / timing ----

    pub fn mapwait(&mut self, dist: i32) {
        // MAPMACS.INC:573-590. Zero emits no instruction at all. Values
        // whose high distance byte fits use WAIT2 and are intentionally
        // quantized to 16-unit resolution; larger values use the full word.
        // This also preserves the important `mapwait 1` behavior: WAIT2 0
        // yields from the VM for one update even though mapcnt remains zero.
        if dist == 0 {
            return;
        }
        let compact = dist >> 4;
        if compact < 256 {
            self.emit8(op::WAIT2);
            self.emit8(compact as u8);
        } else {
            self.emit8(op::WAIT);
            self.emit16(dist as u16);
        }
    }

    pub fn setbgm(&mut self, music_id: i32) {
        self.emit8(op::SETBGM);
        self.emit8(music_id as u8);
    }

    pub fn mapplayeroutview(&mut self) {
        self.mapcodejsl_builtin(cb::PLAYER_OUTVIEW_L);
    }

    /// `mapplayercantdie`: latch `pstf_notdie` through a native map callback.
    pub fn mapplayercantdie(&mut self) {
        self.mapcodejsl_builtin(cb::PLAYER_CANT_DIE);
    }

    /// PLANET.ASM `mapnozremove`: retain authored objects after they pass the
    /// camera until a later strategy or stage transition restores culling.
    pub fn preserve_behind_view_objects(&mut self) {
        self.emit8(op::PRESERVE_BEHIND_VIEW_OBJECTS);
    }

    pub fn setbg(&mut self, bg_id: i32) {
        self.emit8(op::SETBG);
        self.emit16(bg_id as u16);
    }

    /// Start the flight-stage announcement countdown.
    pub fn setstage(&mut self) {
        self.emit8(op::SETSTAGE);
    }

    pub fn qfadeup(&mut self) {
        self.emit8(op::QFADEUP);
    }

    pub fn qfadedown(&mut self) {
        self.emit8(op::QFADEDOWN);
    }

    /// `setfadeup` without the `quick` argument.
    pub fn fadeup(&mut self) {
        self.emit8(op::FADEUP);
    }

    /// `setfadedown` without the `quick` argument.
    pub fn fadedown(&mut self) {
        self.emit8(op::FADEDOWN);
    }

    pub fn waitfade(&mut self) {
        self.emit8(op::WAITFADE);
    }

    /// mb_initbg: WAITSETBG + SETBGINFO pair.
    pub fn initbg(&mut self) {
        self.emit8(op::WAITSETBG);
        self.emit8(op::SETBGINFO);
    }

    pub fn maploop(&mut self, label: &str, count: i32) {
        self.emit8(op::LOOP);
        self.fixup16(label);
        // ROM `maploop` macro emits `dw count-1` (MAPMACS.INC:264-268) and the
        // handler runs the body stored+1 times (WORLD.ASM:1773, oracle-proven
        // in audit_mapvm2.rs). Emitting the raw count made EVERY level loop run
        // one extra iteration (e.g. 4 buzzer groups in 1-1 instead of 3). The C
        // builder had the same bug; the .bin fixtures dumped from it encode the
        // raw count too.
        self.emit16((count - 1) as u16);
    }

    /// mb_mapif_builtin: IF with a 24-bit native callback id and an
    /// else-branch label.
    pub fn mapif_builtin(&mut self, callback_addr24: u32, else_label: &str) {
        self.emit8(op::IF);
        self.emit16((callback_addr24 & 0xFFFF) as u16);
        self.emit8((callback_addr24 >> 16) as u8);
        self.fixup16(else_label);
    }

    pub fn mapgoto(&mut self, label: &str) {
        self.emit8(op::GOTO);
        self.fixup16(label);
        self.emit8(0);
    }

    pub fn mapremove(&mut self, shape: u16) {
        self.emit8(op::REMOVE);
        self.emit16(0);
        self.emit16(shape);
    }

    // ---- alien / external variable writes ----

    pub fn setalvarb(&mut self, offset: u16, value: i32) {
        self.emit8(op::SETALVARB);
        self.emit16(offset);
        self.emit8(value as u8);
    }

    pub fn setalvarw(&mut self, offset: u16, value: i32) {
        self.emit8(op::SETALVARW);
        self.emit16(offset);
        self.emit16(value as u16);
    }

    pub fn setalxvarb(&mut self, offset: u16, value: i32) {
        self.emit8(op::SETALXVARB);
        self.emit16(offset);
        self.emit8(value as u8);
    }

    pub fn setalxvarw(&mut self, offset: u16, value: i32) {
        self.emit8(op::SETALXVARW);
        self.emit16(offset);
        self.emit16s(value as i16);
    }

    pub fn setalvarptrw(&mut self, offset: u16, extptr: u16) {
        self.emit8(op::SETALVARPW);
        self.emit16(offset);
        self.emit16(extptr);
        self.emit8(0);
    }

    pub fn addalvarptrw(&mut self, offset: u16, extptr: u16) {
        self.emit8(op::ADDALVARPW);
        self.emit16(offset);
        self.emit16(extptr);
        self.emit8(0);
    }

    pub fn setvarobj(&mut self, extptr: u16) {
        self.emit8(op::SETVAROBJ);
        self.emit16(extptr);
        self.emit8(0);
    }

    pub fn setvarb(&mut self, extptr: u16, value: i32) {
        self.emit8(op::SETVARB);
        self.emit8(value as u8);
        self.emit16(extptr);
        self.emit8(0);
    }

    /// Emit a byte SETVAR to its real 24-bit address. Most map-owned state
    /// is bank-zero WRAM and uses [`Self::setvarb`]; Super FX RAM variables
    /// such as `m_meters` live in bank $70 and require this form.
    pub fn setvarb24(&mut self, addr24: u32, value: i32) {
        self.emit8(op::SETVARB);
        self.emit8(value as u8);
        self.emit16((addr24 & 0xFFFF) as u16);
        self.emit8((addr24 >> 16) as u8);
    }

    pub fn setvarw(&mut self, extptr: u16, value: i32) {
        self.emit8(op::SETVARW);
        self.emit16(value as u16);
        self.emit16(extptr);
        self.emit8(0);
    }

    /// mb_mapend: setvar.b levelfinished + END.
    pub fn mapend(&mut self, level_finished: i32) {
        self.setvarb(wm::LEVELFINISHED, level_finished);
        self.emit8(op::END);
    }

    pub fn sendmsg(&mut self, msg_id: i32) {
        self.emit8(op::SENDMSG);
        self.emit8(msg_id as u8);
    }

    pub fn mapsetpath(&mut self, path_id: u16) {
        self.emit8(op::SETPATH);
        self.emit16(path_id);
    }

    // ---- native / inline code hooks ----

    /// mb_mapcodejsl_builtin: CODEJSL with the (addr24 - 1) & 0xFFFF
    /// encoding of the low word (the map VM pre-increments).
    pub fn mapcodejsl_builtin(&mut self, callback_addr24: u32) {
        let encoded = (callback_addr24.wrapping_sub(1) & 0xFFFF) as u16;
        self.emit8(op::CODEJSL);
        self.emit16(encoded);
        self.emit8((callback_addr24 >> 16) as u8);
    }

    /// mb_mapcode65816_inline: emits CODE65816 and returns the script ptr
    /// (offset just past the opcode) that identifies this inline hook.
    /// The runtime later registers a native closure under this ptr via
    /// the World_RegisterInlineMapCode equivalent.
    #[must_use]
    pub fn mapcode65816_inline(&mut self) -> u16 {
        let script_ptr = self.len() + 1;
        self.emit8(op::CODE65816);
        script_ptr
    }

    /// mb_mapexploderobot: kill_robot_l builtin.
    pub fn mapexploderobot(&mut self) {
        self.mapcodejsl_builtin(cb::KILL_ROBOT_L);
    }

    pub fn mapjsr(&mut self, label: &str) {
        self.emit8(op::JSR);
        self.fixup16(label);
        self.emit8(0);
    }

    /// mb_mapmother: mother-ship spawn with a mother sub-map reference.
    pub fn mapmother<S: Into<StrategyRef>>(
        &mut self,
        frame: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        strategy: S,
        map_ref: u16,
    ) {
        let (opcode, strategy_word, strategy_tag) = match strategy.into() {
            StrategyRef::Encoded(value) => (op::MOTHER, value as u16, (value >> 16) as u8),
            StrategyRef::Direct(value) => (op::DIRECTMOTHER, value.id() as u16, 0),
        };
        self.emit8(opcode);
        self.emit16(frame as u16);
        self.emit16s(x as i16);
        self.emit16s(y as i16);
        self.emit16s(z as i16);
        self.emit16(shape);
        self.emit16(strategy_word);
        self.emit8(strategy_tag);
        self.emit16(map_ref);
    }

    pub fn maprts(&mut self) {
        self.emit8(op::RTS);
    }

    pub fn mapcspecial(&mut self) {
        self.emit8(op::CSPECIAL);
    }

    // ---- path spawns ----

    /// mb_pathobj: path-following object; hp==10 && ap==10 uses the
    /// compact IS_PATHDHA (default hp/ap) strategy.
    pub fn pathobj(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        path_id: u16,
        hp: i32,
        ap: i32,
    ) {
        if hp == 10 && ap == 10 {
            self.mapobj(0, x, y, z, shape, is::PATHDHA);
        } else {
            self.mapobj(0, x, y, z, shape, is::PATH);
            self.setalvarb(al::HP, hp);
            self.setalvarb(al::AP, ap);
        }
        self.mapsetpath(path_id);
        self.mapwait(wait);
    }

    /// ROM `textpath` (MAPMACS.INC:1837): spawn a `patht_istrat`
    /// null-shape object, attach a MARIO message pointer and path, select the
    /// text colour/size, then apply the call-site wait.
    pub fn textpath(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        message_ptr: u16,
        path_id: u16,
        colour: i32,
        size: Option<i32>,
    ) {
        self.mapobj(0, x, y, z, sh::NULLSHAPE, is::PATHT);
        self.setalxvarw(alx::COLTAB, message_ptr as i32);
        self.mapsetpath(path_id);
        self.setalxvarb(alx::DEPTHOFFSET, colour);
        if let Some(size) = size {
            self.setalxvarb(alx::TX, size);
        }
        self.mapwait(wait);
    }

    /// mb_pathspecial: pathobj + SPECIAL marker.
    pub fn pathspecial(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        path_id: u16,
        hp: i32,
        ap: i32,
    ) {
        if hp == 10 && ap == 10 {
            self.mapobj(0, x, y, z, shape, is::PATHDHA);
        } else {
            self.mapobj(0, x, y, z, shape, is::PATH);
            self.setalvarb(al::HP, hp);
            self.setalvarb(al::AP, ap);
        }
        self.mapsetpath(path_id);
        self.emit8(op::SPECIAL);
        self.mapwait(wait);
    }

    /// mb_pathcspecial: pathobj + CSPECIAL marker.
    pub fn pathcspecial(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        path_id: u16,
        hp: i32,
        ap: i32,
    ) {
        if hp == 10 && ap == 10 {
            self.mapobj(0, x, y, z, shape, is::PATHDHA);
        } else {
            self.mapobj(0, x, y, z, shape, is::PATH);
            self.setalvarb(al::HP, hp);
            self.setalvarb(al::AP, ap);
        }
        self.mapsetpath(path_id);
        self.mapcspecial();
        self.mapwait(wait);
    }

    // ---- far-ship background flights (mb_map_farships*) ----

    fn map_farships_common(
        &mut self,
        shape: u16,
        face_around: bool,
        x: i32,
        y: i32,
        z: i32,
        x_speed: i32,
        y_speed: i32,
        depth: i32,
    ) {
        self.mapobj(300, x, SPACE_VIEWCY + y, z, shape, is::SHIPS);
        self.setalvarw(al::SWORD1, x_speed);
        self.setalvarw(al::SWORD2, y_speed);
        self.setalxvarw(alx::DEPTHOFFSET, depth);
        if face_around {
            self.setalvarb(al::ROTY, DEG180);
        }
    }

    pub fn map_farships0(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        x_speed: i32,
        y_speed: i32,
        depth: i32,
    ) {
        self.map_farships_common(sh::SHIP_S_0, true, x, y, z, x_speed, y_speed, depth);
    }

    pub fn map_farships1(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        x_speed: i32,
        y_speed: i32,
        depth: i32,
    ) {
        self.map_farships_common(sh::SHIP_S_1, true, x, y, z, x_speed, y_speed, depth);
    }

    pub fn map_farships2(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        x_speed: i32,
        y_speed: i32,
        depth: i32,
    ) {
        self.map_farships_common(sh::SHIPS, false, x, y, z, x_speed, y_speed, depth);
    }

    // ---- special/boss markers ----

    /// mb_cspecial: mapobj + CSPECIAL + wait.
    pub fn cspecial(&mut self, wait: i32, x: i32, y: i32, z: i32, shape: u16, strat: u32) {
        self.mapobj(0, x, y, z, shape, strat);
        self.mapcspecial();
        self.mapwait(wait);
    }

    /// mb_special: mapobj + SPECIAL + wait.
    pub fn special(&mut self, wait: i32, x: i32, y: i32, z: i32, shape: u16, strat: u32) {
        self.mapobj(0, x, y, z, shape, strat);
        self.emit8(op::SPECIAL);
        self.mapwait(wait);
    }

    // ---- skill-fly bonus markers ----

    pub fn skillfly_init(&mut self) {
        self.setvarb(wm::SKILLFLY, 0);
    }

    pub fn skillfly_set(&mut self, x: i32, y: i32, z: i32, radius: i32) {
        self.mapobj(0, x, y, z, sh::NULLSHAPE, is::SKILLFLY);
        self.setalvarw(al::SWORD1, radius);
    }

    pub fn skillfly_set_default(&mut self, x: i32, y: i32, z: i32) {
        self.mapobj(0, x, y, z, sh::NULLSHAPE, is::SKILLFLY);
    }

    // ---- MAP3_3A helpers ----

    pub fn mapfadetosea(&mut self) {
        self.emit8(op::FADETOSEA);
    }

    pub fn mapfadetoground(&mut self) {
        self.emit8(op::FADETOGROUND);
    }

    /// roottree MACRO — mapobj stalk/tree3 + setalvar sbyte2 + mapwait.
    pub fn roottree(&mut self, wait: i32, x: i32, y: i32, z: i32, angle: i32, _time_to_tail: i32) {
        self.mapobj(0, x, y, z, sh::STALK, is::TREE3);
        self.setalvarb(al::SBYTE2, angle);
        self.mapwait(wait);
    }

    /// nessie MACRO — mapobj nullshape/lochnessmonster + roty + sword1+1 + wait.
    pub fn nessie(&mut self, wait: i32, y: i32, z: i32, depth: i32, roty: i32, tail_time: i32) {
        self.mapobj(0, y, z, depth, sh::NULLSHAPE, is::LOCHNESSMONSTER);
        self.setalvarb(al::ROTY, roty);
        self.setalvarb(al::SWORD1 + 1, tail_time);
        self.mapwait(wait);
    }

    // ---- space-bar (asteroid-field bar obstacle) machinery ----

    fn spacebar_shape_id(&self, shape: SpacebarShape) -> u16 {
        let solid = self.barshape_mode == BarShapeMode::Solid;
        match shape {
            SpacebarShape::X => {
                if solid {
                    sh::XSOLIDSPACEBAR
                } else {
                    sh::XWIRESPACEBAR
                }
            }
            SpacebarShape::Xp => {
                if solid {
                    sh::XPSOLIDSPACEBAR
                } else {
                    sh::XPWIRESPACEBAR
                }
            }
            SpacebarShape::Y => {
                if solid {
                    sh::YSOLIDSPACEBAR
                } else {
                    sh::YWIRESPACEBAR
                }
            }
            SpacebarShape::Z => {
                if solid {
                    sh::ZSOLIDSPACEBAR
                } else {
                    sh::ZWIRESPACEBAR
                }
            }
            SpacebarShape::Sx => {
                if solid {
                    sh::SXSOLIDSPACEBAR
                } else {
                    sh::SXWIRESPACEBAR
                }
            }
            SpacebarShape::Sxp => {
                if solid {
                    sh::SXPSOLIDSPACEBAR
                } else {
                    sh::SXPWIRESPACEBAR
                }
            }
            SpacebarShape::Sy => {
                if solid {
                    sh::SYSOLIDSPACEBAR
                } else {
                    sh::SYWIRESPACEBAR
                }
            }
            SpacebarShape::Sz => {
                if solid {
                    sh::SZSOLIDSPACEBAR
                } else {
                    sh::SZWIRESPACEBAR
                }
            }
        }
    }

    fn spacebar_units(value: i32) -> i32 {
        ((value as i16).wrapping_mul(SPACEBAR_UNIT_LEN as i16)) as i32
    }

    /// mb_map_setbarshape: selects wire/solid bar shapes and auto-wait mode.
    pub fn map_setbarshape(&mut self, mode: BarShapeMode, autowait: bool) {
        self.barshape_mode = mode;
        self.barshape_autowait = autowait;
        self.barshape_swait = 0;
        self.barshape_pos = 0;
    }

    fn spacebar_calcsbwait(&mut self, z: i32) {
        if !self.barshape_autowait {
            return;
        }
        let delta = (z as i16).wrapping_sub(self.barshape_swait);
        if delta > 0 {
            self.mapwait(Self::spacebar_units(delta as i32));
            self.barshape_swait = z as i16;
        }
    }

    pub fn map_spacebarwait(&mut self, wait: i32) {
        self.mapwait(Self::spacebar_units(wait));
    }

    fn spacebar_z(&self, z: i32) -> i32 {
        SPACEBAR_BASE_DIST
            + if self.barshape_autowait {
                0
            } else {
                Self::spacebar_units(z)
            }
    }

    pub fn map_xspacebar(&mut self, x: i32, y: i32, z: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::X);
        let zz = self.spacebar_z(z);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR,
        );
        self.spacebar_calcsbwait(z);
    }

    pub fn map_yspacebar(&mut self, x: i32, y: i32, z: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Y);
        let zz = self.spacebar_z(z);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR,
        );
        self.spacebar_calcsbwait(z);
    }

    pub fn map_zspacebar(&mut self, x: i32, y: i32, z: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Z);
        let zz = self.spacebar_z(z);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR,
        );
        self.spacebar_calcsbwait(z);
    }

    pub fn map_sxspacebar(&mut self, x: i32, y: i32, z: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Sx);
        let zz = self.spacebar_z(z);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR,
        );
        self.spacebar_calcsbwait(z);
    }

    pub fn map_syspacebar(&mut self, x: i32, y: i32, z: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Sy);
        let zz = self.spacebar_z(z);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR,
        );
        self.spacebar_calcsbwait(z);
    }

    pub fn map_szspacebar(&mut self, x: i32, y: i32, z: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Sz);
        let zz = self.spacebar_z(z);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR,
        );
        self.spacebar_calcsbwait(z);
    }

    /// mb_map_spacebarc: rotating "carrier" bar; records barshape_pos.
    pub fn map_spacebarc(&mut self, x: i32, y: i32, z: i32, init_z_rot: i32, speed: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Xp);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            shape,
            MAP_ISTRAT_SPACEBAR1,
        );
        self.setalvarb(al::ROTZ, init_z_rot * DEG45);
        self.setalvarb(al::SBYTE1, speed);
        self.setvarobj(wm::MAPVAR1);
        self.barshape_pos = Self::spacebar_units(z) as i16;
    }

    /// mb_map_spacebaric: invisible carrier variant (nullshape + sbyte2=1).
    pub fn map_spacebaric(&mut self, x: i32, y: i32, z: i32, init_z_rot: i32, speed: i32) {
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            sh::NULLSHAPE,
            MAP_ISTRAT_SPACEBAR1,
        );
        self.setalvarb(al::ROTZ, init_z_rot * DEG45);
        self.setalvarb(al::SBYTE1, speed);
        self.setalvarb(al::SBYTE2, 1);
        self.setvarobj(wm::MAPVAR1);
        self.barshape_pos = Self::spacebar_units(z) as i16;
    }

    /// mb_map_spacebarx: satellite bar linked to the carrier via al_ptr.
    pub fn map_spacebarx(&mut self, x: i32, y: i32, z: i32, init_z_rot: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Xp);
        let zz = SPACEBAR_BASE_DIST + Self::spacebar_units(z) + self.barshape_pos as i32;
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR3,
        );
        self.setalvarb(al::ROTZ, init_z_rot * DEG45);
        self.setalvarptrw(al::PTR, wm::MAPVAR1);
    }

    pub fn map_spacebarsx(&mut self, x: i32, y: i32, z: i32, init_z_rot: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Sxp);
        let zz = SPACEBAR_BASE_DIST + Self::spacebar_units(z) + self.barshape_pos as i32;
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR3,
        );
        self.setalvarb(al::ROTZ, init_z_rot * DEG45);
        self.setalvarptrw(al::PTR, wm::MAPVAR1);
    }

    pub fn map_spacebarsz(&mut self, x: i32, y: i32, z: i32, init_z_rot: i32) {
        let shape = self.spacebar_shape_id(SpacebarShape::Sz);
        let zz = SPACEBAR_BASE_DIST + Self::spacebar_units(z) + self.barshape_pos as i32;
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            zz,
            shape,
            MAP_ISTRAT_SPACEBAR3,
        );
        self.setalvarb(al::ROTZ, init_z_rot * DEG45);
        self.setalvarptrw(al::PTR, wm::MAPVAR1);
    }

    /// mb_map_xpspacebar: self-spinning bar.
    pub fn map_xpspacebar(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        init_z_rot: i32,
        speed: i32,
    ) {
        let shape = self.spacebar_shape_id(SpacebarShape::Xp);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            shape,
            MAP_ISTRAT_SPINSPACEBAR,
        );
        self.setalvarb(al::ROTZ, init_z_rot * DEG45);
        self.setalvarb(al::SBYTE1, speed);
        self.map_spacebarwait(wait);
    }

    pub fn map_sxpspacebar(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        init_z_rot: i32,
        speed: i32,
    ) {
        let shape = self.spacebar_shape_id(SpacebarShape::Sxp);
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            shape,
            MAP_ISTRAT_SPINSPACEBAR,
        );
        self.setalvarb(al::ROTZ, init_z_rot * DEG45);
        self.setalvarb(al::SBYTE1, speed);
        self.map_spacebarwait(wait);
    }

    /// mb_map_ryspacebar: X bar with a Y rotation (note: C calls
    /// calcsbwait twice — once inside xspacebar, once after).
    pub fn map_ryspacebar(&mut self, x: i32, y: i32, z: i32, y_rot: i32) {
        self.map_xspacebar(x, y, z);
        self.setalvarb(al::ROTY, y_rot * DEG45);
        self.spacebar_calcsbwait(z);
    }

    // ---- composed space-bar patterns (map_SBtype*) ----

    pub fn map_sbtype0(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_yspacebar(x, y, z);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype1(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_xspacebar(x, y, z);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype5(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_zspacebar(x, y, z);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype6(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_xpspacebar(wait, x, y, z, 2, -4);
    }

    pub fn map_sbtype7(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_spacebarc(x, y, z, 0, 4);
        self.map_spacebarx(x + 2, y, 0, 2);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype3(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_spacebarc(x, y, z, 0, -6);
        self.map_spacebarx(x, y, 0, 2);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype8(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_szspacebar(x, y, z);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype_a(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_sxpspacebar(wait, x, y, z, 2, -4);
    }

    pub fn map_sbtype_b(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_syspacebar(x, y, z);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype_c(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_sxspacebar(x, y, z);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype_d(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_xspacebar(x + 2, y - 2, z);
        self.map_yspacebar(x + 3, y, z);
        self.map_sxspacebar(x + 2, y, z);
        self.map_zspacebar(x + 3, y + 2, z + 2);
        self.map_xspacebar(x + 5, y + 2, z + 4);
        self.map_yspacebar(x + 3, y + 1, z + 4);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype_e(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_xspacebar(x - 1, y, z);
        self.map_xspacebar(x - 5, y, z);
        self.map_yspacebar(x - 7, y + 1, z);
        self.map_xspacebar(x - 9, y - 1, z);
        self.map_xspacebar(x - 9, y + 3, z);
        self.map_zspacebar(x - 7, y, z + 2);
        self.map_syspacebar(x - 7, y, z + 4);
        self.map_sxspacebar(x - 8, y - 1, z);
        self.map_sxspacebar(x - 8, y + 1, z);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype_f(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_xspacebar(x, y, z);
        self.map_ryspacebar(x + 2, y, z, 3);
        self.map_ryspacebar(x - 1, y, z + 3, 3);
        self.map_xspacebar(x - 2, y, z + 6);
        self.map_xspacebar(x + 2, y, z + 6);
        self.map_ryspacebar(x + 4, y, z + 6, 3);
        self.map_ryspacebar(x + 1, y, z + 9, 3);
        self.map_xspacebar(x, y, z + 12);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype10(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_spacebarc(x, y, z, 0, 6);
        self.map_spacebarx(x, y, 0, 2);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype11(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_yspacebar(x, y, z);
        self.map_xspacebar(x + 2, y - 2, z);
        self.map_xspacebar(x + 2, y + 2, z);
        self.map_yspacebar(x + 4, y + 2, z);
        self.map_zspacebar(x, y - 2, z - 2);
        self.map_zspacebar(x, y + 2, z + 2);
        self.map_syspacebar(x, y + 1, z + 4);
        self.map_zspacebar(x + 4, y - 2, z + 2);
        self.map_sxspacebar(x + 3, y - 2, z + 4);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype14(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_spacebarc(x, y, z, 0, -3);
        self.map_spacebarsx(x - 2, y - 1, 0, 2);
        self.map_zspacebar(x + 2, y, 2);
        self.map_spacebarx(x + 2, y + 2, 0, 2);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype15(&mut self, wait: i32, x: i32, y: i32, z: i32, init_z_rot: i32, speed: i32) {
        self.map_spacebaric(x, y, z, init_z_rot, speed);
        self.map_spacebarx(x, y - 2, 0, init_z_rot);
        self.map_spacebarx(x, y + 1, 0, init_z_rot);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype16(&mut self, wait: i32, x: i32, y: i32, z: i32, x_vel: i32, spin_speed: i32) {
        let shape = if spin_speed == 0 {
            self.spacebar_shape_id(SpacebarShape::X)
        } else {
            self.spacebar_shape_id(SpacebarShape::Xp)
        };
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            shape,
            is::SPACEBARSHOOT,
        );
        self.setalvarw(al::SWORD1, x_vel);
        self.setalvarb(al::SBYTE1, spin_speed);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype17(&mut self, wait: i32, x: i32, y: i32, z: i32, y_vel: i32, spin_speed: i32) {
        let shape = if spin_speed == 0 {
            self.spacebar_shape_id(SpacebarShape::Y)
        } else {
            self.spacebar_shape_id(SpacebarShape::Xp)
        };
        self.mapobj(
            0,
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            shape,
            is::SPACEBARSHOOT,
        );
        if spin_speed != 0 {
            self.setalvarb(al::ROTZ, DEG90);
        }
        self.setalvarw(al::SWORD2, y_vel);
        self.setalvarb(al::SBYTE1, spin_speed);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype12(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_yspacebar(x, y, z);
        self.map_yspacebar(x + 4, y, z);
        self.map_xspacebar(x + 2, y - 2, z);
        self.map_xspacebar(x + 2, y + 2, z);
        self.map_syspacebar(x, y - 3, z);
        self.map_szspacebar(x, y - 2, z + 1);
        self.map_zspacebar(x, y + 2, z + 2);
        self.map_syspacebar(x, y + 2, z + 4);
        self.map_szspacebar(x + 1, y + 2, z + 4);
        self.map_ryspacebar(x + 4, y - 2, z, 3);
        self.map_szspacebar(x + 4, y - 2, z - 1);
        self.map_spacebarwait(wait);
    }

    pub fn map_sbtype13(&mut self, wait: i32, x: i32, y: i32, z: i32) {
        self.map_xpspacebar(wait, x, y, z, 2, 4);
    }

    pub fn map_sbtype18(&mut self, wait: i32, x: i32, y: i32, z: i32, init_z_rot: i32, speed: i32) {
        self.map_xpspacebar(wait, x, y, z, init_z_rot, speed);
    }

    pub fn map_sbtype19(&mut self, wait: i32, x: i32, y: i32, z: i32, init_z_rot: i32, speed: i32) {
        self.map_sxpspacebar(wait, x, y, z, init_z_rot, speed);
    }

    /// map_SBtypeOBJ: mapobj in spacebar coordinate space.
    pub fn map_sbtype_obj(&mut self, wait: i32, x: i32, y: i32, z: i32, shape: u16, istrat: u32) {
        self.mapobj(
            Self::spacebar_units(wait),
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            shape,
            istrat,
        );
    }

    /// map_SBtypeOBJ variant with a raw strat address (forces NORMOBJ).
    pub fn map_sbtype_obj_nobj(
        &mut self,
        wait: i32,
        x: i32,
        y: i32,
        z: i32,
        shape: u16,
        strat_addr: u32,
    ) {
        self.mapnobj(
            Self::spacebar_units(wait),
            Self::spacebar_units(x),
            SPACE_VIEWCY + Self::spacebar_units(y),
            SPACEBAR_BASE_DIST + Self::spacebar_units(z),
            shape,
            strat_addr,
        );
    }

    // ---- misc composed spawns ----

    /// mb_szaco2_mapobj: swooping zaco with a target point.
    pub fn szaco2_mapobj(&mut self, x: i32, y: i32, to_x: i32, to_y: i32, wait: i32) {
        self.mapobj(0, x, y, 2000, sh::ZACO_8, is::SZACO2);
        self.setalxvarw(alx::SWPX1, to_x);
        self.setalxvarw(alx::SWPY1, to_y);
        self.setalvarb(al::ROTX, -DEG90);
        if x == 0 {
            self.setalvarb(al::ROTY, DEG180);
        } else if x < 0 {
            self.setalvarb(al::ROTY, -DEG90);
        } else {
            self.setalvarb(al::ROTY, DEG90);
        }
        if wait != 0 {
            self.mapwait(wait);
        }
    }

    /// map_sfish MACRO: a school of linked s_fish objects.
    pub fn map_sfish(&mut self, wait: i32, x: i32, y: i32, z: i32, count: i32) {
        self.mapobj(0, x, y, z, sh::S_FISH, is::SFISH);
        self.setvarobj(wm::MAPVAR1);
        for _ in 1..count {
            self.mapobj(0, 0, 0, 4000, sh::S_FISH, is::SFISH);
            self.setalvarptrw(al::PTR, wm::MAPVAR1);
        }
        self.mapwait(wait);
    }
}
