use sf2_data::path::PathAddress;
use sf2_path::*;

struct Host {
    vars: [u8; 256],
    external: Vec<u8>,
    object_extension: Vec<u8>,
    long_external: std::collections::BTreeMap<u32, u8>,
    stack: Vec<u16>,
    found_shape: Option<u16>,
    shape_dead: bool,
    child_dead: bool,
    flagged_children: Vec<u8>,
    fire_weapon_calls: usize,
    face_player_calls: usize,
    face_player_yaw_calls: usize,
    face_mother_calls: usize,
    copy_selected_position_calls: usize,
    hold_calls: usize,
    selected_distance: u16,
    mother_distance: Option<u16>,
    selected_within_range: bool,
    selected_relative_yaw: u8,
    selected_bearing_plus_yaw: u8,
    yaw_rotations: Vec<i8>,
    pitch_rotations: Vec<i8>,
    optional_context_available: bool,
    selected_slot_class: u8,
    selected_aux_flags: u8,
    selected_slot_low_nibble_4_calls: usize,
    allocated_auxiliary_type_0b: Vec<u8>,
    allocated_auxiliary_type_0d: Vec<u8>,
    selected_auxiliary_progress_steps: Vec<u8>,
    selected_auxiliary_progress_settled: bool,
    path_operations: Vec<Sf2PathOperation>,
    evaluated_conditions: Vec<Sf2PathCondition>,
    condition_result: bool,
    path_contact: Option<PathContactClass>,
    refresh_collision_target_calls: usize,
    canceled_triggers: Vec<PathAddress>,
    forced_trigger_paths: Vec<PathAddress>,
    player_target_updates: Vec<PlayerTargetUpdate>,
    selected_markers: Vec<(u8, SelectedMarkerClass)>,
    spawn_linked_object_effects_calls: usize,
    random_bytes: Vec<u8>,
    queue: Option<u8>,
    trail: Option<u8>,
    velocity_regenerations: usize,
    sprite: Option<(u8, u8)>,
    quick_spawns: Vec<(u16, PathAddress, u8, u8)>,
    child_spawns: Vec<ChildSpawn>,
    removed_children: Vec<u8>,
    messages: Vec<u8>,
    triggers: Vec<PathTrigger>,
    transitions: Vec<(ContextTransition, PathAddress)>,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            vars: [0; 256],
            external: Vec::new(),
            object_extension: Vec::new(),
            long_external: std::collections::BTreeMap::new(),
            stack: Vec::new(),
            found_shape: None,
            shape_dead: false,
            child_dead: false,
            flagged_children: Vec::new(),
            fire_weapon_calls: 0,
            face_player_calls: 0,
            face_player_yaw_calls: 0,
            face_mother_calls: 0,
            copy_selected_position_calls: 0,
            hold_calls: 0,
            selected_distance: 0,
            mother_distance: None,
            selected_within_range: false,
            selected_relative_yaw: 0,
            selected_bearing_plus_yaw: 0,
            yaw_rotations: Vec::new(),
            pitch_rotations: Vec::new(),
            optional_context_available: false,
            selected_slot_class: 0,
            selected_aux_flags: 0,
            selected_slot_low_nibble_4_calls: 0,
            allocated_auxiliary_type_0b: Vec::new(),
            allocated_auxiliary_type_0d: Vec::new(),
            selected_auxiliary_progress_steps: Vec::new(),
            selected_auxiliary_progress_settled: false,
            path_operations: Vec::new(),
            evaluated_conditions: Vec::new(),
            condition_result: false,
            path_contact: None,
            refresh_collision_target_calls: 0,
            canceled_triggers: Vec::new(),
            forced_trigger_paths: Vec::new(),
            player_target_updates: Vec::new(),
            selected_markers: Vec::new(),
            spawn_linked_object_effects_calls: 0,
            random_bytes: Vec::new(),
            queue: None,
            trail: None,
            velocity_regenerations: 0,
            sprite: None,
            quick_spawns: Vec::new(),
            child_spawns: Vec::new(),
            removed_children: Vec::new(),
            messages: Vec::new(),
            triggers: Vec::new(),
            transitions: Vec::new(),
        }
    }
}

impl Host {
    fn new() -> Self {
        Self {
            external: vec![0; 65536],
            object_extension: vec![0; 65536],
            ..Self::default()
        }
    }
}

impl Sf2PathHost for Host {
    type Error = ();

    fn read_variable_byte(&self, id: u8) -> Result<u8, Self::Error> {
        Ok(self.vars[id as usize])
    }

    fn write_variable_byte(&mut self, id: u8, value: u8) -> Result<(), Self::Error> {
        self.vars[id as usize] = value;
        Ok(())
    }

    fn read_variable_word(&self, id: u8) -> Result<u16, Self::Error> {
        let index = id as usize;
        Ok(u16::from_le_bytes([self.vars[index], self.vars[index + 1]]))
    }

    fn write_variable_word(&mut self, id: u8, value: u16) -> Result<(), Self::Error> {
        let index = id as usize;
        let bytes = value.to_le_bytes();
        self.vars[index] = bytes[0];
        self.vars[index + 1] = bytes[1];
        Ok(())
    }

    fn read_external_byte(&self, address: u16) -> Result<u8, Self::Error> {
        Ok(self.external[address as usize])
    }

    fn write_external_byte(&mut self, address: u16, value: u8) -> Result<(), Self::Error> {
        self.external[address as usize] = value;
        Ok(())
    }

    fn read_external_word(&self, address: u16) -> Result<u16, Self::Error> {
        let index = address as usize;
        Ok(u16::from_le_bytes([
            self.external[index],
            self.external[index + 1],
        ]))
    }

    fn write_external_word(&mut self, address: u16, value: u16) -> Result<(), Self::Error> {
        let index = address as usize;
        let bytes = value.to_le_bytes();
        self.external[index] = bytes[0];
        self.external[index + 1] = bytes[1];
        Ok(())
    }

    fn read_external_long_byte(&self, address: u32) -> Result<u8, Self::Error> {
        Ok(self.long_external.get(&address).copied().unwrap_or(0))
    }

    fn read_external_long_word(&self, address: u32) -> Result<u16, Self::Error> {
        Ok(u16::from_le_bytes([
            self.long_external.get(&address).copied().unwrap_or(0),
            self.long_external
                .get(&address.wrapping_add(1))
                .copied()
                .unwrap_or(0),
        ]))
    }

    fn read_object_extension_byte(&self, offset: u16) -> Result<u8, Self::Error> {
        Ok(self.object_extension[offset as usize])
    }

    fn write_object_extension_byte(&mut self, offset: u16, value: u8) -> Result<(), Self::Error> {
        self.object_extension[offset as usize] = value;
        Ok(())
    }

    fn read_object_extension_word(&self, offset: u16) -> Result<u16, Self::Error> {
        let index = offset as usize;
        Ok(u16::from_le_bytes([
            self.object_extension[index],
            self.object_extension[index + 1],
        ]))
    }

    fn write_object_extension_word(&mut self, offset: u16, value: u16) -> Result<(), Self::Error> {
        let index = offset as usize;
        let bytes = value.to_le_bytes();
        self.object_extension[index] = bytes[0];
        self.object_extension[index + 1] = bytes[1];
        Ok(())
    }

    fn find_shape(&mut self, shape: u16) -> Result<(), Self::Error> {
        self.found_shape = Some(shape);
        Ok(())
    }

    fn pointed_shape_is_dead(&self) -> Result<bool, Self::Error> {
        Ok(self.shape_dead)
    }

    fn child_is_dead(&mut self, _child_number: u8) -> Result<bool, Self::Error> {
        Ok(self.child_dead)
    }

    fn flag_child(&mut self, child_number: u8) -> Result<(), Self::Error> {
        self.flagged_children.push(child_number);
        Ok(())
    }

    fn fire_weapon(&mut self) -> Result<(), Self::Error> {
        self.fire_weapon_calls += 1;
        Ok(())
    }

    fn face_player(&mut self) -> Result<(), Self::Error> {
        self.face_player_calls += 1;
        Ok(())
    }

    fn face_player_yaw(&mut self) -> Result<(), Self::Error> {
        self.face_player_yaw_calls += 1;
        Ok(())
    }

    fn face_mother(&mut self) -> Result<(), Self::Error> {
        self.face_mother_calls += 1;
        Ok(())
    }

    fn copy_selected_world_position(&mut self) -> Result<(), Self::Error> {
        self.copy_selected_position_calls += 1;
        Ok(())
    }

    fn enter_path_hold(&mut self) -> Result<(), Self::Error> {
        self.hold_calls += 1;
        Ok(())
    }

    fn selected_distance(&mut self) -> Result<u16, Self::Error> {
        Ok(self.selected_distance)
    }

    fn mother_distance(&mut self) -> Result<Option<u16>, Self::Error> {
        Ok(self.mother_distance)
    }

    fn selected_within_range(&mut self, _range: u16) -> Result<bool, Self::Error> {
        Ok(self.selected_within_range)
    }

    fn selected_relative_yaw(&mut self) -> Result<u8, Self::Error> {
        Ok(self.selected_relative_yaw)
    }

    fn selected_bearing_plus_yaw(&mut self) -> Result<u8, Self::Error> {
        Ok(self.selected_bearing_plus_yaw)
    }

    fn rotate_around_selected_yaw(&mut self, angle: i8) -> Result<(), Self::Error> {
        self.yaw_rotations.push(angle);
        Ok(())
    }

    fn rotate_around_selected_pitch(&mut self, angle: i8) -> Result<(), Self::Error> {
        self.pitch_rotations.push(angle);
        Ok(())
    }

