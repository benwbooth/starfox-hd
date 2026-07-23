//! Oracle-only compatibility host shared by the recovered map and path
//! interpreters.
//!
//! The object pool lives in WRAM at its retail addresses.  This is important:
//! map spawns, path variable IDs, linked-object services, and draw-list
//! generation all observe one state instead of adapters backed by different
//! guessed structs.

use sf2_data::draw::{
    DrawRecord, DRAW_COUNT_MIRROR_WRAM_ADDRESS, DRAW_COUNT_WRAM_ADDRESS, DRAW_RECORD_CAPACITY,
};
use sf2_data::map::{MapAddress, SCRIPT_ROOTS};
use sf2_data::shape_data::shape_by_id;
use sf2_map::{MapVm, RunStop as MapRunStop};
use sf2_path::{command_at as path_command_at, PathVm};

use crate::cpu_bridge::CpuBridge;
use crate::memory::Memory;
use crate::object::*;

const PREVIOUS_VIEW_Y_STATE: u16 = 201;
const PREVIOUS_VIEW_Y_RENDER_STATE: u16 = 53_019;
const PREVIOUS_VIEW_DISTANCE_STATE: u16 = 6_329;
const PREVIOUS_VIEW_DISTANCE_RENDER_STATE: u16 = 53_017;
const CULLING_MODE_STATE: u16 = 6_468;
const CULLING_PADDING_STATE: u16 = 6_783;
const PRIMARY_VIEW_OBJECT: u16 = 831;
const SECONDARY_VIEW_OBJECT: u16 = 894;
const SPECIAL_COLOR_OBJECT_STATE: u16 = 5_841;
const GLOBAL_FRAME_STATE: u16 = 196;

const OBJECT_STATE_FLAGS: u16 = 8;
const OBJECT_SORT_FLAGS: u16 = 9;
const OBJECT_EXPLOSION_COUNTER: u16 = 10;
const OBJECT_RENDER_FLAGS: u16 = 32;
const OBJECT_GROUP_FLAGS: u16 = 35;
const OBJECT_VISIBILITY_FLAGS: u16 = 36;
const OBJECT_LIFECYCLE_FLAGS: u16 = 37;
const OBJECT_RENDER_POLICY_FLAGS: u16 = 38;

const OBJECT_ANIMATION_FRAME: u16 = 7_371;
const OBJECT_COLOR_FRAME: u16 = 7_370;
const OBJECT_DEPTH_OFFSET: u16 = 7_368;
const OBJECT_COLOR_TABLE: u16 = 7_373;
const OBJECT_TEXTURE_SCROLL_X: u16 = 7_386;
const OBJECT_TEXTURE_SCROLL_Y: u16 = 7_387;
const OBJECT_RENDER_EXTENSION: u16 = 7_407;

const OBJECT_ACTIVE_MASK: u8 = 0x08;
const OBJECT_FORMAT_PRESERVE_MASK: u8 = 0xE1;
const OBJECT_REMOVED_MASK: u8 = 0x08;
const OBJECT_SUPPRESS_PAIR_MASK: u8 = 0x20;
const OBJECT_GROUP_MEMBER_MASK: u8 = 0x40;
const OBJECT_PRIMARY_VISIBLE_MASK: u8 = 0x80;
const OBJECT_SECONDARY_VISIBLE_MASK: u8 = 0x01;
const OBJECT_CLEAR_WHEN_PRIMARY_MASK: u8 = 0x02;
const OBJECT_SHADOW_MASK: u8 = 0x10;
const OBJECT_EXPLODING_MASK: u8 = 0x01;
const OBJECT_SPECIAL_DEPTH_MASK: u8 = 0x01;

