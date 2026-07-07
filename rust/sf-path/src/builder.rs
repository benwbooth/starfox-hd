//! Path bytecode builder.
//!
//! C origin: `src/path/path_literals.c` (`PathLiteralBuilder` struct and the
//! `pb_*` emit helpers). Each method mirrors the C helper of the same name
//! with the `pb_`/`pb_emit_` prefix stripped; parameter conversion follows C
//! implicit-conversion semantics exactly (arguments are taken as `i32` and
//! truncated to the C parameter type with `as`), so the emitted bytes are
//! identical to the C build.

use crate::ids::PATH_DATA_COUNT_LITERAL;
use crate::opcodes::*;

/// C `PATH_DATA_CAPACITY` (src/path/path_literals.c).
pub const PATH_DATA_CAPACITY: usize = 65536;
/// C `PATH_MISSING_OFFSET` — offset table entry for unported path ids.
pub const PATH_MISSING_OFFSET: u16 = 0xFFFF;

/// C `PathLiteralBuilder` (src/path/path_literals.c). Label/fixup capacities
/// are unbounded here (Vec) — the C arrays only exist because C lacks
/// dynamic containers; overflow there is a build failure, not a data format.
pub struct PathLiteralBuilder {
    pub data: Vec<u8>,
    pub offsets: Vec<u16>,
    labels: Vec<(&'static str, u16)>,
    fixups: Vec<(&'static str, u16)>,
    pub failed: bool,
}

impl Default for PathLiteralBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PathLiteralBuilder {
    /// Mirrors the `build_path_catalog` prologue: zeroed builder, offsets
    /// memset to 0xFF.
    pub fn new() -> Self {
        PathLiteralBuilder {
            data: Vec::with_capacity(PATH_DATA_CAPACITY),
            offsets: vec![PATH_MISSING_OFFSET; PATH_DATA_COUNT_LITERAL as usize],
            labels: Vec::new(),
            fixups: Vec::new(),
            failed: false,
        }
    }

    /// C `pb_fail`.
    fn fail(&mut self, reason: &str) {
        if self.failed {
            return;
        }
        self.failed = true;
        eprintln!("Path literals: {reason}");
    }

    /// C `pb_emit8` (value truncated to uint8 like the C parameter type).
    pub fn emit8(&mut self, value: impl Into<i32>) {
        if self.failed {
            return;
        }
        if self.data.len() >= PATH_DATA_CAPACITY {
            self.fail("bytecode buffer overflow");
            return;
        }
        self.data.push(value.into() as u8);
    }

    /// C `pb_emit16` — little-endian.
    pub fn emit16(&mut self, value: impl Into<i32>) {
        let v = value.into() as u16;
        self.emit8((v & 0xFF) as i32);
        self.emit8((v >> 8) as i32);
    }

    /// C `pb_emit16s`.
    pub fn emit16s(&mut self, value: impl Into<i32>) {
        self.emit16(value.into() as i16 as i32);
    }

    /// C `pb_label`.
    pub fn label(&mut self, name: &'static str) {
        if self.failed {
            return;
        }
        self.labels.push((name, self.data.len() as u16));
    }

    /// C `pb_fixup16` — records a fixup site and emits a 16-bit placeholder.
    pub fn fixup16(&mut self, label: &'static str) {
        if self.failed {
            return;
        }
        self.fixups.push((label, self.data.len() as u16));
        self.emit16(0);
    }

    /// C `pb_start_path` — records the current offset for `path_id`.
    pub fn start_path(&mut self, path_id: impl Into<i32>, label: &'static str) {
        let path_id = path_id.into() as u16;
        if self.failed || path_id >= PATH_DATA_COUNT_LITERAL {
            self.fail("invalid path id");
            return;
        }
        self.offsets[path_id as usize] = self.data.len() as u16;
        self.label(label);
    }

    /// C `pb_lookup_label`.
    fn lookup_label(&self, label: &str) -> Option<u16> {
        self.labels
            .iter()
            .find(|(name, _)| *name == label)
            .map(|&(_, off)| off)
    }

    /// C `pb_resolve` — patches all fixup sites; unresolved labels resolve to
    /// offset 0 with a warning, exactly like the C builder.
    pub fn resolve(&mut self) {
        if self.failed {
            return;
        }
        for i in 0..self.fixups.len() {
            let (label, offset) = self.fixups[i];
            let target = match self.lookup_label(label) {
                Some(t) => t,
                None => {
                    eprintln!("Path literals: unresolved label '{label}'");
                    0
                }
            };
            self.data[offset as usize] = (target & 0xFF) as u8;
            self.data[offset as usize + 1] = (target >> 8) as u8;
        }
    }

    /// C `pb_emit_add` — picks the narrowest add opcode for the slot.
    pub fn emit_add(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        let offset = offset.into() as u8;
        let value = value.into() as i16;
        if offset == PAL_ROTX as u8 {
            self.emit8(P_ADDROTX);
            self.emit8(value as i8 as u8);
            return;
        }
        if offset == PAL_ROTY as u8 {
            self.emit8(P_ADDROTY);
            self.emit8(value as i8 as u8);
            return;
        }
        if offset == PAL_ROTZ as u8 {
            self.emit8(P_ADDROTZ);
            self.emit8(value as i8 as u8);
            return;
        }
        if offset == PAL_WORLDX as u8 && (-128..=127).contains(&value) {
            self.emit8(P_ADDWORLDX);
            self.emit8(value as i8 as u8);
            return;
        }
        if offset == PAL_WORLDY as u8 && (-128..=127).contains(&value) {
            self.emit8(P_ADDWORLDY);
            self.emit8(value as i8 as u8);
            return;
        }
        if offset == PAL_WORLDZ as u8 && (-128..=127).contains(&value) {
            self.emit8(P_ADDWORLDZ);
            self.emit8(value as i8 as u8);
            return;
        }
        if offset == PAL_WORLDX as u8 || offset == PAL_WORLDY as u8 || offset == PAL_WORLDZ as u8 {
            // ROM P_ADD (PATHMACS.ASM:304): word alvars whose value does not
            // fit `(num+128)&$ffffff00 == 0` (i.e. outside -128..=127) get
            // p_addw + dw, never a truncated byte form. The in-range world
            // cases were handled above, so this is the wide fallback.
            self.emit_addw(offset as i32, value as i32);
            return;
        }
        self.emit8(P_ADDB);
        self.emit8(offset);
        self.emit8(value as i8 as u8);
    }

    /// C `pb_emit_addw`.
    pub fn emit_addw(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        self.emit8(P_ADDW);
        self.emit8(offset.into() as u8);
        self.emit16s(value.into() as i16 as i32);
    }

    /// C `pb_is_byte_offset`.
    fn is_byte_offset(offset: u8) -> bool {
        matches!(
            offset as i32,
            PAL_ROTX
                | PAL_ROTY
                | PAL_ROTZ
                | PAL_VEL
                | PAL_HP
                | PAL_AP
                | PAL_CHILDX
                | PAL_CHILDY
                | PAL_CHILDZ
                | PAL_CHILDROTX
                | PAL_CHILDROTY
                | PAL_PBYTE1
                | PAL_PBYTE2
        )
    }

    /// C `pb_emit_set` — byte slots get P_SETB, word slots P_SETW.
    /// ROM P_SET (PATHMACS.ASM:304 region) collapses a literal 0 to P_ZERO
    /// (p_set0b/p_set0w), so value 0 routes through `emit_zero` for byte
    /// parity with the assembled blob.
    pub fn emit_set(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        let offset = offset.into() as u8;
        let value = value.into() as i16;
        if value == 0 {
            self.emit_zero(offset as i32);
            return;
        }
        if Self::is_byte_offset(offset) {
            self.emit8(P_SETB);
            self.emit8(value as u8);
            self.emit8(offset);
            return;
        }
        self.emit8(P_SETW);
        self.emit16s(value as i32);
        self.emit8(offset);
    }

    /// C `pb_emit_setb`.
    pub fn emit_setb(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        self.emit8(P_SETB);
        self.emit8(value.into() as i8 as u8);
        self.emit8(offset.into() as u8);
    }

    /// C `pb_emit_ifsameb`.
    /// ROM P_IFSAME (PATHMACS.ASM) collapses a literal 0 to P_IFZERO
    /// (p_ifzerob) for byte parity with the assembled blob.
    pub fn emit_ifsameb(
        &mut self,
        offset: impl Into<i32>,
        value: impl Into<i32>,
        label: &'static str,
    ) {
        let value = value.into();
        if value as u8 == 0 {
            self.emit_ifzerob(offset, label);
            return;
        }
        self.emit8(P_IFSAMEB);
        self.emit8(offset.into() as u8);
        self.emit8(value as u8);
        self.fixup16(label);
    }

    /// C `pb_emit_ifsamew`.
    /// ROM P_IFSAME collapses a literal 0 to P_IFZERO (p_ifzerow).
    pub fn emit_ifsamew(
        &mut self,
        offset: impl Into<i32>,
        value: impl Into<i32>,
        label: &'static str,
    ) {
        let value = value.into();
        if value as u16 == 0 {
            self.emit8(P_IFZEROW);
            self.emit8(offset.into() as u8);
            self.fixup16(label);
            return;
        }
        self.emit8(P_IFSAMEW);
        self.emit8(offset.into() as u8);
        self.emit16(value as u16 as i32);
        self.fixup16(label);
    }

    /// C `pb_emit_iflevel`.
    pub fn emit_iflevel(&mut self, level_1based: impl Into<i32>, label: &'static str) {
        self.emit8(P_IFLEVEL);
        self.emit8(level_1based.into() as u8);
        self.fixup16(label);
    }

    /// C `pb_emit_zero`.
    pub fn emit_zero(&mut self, offset: impl Into<i32>) {
        let offset = offset.into() as u8;
        self.emit8(if Self::is_byte_offset(offset) {
            P_SET0B
        } else {
            P_SET0W
        });
        self.emit8(offset);
    }

    /// C `pb_emit_goto`.
    pub fn emit_goto(&mut self, opcode: impl Into<i32>, label: &'static str) {
        self.emit8(opcode.into() as u8);
        self.fixup16(label);
    }

    /// C `pb_emit_wait` — 1 frame collapses to P_WAIT1.
    pub fn emit_wait(&mut self, frames: impl Into<i32>) {
        let frames = frames.into() as u8;
        if frames == 1 {
            self.emit8(P_WAIT1);
            return;
        }
        self.emit8(P_WAIT);
        self.emit8(frames);
    }

    /// C `pb_emit_loop`.
    pub fn emit_loop(&mut self, count: impl Into<i32>, label: &'static str) {
        self.emit8(P_LOOP);
        self.emit8(count.into() as u8);
        self.fixup16(label);
    }

    /// C `pb_emit_chaseb`.
    pub fn emit_chaseb(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        self.emit8(P_ACHASEB);
        self.emit8(value.into() as u8);
        self.emit8(offset.into() as u8);
    }

    /// C `pb_emit_chasew`.
    pub fn emit_chasew(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        self.emit8(P_ACHASEW);
        self.emit16s(value.into() as i16 as i32);
        self.emit8(offset.into() as u8);
    }

    /// C `pb_emit_waitchaseb`.
    pub fn emit_waitchaseb(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        self.emit8(P_WAITACHASEB);
        self.emit8(value.into() as u8);
        self.emit8(offset.into() as u8);
    }

    /// C `pb_emit_waitchasew`.
    pub fn emit_waitchasew(&mut self, offset: impl Into<i32>, value: impl Into<i32>) {
        self.emit8(P_WAITACHASEW);
        self.emit16s(value.into() as i16 as i32);
        self.emit8(offset.into() as u8);
    }

    /// C `pb_emit_friend`.
    pub fn emit_friend(&mut self, friend_id: impl Into<i32>) {
        self.emit8(P_FRIEND);
        self.emit8(friend_id.into() as u8);
    }

    /// C `pb_emit_notfriend`.
    pub fn emit_notfriend(&mut self, friend_id: impl Into<i32>, label: &'static str) {
        self.emit8(P_NOTFRIEND);
        self.emit8(friend_id.into() as u8);
        self.fixup16(label);
    }

    /// C `pb_emit_message`.
    pub fn emit_message(&mut self, msg_id: impl Into<i32>) {
        self.emit8(P_MSG);
        self.emit8(msg_id.into() as u8);
    }

    /// C `pb_emit_message2`.
    pub fn emit_message2(&mut self) {
        self.emit8(P_MSG2);
        self.emit8(0);
    }

    /// C `pb_emit_message_meter`.
    pub fn emit_message_meter(&mut self, msg_id: impl Into<i32>) {
        self.emit8(P_MSGWITHMETER);
        self.emit8(msg_id.into() as u8);
    }

    /// C `pb_emit_findshape`.
    pub fn emit_findshape(&mut self, shape: impl Into<i32>) {
        self.emit8(P_FINDSHAPE);
        self.emit16(shape.into() as u16 as i32);
    }

    /// C `pb_emit_setvel`.
    pub fn emit_setvel(&mut self, velocity: impl Into<i32>) {
        self.emit8(P_SETVEL);
        self.emit8(velocity.into() as u8);
    }

    /// C `pb_emit_accel`.
    pub fn emit_accel(&mut self, speed: impl Into<i32>, rate: impl Into<i32>) {
        self.emit8(P_SETACCEL);
        self.emit8(speed.into() as u8);
        self.emit8(rate.into() as u8);
    }

    /// C `pb_emit_break`.
    pub fn emit_break(&mut self, label: &'static str) {
        self.emit8(P_BREAK);
        self.fixup16(label);
    }

    /// C `pb_emit_trigger` — negative trigger unregisters (P_ALWAYSOFF).
    pub fn emit_trigger(&mut self, label: &'static str, trigger: impl Into<i32>) {
        let trigger = trigger.into();
        if trigger < 0 {
            self.emit8(P_ALWAYSOFF);
            self.fixup16(label);
            return;
        }
        self.emit8(P_ALWAYS);
        self.fixup16(label);
        self.emit8(trigger as u8);
    }

    /// C `pb_emit_hitground`.
    pub fn emit_hitground(&mut self, ground_height: impl Into<i32>, label: &'static str) {
        self.emit8(P_HITGROUND);
        self.emit16s(ground_height.into() as i16 as i32);
        self.fixup16(label);
    }

    /// C `pb_emit_distless`.
    pub fn emit_distless(&mut self, distance: impl Into<i32>, label: &'static str) {
        self.emit8(P_DISTLESS);
        self.emit16(distance.into() as u16 as i32);
        self.fixup16(label);
    }

    /// C `pb_emit_distmore` — P_IFNOT prefix on distless.
    pub fn emit_distmore(&mut self, distance: impl Into<i32>, label: &'static str) {
        self.emit8(P_IFNOT);
        self.emit_distless(distance, label);
    }

    /// C `pb_emit_ifbetweenb`.
    pub fn emit_ifbetweenb(
        &mut self,
        offset: impl Into<i32>,
        lo: impl Into<i32>,
        hi: impl Into<i32>,
        label: &'static str,
    ) {
        self.emit8(P_IFBETWEENB);
        self.emit8(offset.into() as u8);
        self.emit8(lo.into() as i8 as u8);
        self.emit8(hi.into() as i8 as u8);
        self.fixup16(label);
    }

    /// C `pb_emit_ifbetweenw`.
    pub fn emit_ifbetweenw(
        &mut self,
        offset: impl Into<i32>,
        lo: impl Into<i32>,
        hi: impl Into<i32>,
        label: &'static str,
    ) {
        self.emit8(P_IFBETWEENW);
        self.emit8(offset.into() as u8);
        self.emit16s(lo.into() as i16 as i32);
        self.emit16s(hi.into() as i16 as i32);
        self.fixup16(label);
    }

    /// C `pb_emit_setrandomb`.
    pub fn emit_setrandomb(&mut self, abs_offset: impl Into<i32>, mask: impl Into<i32>) {
        self.emit8(P_SETRANDOMB);
        self.emit16(abs_offset.into() as u16 as i32);
        self.emit8(mask.into() as u8);
    }

    /// C `pb_emit_setrandomw`.
    pub fn emit_setrandomw(&mut self, abs_offset: impl Into<i32>, mask: impl Into<i32>) {
        self.emit8(P_SETRANDOMW);
        self.emit16(abs_offset.into() as u16 as i32);
        self.emit16(mask.into() as u16 as i32);
    }

    /// C `pb_emit_negb`.
    pub fn emit_negb(&mut self, abs_offset: impl Into<i32>) {
        self.emit8(P_NEGB);
        self.emit16(abs_offset.into() as u16 as i32);
    }

    /// C `pb_emit_negw`.
    pub fn emit_negw(&mut self, abs_offset: impl Into<i32>) {
        self.emit8(P_NEGW);
        self.emit16(abs_offset.into() as u16 as i32);
    }

    /// C `pb_emit_div2b`.
    pub fn emit_div2b(&mut self, abs_offset: impl Into<i32>) {
        self.emit8(P_DIV2B);
        self.emit16(abs_offset.into() as u16 as i32);
    }

    /// C `pb_emit_varop`.
    fn emit_varop(
        &mut self,
        is_add: bool,
        dst_word: bool,
        dst_offset: u8,
        src_word: bool,
        src_offset: u8,
    ) {
        let opcode = if is_add {
            if dst_word {
                if src_word {
                    P_ADDVWW
                } else {
                    P_ADDVWB
                }
            } else if src_word {
                P_ADDVBW
            } else {
                P_ADDVBB
            }
        } else if dst_word {
            if src_word {
                P_SETVWW
            } else {
                P_SETVWB
            }
        } else if src_word {
            P_SETVBW
        } else {
            P_SETVBB
        };
        self.emit8(opcode);
        self.emit8(dst_offset);
        self.emit8(src_offset);
    }

    /// C `pb_emit_setv`.
    pub fn emit_setv(
        &mut self,
        dst_word: bool,
        dst_offset: impl Into<i32>,
        src_word: bool,
        src_offset: impl Into<i32>,
    ) {
        self.emit_varop(
            false,
            dst_word,
            dst_offset.into() as u8,
            src_word,
            src_offset.into() as u8,
        );
    }

    /// C `pb_emit_addv`.
    pub fn emit_addv(
        &mut self,
        dst_word: bool,
        dst_offset: impl Into<i32>,
        src_word: bool,
        src_offset: impl Into<i32>,
    ) {
        self.emit_varop(
            true,
            dst_word,
            dst_offset.into() as u8,
            src_word,
            src_offset.into() as u8,
        );
    }

    /// C `pb_emit_addvwb`.
    pub fn emit_addvwb(&mut self, dst_offset: impl Into<i32>, src_offset: impl Into<i32>) {
        self.emit_addv(true, dst_offset, false, src_offset);
    }

    /// C `pb_emit_spawn` — shared payload for P_SPAWN/P_SPAWNLINK/P_SPAWNCHILD.
    /// Public because the C catalog calls it directly with an explicit opcode
    /// in a few places (e_ufo etc.), not only via the spawn_link/child wrappers.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_spawn(
        &mut self,
        opcode: impl Into<i32>,
        x: impl Into<i32>,
        y: impl Into<i32>,
        z: impl Into<i32>,
        rotx: impl Into<i32>,
        roty: impl Into<i32>,
        rotz: impl Into<i32>,
        shape: impl Into<i32>,
        path_id: impl Into<i32>,
        hp: impl Into<i32>,
        ap: impl Into<i32>,
        child_num: impl Into<i32>,
    ) {
        let opcode = opcode.into() as u8;
        self.emit8(opcode);
        self.emit16(shape);
        self.emit16(path_id);
        self.emit8(rotx);
        self.emit8(roty);
        self.emit8(rotz);
        self.emit8(hp);
        self.emit8(ap);
        let (x, y, z) = (x.into(), y.into(), z.into());
        if opcode == P_SPAWNCHILD {
            // Child offsets stay raw payload bytes: the port's child
            // transform (mother rotpos) adds them unscaled.
            self.emit8(x);
            self.emit8(y);
            self.emit8(z);
            self.emit8(child_num);
        } else {
            // ROM P_SPAWN macro stores ({coord})/4 (PATHMACS.ASM:1167); the
            // runtime scales x4 after rotation (PATHS.ASM:1790). Callers pass
            // the full coordinate; /4 truncates toward zero like the
            // assembler's division.
            self.emit8(x / 4);
            self.emit8(y / 4);
            self.emit8(z / 4);
        }
    }

    /// C `pb_emit_spawn_link`.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_spawn_link(
        &mut self,
        x: impl Into<i32>,
        y: impl Into<i32>,
        z: impl Into<i32>,
        rotx: impl Into<i32>,
        roty: impl Into<i32>,
        rotz: impl Into<i32>,
        shape: impl Into<i32>,
        path_id: impl Into<i32>,
        hp: impl Into<i32>,
        ap: impl Into<i32>,
    ) {
        self.emit_spawn(
            P_SPAWNLINK,
            // Full coordinates pass through — emit_spawn stores coord/4 and
            // pre-truncating to i8 here would wrap out-of-range coords
            // (tow_0 -200, septer -300) before the division.
            x.into(),
            y.into(),
            z.into(),
            rotx.into() as i8,
            roty.into() as i8,
            rotz.into() as i8,
            shape.into() as u16,
            path_id.into() as u16,
            hp.into() as u8,
            ap.into() as u8,
            0,
        );
    }