    fn try_transition_context(
        &mut self,
        transition: ContextTransition,
        resume_at: PathAddress,
    ) -> Result<bool, Self::Error> {
        if self.optional_context_available {
            self.transitions.push((transition, resume_at));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn selected_slot_class(&self) -> Result<u8, Self::Error> {
        Ok(self.selected_slot_class)
    }

    fn selected_aux_flags(&self) -> Result<u8, Self::Error> {
        Ok(self.selected_aux_flags)
    }

    fn or_selected_aux_flags(&mut self, bits: u8) -> Result<(), Self::Error> {
        self.selected_aux_flags |= bits;
        Ok(())
    }

    fn set_selected_slot_low_nibble_4(&mut self) -> Result<(), Self::Error> {
        self.selected_slot_low_nibble_4_calls += 1;
        Ok(())
    }

    fn allocate_auxiliary_type_0b(&mut self, value: u8) -> Result<(), Self::Error> {
        self.allocated_auxiliary_type_0b.push(value);
        Ok(())
    }

    fn allocate_auxiliary_type_0d(&mut self, value: u8) -> Result<(), Self::Error> {
        self.allocated_auxiliary_type_0d.push(value);
        Ok(())
    }

    fn advance_selected_auxiliary_progress(&mut self, step: u8) -> Result<bool, Self::Error> {
        self.selected_auxiliary_progress_steps.push(step);
        Ok(self.selected_auxiliary_progress_settled)
    }

    fn perform_path_operation(&mut self, operation: Sf2PathOperation) -> Result<(), Self::Error> {
        self.path_operations.push(operation);
        Ok(())
    }

    fn evaluate_path_condition(
        &mut self,
        condition: Sf2PathCondition,
    ) -> Result<bool, Self::Error> {
        self.evaluated_conditions.push(condition);
        Ok(self.condition_result)
    }

    fn classify_path_contact(&mut self) -> Result<Option<PathContactClass>, Self::Error> {
        Ok(self.path_contact)
    }

    fn refresh_collision_target(&mut self) -> Result<(), Self::Error> {
        self.refresh_collision_target_calls += 1;
        Ok(())
    }

    fn cancel_trigger(&mut self, path: PathAddress) -> Result<(), Self::Error> {
        self.canceled_triggers.push(path);
        Ok(())
    }

    fn force_trigger_path(&mut self, path: PathAddress) -> Result<(), Self::Error> {
        self.forced_trigger_paths.push(path);
        Ok(())
    }

    fn update_player_target(&mut self, update: PlayerTargetUpdate) -> Result<(), Self::Error> {
        self.player_target_updates.push(update);
        Ok(())
    }

    fn queue_selected_marker(
        &mut self,
        value: u8,
        class: SelectedMarkerClass,
    ) -> Result<(), Self::Error> {
        self.selected_markers.push((value, class));
        Ok(())
    }

    fn spawn_linked_object_effects(&mut self) -> Result<(), Self::Error> {
        self.spawn_linked_object_effects_calls += 1;
        Ok(())
    }

    fn random_byte(&mut self) -> Result<u8, Self::Error> {
        Ok(if self.random_bytes.is_empty() {
            0
        } else {
            self.random_bytes.remove(0)
        })
    }

    fn do_queue(&mut self, queue: u8) -> Result<(), Self::Error> {
        self.queue = Some(queue);
        Ok(())
    }

    fn set_trail(&mut self, trail: u8) -> Result<(), Self::Error> {
        self.trail = Some(trail);
        Ok(())
    }

    fn regenerate_velocity_vectors(&mut self) -> Result<(), Self::Error> {
        self.velocity_regenerations += 1;
        Ok(())
    }

    fn set_sprite(&mut self, x: u8, y: u8) -> Result<(), Self::Error> {
        self.sprite = Some((x, y));
        Ok(())
    }

    fn quick_spawn(
        &mut self,
        shape: u16,
        path: PathAddress,
        hit_points: u8,
        attack_points: u8,
    ) -> Result<(), Self::Error> {
        self.quick_spawns
            .push((shape, path, hit_points, attack_points));
        Ok(())
    }

    fn spawn_child(&mut self, spawn: ChildSpawn) -> Result<(), Self::Error> {
        self.child_spawns.push(spawn);
        Ok(())
    }

    fn remove_child(&mut self, child_number: u8) -> Result<(), Self::Error> {
        self.removed_children.push(child_number);
        Ok(())
    }

    fn start_message(&mut self, message: u8) -> Result<(), Self::Error> {
        self.messages.push(message);
        Ok(())
    }

    fn schedule_trigger(&mut self, trigger: PathTrigger) -> Result<(), Self::Error> {
        self.triggers.push(trigger);
        Ok(())
    }

    fn push_path_value(&mut self, value: u16) -> Result<(), Self::Error> {
        self.stack.push(value);
        Ok(())
    }

    fn pop_path_value(&mut self) -> Result<Option<u16>, Self::Error> {
        Ok(self.stack.pop())
    }

    fn transition_context(
        &mut self,
        transition: ContextTransition,
        resume_at: PathAddress,
    ) -> Result<(), Self::Error> {
        self.transitions.push((transition, resume_at));
        Ok(())
    }
}

fn run_one(vm: &mut PathVm, host: &mut Host) -> RunReport {
    vm.run(host, 1).unwrap()
}

fn first_command(opcode: u16) -> &'static sf2_data::path::PathCommand {
    sf2_data::path::PATH_COMMANDS
        .iter()
        .find(|command| command.opcode == opcode)
        .unwrap_or_else(|| panic!("missing reachable opcode ${opcode:03X}"))
}

fn run_opcode(opcode: u16, host: &mut Host) -> (PathVm, RunReport) {
    let mut vm = PathVm::new(first_command(opcode).address);
    let report = run_one(&mut vm, host);
    (vm, report)
}

#[test]
fn wait_keeps_the_retail_object_counter() {
    let mut host = Host::new();
    let mut wait = PathVm::new(PathAddress { offset: 0x04C2 });
    for expected in 1..=16 {
        assert_eq!(
            run_one(&mut wait, &mut host).stop,
            RunStop::Yielded(YieldReason::Wait)
        );
        assert_eq!(host.vars[VAR_WAIT_COUNTER as usize], expected);
        assert_eq!(wait.cursor().offset, 0x04C2);
    }
    assert_eq!(run_one(&mut wait, &mut host).stop, RunStop::BudgetExhausted);
    assert_eq!(host.vars[VAR_WAIT_COUNTER as usize], 0);
    assert_eq!(wait.cursor().offset, 0x04C4);
}

#[test]
fn variable_and_external_memory_handlers_use_exact_operands() {
    let mut host = Host::new();
    host.vars[0x2D] = 0xA5;
    let mut vm = PathVm::new(PathAddress { offset: 0x3FC1 });
    run_one(&mut vm, &mut host);
    assert_eq!(host.vars[0x27], 0xA5);
    assert_eq!(vm.cursor().offset, 0x3FC4);

    host.external[0x1BB5] = 0x6D;
    let mut vm = PathVm::new(PathAddress { offset: 0x7442 });
    run_one(&mut vm, &mut host);
    assert_eq!(host.vars[0xA1], 0x6D);

    host.external[(INDEXED_VARIABLE_TABLE + 0x96) as usize] = 0x91;
    let mut vm = PathVm::new(PathAddress { offset: 0x04B9 });
    run_one(&mut vm, &mut host);
    assert_eq!(host.vars[0xA1], 0x91);

    host.vars[0x99] = 0x37;
    let mut vm = PathVm::new(PathAddress { offset: 0x7493 });
    run_one(&mut vm, &mut host);
    assert_eq!(host.external[0x1BBB], 0x37);

    host.vars[0xA3] = 0x34;
    host.vars[0xA4] = 0x12;
    let mut vm = PathVm::new(PathAddress { offset: 0x04D4 });
    run_one(&mut vm, &mut host);
    let table = (INDEXED_VARIABLE_TABLE + 0x36) as usize;
    assert_eq!(&host.external[table..table + 2], &[0x34, 0x12]);
}

#[test]
fn branches_yields_and_inversion_match_handler_flow() {
    let mut host = Host::new();
    let mut goto = PathVm::new(PathAddress { offset: 0x051C });
    assert_eq!(
        run_one(&mut goto, &mut host).stop,
        RunStop::Yielded(YieldReason::Goto)
    );
    assert_eq!(goto.cursor().offset, 0x0512);

    let mut inverse = PathVm::new(PathAddress { offset: 0x04BC });
    run_one(&mut inverse, &mut host);
    assert!(inverse.invert_pending());
    host.shape_dead = true;
    inverse = PathVm::new(PathAddress { offset: 0x04BC });
    run_one(&mut inverse, &mut host);
    assert!(inverse.invert_pending());
    // Preserve the pending IFNOT while selecting the reviewed condition.
    inverse.set_cursor(PathAddress { offset: 0x4C78 });
    run_one(&mut inverse, &mut host);
    assert_eq!(inverse.cursor().offset, 0x4C7B);
    assert!(!inverse.invert_pending());
}

#[test]
fn every_reachable_handler_is_proof_gated_and_executable() {
    assert!(sf2_data::path::PATH_HANDLERS
        .iter()
        .all(|handler| handler.semantic.is_some()));
}

#[test]
fn literal_arithmetic_and_mutation_match_retail_operands() {
    let mut host = Host::new();

    let mut set_byte = PathVm::new(PathAddress { offset: 0x7458 });
    run_one(&mut set_byte, &mut host);
    assert_eq!(host.vars[0x14], 0x40);
    assert_eq!(set_byte.cursor().offset, 0x745B);

    let mut set_word = PathVm::new(PathAddress { offset: 0x745B });
    run_one(&mut set_word, &mut host);
    assert_eq!(host.read_variable_word(0x0E).unwrap(), 0x0032);
    assert_eq!(set_word.cursor().offset, 0x745F);

    host.vars[0xA2] = 0xFC;
    let mut add_byte = PathVm::new(PathAddress { offset: 0x766E });
    run_one(&mut add_byte, &mut host);
    assert_eq!(host.vars[0xA2], 3);

    host.write_variable_word(0xA3, 0xFFFE).unwrap();
    let add_word_address = sf2_data::path::PATH_COMMANDS
        .iter()
        .find(|command| command.opcode == 0x008)
        .unwrap()
        .address;
    let add_word = sf2_path::command_at(add_word_address).unwrap();
    let literal = u16::from_le_bytes([add_word.raw[2], add_word.raw[3]]);
    let variable = add_word.raw[1];
    host.write_variable_word(variable, 0xFFFE).unwrap();
    let mut add_word_vm = PathVm::new(add_word_address);
    run_one(&mut add_word_vm, &mut host);
    assert_eq!(
        host.read_variable_word(variable).unwrap(),
        0xFFFEu16.wrapping_add(literal)
    );
}

#[test]
fn comparisons_and_path_call_stack_follow_retail_control_flow() {
    let mut host = Host::new();

    host.vars[0xA1] = 3;
    let mut same = PathVm::new(PathAddress { offset: 0x7485 });
    run_one(&mut same, &mut host);
    assert_eq!(same.cursor().offset, 0x7497);

    host.vars[0xA1] = 4;
    same = PathVm::new(PathAddress { offset: 0x7485 });
    run_one(&mut same, &mut host);
    assert_eq!(same.cursor().offset, 0x748A);

    host.vars[0xA1] = 0;
    let mut between = PathVm::new(PathAddress { offset: 0x744A });
    run_one(&mut between, &mut host);
    assert_eq!(between.cursor().offset, 0x7484);
    host.vars[0xA1] = 0xFF;
    between = PathVm::new(PathAddress { offset: 0x744A });
    run_one(&mut between, &mut host);
    assert_eq!(between.cursor().offset, 0x7450, "lower bound is exclusive");

    let mut gosub = PathVm::new(PathAddress { offset: 0x7450 });
    run_one(&mut gosub, &mut host);
    assert_eq!(gosub.cursor().offset, 0x7458);
    assert_eq!(host.stack, vec![0x7450]);
    gosub.set_cursor(PathAddress { offset: 0x7483 });
    run_one(&mut gosub, &mut host);
    assert_eq!(gosub.cursor().offset, 0x7453);
    assert!(host.stack.is_empty());

    gosub.set_cursor(PathAddress { offset: 0x7483 });
    assert_eq!(
        run_one(&mut gosub, &mut host).stop,
        RunStop::Yielded(YieldReason::Return)
    );
}

#[test]
fn immediate_jump_variable_adds_and_indexed_byte_export_match_retail() {
    let mut host = Host::new();

    let mut immediate = PathVm::new(PathAddress { offset: 0x04FF });
    assert_eq!(
        run_one(&mut immediate, &mut host).stop,
        RunStop::BudgetExhausted
    );
    assert_eq!(immediate.cursor().offset, 0x050E);

    host.vars[0x9A] = 0x91;
    let mut add_byte = PathVm::new(PathAddress { offset: 0x4F50 });
    run_one(&mut add_byte, &mut host);
    assert_eq!(host.vars[0x9A], 0x22);

    host.write_variable_word(0x0C, 5).unwrap();
    host.vars[0xA9] = 0xFE;
    let mut add_signed_byte = PathVm::new(PathAddress { offset: 0x4F65 });
    run_one(&mut add_signed_byte, &mut host);
    assert_eq!(host.read_variable_word(0x0C).unwrap(), 3);

    host.vars[0xA2] = 0xCC;
    let mut zero = PathVm::new(PathAddress { offset: 0x4F2F });
    run_one(&mut zero, &mut host);
    assert_eq!(host.vars[0xA2], 0);

    host.vars[0x27] = 0x5A;
    let mut export = PathVm::new(PathAddress { offset: 0x4B74 });
    run_one(&mut export, &mut host);
    assert_eq!(host.external[(INDEXED_VARIABLE_TABLE + 8) as usize], 0x5A);

    host.vars[0x14] = 0xFF;
    let mut rotate = PathVm::new(PathAddress { offset: 0x7453 });
    run_one(&mut rotate, &mut host);
    assert_eq!(host.vars[0x14], 1);
}

#[test]
fn movement_flags_sprite_spawn_and_do_variable_use_exact_records() {
    let mut host = Host::new();

    let mut velocity = PathVm::new(PathAddress { offset: 0x4F97 });
    run_one(&mut velocity, &mut host);
    assert_eq!(host.vars[VAR_VELOCITY as usize], 0x1E);
    assert_eq!(host.velocity_regenerations, 1);
    host.vars[VAR_PATH_FLAGS as usize] = PATH_FLAG_RELATIVE_TO_PLAYER;
    velocity = PathVm::new(PathAddress { offset: 0x4F9B });
    run_one(&mut velocity, &mut host);
    assert_eq!(host.vars[VAR_VELOCITY as usize], 0);
    assert_eq!(host.velocity_regenerations, 1);

    let mut helicopter = PathVm::new(PathAddress { offset: 0x4AAF });
    run_one(&mut helicopter, &mut host);
    assert_ne!(host.vars[VAR_PATH_FLAGS as usize] & PATH_FLAG_HELICOPTER, 0);

    let mut invisible = PathVm::new(PathAddress { offset: 0x04B8 });
    run_one(&mut invisible, &mut host);
    assert_ne!(
        host.vars[VAR_OBJECT_FLAGS_23 as usize] & OBJECT_FLAG_INVISIBLE,
        0
    );
    assert_ne!(host.vars[VAR_PATH_FLAGS as usize] & PATH_FLAG_INVISIBLE, 0);
    invisible = PathVm::new(PathAddress { offset: 0x74B5 });
    run_one(&mut invisible, &mut host);
    assert_eq!(
        host.vars[VAR_OBJECT_FLAGS_23 as usize] & OBJECT_FLAG_INVISIBLE,
        0
    );
    assert_eq!(host.vars[VAR_PATH_FLAGS as usize] & PATH_FLAG_INVISIBLE, 0);

    let mut sprite = PathVm::new(PathAddress { offset: 0xA525 });
    run_one(&mut sprite, &mut host);
    assert_eq!(host.sprite, Some((0, 0)));

    let mut spawn = PathVm::new(PathAddress { offset: 0xA518 });
    run_one(&mut spawn, &mut host);
    assert_eq!(
        host.quick_spawns,
        vec![(0xC0C4, PathAddress { offset: 0xA524 }, 10, 10)]
    );

    host.vars[0xA2] = 0x9D;
    let mut do_variable = PathVm::new(PathAddress { offset: 0x762A });
    run_one(&mut do_variable, &mut host);
    assert_eq!(host.stack, vec![0x762C, 0x009D]);
}

#[test]
fn low_level_flag_handlers_and_end_match_complete_retail_writes() {
    let mut host = Host::new();
    let cases = [
        (0x8653, VAR_OBJECT_FLAGS_22, 0x08),
        (0x7886, VAR_PATH_FLAGS, 0x01),
        (0x0511, VAR_OBJECT_FLAGS_26, 0x08),
        (0x74B9, VAR_OBJECT_FLAGS_26, 0x10),
        (0x58C4, VAR_OBJECT_FLAGS_24, 0x08),
    ];
    for (address, variable, mask) in cases {
        host.vars[variable as usize] = 0;
        let mut vm = PathVm::new(PathAddress { offset: address });
        run_one(&mut vm, &mut host);
        assert_eq!(
            host.vars[variable as usize] & mask,
            mask,
            "at ${address:04X}"
        );
    }

    host.vars[VAR_OBJECT_FLAGS_20 as usize] = 0xFF;
    let mut clear20 = PathVm::new(PathAddress { offset: 0x4F2B });
    run_one(&mut clear20, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_20 as usize], 0xF7);

