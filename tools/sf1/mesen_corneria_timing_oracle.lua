-- Independently measure the retail SF1 gameplay cadence in Mesen.
--
-- The controller schedule matches sf_oracle::sf1_input through game frame 359.
-- This script only observes retail execution and work RAM; it never writes game
-- state. Raw machine addresses intentionally remain confined to oracle tooling.

local video_frame = 0
local video_frames_per_front_end_tick = 3
local front_end_confirm_cadence_ticks = 60
local front_end_confirm_hold_ticks = 2
local front_end_last_confirm_tick = 360
local game_destination_select_tick = 380
local game_destination_confirm_tick = 420
local route_selection_confirm_tick = 500
local route_selection_confirm_hold_ticks = 12
local planet_dismiss_start_tick = 840
local planet_dismiss_end_tick = 900
local planet_dismiss_cadence_ticks = 2
local user_corneria_initial_game_frame = 0
local input_segment_frames = 30
local pilot_input = {
  center = {},
  up = { up = true },
  down = { down = true },
  left = { left = true },
  right = { right = true },
  up_left = { up = true, left = true },
  up_right = { up = true, right = true },
  down_left = { down = true, left = true },
  down_right = { down = true, right = true },
}
local corneria_route_tape = {
  pilot_input.up_left, pilot_input.up_left, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_right, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
  pilot_input.up_left, pilot_input.up_left, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_right, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
  pilot_input.up_left, pilot_input.up_left, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_left, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_right, pilot_input.down_left,
  pilot_input.up_left, pilot_input.down, pilot_input.down_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up,
  pilot_input.left, pilot_input.right, pilot_input.up_left, pilot_input.down_right, pilot_input.up_left, pilot_input.up_right,
  pilot_input.down_right, pilot_input.center, pilot_input.down_left, pilot_input.up_right, pilot_input.down_right, pilot_input.up_right,
  pilot_input.up_left, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
  pilot_input.up_left, pilot_input.up_left, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_right, pilot_input.down_left, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
  pilot_input.up_left, pilot_input.down, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_right, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
  pilot_input.up_left, pilot_input.up_left, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_right, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
  pilot_input.up_left, pilot_input.up_left, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_right, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
  pilot_input.up_left, pilot_input.up_left, pilot_input.up_left, pilot_input.up_right, pilot_input.up_right, pilot_input.up_right,
  pilot_input.down_right, pilot_input.down_right, pilot_input.down_right, pilot_input.down_left, pilot_input.down_left, pilot_input.down_left,
}
local oracle_timeout_video_frames =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_TIMEOUT_VIDEO_FRAMES")) or 12000
local input_mode = os.getenv("SF1_MESEN_CORNERIA_INPUT") or "route"
local first_scene_game_frame =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_FIRST_SCENE")) or 315
local last_scene_game_frame =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_LAST_SCENE")) or 322
local poll_trace_limit =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_POLL_TRACE_LIMIT")) or 0
local capture_timeline = os.getenv("SF1_MESEN_CORNERIA_TIMELINE") ~= "0"
local capture_gsu_jobs = os.getenv("SF1_MESEN_CORNERIA_GSU_JOBS") == "1"
local capture_bus_trace = os.getenv("SF1_MESEN_CORNERIA_BUS_TRACE") == "1"
local capture_semantic = os.getenv("SF1_MESEN_CORNERIA_SEMANTIC") == "1"
local capture_random_calls =
  os.getenv("SF1_MESEN_CORNERIA_RANDOM_TRACE") == "1"
local capture_restart_writes =
  os.getenv("SF1_MESEN_CORNERIA_RESTART_TRACE") == "1"
local checkpoint_interval =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_CHECKPOINT_INTERVAL")) or 50
local gsu_instruction_trace_limit =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_GSU_INSTRUCTION_TRACE_LIMIT")) or 0
local gsu_instruction_trace_entry =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_GSU_INSTRUCTION_TRACE_ENTRY")) or 0x01B0CB
local runmario_trace_limit =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_RUNMARIO_TRACE_LIMIT")) or 0
local runmario_trace_start =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_RUNMARIO_TRACE_START")) or 440000