    /// C `pb_emit_spawn_child`.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_spawn_child(
        &mut self,
        x: impl Into<i32>,
        y: impl Into<i32>,
        z: impl Into<i32>,
        rotx: impl Into<i32>,
        roty: impl Into<i32>,
        rotz: impl Into<i32>,
        shape: impl Into<i32>,
        path_id: impl Into<i32>,
        hp: impl Into<i32>,
        ap: impl Into<i32>,
        child_num: impl Into<i32>,
    ) {
        self.emit_spawn(
            P_SPAWNCHILD,
            x.into() as i8,
            y.into() as i8,
            z.into() as i8,
            rotx.into() as i8,
            roty.into() as i8,
            rotz.into() as i8,
            shape.into() as u16,
            path_id.into() as u16,
            hp.into() as u8,
            ap.into() as u8,
            child_num.into() as u8,
        );
    }

    /// C `pb_emit_qspawn`.
    pub fn emit_qspawn(
        &mut self,
        shape: impl Into<i32>,
        path_id: impl Into<i32>,
        hp: impl Into<i32>,
        ap: impl Into<i32>,
    ) {
        self.emit8(P_QSPAWN);
        self.emit16(shape.into() as u16 as i32);
        self.emit16(path_id.into() as u16 as i32);
        self.emit8(hp.into() as u8);
        self.emit8(ap.into() as u8);
    }

    /// C `pb_emit_gotopos`.
    pub fn emit_gotopos(
        &mut self,
        x: impl Into<i32>,
        y: impl Into<i32>,
        z: impl Into<i32>,
        speed: impl Into<i32>,
    ) {
        self.emit8(P_GOTOPOS);
        self.emit16s(x.into() as i16 as i32);
        self.emit16s(y.into() as i16 as i32);
        self.emit16s(z.into() as i16 as i32);
        self.emit8(speed.into() as u8);
    }

    /// C `pb_emit_sprite`.
    pub fn emit_sprite(&mut self, colour: impl Into<i32>, size: impl Into<i32>) {
        self.emit8(P_SPRITE);
        self.emit8(colour.into() as u8);
        self.emit8(size.into() as u8);
    }

    /// C `pb_emit_soundeffect`.
    pub fn emit_soundeffect(&mut self, sound_id: impl Into<i32>) {
        self.emit8(P_SOUND);
        self.emit8(sound_id.into() as u8);
    }

    /// C `pb_emit_sound2`.
    pub fn emit_sound2(&mut self, sound_id: impl Into<i32>) {
        self.emit8(P_SOUND2);
        self.emit8(sound_id.into() as u8);
    }

    /// C `pb_emit_exportw`.
    pub fn emit_exportw(&mut self, offset: impl Into<i32>, addr: impl Into<i32>) {
        self.emit8(P_EXPORTW);
        self.emit8(offset.into() as u8);
        self.emit16(addr.into() as u16 as i32);
    }

    /// C `pb_emit_exportb`.
    pub fn emit_exportb(&mut self, offset: impl Into<i32>, addr: impl Into<i32>) {
        self.emit8(P_EXPORTB);
        self.emit8(offset.into() as u8);
        self.emit16(addr.into() as u16 as i32);
    }

    /// C `pb_emit_importw`.
    pub fn emit_importw(&mut self, offset: impl Into<i32>, addr: impl Into<i32>) {
        self.emit8(P_IMPORTW);
        self.emit8(offset.into() as u8);
        self.emit16(addr.into() as u16 as i32);
    }

    /// C `pb_emit_importb`.
    pub fn emit_importb(&mut self, offset: impl Into<i32>, addr: impl Into<i32>) {
        self.emit8(P_IMPORTB);
        self.emit8(offset.into() as u8);
        self.emit16(addr.into() as u16 as i32);
    }

    /// C `pb_emit_pushb`.
    pub fn emit_pushb(&mut self, offset: impl Into<i32>) {
        self.emit8(P_PUSHB);
        self.emit8(offset.into() as u8);
    }

    /// C `pb_emit_pullb`.
    pub fn emit_pullb(&mut self, offset: impl Into<i32>) {
        self.emit8(P_PULLB);
        self.emit8(offset.into() as u8);
    }

    /// C `pb_emit_ifzerob`.
    pub fn emit_ifzerob(&mut self, offset: impl Into<i32>, label: &'static str) {
        self.emit8(P_IFZEROB);
        self.emit8(offset.into() as u8);
        self.fixup16(label);
    }

    /// C `pb_emit_do`.
    pub fn emit_do(&mut self, count: impl Into<i32>) {
        self.emit8(P_DO);
        self.emit16(count.into() as u16 as i32);
    }

    /// C `pb_emit_setstrat_flat`.
    pub fn emit_setstrat_flat(&mut self, strat_id: impl Into<i32>) {
        self.emit8(P_SETSTRAT);
        self.emit16(strat_id.into() as u16 as i32);
        self.emit8(0);
    }

    /// C `pb_emit_indexb`.
    pub fn emit_indexb(
        &mut self,
        table_addr: impl Into<i32>,
        index_abs: impl Into<i32>,
        dest_abs: impl Into<i32>,
    ) {
        self.emit8(P_INDEXB);
        self.emit16(table_addr.into() as u16 as i32);
        self.emit8(0);
        self.emit16(index_abs.into() as u16 as i32);
        self.emit16(dest_abs.into() as u16 as i32);
    }

    /// C `pb_emit_start65816` — records the opcode offset (returned instead of
    /// the C out-pointer) and emits the inline-65816 marker blob:
    /// `P_START65816, A9 <resume_label:16> 6B` (lda #imm16 ; rtl).
    pub fn emit_start65816(&mut self, resume_label: &'static str) -> u16 {
        let ip = self.data.len() as u16;
        self.emit8(P_START65816);
        self.emit8(0xA9);
        self.fixup16(resume_label);
        self.emit8(0x6B);
        ip
    }
}