    host.vars[VAR_OBJECT_FLAGS_22 as usize] = 0xFF;
    let mut clear22 = PathVm::new(PathAddress { offset: 0x4053 });
    run_one(&mut clear22, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_22 as usize], 0xF7);

    host.vars[VAR_OBJECT_FLAGS_26 as usize] = 0xFF;
    let mut clear26 = PathVm::new(PathAddress { offset: 0x865E });
    run_one(&mut clear26, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_26 as usize], 0xEF);

    for variable in [
        VAR_OBJECT_FLAGS_25,
        VAR_OBJECT_FLAGS_22,
        VAR_PATH_FLAGS,
        VAR_OBJECT_FLAGS_26,
    ] {
        host.vars[variable as usize] = 0xFF;
    }
    let mut end = PathVm::new(PathAddress { offset: 0x04FE });
    assert_eq!(
        run_one(&mut end, &mut host).stop,
        RunStop::Yielded(YieldReason::End)
    );
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_25 as usize], 0xFF);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_22 as usize], 0xFD);
    assert_eq!(host.vars[VAR_PATH_FLAGS as usize], 0x7F);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_26 as usize], 0xF9);
}

#[test]
fn variable_bit_table_mutation_and_branch_match_retail() {
    let mut host = Host::new();
    host.vars[0x99] = 3;
    host.write_variable_word(0xA3, 0x1000).unwrap();
    let mut set = PathVm::new(PathAddress { offset: 0x74A8 });
    run_one(&mut set, &mut host);
    assert_eq!(host.read_variable_word(0xA3).unwrap(), 0x1004);

    let mut branch = PathVm::new(PathAddress { offset: 0x74A3 });
    run_one(&mut branch, &mut host);
    assert_eq!(branch.cursor().offset, 0x74B5);
    host.write_variable_word(0xA3, 0).unwrap();
    branch = PathVm::new(PathAddress { offset: 0x74A3 });
    run_one(&mut branch, &mut host);
    assert_eq!(branch.cursor().offset, 0x74A8);

    host.vars[0x99] = 0;
    set = PathVm::new(PathAddress { offset: 0x74A8 });
    assert_eq!(
        set.run(&mut host, 1),
        Err(PathVmError::InvalidBitIndex {
            address: PathAddress { offset: 0x74A8 },
            index: 0,
        })
    );
}