local game_frame_address = 0x15BB
local measured_motion_address = 0x14E3
local video_interrupt_count_address = 0x1200
local transfer_state_address = 0x0000
local transfer_counter_address = 0x18BB
local object_pool_base = 0x0336
local object_stride = 54
local object_count = 70
local active_object_head_address = 0x121D
local free_object_head_address = 0x121F
local current_background_address = 0x1741
local game_flags_address = 0x14D0
local player_ship_flags_address = 0x14D6
local player_strategy_flags_address = 0x14DD
local player_fly_mode_address = 0x14DA
local player_roll_address = 0x1234
local player_depth_shake_address = 0x1503
local player_depth_tilt_address = 0x1507
local player_object_address = 0x1238
local map_countdown_address = 0x16FB
local view_kind_address = 0x15CA
local player_view_x_address = 0x14F6
local player_view_y_address = 0x14F8
local player_view_z_address = 0x14FA
local view_float_x_address = 0x14E6
local view_float_y_address = 0x14E8
local view_shake_x_address = 0x1595
local view_position_x_address = 0x00C1
local view_position_y_address = 0x00C3
local view_position_z_address = 0x00C5
local view_pitch_address = 0x18C5
local view_yaw_address = 0x18C7
local effective_view_yaw_address = 0x1635
local view_distance_address = 0x18CB
local forward_velocity_address = 0x14F4
local previous_player_depth_address = 0x16FF
local last_depth_change_address = 0x1701
local presentation_bytes_address = 0x1551
local random_state_address = 0x00EF
-- Retail global locations re-derived from the Rev 2 cart. These differ from
-- the leaked build's allocation addresses and intentionally live only in the
-- oracle.
local which_friend_address = 0x189A
local message_count_address = 0x189D
local object_shape_offset = 0x04
local object_pointer_offset = 0x06
local object_flags_offset = 0x08
local object_world_x_offset = 0x0C
local object_world_y_offset = 0x0E
local object_world_z_offset = 0x10
local object_rotation_x_offset = 0x12
local object_rotation_y_offset = 0x13
local object_rotation_z_offset = 0x14
local object_speed_offset = 0x15
local object_collision_flags_offset = 0x1E
local object_damage_flags_offset = 0x1F
local object_hit_timer_offset = 0x22
local object_path_wait_offset = 0x24
local object_vertical_offset = 0x28
local object_durability_offset = 0x2A
local object_velocity_x_offset = 0x2F
local object_velocity_y_offset = 0x31
local object_velocity_z_offset = 0x33
local object_hit_flags_offset = 0x35

local cadence_reset_entry = 0x02D960
local horizontal_poll_entry = 0x02DCC5
local horizontal_position_sample = 0x02DCC8
local horizontal_safe_window_ready = 0x02DCDA
local horizontal_dma_complete = 0x02DD04
local horizontal_transfer_complete = 0x02D9EB
local motion_sample_complete = 0x02DA7E
local corneria_game_start_entry = 0x03C437
local random_wrapper_entry = 0x02FC58
local post_game_loop_message_update = 0x02E403
local timeline_markers = {
  { name = "transfer_slot_ready", address = 0x02D967 },
  { name = "transfer_started", address = 0x02D96E },
  { name = "circle_effect_complete", address = 0x02D971 },
  { name = "background_scroll_complete", address = 0x02D975 },
  { name = "vertical_offsets_complete", address = 0x02D978 },
  { name = "horizontal_offsets_begin", address = 0x02D9E5 },
  { name = "horizontal_offsets_complete", address = 0x02D9E8 },
  { name = "horizontal_transfer_complete", address = 0x02D9EB },
  { name = "window_priority_complete", address = 0x02D9EF },
  { name = "strategies_begin", address = 0x02DAF2 },
  { name = "strategies_complete", address = 0x02DA08 },
  { name = "pre_transfer_work_complete", address = 0x02DA3B },
  { name = "first_transfer_ready", address = 0x02DA40 },
  { name = "second_transfer_wait_begin", address = 0x02DA4C },
  { name = "second_transfer_ready", address = 0x02DA53 },
  { name = "scene_render_begin", address = 0x02DA65 },
  { name = "scene_render_complete", address = 0x02DA69 },
}

local active_measurement = nil
local completed_measurements = 0
local corneria_started = false
local corneria_start_count = 0
local active_gsu_job = nil
local last_gsu_running = nil
local pending_semantic_scene = nil
local write_output
local output_lines = {
  "input_mode scene_game_frame reset_game_frame sample_game_frame reset_video_frame sample_video_frame reset_master poll_begin_master safe_master dma_master transfer_complete_master sample_master reset_to_sample reset_cpu_cycle sample_cpu_cycle reset_to_sample_cpu_cycles safe_wait safe_to_transfer_complete polls video_interrupt_count measured_motion strategies_begin_master strategies_begin_video_frame strategies_begin_motion reset_transfer_state sample_transfer_state\n"
}

assert(input_mode == "route" or input_mode == "neutral",
  "SF1_MESEN_CORNERIA_INPUT must be route or neutral")
assert(first_scene_game_frame <= last_scene_game_frame,
  "Corneria timing scene range must be ordered")
assert(poll_trace_limit >= 0, "poll trace limit must not be negative")
assert(gsu_instruction_trace_limit >= 0,
  "GSU instruction trace limit must not be negative")
assert(runmario_trace_limit >= 0,
  "runmario instruction trace limit must not be negative")
assert(oracle_timeout_video_frames > 0,
  "oracle video-frame timeout must be positive")