// ---------------------------------------------------------------------------
// PALVAROFFSET slot constants (C origin: src/path/path_literals.c anonymous
// enum PAL_*). Values are the SNES al_/alx_ byte offsets; bit7 selects the
// alx block, 0x100+ addresses select the absolute alx form used by
// P_SETRANDOM*/P_NEG*/P_DIV2*.
// Typed i32 so they flow through the builder's C-style argument conversion.
// ---------------------------------------------------------------------------
pub const PAL_ABS_PBYTE1: i32 = 0x100 + 50;
pub const PAL_ABS_PBYTE2: i32 = 0x100 + 51;
pub const PAL_ABS_PWORD1: i32 = 0x100 + 52;
pub const PAL_SHAPE: i32 = 4;
pub const PAL_WORLDX: i32 = 12;
pub const PAL_WORLDY: i32 = 14;
pub const PAL_WORLDZ: i32 = 16;
pub const PAL_ROTX: i32 = 18;
pub const PAL_ROTY: i32 = 19;
pub const PAL_ROTZ: i32 = 20;
pub const PAL_VEL: i32 = 21;
pub const PAL_HP: i32 = 42;
pub const PAL_AP: i32 = 43;
pub const PAL_COLTAB: i32 = 0x80 | 32;
pub const PAL_CHILDX: i32 = 0x80 | 34;
pub const PAL_CHILDY: i32 = 0x80 | 35;
pub const PAL_CHILDZ: i32 = 0x80 | 36;
pub const PAL_CHILDROTX: i32 = 0x80 | 37;
pub const PAL_CHILDROTY: i32 = 0x80 | 38;
pub const PAL_PBYTE1: i32 = 0x80 | 50;
pub const PAL_PBYTE2: i32 = 0x80 | 51;
pub const PAL_PWORD1: i32 = 0x80 | 52;
pub const PAL_DEPTHOFFSET: i32 = 0x80 | 21;