#[test]
fn child_mother_and_auxiliary_service_handlers_match_retail_records() {
    let mut host = Host::new();

    let mut flag_child = PathVm::new(PathAddress { offset: 0x5955 });
    run_one(&mut flag_child, &mut host);
    assert_eq!(host.flagged_children, vec![10]);
    assert_eq!(flag_child.cursor().offset, 0x5957);

    // `$0D9 $A2 $A1` reads the one-based bit index from variable `$A2`
    // and clears that bit in the word selected by `$A1`.
    host.vars[0xA1] = 0xFF;
    host.vars[0xA2] = 3;
    let mut clear_bit = PathVm::new(PathAddress { offset: 0x8B3C });
    run_one(&mut clear_bit, &mut host);
    assert_eq!(host.read_variable_word(0xA1).unwrap(), 0x03FB);
    assert_eq!(clear_bit.cursor().offset, 0x8B3F);

    let mut face_mother = PathVm::new(PathAddress { offset: 0x4260 });
    run_one(&mut face_mother, &mut host);
    assert_eq!(host.face_mother_calls, 1);
    assert_eq!(face_mother.cursor().offset, 0x4262);

    let mut selected_slot = PathVm::new(PathAddress { offset: 0x04C4 });
    run_one(&mut selected_slot, &mut host);
    assert_eq!(host.selected_slot_low_nibble_4_calls, 1);
    assert_eq!(selected_slot.cursor().offset, 0x04C6);

    let mut allocate_aux = PathVm::new(PathAddress { offset: 0x3FE6 });
    run_one(&mut allocate_aux, &mut host);
    assert_eq!(host.allocated_auxiliary_type_0b, vec![3]);
    assert_eq!(allocate_aux.cursor().offset, 0x3FE9);
}

#[test]
fn expanded_graph_handlers_and_inline_65816_blocks_match_retail() {
    let mut host = Host::new();

    let mut object_bytes = PathVm::new(PathAddress { offset: 0xA8CA });
    run_one(&mut object_bytes, &mut host);
    assert_eq!((host.vars[0x0A], host.vars[0x0B]), (0x1E, 0x01));

    host.write_external_word(0x0008, 0xBEEF).unwrap();
    let mut import_word = PathVm::new(PathAddress { offset: 0xB121 });
    run_one(&mut import_word, &mut host);
    assert_eq!(host.read_variable_word(0xA3).unwrap(), 0xBEEF);

    let mut store_index = PathVm::new(PathAddress { offset: 0xA8C6 });
    run_one(&mut store_index, &mut host);
    assert_eq!(host.external[INLINE_DISPATCH_INDEX as usize], 0);
    let mut increment_index = PathVm::new(PathAddress { offset: 0xA8D3 });
    run_one(&mut increment_index, &mut host);
    assert_eq!(host.external[INLINE_DISPATCH_INDEX as usize], 2);

    host.selected_relative_yaw = 0xFF;
    let mut yaw_between = PathVm::new(PathAddress { offset: 0xB110 });
    run_one(&mut yaw_between, &mut host);
    assert_eq!(yaw_between.cursor().offset, 0xB136);
    host.selected_relative_yaw = 0x20;
    yaw_between = PathVm::new(PathAddress { offset: 0xB110 });
    run_one(&mut yaw_between, &mut host);
    assert_eq!(yaw_between.cursor().offset, 0xB116);

    host.vars[VAR_OBJECT_FLAGS_24 as usize] = 0xFF;
    let mut clear_flag = PathVm::new(PathAddress { offset: 0x8D54 });
    run_one(&mut clear_flag, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_24 as usize], 0xFB);
    assert_eq!(clear_flag.cursor().offset, 0x8D61);

    host.write_external_word(INLINE_DISPATCH_INDEX, 0).unwrap();
    let mut dispatch = PathVm::new(PathAddress { offset: 0xAB2A });
    run_one(&mut dispatch, &mut host);
    assert_eq!(dispatch.cursor().offset, 0xAA3A);
    host.write_external_word(INLINE_DISPATCH_INDEX, 18).unwrap();
    dispatch = PathVm::new(PathAddress { offset: 0xAB2A });
    run_one(&mut dispatch, &mut host);
    assert_eq!(dispatch.cursor().offset, 0x8D81);
    host.write_external_word(INLINE_DISPATCH_INDEX, 19).unwrap();
    dispatch = PathVm::new(PathAddress { offset: 0xAB2A });
    assert_eq!(
        dispatch.run(&mut host, 1),
        Err(PathVmError::InvalidInlineDispatchIndex {
            address: PathAddress { offset: 0xAB2A },
            index: 19,
        })
    );

    let mut copy_position = PathVm::new(PathAddress { offset: 0xB0CB });
    run_one(&mut copy_position, &mut host);
    assert_eq!(host.copy_selected_position_calls, 1);
    assert_eq!(copy_position.cursor().offset, 0xB0E6);

    let mut refresh_collision = PathVm::new(PathAddress { offset: 0xB116 });
    run_one(&mut refresh_collision, &mut host);
    assert_eq!(host.refresh_collision_target_calls, 1);
    assert_eq!(refresh_collision.cursor().offset, 0xB121);

    let mut set_flag = PathVm::new(PathAddress { offset: 0xB129 });
    run_one(&mut set_flag, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_25 as usize] & 0x08, 0x08);
    assert_eq!(set_flag.cursor().offset, 0xB136);
}

#[test]
fn every_reachable_inline_site_has_typed_control_flow() {
    let sites: Vec<_> = sf2_data::path::PATH_COMMANDS
        .iter()
        .filter(|command| command.opcode == 0x089)
        .collect();
    assert_eq!(sites.len(), 45);

    for command in sites {
        let mut host = Host::new();
        let mut vm = PathVm::new(command.address);
        assert_eq!(
            run_one(&mut vm, &mut host).stop,
            RunStop::BudgetExhausted,
            "inline site ${:04X}",
            command.address.offset
        );
    }
}

#[test]
fn simple_inline_blocks_use_typed_state_and_operations() {
    let mut host = Host::new();

    for address in [0x2059, 0x9122, 0x919F, 0x91DA, 0xE690, 0xF9A1] {
        host.path_operations.clear();
        let mut vm = PathVm::new(PathAddress { offset: address });
        run_one(&mut vm, &mut host);
        assert_eq!(
            host.path_operations,
            vec![Sf2PathOperation::LinkSpawnedObjectToCurrent]
        );
        assert_eq!(
            vm.cursor(),
            command_at(PathAddress { offset: address })
                .unwrap()
                .successors[0]
        );
    }

    host.vars[VAR_OBJECT_FLAGS_24 as usize] = 0x80;
    let mut set_flag = PathVm::new(PathAddress { offset: 0x8D62 });
    run_one(&mut set_flag, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_24 as usize], 0x84);

    host.vars[VAR_OBJECT_FLAGS_25 as usize] = 0;
    let mut set_flag = PathVm::new(PathAddress { offset: 0x4B4D });
    run_one(&mut set_flag, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_25 as usize], 0x02);
    assert_eq!(set_flag.cursor().offset, 0x4B5A);

    host.path_operations.clear();
    let mut preserve_current = PathVm::new(PathAddress { offset: 0x9E71 });
    run_one(&mut preserve_current, &mut host);
    assert_eq!(
        host.path_operations,
        vec![Sf2PathOperation::PreserveCurrentObjectForParent]
    );
    assert_eq!(preserve_current.cursor().offset, 0x9E7B);

    host.path_operations.clear();
    let mut scale_motion = PathVm::new(PathAddress { offset: 0xF6E6 });
    run_one(&mut scale_motion, &mut host);
    assert_eq!(
        host.path_operations,
        vec![Sf2PathOperation::ScaleHorizontalMotion]
    );
    assert_eq!(scale_motion.cursor().offset, 0xF6F1);

    host.object_extension[0x1CCA] = 6;
    let mut color_phase = PathVm::new(PathAddress { offset: 0x9808 });
    run_one(&mut color_phase, &mut host);
    assert_eq!(host.object_extension[0x1CCA], 7);

    let mut control_mode = PathVm::new(PathAddress { offset: 0xB8C5 });
    run_one(&mut control_mode, &mut host);
    assert_eq!(host.external[0x1CDA], 2);
    assert_eq!(host.external[0x1CD9], 0);

    for (variable, value) in [(VAR_WORLD_X, 1), (VAR_WORLD_Y, 2), (VAR_WORLD_Z, 3)] {
        host.write_variable_word(variable, value).unwrap();
    }
    let mut opposite_world = PathVm::new(PathAddress { offset: 0xB8E4 });
    run_one(&mut opposite_world, &mut host);
    for (variable, expected) in [
        (VAR_WORLD_X, 0x8001),
        (VAR_WORLD_Y, 0x8002),
        (VAR_WORLD_Z, 0x8003),
    ] {
        assert_eq!(host.read_variable_word(variable).unwrap(), expected);
    }
    assert_eq!(host.vars[VAR_ROTATION_X as usize], 26);
    assert_eq!(host.vars[VAR_ROTATION_Y as usize], 64);
    assert_eq!(host.vars[VAR_ROTATION_Z as usize], 0);

    for (address, mask) in [
        (0xCFF8, 0x04),
        (0xD098, 0x08),
        (0xD253, 0x08),
        (0xE845, 0x01),
    ] {
        host.external[0x1D74] = 0;
        run_one(&mut PathVm::new(PathAddress { offset: address }), &mut host);
        assert_eq!(host.external[0x1D74], mask);
    }

    host.vars[VAR_OBJECT_FLAGS_25 as usize] = 0x80;
    run_one(&mut PathVm::new(PathAddress { offset: 0xF313 }), &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_25 as usize], 0x82);

    host.condition_result = false;
    host.object_extension[0x1CE2] = 0;
    run_one(&mut PathVm::new(PathAddress { offset: 0xF348 }), &mut host);
    assert_eq!(host.object_extension[0x1CE2], 1);
    assert_eq!(
        host.evaluated_conditions.last(),
        Some(&Sf2PathCondition::PlayerOneFlag25Bit20)
    );

    host.write_external_word(INLINE_DISPATCH_INDEX, 18).unwrap();
    let mut late_dispatch = PathVm::new(PathAddress { offset: 0xF668 });
    run_one(&mut late_dispatch, &mut host);
    assert_eq!(late_dispatch.cursor().offset, 0xF7C9);
}