const BASE_CULLING_LIMIT: u16 = 12_000;
const RETAINED_VISIBILITY_PADDING: u16 = 500;
const SPECIAL_SORT_DEPTH: i16 = 15_000;
const FRAME_VALUE_MASK: u8 = 0x7F;
const EXPLICIT_FRAME_MASK: u8 = 0x80;
const SPECIAL_COLOR_PHASE_MASK: u8 = 0x07;
const SPECIAL_COLOR_PHASE_SPLIT: u8 = 4;
const SPECIAL_COLOR_FIRST: u16 = 0x8074;
const SPECIAL_COLOR_SECOND: u16 = 0x8174;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidRoot(usize),
    InvalidObject(u16),
    ObjectPoolExhausted,
    MapAuxiliaryPoolExhausted,
    AuxiliaryHeapExhausted,
    UnsupportedMapRoutine(u32),
    NativeStrategyMissing(u32),
    Cpu(String),
    Draw(String),
    Map(String),
    Path(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub rotation_x: u8,
    pub rotation_y: u8,
    pub rotation_z: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapMarker {
    pub kind: u8,
    /// Exact retail marker-table selector. Some path handlers use full
    /// 16-bit selectors (`$1400` and `$0320`).
    pub table_index: u16,
}

pub struct Game {
    pub memory: Memory,
    #[cfg(feature = "oracle-bridge")]
    cpu_bridge: CpuBridge,
    map_vm: MapVm,
    pub frame: u64,
    pub pad: u16,
    pub pad_trigger: u16,
    previous_pad: u16,
    pub mode: u8,
    pub display_ready: bool,
    pub load_table_idle: bool,
    pub stage_load: Option<u16>,
    pub post_load_requested: bool,
    pub map_markers: Vec<MapMarker>,
    pub messages: Vec<u8>,
    pub last_error: Option<Error>,
    render_records: Vec<(u16, DrawRecord)>,
}

impl Game {
    pub fn new(rom: Vec<u8>) -> Result<Self, Error> {
        Self::from_root(rom, 0)
    }

    /// Build the retail player-selection objects first, then enter one of the
    /// host-selected campaign/mission scripts without clearing their WRAM.
    ///
    /// The 25 extracted roots are entry points installed by the outer game
    /// state machine, not 25 independent programs. In particular, roots 1+
    /// assume root 0 has created and initialized both selection ships. This
    /// constructor supplies that otherwise-missing lifecycle step for the PC
    /// application while [`from_root`](Self::from_root) remains available for
    /// raw script auditing.
    pub fn from_playable_root(rom: Vec<u8>, root: usize) -> Result<Self, Error> {
        if root == 0 {
            return Self::from_root(rom, root);
        }
        SCRIPT_ROOTS.get(root).ok_or(Error::InvalidRoot(root))?;
        let mut game = Self::from_root(rom, 0)?;
        // Tick 1 runs both retail player initializers. Tick 2 runs their first
        // entry path and installs the proven `$06:9C27` per-frame strategy.
        game.tick(0)?;
        game.tick(0)?;
        game.install_script_root(root)?;
        Ok(game)
    }

    pub fn from_root(rom: Vec<u8>, root: usize) -> Result<Self, Error> {
        let script = SCRIPT_ROOTS.get(root).ok_or(Error::InvalidRoot(root))?;
        #[cfg(feature = "oracle-bridge")]
        let cpu_bridge = CpuBridge::new(rom.clone());
        let mut memory = Memory::new(rom);
        initialize_pool(&mut memory);
        memory.write_byte(0x192E, script.address.bank);
        memory.write_word(0x1657, retail_map_pointer(script.address));
        Ok(Self {
            memory,
            #[cfg(feature = "oracle-bridge")]
            cpu_bridge,
            map_vm: MapVm::new(script.address),
            frame: 0,
            pad: 0,
            pad_trigger: 0,
            previous_pad: 0,
            mode: 0,
            display_ready: true,
            load_table_idle: true,
            stage_load: None,
            post_load_requested: false,
            map_markers: Vec::new(),
            messages: Vec::new(),
            last_error: None,
            render_records: Vec::with_capacity(DRAW_RECORD_CAPACITY),
        })
    }

    /// Verification-only execution boundary for behavior not yet expressed
    /// as native Rust. Release builds fail closed instead of interpreting
    /// original machine code.
    pub(crate) fn run_unported_strategy(&mut self, target: u32, object: u16) -> Result<(), Error> {
        #[cfg(feature = "oracle-bridge")]
        {
            return self
                .cpu_bridge
                .call_strategy(&mut self.memory, target, object)
                .map_err(Error::Cpu);
        }
        #[cfg(not(feature = "oracle-bridge"))]
        {
            let _ = object;
            Err(Error::NativeStrategyMissing(target))
        }
    }

    pub(crate) fn rotate_collision_probe(&mut self, yaw: u8, x: i16, z: i16) -> (i16, i16) {
        self.cpu_bridge
            .rotate_collision_probe(&mut self.memory, yaw, x, z)
    }

    pub(crate) fn collision_polygon_contains(
        &mut self,
        source_address: u16,
        scale: u8,
        x: i16,
        z: i16,
    ) -> bool {
        self.cpu_bridge
            .collision_polygon_contains(&mut self.memory, source_address, scale, x, z)
    }

    pub(crate) fn project_collision_surface(
        &mut self,
        normal: [i8; 3],
        plane_offset: i16,
        x: i16,
        z: i16,
    ) -> i16 {
        self.cpu_bridge
            .project_collision_surface(&mut self.memory, normal, plane_offset, x, z)
    }

    /// Run one exact retail routine for differential verification. This API
    /// exists only in the feature-gated oracle compatibility host; the native
    /// shipping game has no processor executor or source-address interface.
    #[cfg(feature = "oracle-bridge")]
    pub fn run_retail_oracle_routine(&mut self, target: u32, object: u16) -> Result<(), Error> {
        self.run_unported_strategy(target, object)
    }

    /// Evaluate one retail collision plane for oracle-test seed construction.
    #[cfg(feature = "oracle-bridge")]
    pub fn retail_collision_surface(
        &mut self,
        normal: [i8; 3],
        plane_offset: i16,
        x: i16,
        z: i16,
    ) -> i16 {
        self.project_collision_surface(normal, plane_offset, x, z)
    }

    /// Rotate one oracle-test seed through retail's collision yaw kernel.
    #[cfg(feature = "oracle-bridge")]
    pub fn retail_rotate_collision_probe(&mut self, yaw: u8, x: i16, z: i16) -> (i16, i16) {
        self.rotate_collision_probe(yaw, x, z)
    }

    pub fn map_cursor(&self) -> MapAddress {
        self.map_vm.cursor()
    }

    pub fn map_counter(&self) -> u16 {
        self.map_vm.counter()
    }

    /// Install a root exactly as the retail outer state machine writes
    /// `$192E/$1657`, preserving the existing object pool and global state.
    pub fn install_script_root(&mut self, root: usize) -> Result<(), Error> {
        let script = SCRIPT_ROOTS.get(root).ok_or(Error::InvalidRoot(root))?;
        self.map_vm = MapVm::new(script.address);
        self.sync_map_state();
        Ok(())
    }

    pub fn release_external_phase(&mut self) -> bool {
        let released = self.map_vm.release_external_phase().is_some();
        if released {
            self.sync_map_state();
        }
        released
    }

    pub fn tick(&mut self, pad: u16) -> Result<(), Error> {
        self.pad = pad;
        self.pad_trigger = pad & !self.previous_pad;
        self.previous_pad = pad;
        // Retail's controller words: current state at `$1936`, newly pressed
        // edges at `$1938`. The high bytes (`$1937/$1939`) are read directly
        // by the flight routines for the d-pad and face buttons.
        self.memory.write_word(0x1936, self.pad);
        self.memory.write_word(0x1938, self.pad_trigger);
        self.frame = self.frame.wrapping_add(1);
        let global_frame = self.memory.read_byte(0x00C4).wrapping_add(1);
        self.memory.write_byte(0x00C4, global_frame);

        // Retail phase gates are released by the surrounding menu/stage state
        // machine.  Until that state machine is fully named, the same edge
        // inputs which accept the retail menus release only a proven gate.
        if self.pad_trigger & (sf_core::pad::START | sf_core::pad::A | sf_core::pad::B) != 0 {
            self.release_external_phase();
        }

        self.tick_map()?;
        self.tick_strategies()?;
        self.tick_paths()?;
        self.sync_camera_from_anchor();
        self.build_retail_draw_list()?;
        Ok(())
    }

    /// Build the native render list using the source routine's exact object
    /// filtering, hysteresis, and field-selection rules.
    fn build_retail_draw_list(&mut self) -> Result<(), Error> {
        self.memory.write_word(
            PREVIOUS_VIEW_DISTANCE_RENDER_STATE,
            self.memory.read_word(PREVIOUS_VIEW_DISTANCE_STATE),
        );
        self.memory.write_word(
            PREVIOUS_VIEW_Y_RENDER_STATE,
            self.memory.read_word(PREVIOUS_VIEW_Y_STATE),
        );

        let old_records = std::mem::take(&mut self.render_records);
        let mut records = Vec::with_capacity(DRAW_RECORD_CAPACITY);
        let secondary_view = self.memory.read_byte(CULLING_MODE_STATE) != 0;

        for object in active_objects(&self.memory) {
            let formatted = (self.memory.read_byte(object + OBJECT_STATE_FLAGS)
                & OBJECT_FORMAT_PRESERVE_MASK)
                | OBJECT_ACTIVE_MASK;
            self.memory
                .write_byte(object + OBJECT_STATE_FLAGS, formatted);

            let removed = self.memory.read_byte(object + OBJECT_GROUP_FLAGS) & 0x02 != 0
                || self.memory.read_byte(object + OBJECT_LIFECYCLE_FLAGS) & OBJECT_REMOVED_MASK
                    != 0;
            let visible = !removed && self.update_object_visibility(object, secondary_view);
            if visible && records.len() < DRAW_RECORD_CAPACITY {
                let previous = old_records
                    .get(records.len())
                    .map(|(_, record)| *record)
                    .unwrap_or_default();
                records.push((object, self.build_draw_record(object, previous)));
            }

            if !secondary_view {
                let flags = self.memory.read_byte(object + OBJECT_RENDER_FLAGS)
                    & !OBJECT_CLEAR_WHEN_PRIMARY_MASK;
                self.memory.write_byte(object + OBJECT_RENDER_FLAGS, flags);
            }
        }

        let count = records.len() as u16;
        self.memory.write_word(DRAW_COUNT_WRAM_ADDRESS, count);
        self.memory
            .write_word(DRAW_COUNT_MIRROR_WRAM_ADDRESS, count);
        self.render_records = records;
        Ok(())
    }

    fn update_object_visibility(&mut self, object: u16, secondary_view: bool) -> bool {
        let lifecycle_flags = self.memory.read_byte(object + OBJECT_LIFECYCLE_FLAGS);
        let group_flags = self.memory.read_byte(object + OBJECT_GROUP_FLAGS);
        let paired_suppression = lifecycle_flags & OBJECT_SUPPRESS_PAIR_MASK != 0;
        let group_member = group_flags & OBJECT_GROUP_MEMBER_MASK != 0;
        if paired_suppression && group_member == secondary_view {
            return false;
        }

        let shape = self.memory.read_word(object + FIELD_SHAPE);
        let shape_extent = shape_by_id(shape)
            .map(|entry| entry.size.wrapping_mul(4))
            .unwrap_or(0);
        let mut radius = shape_extent
            .wrapping_add(self.memory.read_word(CULLING_PADDING_STATE))
            .min(BASE_CULLING_LIMIT);

        let (view, visibility_field, visibility_mask) = if secondary_view {
            (
                SECONDARY_VIEW_OBJECT,
                OBJECT_VISIBILITY_FLAGS,
                OBJECT_SECONDARY_VISIBLE_MASK,
            )
        } else {
            (
                PRIMARY_VIEW_OBJECT,
                OBJECT_GROUP_FLAGS,
                OBJECT_PRIMARY_VISIBLE_MASK,
            )
        };
        let was_visible = self.memory.read_byte(object + visibility_field) & visibility_mask != 0;
        if was_visible {
            radius = radius.wrapping_add(RETAINED_VISIBILITY_PADDING);
        }

        let diameter = radius.wrapping_mul(2);
        let x_delta = self
            .memory
            .read_word(object + FIELD_X)
            .wrapping_sub(self.memory.read_word(view + FIELD_X));
        let z_delta = self
            .memory
            .read_word(object + FIELD_Z)
            .wrapping_sub(self.memory.read_word(view + FIELD_Z));
        let visible =
            x_delta.wrapping_add(radius) < diameter && z_delta.wrapping_add(radius) < diameter;

        let flags = self.memory.read_byte(object + visibility_field);
        self.memory.write_byte(
            object + visibility_field,
            if visible {
                flags | visibility_mask
            } else {
                flags & !visibility_mask
            },
        );
        visible
    }

    fn build_draw_record(&self, object: u16, mut record: DrawRecord) -> DrawRecord {
        let frame = self.memory.read_byte(GLOBAL_FRAME_STATE);
        let state_flags = self.memory.read_byte(object + OBJECT_STATE_FLAGS);
        let render_flags = self.memory.read_byte(object + OBJECT_RENDER_FLAGS);

        record.sort_z =
            if self.memory.read_byte(object + OBJECT_SORT_FLAGS) & OBJECT_SPECIAL_DEPTH_MASK != 0 {
                SPECIAL_SORT_DEPTH
            } else {
                0
            };
        record.x = self.memory.read_word(object + FIELD_X) as i16;
        record.y = self.memory.read_word(object + FIELD_Y) as i16;
        record.z = self.memory.read_word(object + FIELD_Z) as i16;
        record.shape = self.memory.read_word(object + FIELD_SHAPE);
        record.rotation_x = self.memory.read_byte(object + FIELD_ROT_X);
        record.rotation_y = self.memory.read_byte(object + FIELD_ROT_Y);
        record.rotation_z = self.memory.read_byte(object + FIELD_ROT_Z);
        record.shape_flags = render_flags;
        record.explosion_count = if state_flags & OBJECT_EXPLODING_MASK != 0 {
            self.memory.read_byte(object + OBJECT_EXPLOSION_COUNTER)
        } else {
            0
        };

        if render_flags & OBJECT_SHADOW_MASK != 0 {
            record.shadow_y = object as i16;
            record.shadow_x = i16::from_le_bytes([
                self.memory.read_byte(object + FIELD_ROT_X + 1),
                self.memory.read_byte(object + FIELD_ROT_Y + 1),
            ]);
            record.shadow_z = i16::from_le_bytes([
                self.memory.read_byte(object + FIELD_ROT_Z + 1),
                record.shadow_z.to_le_bytes()[1],
            ]);
        }

        record.animation_frame = Self::resolved_frame(
            self.memory
                .read_byte(object.wrapping_add(OBJECT_ANIMATION_FRAME)),
            frame,
        );
        record.color_frame = Self::resolved_frame(
            self.memory
                .read_byte(object.wrapping_add(OBJECT_COLOR_FRAME)),
            frame,
        );
        record.field_1e = self
            .memory
            .read_byte(object.wrapping_add(OBJECT_RENDER_EXTENSION));
        record.depth_offset = self
            .memory
            .read_byte(object.wrapping_add(OBJECT_DEPTH_OFFSET));
        record.texture_scroll_x = self
            .memory
            .read_byte(object.wrapping_add(OBJECT_TEXTURE_SCROLL_X));
        record.texture_scroll_y = self
            .memory
            .read_byte(object.wrapping_add(OBJECT_TEXTURE_SCROLL_Y));
        record.color_table = if object == self.memory.read_word(SPECIAL_COLOR_OBJECT_STATE) {
            if frame & SPECIAL_COLOR_PHASE_MASK < SPECIAL_COLOR_PHASE_SPLIT {
                SPECIAL_COLOR_SECOND
            } else {
                SPECIAL_COLOR_FIRST
            }
        } else {
            self.memory
                .read_word(object.wrapping_add(OBJECT_COLOR_TABLE))
        };
        record
    }

    fn resolved_frame(value: u8, global_frame: u8) -> u8 {
        if value & EXPLICIT_FRAME_MASK != 0 {
            value & FRAME_VALUE_MASK
        } else {
            global_frame & FRAME_VALUE_MASK
        }
    }

    /// The retail frame host mirrors the fixed view object at `$033F` into
    /// direct-page camera globals `$C7..$CF` before submitting the draw list.
    /// Player/mission strategies already update the view object; performing
    /// the host copy here keeps both later CPU reads and the PC renderer on
    /// the same camera state.
    fn sync_camera_from_anchor(&mut self) {
        const VIEW_OBJECT: u16 = 0x033F;
        for (source, destination) in [
            (VIEW_OBJECT + FIELD_X, 0x00C7u16),
            (VIEW_OBJECT + FIELD_Y, 0x00C9),
            (VIEW_OBJECT + FIELD_Z, 0x00CB),
        ] {
            let value = self.memory.read_word(source);
            self.memory.write_word(destination, value);
        }
        for (source, destination) in [
            (VIEW_OBJECT + FIELD_ROT_X, 0x00CDu16),
            (VIEW_OBJECT + FIELD_ROT_Y, 0x00CE),
            (VIEW_OBJECT + FIELD_ROT_Z, 0x00CF),
        ] {
            let value = self.memory.read_byte(source);
            self.memory.write_byte(destination, value);
        }
    }

    fn tick_map(&mut self) -> Result<(), Error> {
        let counter = self.map_vm.counter();
        if counter != 0 {
            self.map_vm.set_counter(counter - 1);
            self.sync_map_state();
            return Ok(());
        }

        let placeholder = MapVm::new(self.map_vm.cursor());
        let mut vm = std::mem::replace(&mut self.map_vm, placeholder);
        let result = vm.run(self, 512);
        self.map_vm = vm;
        let report = result.map_err(|error| Error::Map(format!("{error:?}")))?;
        if matches!(report.stop, MapRunStop::BudgetExhausted) {
            // A 512-instant-command run is not a normal retail yield.
            return Err(Error::Map(format!(
                "map budget exhausted at {:02X}:{:04X}",
                self.map_vm.cursor().bank,
                self.map_vm.cursor().address
            )));
        }
        self.sync_map_state();
        Ok(())
    }

    fn sync_map_state(&mut self) {
        self.memory.write_byte(0x192E, self.map_vm.cursor().bank);
        self.memory
            .write_word(0x1657, retail_map_pointer(self.map_vm.cursor()));
        self.memory.write_word(0x1655, self.map_vm.counter());
    }

    fn tick_paths(&mut self) -> Result<(), Error> {
        let objects = active_objects(&self.memory);
        for object in objects {
            let mut strategy = u32::from(self.memory.read_word(object + FIELD_STRATEGY))
                | (u32::from(self.memory.read_byte(object + FIELD_STRATEGY + 2)) << 16);

            // Retail path objects enter through one of these two initializer
            // thunks. `$7F:7E00` supplies the default combat bytes before
            // joining `$7F:7E1E`; the common body installs the actual path
            // dispatcher and initializes its exact flag fields.
            if strategy == 0x7F7E00 || strategy == 0x7F7E1E {
                if strategy == 0x7F7E00 {
                    self.memory.write_byte(object + 0x2D, 0x0A);
                    self.memory.write_byte(object + 0x2E, 0x0A);
                }
                self.memory.write_word(object + FIELD_STRATEGY, 0x7E53);
                self.memory.write_byte(object + FIELD_STRATEGY + 2, 0x7F);
                for (field, bits) in [(0x31u16, 0x10u8), (0x20, 0x08), (0x24, 0x04)] {
                    let value = self.memory.read_byte(object + field) | bits;
                    self.memory.write_byte(object + field, value);
                }
                let flags = self.memory.read_byte(object + 0x26) | 0x11;
                self.memory.write_byte(object + 0x26, flags);
                self.memory.write_byte(object + 0x28, 0);
                self.memory.write_word(0xB26D, 0);
                strategy = 0x7F7E53;
            }

            // Fields such as the player slot at +$2B can look like valid path
            // offsets.  The retail engine dispatches bytecode only through
            // `$7F:7E53`, so gate on the strategy instead of interpreting any
            // nonzero +$2B value as code.
            if strategy != 0x7F7E53 {
                continue;
            }

            let cursor = self.memory.read_word(object + FIELD_PATH);
            if cursor == 0 {
                continue;
            }
            if path_command_at(sf2_data::path::PathAddress { offset: cursor }).is_none() {
                // Native 65816 strategies install a second family of path
                // roots which the original map-only extractor did not seed.
                // Run the retail dispatcher against the shared WRAM until
                // those roots are regenerated into the clean Rust catalog;
                // silently parking such an object leaks it forever.
                self.run_unported_strategy(0x7F7E53, object)?;
                continue;
            }
            self.memory.write_word(CURRENT_OBJECT, object);
            let selected = if self.memory.read_byte(object + 0x24) & 0x80 != 0 {
                self.memory.read_word(0x12C5)
            } else {
                self.memory.read_word(0x12C3)
            };
            self.memory.write_word(SELECTED_OBJECT, selected);
            let mut vm = PathVm::new(sf2_data::path::PathAddress { offset: cursor });
            // SF2 initialization paths legitimately execute long runs of
            // immediate table/index operations before their first retail
            // yield.  The closed recovered graph contains 7,210 commands, so
            // one full-graph budget still catches a non-yielding cycle while
            // permitting every acyclic initialization run.
            let report = vm
                .run(self, sf2_data::path::PATH_COMMAND_COUNT)
                .map_err(|error| {
                    Error::Path(format!(
                        "object {object:04X}, start {cursor:04X}, cursor {:04X}: {error:?}",
                        vm.cursor().offset
                    ))
                })?;
            let final_object = self.memory.read_word(CURRENT_OBJECT);
            let path_owner = object_index(final_object)
                .map(|_| final_object)
                .unwrap_or(object);
            self.memory
                .write_word(path_owner + FIELD_PATH, vm.cursor().offset);
            if matches!(report.stop, sf2_path::RunStop::BudgetExhausted) {
                return Err(Error::Path(format!(
                    "path budget exhausted at {:04X} for object {:04X}",
                    vm.cursor().offset,
                    object
                )));
            }
        }
        self.reap_finished_path_objects();
        Ok(())
    }

    /// The common object dispatcher removes records whose path set flag
    /// `$25.3` (the exact flag written by END and REMOVECHILD).  Reaping only
    /// after the frame's stable active-list walk mirrors the retail loop and
    /// prevents newly inserted records from being visited twice.
    fn reap_finished_path_objects(&mut self) {
        let finished: Vec<u16> = active_objects(&self.memory)
            .into_iter()
            .filter(|object| self.memory.read_byte(*object + 0x25) & 0x08 != 0)
            .collect();
        for object in finished {
            // Detach a child from the mother's `$29` sibling chain before its
            // object record is returned to the free list.
            if self.memory.read_byte(object + 0x23) & 0x04 != 0 {
                let mother = self.memory.read_word(object + 0x06);
                if object_index(mother).is_some() {
                    let mut predecessor = mother;
                    while object_index(self.memory.read_word(predecessor + 0x29)).is_some() {
                        let candidate = self.memory.read_word(predecessor + 0x29);
                        if candidate == object {
                            let sibling = self.memory.read_word(object + 0x29);
                            self.memory.write_word(predecessor + 0x29, sibling);
                            if self.memory.read_word(mother + 0x29) == 0 {
                                let flags = self.memory.read_byte(mother + 0x23) & !0x10;
                                self.memory.write_byte(mother + 0x23, flags);
                            }
                            break;
                        }
                        predecessor = candidate;
                    }
                }
            }
            free_all_auxiliary(&mut self.memory, object);
            free(&mut self.memory, object);
        }
    }

    pub fn active_objects(&self) -> Vec<u16> {
        active_objects(&self.memory)
    }

    pub fn camera(&self) -> Camera {
        Camera {
            x: self.memory.read_word(0x00C7) as i16,
            y: self.memory.read_word(0x00C9) as i16,
            z: self.memory.read_word(0x00CB) as i16,
            rotation_x: self.memory.read_byte(0x00CD),
            rotation_y: self.memory.read_byte(0x00CE),
            rotation_z: self.memory.read_byte(0x00CF),
        }
    }

    pub fn draw_records(&self) -> Vec<(u16, DrawRecord)> {
        active_objects(&self.memory)
            .into_iter()
            .filter_map(|object| self.draw_record(object).map(|record| (object, record)))
            .collect()
    }

    /// Return the native render list with stable object identities.
    pub fn render_records(&self) -> Result<Vec<(u16, DrawRecord)>, Error> {
        Ok(self.render_records.clone())
    }

    fn draw_record(&self, object: u16) -> Option<DrawRecord> {
        let shape = self.memory.read_word(object + FIELD_SHAPE);
        if shape == 0 {
            return None;
        }
        let flags = self.memory.read_byte(object + 0x20);
        if flags & 0x08 == 0 {
            return None;
        }
        let animation = self.memory.read_byte(object.wrapping_add(0x1CCB));
        let color = self.memory.read_byte(object.wrapping_add(0x1CCA));
        let global_frame = self.memory.read_byte(0x00C4) & 0x7F;
        Some(DrawRecord {
            next: 0,
            sort_z: 0,
            rotation_x: self.memory.read_byte(object + FIELD_ROT_X),
            rotation_y: self.memory.read_byte(object + FIELD_ROT_Y),
            rotation_z: self.memory.read_byte(object + FIELD_ROT_Z),
            shape_flags: flags,
            shape,
            shadow_y: 0,
            shadow_x: 0,
            shadow_z: 0,
            projected_y: 0,
            projected_x: 0,
            projected_z: 0,
            color_table: self.memory.read_word(object.wrapping_add(0x1CCD)),
            explosion_count: if self.memory.read_byte(object + 0x08) & 1 != 0 {
                self.memory.read_byte(object + 0x0A)
            } else {
                0
            },
            animation_frame: Self::resolved_frame(animation, global_frame),
            color_frame: Self::resolved_frame(color, global_frame),
            depth_offset: self.memory.read_byte(object.wrapping_add(0x1CC8)),
            texture_scroll_x: self.memory.read_byte(object.wrapping_add(0x1CDA)),
            texture_scroll_y: self.memory.read_byte(object.wrapping_add(0x1CDB)),
            field_1e: self.memory.read_byte(object.wrapping_add(0x1CEF)),
            reserved_1f: 0,
            x: self.memory.read_word(object + FIELD_X) as i16,
            y: self.memory.read_word(object + FIELD_Y) as i16,
            z: self.memory.read_word(object + FIELD_Z) as i16,
        })
    }

    pub(crate) fn random_byte(&mut self) -> u8 {
        #[inline]
        fn sbc(accumulator: u8, operand: u8, carry: bool) -> (u8, bool) {
            let borrow = u16::from(!carry);
            let subtrahend = u16::from(operand) + borrow;
            (
                accumulator.wrapping_sub(operand).wrapping_sub(borrow as u8),
                u16::from(accumulator) >= subtrahend,
            )
        }

        // Retail `random_l` (`$7F:7BD0`) is a four-byte subtract-with-borrow
        // generator in direct-page bytes `$E0..$E3`; CLC deliberately starts
        // the chain with a borrow.
        let original_e0 = self.memory.read_byte(0x00E0);
        let (mut value, mut carry) = sbc(original_e0, self.memory.read_byte(0x00E1), false);
        self.memory.write_byte(0x00E1, value);
        (value, carry) = sbc(value, self.memory.read_byte(0x00E2), carry);
        self.memory.write_byte(0x00E2, value);
        (value, carry) = sbc(value, self.memory.read_byte(0x00E3), carry);
        self.memory.write_byte(0x00E3, value);
        (value, _) = sbc(value, original_e0, carry);
        self.memory.write_byte(0x00E0, value);
        value
    }
}

/// Convert an extracted source-script address into its catalog-relative
/// stream offset. This conversion is data import, not runtime addressing.
#[inline]
fn retail_map_pointer(address: MapAddress) -> u16 {
    address.address.wrapping_sub(0x8000)
}