assert(checkpoint_interval > 0,
  "oracle checkpoint interval must be positive")

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function signed_byte(value)
  if value >= 128 then return value - 256 end
  return value
end

local function signed_word(value)
  if value >= 32768 then return value - 65536 end
  return value
end

local function object_slot(pointer)
  assert(pointer >= object_pool_base, "object pointer precedes retail pool")
  local offset = pointer - object_pool_base
  assert(offset % object_stride == 0, "object pointer is not aligned")
  local slot = math.floor(offset / object_stride)
  assert(slot < object_count, "object pointer exceeds retail pool")
  return slot
end

local function object_order(head_address)
  local slots = {}
  local pointer = work_word(head_address)
  while pointer ~= 0 do
    slots[#slots + 1] = object_slot(pointer)
    assert(#slots <= object_count, "object list contains a cycle")
    pointer = work_word(pointer)
  end
  return slots
end

local function join_slots(slots)
  local values = {}
  for index, slot in ipairs(slots) do values[index] = tostring(slot) end
  return table.concat(values, ",")
end

local function record_semantic_state(scene_game_frame)
  if not capture_semantic then return end
  local active_order = object_order(active_object_head_address)
  local free_order = object_order(free_object_head_address)
  output_lines[#output_lines + 1] = string.format(
    "kind=semantic scene=%d background_source=%d game_flags=%d player_ship_flags=%d player_ship_flags_2=%d player_ship_flags_3=%d player_strategy_flags=%d player_fly_mode=%d player_object=%d map_countdown=%d view_kind=%d player_view_x=%d player_view_y=%d player_view_z=%d view_float_x=%d view_float_y=%d view_shake_x=%d view_shake_y=%d view_shake_z=%d view_position_x=%d view_position_y=%d view_position_z=%d view_pitch=%d view_yaw=%d effective_view_yaw=%d view_distance=%d forward_velocity=%d previous_player_depth=%d last_depth_change=%d player_hit_timer=%d player_hit_flags=%d player_body_durability=%d presentation_rotation=%d presentation_vertical=%d presentation_boost_delay=%d message_count=%d message_opening_frame=%d message_speaker=%d random_0=%d random_1=%d random_2=%d random_3=%d active_order=%s free_order=%s\n",
    scene_game_frame,
    work_word(current_background_address),
    work_byte(game_flags_address),
    work_byte(player_ship_flags_address),
    work_byte(player_ship_flags_address + 1),
    work_byte(player_ship_flags_address + 2),
    work_byte(player_strategy_flags_address),
    work_byte(player_fly_mode_address),
    object_slot(work_word(player_object_address)),
    work_word(map_countdown_address),
    work_byte(view_kind_address),
    signed_word(work_word(player_view_x_address)),
    signed_word(work_word(player_view_y_address)),
    signed_word(work_word(player_view_z_address)),
    signed_word(work_word(view_float_x_address)),
    signed_word(work_word(view_float_y_address)),
    signed_byte(work_byte(view_shake_x_address)),
    signed_byte(work_byte(view_shake_x_address + 1)),
    signed_byte(work_byte(view_shake_x_address + 2)),
    signed_word(work_word(view_position_x_address)),
    signed_word(work_word(view_position_y_address)),
    signed_word(work_word(view_position_z_address)),
    signed_word(work_word(view_pitch_address)),
    signed_word(work_word(view_yaw_address)),
    signed_word(work_word(effective_view_yaw_address)),
    signed_word(work_word(view_distance_address)),
    signed_word(work_word(forward_velocity_address)),
    signed_word(work_word(previous_player_depth_address)),
    signed_word(work_word(last_depth_change_address)),
    work_byte(object_pool_base + object_hit_timer_offset),
    work_byte(object_pool_base + object_hit_flags_offset),
    work_byte(object_pool_base + object_stride + object_durability_offset),
    work_byte(presentation_bytes_address),
    work_byte(presentation_bytes_address + 1),
    work_byte(presentation_bytes_address + 2),
    work_byte(message_count_address),
    work_byte(message_count_address + 1),
    work_byte(which_friend_address),
    work_byte(random_state_address),
    work_byte(random_state_address + 1),
    work_byte(random_state_address + 2),
    work_byte(random_state_address + 3),
    join_slots(active_order),
    join_slots(free_order))
  for _, slot in ipairs(active_order) do
    local base = object_pool_base + slot * object_stride
    output_lines[#output_lines + 1] = string.format(
      "kind=semantic_object scene=%d slot=%d shape_source=%d flags=%d x=%d y=%d z=%d durability=%d hit_flags=%d collision_flags=%d damage_flags=%d hit_timer=%d path_wait=%d rotation_x=%d rotation_y=%d rotation_z=%d speed=%d velocity_x=%d velocity_y=%d velocity_z=%d pointer=%d vertical_offset=%d\n",
      scene_game_frame,
      slot,
      work_word(base + object_shape_offset),
      work_word(base + object_flags_offset),
      signed_word(work_word(base + object_world_x_offset)),
      signed_word(work_word(base + object_world_y_offset)),
      signed_word(work_word(base + object_world_z_offset)),
      work_byte(base + object_durability_offset),
      work_byte(base + object_hit_flags_offset),
      work_byte(base + object_collision_flags_offset),
      work_byte(base + object_damage_flags_offset),
      work_byte(base + object_hit_timer_offset),
      work_byte(base + object_path_wait_offset),
      work_byte(base + object_rotation_x_offset),
      work_byte(base + object_rotation_y_offset),
      work_byte(base + object_rotation_z_offset),
      work_byte(base + object_speed_offset),
      signed_word(work_word(base + object_velocity_x_offset)),
      signed_word(work_word(base + object_velocity_y_offset)),
      signed_word(work_word(base + object_velocity_z_offset)),
      signed_word(work_word(base + object_pointer_offset)),
      signed_word(work_word(base + object_vertical_offset)))
  end
end

local function record_random_call()
  if not capture_random_calls or not corneria_started then return end
  local scene = work_word(game_frame_address)
  if scene < first_scene_game_frame - 1 or scene > last_scene_game_frame then
    return
  end
  local state = emu.getState()
  local stack_pointer = state["cpu.sp"] or state["cpu.s"] or 0x01FF
  local return_low = emu.read(
    (stack_pointer + 1) & 0xFFFF, emu.memType.snesMemory, false)
  local return_high = emu.read(
    (stack_pointer + 2) & 0xFFFF, emu.memType.snesMemory, false)
  local return_bank = emu.read(
    (stack_pointer + 3) & 0xFFFF, emu.memType.snesMemory, false)
  local return_address =
    (return_bank << 16) | (return_high << 8) | return_low
  output_lines[#output_lines + 1] = string.format(
    "kind=random_call game_frame=%d return_address=%06X stack_pointer=%04X random_0=%d random_1=%d random_2=%d random_3=%d message_count=%d message_opening_frame=%d\n",
    scene,
    return_address,
    stack_pointer,
    work_byte(random_state_address),
    work_byte(random_state_address + 1),
    work_byte(random_state_address + 2),
    work_byte(random_state_address + 3),
    work_byte(message_count_address),
    work_byte(message_count_address + 1))
end

local function record_post_game_loop_semantic()
  if pending_semantic_scene == nil then return end
  local scene = pending_semantic_scene
  pending_semantic_scene = nil
  if capture_restart_writes and scene >= 940 and scene <= 946 then
    local state = emu.getState()
    output_lines[#output_lines + 1] = string.format(
      "kind=restart_boundary scene=%d game_frame=%d source=%06X master=%d player_x=%d player_y=%d player_z=%d map_countdown=%d ship_flags=%d ship_flags_2=%d ship_flags_3=%d player_strategy_flags=%d player_fly_mode=%d\n",
      scene,
      work_word(game_frame_address),
      ((state["cpu.pbr"] or 0) << 16) | (state["cpu.pc"] or 0),
      state["memoryManager.masterClock"] or 0,
      signed_word(work_word(object_pool_base + object_world_x_offset)),
      signed_word(work_word(object_pool_base + object_world_y_offset)),
      signed_word(work_word(object_pool_base + object_world_z_offset)),
      work_word(map_countdown_address),
      work_byte(player_ship_flags_address),
      work_byte(player_ship_flags_address + 1),
      work_byte(player_ship_flags_address + 2),
      work_byte(player_strategy_flags_address),
      work_byte(player_fly_mode_address))
  end
  record_semantic_state(scene)
  if completed_measurements % checkpoint_interval == 0
      or scene == last_scene_game_frame then
    write_output()
  end
  if scene == last_scene_game_frame then
    assert(completed_measurements == last_scene_game_frame - first_scene_game_frame + 1,
      "did not capture every requested gameplay scene")
    emu.log("SF1_CORNERIA_TIMING_ORACLE_DONE")
    emu.stop(0)
  end
end

local function record_restart_write(address, value)
  if not capture_restart_writes or not corneria_started then return end
  local scene = work_word(game_frame_address)
  if scene < 940 or scene > 946 then return end
  local state = emu.getState()
  output_lines[#output_lines + 1] = string.format(
    "kind=restart_write game_frame=%d target=%04X value=%d source=%06X master=%d player_x=%d player_y=%d player_z=%d map_countdown=%d ship_flags=%d ship_flags_2=%d ship_flags_3=%d player_strategy_flags=%d player_fly_mode=%d player_roll=%d player_depth_shake=%d player_depth_tilt=%d\n",
    scene,
    address,
    value,
    ((state["cpu.pbr"] or 0) << 16) | (state["cpu.pc"] or 0),
    state["memoryManager.masterClock"] or 0,
    signed_word(work_word(object_pool_base + object_world_x_offset)),
    signed_word(work_word(object_pool_base + object_world_y_offset)),
    signed_word(work_word(object_pool_base + object_world_z_offset)),
    work_word(map_countdown_address),
    work_byte(player_ship_flags_address),
    work_byte(player_ship_flags_address + 1),
    work_byte(player_ship_flags_address + 2),
    work_byte(player_strategy_flags_address),
    work_byte(player_fly_mode_address),
    signed_word(work_word(player_roll_address)),
    signed_word(work_word(player_depth_shake_address)),
    signed_byte(work_byte(player_depth_tilt_address)))
end

local function master_clock()
  return emu.getState()["memoryManager.masterClock"] or 0
end

write_output = function()
  local file = assert(io.open(
    emu.getScriptDataFolder()
      .. "/sf1_corneria_timing_" .. input_mode .. ".txt", "wb"))
  file:write(table.concat(output_lines))
  file:close()
end

local function front_end_input(tick)
  if tick >= game_destination_select_tick
      and tick < game_destination_select_tick + front_end_confirm_hold_ticks then
    return { down = true }
  end
  if tick >= game_destination_confirm_tick
      and tick < game_destination_confirm_tick + front_end_confirm_hold_ticks then
    return { start = true }
  end
  if tick <= front_end_last_confirm_tick
      and tick % front_end_confirm_cadence_ticks < front_end_confirm_hold_ticks then
    return { start = true }
  end
  if tick >= route_selection_confirm_tick
      and tick < route_selection_confirm_tick + route_selection_confirm_hold_ticks then
    return { start = true }
  end
  if tick >= planet_dismiss_start_tick and tick < planet_dismiss_end_tick
      and (tick - planet_dismiss_start_tick) % planet_dismiss_cadence_ticks == 0 then
    return { b = true }
  end
  return {}
end

local function corneria_route_input(game_frame)
  local segment = math.floor(game_frame / input_segment_frames)
  return corneria_route_tape[segment + 1] or pilot_input.center
end

local function provide_input()
  if not corneria_started and corneria_start_count > 0
      and work_word(game_frame_address) == user_corneria_initial_game_frame then
    corneria_started = true
  end
  if not corneria_started then
    emu.setInput(
      front_end_input(math.floor(video_frame / video_frames_per_front_end_tick)),
      0)
    return
  end
  if input_mode == "neutral" then
    emu.setInput({}, 0)
    return
  end
  emu.setInput(corneria_route_input(work_word(game_frame_address)), 0)
end

local function reset_cadence()
  if not corneria_started then
    active_measurement = nil
    return
  end
  local game_frame = work_word(game_frame_address)
  if game_frame < first_scene_game_frame - 1
      or game_frame > last_scene_game_frame then
    active_measurement = nil
    return
  end
  active_measurement = {
    reset_game_frame = game_frame,
    reset_video_frame = video_frame,
    reset_master = master_clock(),
    reset_cpu_cycle = emu.getState()["cpu.cycleCount"] or 0,
    poll_begin_master = nil,
    safe_master = nil,
    dma_master = nil,
    transfer_complete_master = nil,
    polls = 0,
    poll_trace = {},
    last_horizontal_position = -1,
    runmario_trace_count = 0,
    strategies_begin_master = nil,
    strategies_begin_video_frame = nil,
    strategies_begin_motion = nil,
    reset_transfer_state = work_byte(transfer_state_address),
  }
  last_gsu_running = nil
end

local function arm_corneria_measurement()
  corneria_start_count = corneria_start_count + 1
  output_lines[#output_lines + 1] = string.format(
    "kind=corneria_start occurrence=%d video_frame=%d game_frame=%d\n",
    corneria_start_count,
    video_frame,
    work_word(game_frame_address))
  write_output()
end

local function timeline_callback(name)
  return function()
    if active_measurement == nil then return end
    if name == "strategies_begin"
        and active_measurement.strategies_begin_master == nil then
      active_measurement.strategies_begin_master = master_clock()
      active_measurement.strategies_begin_video_frame = video_frame
      active_measurement.strategies_begin_motion =
        work_byte(measured_motion_address)
    end
    if not capture_timeline then return end
    local state = emu.getState()
    output_lines[#output_lines + 1] = string.format(
      "kind=marker input_mode=%s scene_game_frame=%d name=%s elapsed_master=%d elapsed_cpu_cycles=%d\n",
      input_mode,
      active_measurement.reset_game_frame + 1,
      name,
      (state["memoryManager.masterClock"] or 0) - active_measurement.reset_master,
      (state["cpu.cycleCount"] or 0) - active_measurement.reset_cpu_cycle)
  end
end

local function track_gsu_job(address, opcode)
  if active_measurement == nil or not capture_gsu_jobs then
    active_gsu_job = nil
    return
  end
  local state = emu.getState()
  if active_gsu_job == nil then
    active_gsu_job = {
      entry_address = address,
      entry_master = state["memoryManager.masterClock"] or 0,
      entry_cycle = state["cart.coprocessor.cycleCount"] or 0,
      clock_select = state["cart.coprocessor.clockSelect"] and 1 or 0,
      high_speed_mode = state["cart.coprocessor.highSpeedMode"] and 1 or 0,
      steps = 0,
      instruction_trace = {},
    }
  end
  active_gsu_job.steps = active_gsu_job.steps + 1
  if active_gsu_job.entry_address == gsu_instruction_trace_entry
      and #active_gsu_job.instruction_trace < gsu_instruction_trace_limit then
    active_gsu_job.instruction_trace[#active_gsu_job.instruction_trace + 1] =
      string.format(
        "kind=gsu_instruction input_mode=%s scene_game_frame=%d sequence=%d address=%06X opcode=%02X elapsed_gsu_cycles=%d\n",
        input_mode,
        active_measurement.reset_game_frame + 1,
        active_gsu_job.steps,
        address,
        opcode,
        (state["cart.coprocessor.cycleCount"] or 0)
          - active_gsu_job.entry_cycle)
  end
  if opcode ~= 0 then return end
  local stop_master = state["memoryManager.masterClock"] or 0
  local stop_cycle = state["cart.coprocessor.cycleCount"] or 0
  output_lines[#output_lines + 1] = string.format(
    "kind=gsu_job input_mode=%s scene_game_frame=%d entry=%06X steps=%d elapsed_master=%d duration_master=%d duration_gsu_cycles=%d clock_select=%d high_speed_mode=%d\n",
    input_mode,
    active_measurement.reset_game_frame + 1,
    active_gsu_job.entry_address,
    active_gsu_job.steps,
    active_gsu_job.entry_master - active_measurement.reset_master,
    stop_master - active_gsu_job.entry_master,
    stop_cycle - active_gsu_job.entry_cycle,
    active_gsu_job.clock_select,
    active_gsu_job.high_speed_mode)
  for _, line in ipairs(active_gsu_job.instruction_trace) do
    output_lines[#output_lines + 1] = line
  end
  active_gsu_job = nil
end

local function record_video_interrupt_count(address, value)
  if active_measurement == nil or not capture_bus_trace or value == 0 then return end
  local state = emu.getState()
  output_lines[#output_lines + 1] = string.format(
    "kind=video_interrupt input_mode=%s scene_game_frame=%d count=%d elapsed_master=%d elapsed_cpu_cycles=%d source=%06X\n",
    input_mode,
    active_measurement.reset_game_frame + 1,
    value,
    (state["memoryManager.masterClock"] or 0) - active_measurement.reset_master,
    (state["cpu.cycleCount"] or 0) - active_measurement.reset_cpu_cycle,
    ((state["cpu.pbr"] or 0) << 16) | (state["cpu.pc"] or 0))
end

local function record_transfer_counter(address, value)
  if active_measurement == nil or not capture_bus_trace then return end
  local state = emu.getState()
  output_lines[#output_lines + 1] = string.format(
    "kind=transfer_counter input_mode=%s scene_game_frame=%d value=%d elapsed_master=%d elapsed_cpu_cycles=%d source=%06X\n",
    input_mode,
    active_measurement.reset_game_frame + 1,
    value,
    (state["memoryManager.masterClock"] or 0) - active_measurement.reset_master,
    (state["cpu.cycleCount"] or 0) - active_measurement.reset_cpu_cycle,
    ((state["cpu.pbr"] or 0) << 16) | (state["cpu.pc"] or 0))
end

local function record_interrupt_handler(address, opcode)
  if active_measurement == nil or not capture_bus_trace then return end
  if address ~= 0x000108 and address ~= 0x00010C and opcode ~= 0x40 then
    return
  end
  local state = emu.getState()
  output_lines[#output_lines + 1] = string.format(
    "kind=interrupt_handler input_mode=%s scene_game_frame=%d address=%06X opcode=%02X elapsed_master=%d elapsed_cpu_cycles=%d\n",
    input_mode,
    active_measurement.reset_game_frame + 1,
    address,
    opcode,
    (state["memoryManager.masterClock"] or 0) - active_measurement.reset_master,
    (state["cpu.cycleCount"] or 0) - active_measurement.reset_cpu_cycle)
end

local function record_gsu_status(address, value)
  if active_measurement == nil or not capture_bus_trace then return end
  local running = (value & 0x20) ~= 0
  if last_gsu_running == running then return end
  last_gsu_running = running
  local state = emu.getState()
  output_lines[#output_lines + 1] = string.format(
    "kind=gsu_status input_mode=%s scene_game_frame=%d running=%d value=%02X elapsed_master=%d elapsed_cpu_cycles=%d gsu_cycles=%d source=%06X\n",
    input_mode,
    active_measurement.reset_game_frame + 1,
    running and 1 or 0,
    value,
    (state["memoryManager.masterClock"] or 0) - active_measurement.reset_master,
    (state["cpu.cycleCount"] or 0) - active_measurement.reset_cpu_cycle,
    state["cart.coprocessor.cycleCount"] or 0,
    ((state["cpu.pbr"] or 0) << 16) | (state["cpu.pc"] or 0))
end

local function record_runmario_instruction(address, opcode)
  if active_measurement == nil
      or active_measurement.runmario_trace_count >= runmario_trace_limit then
    return
  end
  local elapsed = master_clock() - active_measurement.reset_master
  if elapsed < runmario_trace_start or elapsed > 860000 then return end
  active_measurement.runmario_trace_count =
    active_measurement.runmario_trace_count + 1
  local state = emu.getState()
  output_lines[#output_lines + 1] = string.format(
    "kind=runmario_instruction input_mode=%s scene_game_frame=%d sequence=%d address=%06X opcode=%02X elapsed_master=%d elapsed_cpu_cycles=%d gsu_cycles=%d\n",
    input_mode,
    active_measurement.reset_game_frame + 1,
    active_measurement.runmario_trace_count,
    address,
    opcode,
    elapsed,
    (state["cpu.cycleCount"] or 0) - active_measurement.reset_cpu_cycle,
    state["cart.coprocessor.cycleCount"] or 0)
end

local function begin_horizontal_poll()
  if active_measurement == nil then return end
  if active_measurement.poll_begin_master == nil then
    active_measurement.poll_begin_master = master_clock()
  end
  active_measurement.polls = active_measurement.polls + 1
end

local function sample_horizontal_position()
  if active_measurement == nil then return end
  local state = emu.getState()
  active_measurement.last_horizontal_position = (state["cpu.x"] or -1) & 0xFF
  if #active_measurement.poll_trace >= poll_trace_limit then return end
  local scanline = state["ppu.scanline"] or -1
  local cycle = state["ppu.cycle"] or -1
  active_measurement.poll_trace[#active_measurement.poll_trace + 1] = string.format(
    "kind=poll input_mode=%s reset_game_frame=%d sequence=%d master=%d scanline=%d cycle=%d horizontal_position=%d\n",
    input_mode,
    active_measurement.reset_game_frame,
    active_measurement.polls,
    state["memoryManager.masterClock"] or 0,
    scanline,
    cycle,
    active_measurement.last_horizontal_position)
end

local function mark_safe_window()
  if active_measurement == nil then return end
  active_measurement.safe_master = master_clock()
end

local function mark_dma_complete()
  if active_measurement == nil then return end
  active_measurement.dma_master = master_clock()
end

local function mark_horizontal_transfer_complete()
  if active_measurement == nil then return end
  active_measurement.transfer_complete_master = master_clock()
end

local function complete_motion_sample()
  if active_measurement == nil then return end
  local scene_game_frame = work_word(game_frame_address)
  if scene_game_frame < first_scene_game_frame
      or scene_game_frame > last_scene_game_frame then
    active_measurement = nil
    return
  end
  assert(active_measurement.poll_begin_master ~= nil,
    "motion sample completed without horizontal polling")
  assert(active_measurement.safe_master ~= nil,
    "motion sample completed without reaching the horizontal safe window")
  assert(active_measurement.dma_master ~= nil,
    "motion sample completed without horizontal DMA completion")
  assert(active_measurement.transfer_complete_master ~= nil,
    "motion sample completed without horizontal transfer completion")
  assert(active_measurement.strategies_begin_master ~= nil,
    "motion sample completed without a strategy-update boundary")
  assert(active_measurement.strategies_begin_video_frame ~= nil,
    "motion sample completed without a strategy-update video frame")
  assert(active_measurement.strategies_begin_motion ~= nil,
    "motion sample completed without a strategy-update motion value")

  local sample_master = master_clock()
  local sample_cpu_cycle = emu.getState()["cpu.cycleCount"] or 0
  for _, line in ipairs(active_measurement.poll_trace) do
    output_lines[#output_lines + 1] = line
  end
  output_lines[#output_lines + 1] = string.format(
    "%s %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n",
    input_mode,
    scene_game_frame,
    active_measurement.reset_game_frame,
    scene_game_frame,
    active_measurement.reset_video_frame,
    video_frame,
    active_measurement.reset_master,
    active_measurement.poll_begin_master,
    active_measurement.safe_master,
    active_measurement.dma_master,
    active_measurement.transfer_complete_master,
    sample_master,
    sample_master - active_measurement.reset_master,
    active_measurement.reset_cpu_cycle,
    sample_cpu_cycle,
    sample_cpu_cycle - active_measurement.reset_cpu_cycle,
    active_measurement.safe_master - active_measurement.poll_begin_master,
    active_measurement.transfer_complete_master - active_measurement.safe_master,
    active_measurement.polls,
    work_byte(video_interrupt_count_address),
    work_byte(measured_motion_address),
    active_measurement.strategies_begin_master,
    active_measurement.strategies_begin_video_frame,
    active_measurement.strategies_begin_motion,
    active_measurement.reset_transfer_state,
    work_byte(transfer_state_address))
  if capture_semantic then
    assert(pending_semantic_scene == nil,
      "previous semantic scene did not reach the post-message boundary")
    pending_semantic_scene = scene_game_frame
  end
  completed_measurements = completed_measurements + 1
  active_measurement = nil
  if not capture_semantic and completed_measurements % checkpoint_interval == 0 then
    write_output()
  end

  if scene_game_frame == last_scene_game_frame and not capture_semantic then
    assert(completed_measurements == last_scene_game_frame - first_scene_game_frame + 1,
      "did not capture every requested gameplay scene")
    write_output()
    emu.log("SF1_CORNERIA_TIMING_ORACLE_DONE")
    emu.stop(0)
  end
end

local function end_frame()
  video_frame = video_frame + 1
  if video_frame >= oracle_timeout_video_frames then
    emu.log("SF1_CORNERIA_TIMING_ORACLE_TIMEOUT")
    emu.stop(2)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(
  arm_corneria_measurement,
  emu.callbackType.exec,
  corneria_game_start_entry,
  corneria_game_start_entry,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  reset_cadence,
  emu.callbackType.exec,
  cadence_reset_entry,
  cadence_reset_entry,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  begin_horizontal_poll,
  emu.callbackType.exec,
  horizontal_poll_entry,
  horizontal_poll_entry,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  sample_horizontal_position,
  emu.callbackType.exec,
  horizontal_position_sample,
  horizontal_position_sample,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  mark_safe_window,
  emu.callbackType.exec,
  horizontal_safe_window_ready,
  horizontal_safe_window_ready,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  mark_dma_complete,
  emu.callbackType.exec,
  horizontal_dma_complete,
  horizontal_dma_complete,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  mark_horizontal_transfer_complete,
  emu.callbackType.exec,
  horizontal_transfer_complete,
  horizontal_transfer_complete,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  complete_motion_sample,
  emu.callbackType.exec,
  motion_sample_complete,
  motion_sample_complete,
  emu.cpuType.snes,
  emu.memType.snesMemory)
for _, marker in ipairs(timeline_markers) do
  emu.addMemoryCallback(
    timeline_callback(marker.name),
    emu.callbackType.exec,
    marker.address,
    marker.address,
    emu.cpuType.snes,
    emu.memType.snesMemory)
end
emu.addMemoryCallback(
  track_gsu_job,
  emu.callbackType.exec,
  0,
  0x7FFFFF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  record_video_interrupt_count,
  emu.callbackType.write,
  video_interrupt_count_address,
  video_interrupt_count_address,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_transfer_counter,
  emu.callbackType.write,
  transfer_counter_address,
  transfer_counter_address,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_interrupt_handler,
  emu.callbackType.exec,
  0x000100,
  0x000180,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_gsu_status,
  emu.callbackType.read,
  0x003030,
  0x003030,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_runmario_instruction,
  emu.callbackType.exec,
  0x7E4EE9,
  0x7E4F20,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_random_call,
  emu.callbackType.exec,
  random_wrapper_entry,
  random_wrapper_entry,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_post_game_loop_semantic,
  emu.callbackType.exec,
  post_game_loop_message_update,
  post_game_loop_message_update,
  emu.cpuType.snes,
  emu.memType.snesMemory)
for _, watched_range in ipairs({
  { object_pool_base + object_pointer_offset,
    object_pool_base + object_flags_offset + 1 },
  { object_pool_base + object_world_x_offset,
    object_pool_base + object_world_z_offset + 1 },
  { object_pool_base + object_rotation_z_offset,
    object_pool_base + object_rotation_z_offset },
  { player_ship_flags_address, player_ship_flags_address + 2 },
  { player_fly_mode_address, player_fly_mode_address },
  { player_strategy_flags_address, player_strategy_flags_address },
  { player_roll_address, player_roll_address + 1 },
  { player_depth_shake_address, player_depth_shake_address + 1 },
  { player_depth_tilt_address, player_depth_tilt_address },
  { map_countdown_address, map_countdown_address + 1 },
}) do
  emu.addMemoryCallback(
    record_restart_write,
    emu.callbackType.write,
    watched_range[1],
    watched_range[2],
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
emu.log("SF1_CORNERIA_TIMING_ORACLE_LOADED")