#[test]
fn force_trigger_expansion_handlers_use_typed_state_and_control_flow() {
    let mut host = Host::new();

    for (address, next, operation) in [
        (
            0x9E6C,
            0x9E6D,
            Sf2PathOperation::ClearObjectRelativeReference,
        ),
        (
            0x1682,
            0x1683,
            Sf2PathOperation::PreserveCurrentPathContinuation,
        ),
        (
            0x45CB,
            0x45CD,
            Sf2PathOperation::IncrementSelectedAuxiliaryStage,
        ),
    ] {
        host.path_operations.clear();
        let mut vm = PathVm::new(PathAddress { offset: address });
        run_one(&mut vm, &mut host);
        assert_eq!(host.path_operations, vec![operation]);
        assert_eq!(vm.cursor().offset, next);
    }

    let mut progress = PathVm::new(PathAddress { offset: 0x45B2 });
    run_one(&mut progress, &mut host);
    assert_eq!(host.selected_auxiliary_progress_steps, vec![1]);
    assert_eq!(progress.cursor().offset, 0x45B6);

    host.selected_auxiliary_progress_settled = true;
    progress = PathVm::new(PathAddress { offset: 0x45B2 });
    run_one(&mut progress, &mut host);
    assert_eq!(host.selected_auxiliary_progress_steps, vec![1, 1]);
    assert_eq!(progress.cursor().offset, 0x455F);

    let mut state = PathVm::new(PathAddress { offset: 0x73EA });
    run_one(&mut state, &mut host);
    assert_eq!(state.cursor().offset, 0x73ED);
    assert_eq!(
        host.evaluated_conditions.last(),
        Some(&Sf2PathCondition::SelectedAuxiliaryStateMatchesGlobal)
    );

    host.condition_result = true;
    state = PathVm::new(PathAddress { offset: 0x73EA });
    run_one(&mut state, &mut host);
    assert_eq!(state.cursor().offset, 0x8D53);
}

#[test]
fn final_reachable_sf2_specific_handlers_preserve_effects_and_operands() {
    let mut host = Host::new();

    let mut cancel = PathVm::new(PathAddress { offset: 0x4024 });
    run_one(&mut cancel, &mut host);
    assert_eq!(host.canceled_triggers, vec![PathAddress { offset: 0x40B7 }]);
    assert_eq!(cancel.cursor().offset, 0x4027);

    let mut force = PathVm::new(PathAddress { offset: 0x054C });
    run_one(&mut force, &mut host);
    assert_eq!(
        host.forced_trigger_paths,
        vec![PathAddress { offset: 0x0550 }]
    );

    host.selected_bearing_plus_yaw = 0;
    let mut yaw_arc = PathVm::new(PathAddress { offset: 0x4214 });
    run_one(&mut yaw_arc, &mut host);
    assert_eq!(yaw_arc.cursor().offset, 0x4219);
    host.selected_bearing_plus_yaw = 0x20;
    yaw_arc = PathVm::new(PathAddress { offset: 0x4214 });
    run_one(&mut yaw_arc, &mut host);
    assert_eq!(yaw_arc.cursor().offset, 0x4218);

    run_one(&mut PathVm::new(PathAddress { offset: 0x8E82 }), &mut host);
    run_one(&mut PathVm::new(PathAddress { offset: 0x8E54 }), &mut host);
    assert_eq!(host.yaw_rotations, vec![-1]);
    assert_eq!(host.pitch_rotations, vec![10]);

    run_one(&mut PathVm::new(PathAddress { offset: 0x8D08 }), &mut host);
    run_one(&mut PathVm::new(PathAddress { offset: 0x5468 }), &mut host);
    assert_eq!(
        host.player_target_updates,
        vec![PlayerTargetUpdate::FlagLinked, PlayerTargetUpdate::Flag08]
    );

    run_one(&mut PathVm::new(PathAddress { offset: 0x78AD }), &mut host);
    run_one(&mut PathVm::new(PathAddress { offset: 0x4E94 }), &mut host);
    run_one(&mut PathVm::new(PathAddress { offset: 0x40C5 }), &mut host);
    assert_eq!(
        host.selected_markers,
        vec![
            (0x45, SelectedMarkerClass::Direct),
            (0x76, SelectedMarkerClass::Class1),
            (0xC9, SelectedMarkerClass::Class2),
        ]
    );

    host.vars[0xA2] = 2;
    host.write_variable_word(0x0E, 0x0107).unwrap();
    host.long_external.insert(0x00B349, 0xFE);
    let mut indexed_delta = PathVm::new(PathAddress { offset: 0x4BA0 });
    run_one(&mut indexed_delta, &mut host);
    assert_eq!(host.read_variable_word(0x0E).unwrap(), 0x0105);
    assert_eq!(host.vars[0xA2], 3, "retail advances the index variable");
    assert_eq!(indexed_delta.cursor().offset, 0x4BA8);

    run_one(&mut PathVm::new(PathAddress { offset: 0x89E3 }), &mut host);
    assert_eq!(host.spawn_linked_object_effects_calls, 1);
}

#[test]
fn child_records_and_banked_indexing_preserve_all_retail_operands() {
    let mut host = Host::new();

    let mut spawn = PathVm::new(PathAddress { offset: 0x76CA });
    run_one(&mut spawn, &mut host);
    assert_eq!(
        host.child_spawns,
        vec![ChildSpawn {
            shape: 0xDB74,
            path: PathAddress { offset: 0x5667 },
            rotation: [0, 0, 0],
            hit_points: 10,
            attack_points: 10,
            offset: [0, -50, 750],
            child_number: 1,
        }]
    );

    let mut alias = PathVm::new(PathAddress { offset: 0x75F0 });
    run_one(&mut alias, &mut host);
    assert_eq!(host.child_spawns[1].shape, 0xBC9C);
    assert_eq!(host.child_spawns[1].path.offset, 0x782B);
    assert_eq!(host.child_spawns[1].rotation, [0, 0xC0, 0]);
    assert_eq!(host.child_spawns[1].hit_points, 10);
    assert_eq!(host.child_spawns[1].attack_points, 10);
    assert_eq!(host.child_spawns[1].offset, [0, 450, 1000]);
    assert_eq!(host.child_spawns[1].child_number, 7);

    let mut remove = PathVm::new(PathAddress { offset: 0x7766 });
    run_one(&mut remove, &mut host);
    assert_eq!(host.removed_children, vec![1]);

    host.vars[0xA2] = 3;
    host.long_external.insert(0x06FC34, 0xA5);
    let mut index_byte = PathVm::new(PathAddress { offset: 0x4F36 });
    run_one(&mut index_byte, &mut host);
    assert_eq!(host.vars[0x9A], 0xA5);

    host.vars[0xA1] = 2;
    host.long_external.insert(0x06FD49, 0x34);
    host.long_external.insert(0x06FD4A, 0x12);
    let mut index_word = PathVm::new(PathAddress { offset: 0x77A3 });
    run_one(&mut index_word, &mut host);
    assert_eq!(host.read_variable_word(0x92).unwrap(), 0x1234);
}

#[test]
fn direct_external_stores_increment_and_flag_mask_match_retail() {
    let mut host = Host::new();

    host.external[0xD764] = 0xFF;
    let mut increment = PathVm::new(PathAddress { offset: 0x7674 });
    run_one(&mut increment, &mut host);
    assert_eq!(host.external[0xD764], 0);
    assert_eq!(increment.cursor().offset, 0x7677);

    host.vars[VAR_OBJECT_FLAGS_31 as usize] = 0xF7;
    let mut mask = PathVm::new(PathAddress { offset: 0x050E });
    run_one(&mut mask, &mut host);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_31 as usize], 0xE7);
    assert_eq!(mask.cursor().offset, 0x0510);

    let mut store_byte = PathVm::new(PathAddress { offset: 0x04DF });
    run_one(&mut store_byte, &mut host);
    assert_eq!(host.external[0xD786], 0x50);
    assert_eq!(store_byte.cursor().offset, 0x04E3);

    let mut store_word = PathVm::new(PathAddress { offset: 0x3FDE });
    run_one(&mut store_word, &mut host);
    assert_eq!(host.read_external_word(0xD777).unwrap(), 0x8964);
    assert_eq!(store_word.cursor().offset, 0x3FE3);
}

#[test]
fn messages_negation_decrements_and_low_level_writes_match_retail() {
    let mut host = Host::new();

    let mut literal_message = PathVm::new(PathAddress { offset: 0x04F2 });
    run_one(&mut literal_message, &mut host);
    host.vars[0xA3] = 0x77;
    let mut variable_message = PathVm::new(PathAddress { offset: 0x04F7 });
    run_one(&mut variable_message, &mut host);
    assert_eq!(host.messages, vec![0xD5, 0x77]);

    host.write_variable_word(0x8E, 0x1234).unwrap();
    let mut negate = PathVm::new(PathAddress { offset: 0x06A5 });
    run_one(&mut negate, &mut host);
    assert_eq!(host.read_variable_word(0x8E).unwrap(), 0xEDCC);

    host.external[0xD786] = 0;
    let mut decrement_byte = PathVm::new(PathAddress { offset: 0x0519 });
    run_one(&mut decrement_byte, &mut host);
    assert_eq!(host.external[0xD786], 0xFF);
    host.write_external_word(0xD77D, 0x5001).unwrap();
    let mut or_global = PathVm::new(PathAddress { offset: 0x8DF7 });
    run_one(&mut or_global, &mut host);
    assert_eq!(host.read_external_word(0xD77D).unwrap(), 0x5009);

    let mut set_1cef = PathVm::new(PathAddress { offset: 0x4F24 });
    run_one(&mut set_1cef, &mut host);
    let mut set_1ccb = PathVm::new(PathAddress { offset: 0x4AB1 });
    run_one(&mut set_1ccb, &mut host);
    let mut set_1ccc = PathVm::new(PathAddress { offset: 0x4F0C });
    run_one(&mut set_1ccc, &mut host);
    assert_eq!(host.object_extension[0x1CEF], 1);
    assert_eq!(host.object_extension[0x1CCB], 0x80);
    assert_eq!(host.object_extension[0x1CCC], 9);

    let mut reset_1cc1 = PathVm::new(PathAddress { offset: 0x4F29 });
    host.object_extension[0x1CC1] = 0xAA;
    host.object_extension[0x1CC2] = 0xBB;
    run_one(&mut reset_1cc1, &mut host);
    assert_ne!(host.vars[VAR_PATH_FLAGS as usize] & 0x20, 0);
    assert_eq!(host.read_object_extension_word(0x1CC1).unwrap(), 0);

    host.external[0x1DDD] = 0x12;
    let mut set_global_bit = PathVm::new(PathAddress { offset: 0x78B0 });
    run_one(&mut set_global_bit, &mut host);
    assert_eq!(host.external[0x1DDD], 0x92);

    let mut no_op = PathVm::new(PathAddress { offset: 0x4F2E });
    run_one(&mut no_op, &mut host);
    assert_eq!(no_op.cursor().offset, 0x4F2F);
}

#[test]
fn next_and_trigger_records_follow_the_retail_heap_protocol() {
    let mut host = Host::new();

    host.stack.extend([0x0600, 2]);
    let mut next = PathVm::new(PathAddress { offset: 0x04FD });
    assert_eq!(
        run_one(&mut next, &mut host).stop,
        RunStop::Yielded(YieldReason::Next)
    );
    assert_eq!(next.cursor().offset, 0x0600);
    assert_eq!(host.stack, vec![0x0600, 1]);

    let mut immediate = PathVm::new(PathAddress { offset: 0x4F6B });
    assert_eq!(
        run_one(&mut immediate, &mut host).stop,
        RunStop::BudgetExhausted
    );
    assert_eq!(immediate.cursor().offset, 0x4F6C);
    assert!(host.stack.is_empty());

    for address in [0x0594, 0x06AF, 0x3FEB, 0x0502] {
        let mut vm = PathVm::new(PathAddress { offset: address });
        run_one(&mut vm, &mut host);
    }
    assert_eq!(
        host.triggers,
        vec![
            PathTrigger {
                path: PathAddress { offset: 0x070C },
                delay: 0,
                trigger: 0,
            },
            PathTrigger {
                path: PathAddress { offset: 0x070C },
                delay: 0,
                trigger: 0,
            },
            PathTrigger {
                path: PathAddress { offset: 0x8772 },
                delay: 3,
                trigger: 0,
            },
            PathTrigger {
                path: PathAddress { offset: 0x051F },
                delay: 0x11,
                trigger: 0x79,
            },
        ]
    );
}

#[test]
fn facing_animation_and_numeric_branches_match_retail() {
    let mut host = Host::new();

    let mut face_yaw = PathVm::new(PathAddress { offset: 0x4210 });
    run_one(&mut face_yaw, &mut host);
    assert_eq!(host.face_player_yaw_calls, 1);
    assert_eq!(face_yaw.cursor().offset, 0x4211);

    let mut init_animation = PathVm::new(PathAddress { offset: 0x42DE });
    run_one(&mut init_animation, &mut host);
    assert_eq!(host.object_extension[0x1CCB], 0x88);
    let mut add_animation = PathVm::new(PathAddress { offset: 0x40B7 });
    host.object_extension[0x1CCB] = 0x8F;
    run_one(&mut add_animation, &mut host);
    assert_eq!(host.object_extension[0x1CCB], 0x80);

    host.random_bytes.extend([0x7E, 0x7F]);
    let mut random = PathVm::new(PathAddress { offset: 0x4CBB });
    run_one(&mut random, &mut host);
    assert_eq!(random.cursor().offset, 0x4C7E);
    random = PathVm::new(PathAddress { offset: 0x4CBB });
    run_one(&mut random, &mut host);
    assert_eq!(random.cursor().offset, 0x4CBE);

    host.write_variable_word(0x0E, 1).unwrap();
    let mut between = PathVm::new(PathAddress { offset: 0x8D45 });
    run_one(&mut between, &mut host);
    assert_eq!(between.cursor().offset, 0x8D4E);
    host.write_variable_word(0x0E, 0).unwrap();
    between = PathVm::new(PathAddress { offset: 0x8D45 });
    run_one(&mut between, &mut host);
    assert_eq!(between.cursor().offset, 0x8D4D);
}

#[test]
fn exact_reachable_variable_family_and_chase_handlers_execute() {
    let mut host = Host::new();

    let mut weapon = PathVm::new(PathAddress { offset: 0x87CF });
    run_one(&mut weapon, &mut host);
    assert_eq!(host.vars[0x2F], 0x12);

    host.child_dead = true;
    let mut child = PathVm::new(PathAddress { offset: 0x48A8 });
    run_one(&mut child, &mut host);
    assert_eq!(child.cursor().offset, 0x48A7);
    host.child_dead = false;
    child = PathVm::new(PathAddress { offset: 0x48A8 });
    run_one(&mut child, &mut host);
    assert_eq!(child.cursor().offset, 0x48AC);

    host.vars[0x12] = 5;
    host.vars[0xA3] = 7;
    let mut add_alias = PathVm::new(PathAddress { offset: 0x49B6 });
    run_one(&mut add_alias, &mut host);
    assert_eq!(host.vars[0x12], 12);

    host.vars[0xA2] = 5;
    let mut negate = PathVm::new(PathAddress { offset: 0xAC17 });
    run_one(&mut negate, &mut host);
    assert_eq!(host.vars[0xA2], 0xFB);

    host.write_variable_word(0x3B, 0).unwrap();
    let mut zero_word = PathVm::new(PathAddress { offset: 0x05A6 });
    run_one(&mut zero_word, &mut host);
    assert_eq!(zero_word.cursor().offset, 0x05B4);
    host.vars[0x27] = 1;
    let mut not_zero_byte = PathVm::new(PathAddress { offset: 0x0632 });
    run_one(&mut not_zero_byte, &mut host);
    assert_eq!(not_zero_byte.cursor().offset, 0x0667);
    host.write_variable_word(0xA3, 1).unwrap();
    let mut not_zero_word = PathVm::new(PathAddress { offset: 0x062E });
    run_one(&mut not_zero_word, &mut host);
    assert_eq!(not_zero_word.cursor().offset, 0x0667);

    host.write_variable_word(0x0E, 2).unwrap();
    let mut world_y = PathVm::new(PathAddress { offset: 0x4888 });
    run_one(&mut world_y, &mut host);
    assert_eq!(host.read_variable_word(0x0E).unwrap(), 0xFFD0);
    host.write_variable_word(0xA3, 4).unwrap();
    let mut add_signed = PathVm::new(PathAddress { offset: 0x0654 });
    run_one(&mut add_signed, &mut host);
    assert_eq!(host.read_variable_word(0xA3).unwrap(), 0x0064);

    host.write_variable_word(0x0C, 0x5678).unwrap();
    let mut export_word = PathVm::new(PathAddress { offset: 0x4DE4 });
    run_one(&mut export_word, &mut host);
    assert_eq!(host.read_external_word(0x1D88).unwrap(), 0x5678);

    host.vars[0x94] = 0xD8;
    let mut chase_byte = PathVm::new(PathAddress { offset: 0x8E5D });
    run_one(&mut chase_byte, &mut host);
    assert_eq!(host.vars[0x94], 0xD9);

    host.stack.push(0xABCD);
    let mut pull_word = PathVm::new(PathAddress { offset: 0x8A18 });
    run_one(&mut pull_word, &mut host);
    assert_eq!(host.read_variable_word(0x39).unwrap(), 0xABCD);
}

#[test]
fn newly_reviewed_reachable_handlers_preserve_retail_effects() {
    let mut host = Host::new();

    host.vars[VAR_PATH_FLAGS as usize] = 0xFF;
    run_one(&mut PathVm::new(PathAddress { offset: 0xAC30 }), &mut host);
    assert_eq!(host.vars[VAR_PATH_FLAGS as usize], 0xF7);

    run_one(&mut PathVm::new(PathAddress { offset: 0x87D1 }), &mut host);
    assert_eq!(host.fire_weapon_calls, 1);

    host.vars[VAR_OBJECT_FLAGS_23 as usize] = 0x08;
    let mut hit = PathVm::new(PathAddress { offset: 0x595E });
    run_one(&mut hit, &mut host);
    assert_eq!(hit.cursor().offset, 0x5964);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_23 as usize] & 0x08, 0);

    host.stack.extend([0x1111, 0x2222]);
    let mut break_vm = PathVm::new(PathAddress { offset: 0x54EC });
    run_one(&mut break_vm, &mut host);
    assert_eq!(break_vm.cursor().offset, 0x54F5);
    assert!(host.stack.is_empty());

    host.write_variable_word(0x0E, 5).unwrap();
    host.write_variable_word(0xA3, 7).unwrap();
    run_one(&mut PathVm::new(PathAddress { offset: 0xABFE }), &mut host);
    assert_eq!(host.read_variable_word(0x0E).unwrap(), 12);

    host.random_bytes.push(0xB5);
    run_one(&mut PathVm::new(PathAddress { offset: 0x4033 }), &mut host);
    assert_eq!(host.vars[0x95], 0xB5);

    host.write_variable_word(0x0C, 0xFFFE).unwrap();
    run_one(&mut PathVm::new(PathAddress { offset: 0x8EAE }), &mut host);
    assert_eq!(host.read_variable_word(0x0C).unwrap(), 3);

    run_one(&mut PathVm::new(PathAddress { offset: 0x8D4D }), &mut host);
    assert_ne!(host.vars[VAR_OBJECT_FLAGS_20 as usize] & 0x08, 0);

    host.vars[0xA1] = (-5i8) as u8;
    run_one(&mut PathVm::new(PathAddress { offset: 0x8F4A }), &mut host);
    assert_eq!(host.vars[0xA1] as i8, -2);

    host.write_external_word(0x12C3, 0x3456).unwrap();
    host.vars[VAR_OBJECT_FLAGS_24 as usize] = 0xFF;
    run_one(&mut PathVm::new(PathAddress { offset: 0xABBF }), &mut host);
    assert_eq!(host.read_external_word(0xCF1F).unwrap(), 0x3456);
    assert_eq!(host.vars[VAR_OBJECT_FLAGS_24 as usize], 0x7F);

    host.write_external_word(0xD77D, 0xFFFF).unwrap();
    run_one(&mut PathVm::new(PathAddress { offset: 0x8E04 }), &mut host);
    assert_eq!(host.read_external_word(0xD77D).unwrap(), 0xFFE7);
    run_one(&mut PathVm::new(PathAddress { offset: 0x8DAE }), &mut host);
    assert_eq!(host.read_external_word(0xD77D).unwrap(), 0);

    host.vars[0x94] = 2;
    host.external[0xD767] = 5;
    run_one(&mut PathVm::new(PathAddress { offset: 0x49C6 }), &mut host);
    assert_eq!(host.vars[0x94], 7);

    run_one(&mut PathVm::new(PathAddress { offset: 0x058B }), &mut host);
    assert_eq!(host.copy_selected_position_calls, 1);

    run_one(&mut PathVm::new(PathAddress { offset: 0x78A3 }), &mut host);
    assert_eq!(host.face_player_calls, 1);

    run_one(&mut PathVm::new(PathAddress { offset: 0x5927 }), &mut host);
    assert_ne!(host.vars[VAR_OBJECT_FLAGS_09 as usize] & 0x01, 0);

    host.external[0x1D74] = 0x01;
    run_one(&mut PathVm::new(PathAddress { offset: 0x4DE2 }), &mut host);
    assert_eq!(host.external[0x1D74], 0x41);

    host.external[0x1DDD] = 0;
    let mut global_branch = PathVm::new(PathAddress { offset: 0x787D });
    run_one(&mut global_branch, &mut host);
    assert_eq!(global_branch.cursor().offset, 0x7881);
    host.external[0x1DDD] = 0x80;
    global_branch = PathVm::new(PathAddress { offset: 0x787D });
    run_one(&mut global_branch, &mut host);
    assert_eq!(global_branch.cursor().offset, 0x78BF);
}

#[test]
fn comparison_context_and_auxiliary_handlers_match_retail() {
    let mut host = Host::new();

    host.selected_distance = 1000;
    let mut selected_distance = PathVm::new(PathAddress { offset: 0x421A });
    run_one(&mut selected_distance, &mut host);
    assert_eq!(selected_distance.cursor().offset, 0x422D);

    host.mother_distance = Some(500);
    let mut mother_distance = PathVm::new(PathAddress { offset: 0x4001 });
    run_one(&mut mother_distance, &mut host);
    assert_eq!(mother_distance.cursor().offset, 0x4024);

    let mut hold = PathVm::new(PathAddress { offset: 0x05DA });
    assert_eq!(
        run_one(&mut hold, &mut host).stop,
        RunStop::Yielded(YieldReason::Hold)
    );
    assert_eq!(hold.cursor().offset, 0x05DA);
    assert_eq!(host.hold_calls, 1);
    assert_ne!(host.vars[VAR_OBJECT_FLAGS_09 as usize] & 0x08, 0);

    host.vars[0x14] = 0;
    host.vars[0x95] = 4;
    run_one(&mut PathVm::new(PathAddress { offset: 0x4038 }), &mut host);
    assert_eq!(host.vars[0x14], 1);
    host.write_variable_word(0x0E, 100).unwrap();
    host.write_variable_word(0xA3, 120).unwrap();
    run_one(&mut PathVm::new(PathAddress { offset: 0xAC23 }), &mut host);
    assert_eq!(host.read_variable_word(0x0E).unwrap(), 102);

    host.selected_within_range = true;
    let mut range = PathVm::new(PathAddress { offset: 0x4862 });
    run_one(&mut range, &mut host);
    assert_eq!(range.cursor().offset, 0x486A);

    host.optional_context_available = true;
    let mut child = PathVm::new(PathAddress { offset: 0x4231 });
    run_one(&mut child, &mut host);
    assert_eq!(child.cursor().offset, 0x4235);
    assert_eq!(
        host.transitions.last(),
        Some(&(
            ContextTransition::BecomeChild(4),
            PathAddress { offset: 0x4235 }
        ))
    );
    let mut mother = PathVm::new(PathAddress { offset: 0x4CC2 });
    run_one(&mut mother, &mut host);
    assert_eq!(mother.cursor().offset, 0x4CC5);
    assert_eq!(
        host.transitions.last().unwrap().0,
        ContextTransition::BecomeMother
    );

    host.vars[0xA2] = 7;
    let mut variable_child = PathVm::new(PathAddress { offset: 0x48B1 });
    run_one(&mut variable_child, &mut host);
    assert_eq!(variable_child.cursor().offset, 0x48B5);
    assert_eq!(
        host.transitions.last().unwrap().0,
        ContextTransition::BecomeChild(7)
    );
    host.optional_context_available = false;
    variable_child = PathVm::new(PathAddress { offset: 0x48B1 });
    run_one(&mut variable_child, &mut host);
    assert_eq!(variable_child.cursor().offset, 0x48B9);

    for (address, class, target) in [
        (0x060E, 0x10, 0x0620),
        (0x0621, 0x20, 0x0620),
        (0x065D, 0x30, 0x0662),
    ] {
        host.selected_slot_class = class;
        let mut vm = PathVm::new(PathAddress { offset: address });
        run_one(&mut vm, &mut host);
        assert_eq!(vm.cursor().offset, target);
    }

    host.write_external_word(0xD77D, 1).unwrap();
    let mut bits_set = PathVm::new(PathAddress { offset: 0x8E37 });
    run_one(&mut bits_set, &mut host);
    assert_eq!(bits_set.cursor().offset, 0x8E3D);
    let mut bits_clear = PathVm::new(PathAddress { offset: 0x8DFF });
    run_one(&mut bits_clear, &mut host);
    assert_eq!(bits_clear.cursor().offset, 0x8DFE);

    host.vars[0xA1] = 10;
    host.vars[0xA2] = 5;
    let mut less_byte = PathVm::new(PathAddress { offset: 0x0537 });
    run_one(&mut less_byte, &mut host);
    assert_eq!(less_byte.cursor().offset, 0x054F);
    host.write_variable_word(0x0C, 10).unwrap();
    host.write_variable_word(0x39, 5).unwrap();
    let mut less_word = PathVm::new(PathAddress { offset: 0x0760 });
    run_one(&mut less_word, &mut host);
    assert_eq!(less_word.cursor().offset, 0x0777);

    host.vars[0x14] = 9;
    host.vars[0xA9] = 9;
    let mut same = PathVm::new(PathAddress { offset: 0x4F45 });
    run_one(&mut same, &mut host);
    assert_eq!(same.cursor().offset, 0x4F50);

    host.selected_aux_flags = 0x40;
    let mut aux = PathVm::new(PathAddress { offset: 0x066A });
    run_one(&mut aux, &mut host);
    assert_eq!(aux.cursor().offset, 0x0671);
    run_one(&mut PathVm::new(PathAddress { offset: 0xA54E }), &mut host);
    assert_eq!(host.selected_aux_flags, 0x60);

    host.external[0x00C4] = 0x01;
    let mut external = PathVm::new(PathAddress { offset: 0xABA9 });
    run_one(&mut external, &mut host);
    assert_eq!(external.cursor().offset, 0xABB1);
}

#[test]
fn expanded_retail_arithmetic_handlers_preserve_width_sign_and_operand_order() {
    let mut host = Host::new();

    host.write_variable_word(0x0E, 0x12A5).unwrap();
    run_opcode(0x04F, &mut host);
    assert_eq!(host.vars[0x27], 0xA5);

    host.random_bytes.extend([0x34, 0xF2]);
    run_opcode(0x059, &mut host);
    assert_eq!(host.read_variable_word(0x03).unwrap(), 0xF230 & 0xFF90);

    host.vars[0x12] = 1;
    host.vars[0x14] = 2;
    host.vars[0x16] = 3;
    run_one(&mut PathVm::new(PathAddress { offset: 0x1D46 }), &mut host);
    run_one(&mut PathVm::new(PathAddress { offset: 0x0F96 }), &mut host);
    run_one(&mut PathVm::new(PathAddress { offset: 0x038B }), &mut host);
    assert_eq!(host.vars[0x12], 0xFF);
    assert_eq!(host.vars[0x14], 0x82);
    assert_eq!(host.vars[0x16], 0xF9);

    host.write_variable_word(0x0C, 16).unwrap();
    run_opcode(0x082, &mut host);
    assert_eq!(host.read_variable_word(0x0C).unwrap(), 14);

    host.vars[0x16] = 16;
    let (_, report) = run_opcode(0x083, &mut host);
    assert_eq!(report.stop, RunStop::Yielded(YieldReason::Wait));
    assert_eq!(host.vars[0x16], 14);

    host.write_variable_word(0x80, (-3i16) as u16).unwrap();
    run_opcode(0x08F, &mut host);
    assert_eq!(host.read_variable_word(0x80).unwrap(), (-1i16) as u16);

    host.vars[0xA2] = 0;
    host.write_variable_word(0x16, 1).unwrap();
    host.long_external.insert(0x06FF36, 0xFE);
    run_opcode(0x092, &mut host);
    assert_eq!(host.read_variable_word(0x16).unwrap(), 0x00FF);
    assert_eq!(host.vars[0xA2], 1);

    host.write_variable_word(0x0E, 10).unwrap();
    host.write_external_word(0xD7D3, 5).unwrap();
    run_opcode(0x0EA, &mut host);
    assert_eq!(host.read_variable_word(0x0E).unwrap(), 15);

    host.write_external_word(0xD767, 7).unwrap();
    host.write_variable_word(0xA3, 9).unwrap();
    run_opcode(0x0EC, &mut host);
    assert_eq!(host.read_external_word(0xD767).unwrap(), 16);

    host.vars[0x27] = 0x85;
    run_one(&mut PathVm::new(PathAddress { offset: 0x8832 }), &mut host);
    assert_eq!(host.vars[0x27], 0x42);
}

#[test]
fn expanded_object_handlers_emit_typed_non_optional_engine_services() {
    let mut host = Host::new();

    for (opcode, operation) in [
        (0x009, Sf2PathOperation::FaceSelectedSmooth),
        (0x00E, Sf2PathOperation::FaceLinkedSmooth),
        (0x010, Sf2PathOperation::ExplodeObject),
        (0x03C, Sf2PathOperation::FlagLinkedObject),
        (0x0A2, Sf2PathOperation::RefreshSelectedRelativeTransform),
        (0x0B5, Sf2PathOperation::SelectSelfAndClearRelativeTransform),
        (0x0F2, Sf2PathOperation::RefreshLinkedRotationDeltas),
        (0x0FF, Sf2PathOperation::UpdatePilotAuxState),
        (0x10F, Sf2PathOperation::FaceSelectedImmediate),
        (0x110, Sf2PathOperation::ChasePlayerTowardObject),
        (0x111, Sf2PathOperation::SnapPlayerToObject),
        (
            0x130,
            Sf2PathOperation::PositionExternalObjectAndFaceSelected,
        ),
        (0x14F, Sf2PathOperation::CopySelectedAuxRotation),
        (0x164, Sf2PathOperation::ApplyFormationOffset),
        (0x17B, Sf2PathOperation::FreeObjectAuxiliaryAndResetD742),
        (0x0DB, Sf2PathOperation::ResetSelectedAuxiliaryMotion),
        (0x132, Sf2PathOperation::IncrementLinkedAuxiliaryCounter),
        (0x133, Sf2PathOperation::DecrementLinkedAuxiliaryCounter),
        (0x13A, Sf2PathOperation::SelectCurrentAsRotationTarget),
        (0x162, Sf2PathOperation::ClearSelectedAuxiliaryFlag01),
        (0x16E, Sf2PathOperation::SetSelectedSlotLowNibble1),
        (
            0x142,
            Sf2PathOperation::ChaseObjectPositionTowardCurrent(0x033F),
        ),
        (0x144, Sf2PathOperation::CopySelectedRotation),
        (0x14C, Sf2PathOperation::CopyPositionToObject(0x033F)),
        (0x14D, Sf2PathOperation::CopyRotationToObjectFixed(0x033F)),
        (0x047, Sf2PathOperation::PopPathStackPair),
        (
            0x138,
            Sf2PathOperation::SetObjectRotationTowardTarget {
                object: 0x033F,
                shift: 1,
            },
        ),
        (
            0x139,
            Sf2PathOperation::ChaseObjectRotationTowardTarget {
                object: 0x033F,
                shift: 1,
            },
        ),
        (0x156, Sf2PathOperation::RefreshOwnedPlayerAuxiliaryOrigin),
        (
            0x15E,
            Sf2PathOperation::InstallStrategyAndStop {
                strategy: 0xAFE4,
                state: 9,
            },
        ),
        (0x0DE, Sf2PathOperation::CaptureSelectedAuxiliaryMotion),
    ] {
        host.path_operations.clear();
        run_opcode(opcode, &mut host);
        assert_eq!(
            host.path_operations,
            vec![operation],
            "opcode ${opcode:03X}"
        );
    }

    host.path_operations.clear();
    run_one(&mut PathVm::new(PathAddress { offset: 0xF38E }), &mut host);
    assert_eq!(
        host.path_operations,
        vec![Sf2PathOperation::ConfigurePlayerAuxiliary(0xFFF8)]
    );

    host.path_operations.clear();
    run_opcode(0x031, &mut host);
    assert_eq!(
        host.path_operations,
        vec![Sf2PathOperation::SpawnObject(ObjectSpawn {
            shape: 0xCEE0,
            path: PathAddress { offset: 0x12E5 },
            rotation: [0, 0, 0],
            hit_points: 100,
            attack_points: 4,
            offset: [0, -32, 32],
        })]
    );

    host.path_operations.clear();
    run_one(&mut PathVm::new(PathAddress { offset: 0x5D63 }), &mut host);
    run_opcode(0x157, &mut host);
    run_opcode(0x158, &mut host);
    assert_eq!(
        host.path_operations,
        vec![
            Sf2PathOperation::InitializePlayerAuxWord(0x0080),
            Sf2PathOperation::ConfigurePilotAuxModeA(1),
            Sf2PathOperation::ConfigurePilotAuxModeB(1),
        ]
    );
}

#[test]
fn contact_class_selects_each_reviewed_target_or_falls_through() {
    let command = first_command(0x131);
    let target = |operand: usize| {
        let index = command.prefix_size as usize + operand;
        u16::from_le_bytes([command.raw[index], command.raw[index + 1]])
    };

    for (contact, expected) in [
        (Some(PathContactClass::NoObject), target(1)),
        (Some(PathContactClass::AuxiliaryType0b), target(3)),
        (Some(PathContactClass::OtherObject), target(5)),
        (
            None,
            command
                .address
                .offset
                .wrapping_add(u16::from(command.raw_len)),
        ),
    ] {
        let mut host = Host::new();
        host.path_contact = contact;
        let (vm, _) = run_opcode(0x131, &mut host);
        assert_eq!(vm.cursor().offset, expected, "contact {contact:?}");
    }
}

#[test]
fn map_cursor_handlers_use_typed_external_state_and_exact_branches() {
    let mut host = Host::new();

    let mut set_selector = PathVm::new(PathAddress { offset: 0xCF1E });
    run_one(&mut set_selector, &mut host);
    assert_eq!(host.read_external_byte(0x1D72).unwrap(), 2);
    assert_eq!(set_selector.cursor().offset, 0xCF21);

    host.write_external_byte(0x1D77, 5).unwrap();
    host.write_external_word(0x1D78, 0x2468).unwrap();
    let (restore_cursor, _) = run_opcode(0x13E, &mut host);
    assert_eq!(host.read_external_byte(0x192E).unwrap(), 5);
    assert_eq!(host.read_external_word(0x1657).unwrap(), 0x2468);
    assert_eq!(
        restore_cursor.cursor().offset,
        first_command(0x13E).successors[0].offset
    );

    for (selector, expected_cursor) in [(3, 0xCF3D), (2, 0xCF3A)] {
        host.write_external_byte(0x1D72, selector).unwrap();
        let mut branch = PathVm::new(PathAddress { offset: 0xCF35 });
        run_one(&mut branch, &mut host);
        assert_eq!(branch.cursor().offset, expected_cursor);
    }
}

#[test]
fn selected_auxiliary_branches_use_reviewed_typed_predicates() {
    for (opcode, condition) in [
        (0x15F, Sf2PathCondition::SelectedAuxiliaryMapCellOccupied),
        (0x160, Sf2PathCondition::SelectedAuxiliaryFlag04Clear),
    ] {
        let command = first_command(opcode);
        let target = u16::from_le_bytes([
            command.raw[command.prefix_size as usize + 1],
            command.raw[command.prefix_size as usize + 2],
        ]);

        let mut matched_host = Host::new();
        matched_host.condition_result = true;
        let (matched, _) = run_opcode(opcode, &mut matched_host);
        assert_eq!(matched_host.evaluated_conditions, vec![condition]);
        assert_eq!(matched.cursor().offset, target);

        let mut clear_host = Host::new();
        let (clear, _) = run_opcode(opcode, &mut clear_host);
        assert_eq!(clear_host.evaluated_conditions, vec![condition]);
        assert_eq!(
            clear.cursor().offset,
            command
                .address
                .offset
                .wrapping_add(u16::from(command.raw_len))
        );
    }
}

#[test]
fn expanded_object_conditions_branch_only_on_the_reviewed_retail_predicate() {
    let cases = [
        (0x01A, Sf2PathCondition::HitGround { offset: 0 }, 3usize),
        (0x023, Sf2PathCondition::ProjectedSelectedPointNegative, 1),
        (0x024, Sf2PathCondition::SelectedLeftOfObject, 1),
        (
            0x05F,
            Sf2PathCondition::ProjectedSelectedForwardPointNegative,
            1,
        ),
        (0x101, Sf2PathCondition::SelectedBelowObject, 1),
        (0x113, Sf2PathCondition::SelectedOrCurrentAuxState, 1),
    ];

    for (opcode, expected_condition, target_operand) in cases {
        let command = first_command(opcode);
        let expected_condition = if opcode == 0x01A {
            let raw_index = command.prefix_size as usize + 1;
            Sf2PathCondition::HitGround {
                offset: u16::from_le_bytes([command.raw[raw_index], command.raw[raw_index + 1]]),
            }
        } else {
            expected_condition
        };
        let raw_index = command.prefix_size as usize + target_operand;
        let target = u16::from_le_bytes([command.raw[raw_index], command.raw[raw_index + 1]]);
        let mut host = Host::new();
        host.condition_result = true;
        let (vm, _) = run_opcode(opcode, &mut host);
        assert_eq!(host.evaluated_conditions, vec![expected_condition]);
        assert_eq!(vm.cursor().offset, target, "opcode ${opcode:03X}");
    }
}
