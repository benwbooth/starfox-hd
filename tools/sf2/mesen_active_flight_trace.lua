-- Clean-room input/pose trace for the first controllable retail sortie.
-- Address-shaped access is deliberately confined to this oracle script. The
-- shipping Rust port consumes only the resulting typed movement and weapon
-- behavior.

local frame = 0
local armed = false
local armed_frame = -1
local capture_oam_sequence = os.getenv("SF2_ORACLE_CAPTURE_OAM_SEQUENCE") == "1"
local capture_hud_sequence = os.getenv("SF2_ORACLE_CAPTURE_HUD_SEQUENCE") == "1"
local capture_target_auxiliary = os.getenv("SF2_ORACLE_CAPTURE_TARGET_AUXILIARY") == "1"
local capture_player_motion_writes = os.getenv("SF2_ORACLE_CAPTURE_PLAYER_MOTION_WRITES") == "1"
local capture_fighter_logic = os.getenv("SF2_ORACLE_CAPTURE_FIGHTER_LOGIC") == "1"
local capture_mission_transition = os.getenv("SF2_ORACLE_CAPTURE_MISSION_TRANSITION") == "1"
local capture_target_display = os.getenv("SF2_ORACLE_CAPTURE_TARGET_DISPLAY") == "1"
local capture_craft_forms = os.getenv("SF2_ORACLE_CAPTURE_CRAFT_FORMS") == "1"
local requested_stop_elapsed = tonumber(os.getenv("SF2_ORACLE_STOP_ELAPSED"))
local stop_elapsed = capture_oam_sequence and (requested_stop_elapsed or 7520)
  or capture_hud_sequence and 7400
  or requested_stop_elapsed
  or 7560
local lines = {}
local collision_lines = {}
local player_motion_lines = {}
local fighter_logic_lines = {}
local mission_transition_lines = {}
local target_display_lines = {}
local craft_form_lines = {}
local last_craft_shapes = {}
local fighter_logic_objects = {
  [0x0633] = true,
  [0x05F4] = true,
  [0x05B5] = true,
  [0x0576] = true,
}
local current_input = "neutral"
local input_profile = os.getenv("SF2_ORACLE_INPUT_PROFILE") or "control_sweep"
local forced_charge_threshold = tonumber(os.getenv("SF2_ORACLE_CHARGE_THRESHOLD"))
local laser_release_elapsed = tonumber(os.getenv("SF2_ORACLE_LASER_RELEASE_ELAPSED")) or 6900
local main_layer_mask_text = os.getenv("SF2_ORACLE_MAIN_LAYER_MASK")
local main_layer_mask = main_layer_mask_text and tonumber(main_layer_mask_text) or nil
local trace_filename = input_profile == "laser_hold"
  and "sf2_active_flight_laser_hold_trace.txt"
  or input_profile == "laser_release"
    and "sf2_active_flight_laser_release_trace.txt"
    or "sf2_active_flight_trace.txt"
local screenshot_frames = {
  [7270] = true,
  [7290] = true,
  [7310] = true,
}
local timer_probe_frames = {
  [7258] = true,
  [7259] = true,
  [7260] = true,
  [7372] = true,
  [7373] = true,
  [7374] = true,
}
local mission_transition_probe_frames = {
  [14000] = true,
  [14400] = true,
  [14460] = true,
  [14465] = true,
  [14466] = true,
  [14468] = true,
  [14500] = true,
  [15000] = true,
}
if main_layer_mask then
  for elapsed = 7200, 7400, 10 do
    screenshot_frames[elapsed] = true
  end
end
if capture_hud_sequence then
  for elapsed = 7100, 7400 do
    screenshot_frames[elapsed] = true
  end
end
if stop_elapsed > 7560 then
  for elapsed = 8000, stop_elapsed, 500 do
    screenshot_frames[elapsed] = true
  end
end

local function script_path(filename)
  return emu.getScriptDataFolder() .. "/" .. filename
end

local function write_binary(filename, contents)
  local file = assert(io.open(script_path(filename), "w+b"))
  file:write(contents)
  file:close()
end

local function dump_memory(filename, kind, length)
  local contents = {}
  for address = 0, length - 1 do
    contents[#contents + 1] = string.char(emu.read(address, kind, false))
  end
  write_binary(filename, table.concat(contents))
end

local function dump_bus_range(filename, start_address, length)
  local contents = {}
  for offset = 0, length - 1 do
    contents[#contents + 1] = string.char(
      emu.read(start_address + offset, emu.memType.snesMemory, false))
  end
  write_binary(filename, table.concat(contents))
end

local function capture_ppu_state(elapsed)
  dump_memory(
    string.format("sf2_active_flight_%04d_vram.bin", elapsed),
    emu.memType.snesVideoRam,
    0x10000)
  dump_memory(
    string.format("sf2_active_flight_%04d_cgram.bin", elapsed),
    emu.memType.snesCgRam,
    0x200)
  dump_memory(
    string.format("sf2_active_flight_%04d_oam.bin", elapsed),
    emu.memType.snesSpriteRam,
    544)
  local state = emu.getState()
  local keys = {}
  for key, _ in pairs(state) do
    local lower = string.lower(key)
    if string.find(lower, "ppu", 1, true)
      or string.find(lower, "bg", 1, true)
      or string.find(lower, "screen", 1, true)
      or string.find(lower, "brightness", 1, true) then
      keys[#keys + 1] = key
    end
  end
  table.sort(keys)
  local lines = {}
  for _, key in ipairs(keys) do
    lines[#lines + 1] = key .. "=" .. tostring(state[key]) .. "\n"
  end
  write_binary(
    string.format("sf2_active_flight_%04d_ppu_state.txt", elapsed),
    table.concat(lines))
end

local function capture_screen(elapsed)
  local size = emu.getScreenSize()
  local screen = emu.getScreenBuffer()
  local output = { string.format("P6\n%d %d\n255\n", size.width, size.height) }
  for index = 1, size.width * size.height do
    local pixel = screen[index] or 0
    output[#output + 1] = string.char(
      (pixel >> 16) & 0xFF,
      (pixel >> 8) & 0xFF,
      pixel & 0xFF)
  end
  write_binary(string.format("sf2_active_flight_%04d.ppm", elapsed), table.concat(output))
end

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function work_long(address)
  return work_word(address) | (work_byte(address + 2) << 16)
end

local function signed_word(address)
  local value = work_word(address)
  if value >= 0x8000 then return value - 0x10000 end
  return value
end

local function bytes_hex(address, count)
  local output = {}
  for offset = 0, count - 1 do
    output[#output + 1] = string.format("%02X", work_byte(address + offset))
  end
  return table.concat(output)
end

local function pose(address)
  return string.format(
    "%d,%d,%d,%d,%d,%d,%d",
    signed_word(address + 12),
    signed_word(address + 14),
    signed_word(address + 16),
    work_byte(address + 18),
    work_byte(address + 20),
    work_byte(address + 22),
    work_byte(address + 24))
end

local function active_objects()
  local output = {}
  local seen = {}
  local object = work_word(0x12A8)
  while object ~= 0 and not seen[object] and #output < 60 do
    seen[object] = true
    output[#output + 1] = string.format(
      "%04X,%04X,%d,%d,%d,%d,%d,%d,%d,%06X,%04X,%04X,%d,%d,%d,%d,%d,%d,%d,%d",
      object,
      work_word(object + 4),
      signed_word(object + 12),
      signed_word(object + 14),
      signed_word(object + 16),
      work_byte(object + 18),
      work_byte(object + 20),
      work_byte(object + 22),
      work_byte(object + 24),
      work_long(object + 25),
      work_word(object + 0x2B),
      work_word(object + 0x1CCD),
      signed_word(object + 0x32),
      signed_word(object + 0x34),
      signed_word(object + 0x36),
      work_byte(object + 0x2D),
      work_byte(object + 0x2E),
      work_byte(object + 0x2F),
      work_byte(object + 0x30),
      work_byte(object + 0x31))
    object = work_word(object)
  end
  return table.concat(output, ";")
end

local function record(elapsed)
  local player = work_word(0x12C3)
  local wingmate = work_word(0x12C5)
  local slot = work_word(player + 0x2B)
  local target_slot = work_word(0x05B5 + 0x2B)
  local target_trigger_list = work_word(0x05B5 + 0x1CE0)
  local target_auxiliary = capture_target_auxiliary
    and bytes_hex(0x6A61 + target_slot, 0x1D8)
    or "-"
  local target_extension = capture_target_auxiliary
    and bytes_hex(0x05B5 + 0x1CC1, 0x3F)
    or "-"
  local target_object = capture_target_auxiliary
    and bytes_hex(0x05B5, 0x3F)
    or "-"
  local target_triggers = capture_target_auxiliary and target_trigger_list ~= 0
    and bytes_hex(0x6A61 + target_trigger_list, 0x40)
    or "-"
  lines[#lines + 1] = string.format(
    "elapsed=%d input=%s pad=%04X mode=%d phase=%d player=%04X wingmate=%04X slot=%04X " ..
      "camera=%d,%d,%d,%d,%d,%d pose=%s wingpose=%s active=[%s] object=%s extension=%s auxiliary=%s targetaux=%s targetextension=%s targetobject=%s targettriggers=%s rng=%s relativemotion=%d,%d",
    elapsed,
    current_input,
    work_word(0x1936),
    work_byte(0x1B68),
    work_byte(0x1BE0),
    player,
    wingmate,
    slot,
    signed_word(0x034B),
    signed_word(0x034D),
    signed_word(0x034F),
    work_byte(0x0351),
    work_byte(0x0353),
    work_byte(0x0355),
    pose(player),
    pose(wingmate),
    active_objects(),
    bytes_hex(player, 0x3F),
    bytes_hex(player + 0x1CC1, 0x3F),
    bytes_hex(0x6A61 + slot, 0x1D8),
    target_auxiliary,
    target_extension,
    target_object,
    target_triggers,
    bytes_hex(0x00E0, 4),
    signed_word(0x1E1C),
    signed_word(0x1E20))
end

local function pulse(value, period, offset)
  local phase = value % period
  return phase == offset or phase == offset + 1
end

local function active_input(elapsed)
  if input_profile == "laser_hold" then
    current_input = "b"
    return { b = true }
  end
  if input_profile == "laser_release" then
    if elapsed < laser_release_elapsed then
      current_input = "b"
      return { b = true }
    end
    current_input = "neutral"
    return {}
  end
  if input_profile == "laser_left" then
    current_input = "b+left"
    return { b = true, left = true }
  end
  if input_profile == "laser_right" then
    current_input = "b+right"
    return { b = true, right = true }
  end
  if input_profile == "laser_up" then
    current_input = "b+up"
    return { b = true, up = true }
  end
  if input_profile == "laser_down" then
    current_input = "b+down"
    return { b = true, down = true }
  end
  if elapsed < 6880 then
    current_input = "neutral"
    return {}
  elseif elapsed < 6960 then
    current_input = "left"
    return { left = true }
  elseif elapsed < 7040 then
    current_input = "right"
    return { right = true }
  elseif elapsed < 7120 then
    current_input = "up"
    return { up = true }
  elseif elapsed < 7200 then
    current_input = "down"
    return { down = true }
  elseif elapsed < 7280 then
    current_input = "y"
    return { y = pulse(elapsed, 16, 0) }
  elseif elapsed < 7360 then
    current_input = "b"
    return { b = true }
  elseif elapsed < 7440 then
    current_input = "a"
    return { a = pulse(elapsed, 16, 0) }
  end
  current_input = "x"
  return { x = pulse(elapsed, 16, 0) }
end

local function provide_input()
  if not armed then
    emu.setInput({ start = pulse(frame, 180, 120) }, 0)
    return
  end
  local elapsed = frame - armed_frame
  if elapsed >= 6800 then
    emu.setInput(active_input(elapsed), 0)
    return
  end
  current_input = "front_end"
  emu.setInput({
    start = pulse(frame, 180, 120) and elapsed <= 600,
    b = elapsed >= 210 and elapsed < 6450 and pulse(elapsed, 90, 30),
    up = elapsed >= 6000 and elapsed < 6045,
    right = elapsed >= 6045 and elapsed < 6070,
  }, 0)
end

local function arm_for_target_stream()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame
  end
end

local function isolate_main_layer()
  if not armed or not main_layer_mask then return end
  local visible_frame = frame - armed_frame + 1
  if visible_frame < 7200 or visible_frame > 7400 then return end
  -- startFrame fires after vblank register setup and immediately before the
  -- visible raster, making this an oracle-only layer isolation point.
  emu.write(0x212C, main_layer_mask, emu.memType.snesMemory)
  emu.write(0x212D, 0, emu.memType.snesMemory)
  emu.write(0x2131, 0, emu.memType.snesMemory)
end

local function end_frame()
  frame = frame + 1
  if not armed then return end
  local elapsed = frame - armed_frame
  if forced_charge_threshold and elapsed >= 6700 then
    -- Oracle-only threshold substitution lets one deterministic flight replay
    -- measure all three values from the retail pilot table without changing
    -- shipping state or bypassing the retail weapon routine.
    emu.write(0x1DD6, forced_charge_threshold, emu.memType.snesWorkRam)
  end
  if elapsed >= 6740 and elapsed <= stop_elapsed then
    record(elapsed)
    if capture_target_display then
      target_display_lines[#target_display_lines + 1] = string.format(
        "elapsed=%d digits=%d,%d,%d timer=%d,%d,%d selected=%04X",
        elapsed,
        work_byte(0xE961),
        work_byte(0xE965),
        work_byte(0xE871),
        work_word(0x1C0A),
        work_word(0xDA63),
        work_word(0xDA61),
        work_word(0x12C1))
    end
    if capture_oam_sequence and elapsed >= 7100 then
      dump_memory(
        string.format("sf2_active_flight_%04d_oam.bin", elapsed),
        emu.memType.snesSpriteRam,
        544)
      if timer_probe_frames[elapsed] then
        dump_memory(
          string.format("sf2_active_flight_%04d_work.bin", elapsed),
          emu.memType.snesWorkRam,
          0x20000)
      end
    end
    if screenshot_frames[elapsed] then
      capture_screen(elapsed)
      if elapsed == 7290 or (main_layer_mask and not capture_hud_sequence) then
        capture_ppu_state(elapsed)
      end
    end
    if capture_mission_transition and mission_transition_probe_frames[elapsed] then
      dump_memory(
        string.format("sf2_active_flight_%04d_work.bin", elapsed),
        emu.memType.snesWorkRam,
        0x20000)
    end
  end
  if elapsed >= stop_elapsed then
    write_binary(trace_filename, table.concat(lines, "\n") .. "\n")
    if capture_target_auxiliary then
      write_binary("sf2_active_flight_collision_writes.txt", table.concat(collision_lines, "\n") .. "\n")
      dump_memory("sf2_active_flight_collision_work.bin", emu.memType.snesWorkRam, 0x20000)
    end
    if capture_player_motion_writes then
      write_binary("sf2_active_flight_player_motion_writes.txt", table.concat(player_motion_lines, "\n") .. "\n")
    end
    if capture_fighter_logic then
      write_binary("sf2_active_flight_fighter_logic.txt", table.concat(fighter_logic_lines, "\n") .. "\n")
      dump_bus_range("sf2_active_flight_fighter_pitch_wave.bin", 0x008E66, 256)
    end
    if capture_mission_transition then
      write_binary(
        "sf2_active_flight_mission_transition_writes.txt",
        table.concat(mission_transition_lines, "\n") .. "\n")
    end
    if capture_target_display then
      write_binary(
        "sf2_active_flight_target_display.txt",
        table.concat(target_display_lines, "\n") .. "\n")
    end
    if capture_craft_forms then
      write_binary(
        "sf2_active_flight_craft_forms.txt",
        table.concat(craft_form_lines, "\n") .. "\n")
    end
    emu.stop(0)
  end
end

local function record_craft_form_service(service)
  if not capture_craft_forms then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  craft_form_lines[#craft_form_lines + 1] = string.format(
    "frame=%d elapsed=%d event=service service=%s object=%04X shape=%04X primary=%04X wingmate=%04X selected=%d alternate=%d source=%02X:%04X",
    frame,
    armed and frame - armed_frame or -1,
    service,
    object,
    work_word(object + 4),
    work_word(0x12C3),
    work_word(0x12C5),
    work_word(0x1E14) & 7,
    work_word(0x1E70) & 7,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function craft_form_service(service)
  return function() record_craft_form_service(service) end
end

local function record_player_shape_write(source, address, value)
  if not capture_craft_forms then return end
  local primary = work_word(0x12C3)
  local wingmate = work_word(0x12C5)
  local primary_shape = primary + 4
  local wingmate_shape = wingmate + 4
  local local_address = address & 0xFFFF
  if local_address ~= ((primary_shape + 1) & 0xFFFF)
    and local_address ~= ((wingmate_shape + 1) & 0xFFFF) then return end
  local object = local_address == ((primary_shape + 1) & 0xFFFF) and primary or wingmate
  local shape = work_byte(object + 4) | ((value & 0xFF) << 8)
  if last_craft_shapes[object] == shape then return end
  last_craft_shapes[object] = shape
  local state = emu.getState()
  craft_form_lines[#craft_form_lines + 1] = string.format(
    "frame=%d elapsed=%d event=shape-write source=%s address=%04X value=%d written_shape=%04X primary=%04X primary_shape=%04X wingmate=%04X wingmate_shape=%04X caller=%02X:%04X",
    frame,
    armed and frame - armed_frame or -1,
    source,
    address,
    value,
    shape,
    primary,
    work_word(primary_shape),
    wingmate,
    work_word(wingmate_shape),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function record_main_player_shape_write(address, value)
  record_player_shape_write("main", address, value)
end

local function record_target_display_write(address, value)
  if not capture_target_display or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 6700 then return end
  local state = emu.getState()
  local stack = {}
  local stack_pointer = state["cpu.s"] or state["cpu.sp"] or 0x01FF
  for offset = 1, 16 do
    stack[#stack + 1] = string.format(
      "%02X",
      emu.read(
        (stack_pointer + offset) & 0xFFFF,
        emu.memType.snesMemory,
        false))
  end
  target_display_lines[#target_display_lines + 1] = string.format(
    "elapsed=%d write=%04X value=%d source=%02X:%04X stack_pointer=%04X stack=%s selected=%04X",
    elapsed,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    stack_pointer,
    table.concat(stack),
    work_word(0x12C1))
end

local function record_target_display_print()
  if not capture_target_display or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 6700 then return end
  local state = emu.getState()
  local stack = {}
  local stack_pointer = state["cpu.s"] or state["cpu.sp"] or 0x01FF
  for offset = 1, 12 do
    stack[#stack + 1] = string.format(
      "%02X",
      emu.read(
        (stack_pointer + offset) & 0xFFFF,
        emu.memType.snesMemory,
        false))
  end
  target_display_lines[#target_display_lines + 1] = string.format(
    "elapsed=%d print_y=%04X value=%d count=%d stack=%s",
    elapsed,
    state["cpu.y"] or 0,
    work_word(0x0004),
    work_word(0x0008),
    table.concat(stack))
end

local function record_mission_transition_write(address, value)
  if not capture_mission_transition or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14000 then return end
  local state = emu.getState()
  mission_transition_lines[#mission_transition_lines + 1] = string.format(
    "elapsed=%d address=%04X value=%d main=%02X:%04X coprocessor=%02X:%04X",
    elapsed,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0)
end

local function record_target_behavior_write(source, address, value)
  if not capture_target_auxiliary or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 7880 then return end
  local state = emu.getState()
  collision_lines[#collision_lines + 1] = string.format(
    "elapsed=%d source=%s value=%d main=%02X:%04X coprocessor=%02X:%04X current=%04X selected=%04X",
    elapsed,
    source,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    work_word(0x12C1),
    work_word(0x12C3))
end

local function record_gsu_target_behavior_write(address, value)
  record_target_behavior_write("coprocessor-work", address, value)
end

local function record_main_target_behavior_write(address, value)
  record_target_behavior_write("main-work", address, value)
end

local function record_player_motion_write(source, address, value)
  if not capture_player_motion_writes or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 6790 then return end
  local state = emu.getState()
  player_motion_lines[#player_motion_lines + 1] = string.format(
    "elapsed=%d source=%s address=%04X value=%d main=%02X:%04X coprocessor=%02X:%04X pose=%s velocity=%d,%d,%d",
    elapsed,
    source,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    pose(0x03BD),
    signed_word(0x03EF),
    signed_word(0x03F1),
    signed_word(0x03F3))
end

local function record_gsu_player_motion_write(address, value)
  record_player_motion_write("coprocessor-work", address, value)
end

local function record_main_player_motion_write(address, value)
  record_player_motion_write("main-work", address, value)
end

local function record_fighter_logic(event)
  if not capture_fighter_logic or not armed then return end
  local state = emu.getState()
  local fighter = state["cpu.x"] or 0
  if not fighter_logic_objects[fighter] then return end
  local trigger_list = work_word(fighter + 0x1CE0)
  local elapsed = frame - armed_frame
  fighter_logic_lines[#fighter_logic_lines + 1] = string.format(
    "elapsed=%d event=%s object=%04X path=%04X pose=%s velocity=%d,%d,%d rng=%s relativemotion=%d,%d base=%s extension=%s selected=%04X selectedpose=%s triggers=%s",
    elapsed,
    event,
    fighter,
    work_word(fighter + 0x2B),
    pose(fighter),
    signed_word(fighter + 0x32),
    signed_word(fighter + 0x34),
    signed_word(fighter + 0x36),
    bytes_hex(0x00E0, 4),
    signed_word(0x1E1C),
    signed_word(0x1E20),
    bytes_hex(fighter, 0x39),
    bytes_hex(fighter + 0x1CC1, 0x3F),
    work_word(0xCF1F),
    pose(work_word(0xCF1F)),
    trigger_list ~= 0 and bytes_hex(0x6A61 + trigger_list, 0x40) or "-")
end

local function record_fighter_move()
  record_fighter_logic("move")
end

local function record_fighter_random_branch()
  record_fighter_logic("random-branch")
end

local function record_fighter_wait_for_angle()
  record_fighter_logic("wait-for-angle")
end

local function record_fighter_wait()
  record_fighter_logic("wait")
end

local function record_fighter_random_value()
  record_fighter_logic("random-value")
end

local function record_fighter_chase_angle()
  record_fighter_logic("chase-angle")
end

local function record_fighter_divide_angle()
  record_fighter_logic("divide-angle")
end

local function record_fighter_schedule()
  record_fighter_logic("schedule")
end

local function record_fighter_face_player()
  record_fighter_logic("face-player")
end

local function record_fighter_fire()
  record_fighter_logic("fire")
end

local function record_fighter_vertical_step()
  record_fighter_logic("vertical-step")
end

local function capital_object_for_state_address(address)
  if (address >= 0x0600 and address <= 0x0605)
    or (address >= 0x0626 and address <= 0x062B) then
    return 0x05F4
  end
  if (address >= 0x063F and address <= 0x0644)
    or (address >= 0x0665 and address <= 0x066A) then
    return 0x0633
  end
  return nil
end

local function record_capital_state_write(source, address, value)
  if not capture_fighter_logic or not armed then return end
  local object = capital_object_for_state_address(address)
  if not object then return end
  local state = emu.getState()
  fighter_logic_lines[#fighter_logic_lines + 1] = string.format(
    "elapsed=%d event=capital-state-write object=%04X source=%s address=%04X value=%d main=%02X:%04X coprocessor=%02X:%04X pose=%s velocity=%d,%d,%d",
    frame - armed_frame,
    object,
    source,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    pose(object),
    signed_word(object + 0x32),
    signed_word(object + 0x34),
    signed_word(object + 0x36))
end

local function record_main_capital_state_write(address, value)
  record_capital_state_write("main-work", address, value)
end

local function record_gsu_capital_state_write(address, value)
  record_capital_state_write("coprocessor-work", address, value)
end

local function record_fighter_pitch_target_write(address, value)
  if not capture_fighter_logic or not armed then return end
  local state = emu.getState()
  local fighter = state["cpu.x"] or 0
  if not fighter_logic_objects[fighter] then return end
  fighter_logic_lines[#fighter_logic_lines + 1] = string.format(
    "elapsed=%d event=pitch-target-write object=%04X value=%d source=%02X:%04X",
    frame - armed_frame,
    fighter,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function record_fighter_position_y_write(address, value)
  if not capture_fighter_logic or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 6988 then return end
  local state = emu.getState()
  local fighter = state["cpu.x"] or 0
  if not fighter_logic_objects[fighter] then return end
  fighter_logic_lines[#fighter_logic_lines + 1] = string.format(
    "elapsed=%d event=position-y-write object=%04X address=%04X value=%d source=%02X:%04X pose=%s",
    elapsed,
    fighter,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    pose(fighter))
end

local function record_fighter_random_state_write(source, address, value)
  if not capture_fighter_logic or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 8200 then return end
  local state = emu.getState()
  fighter_logic_lines[#fighter_logic_lines + 1] = string.format(
    "elapsed=%d event=random-state-write source=%s address=%04X value=%d main=%02X:%04X coprocessor=%02X:%04X rng=%s",
    elapsed,
    source,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    bytes_hex(0x00E0, 4))
end

local function record_main_fighter_random_state_write(address, value)
  record_fighter_random_state_write("main-work", address, value)
end

local function record_gsu_fighter_random_state_write(address, value)
  record_fighter_random_state_write("coprocessor-work", address, value)
end

emu.addMemoryCallback(
  arm_for_target_stream,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  record_target_display_print,
  emu.callbackType.exec,
  0x049D79,
  0x049D79,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_gsu_target_behavior_write,
  emu.callbackType.write,
  0x05D5,
  0x05D5,
  emu.cpuType.gsu,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_main_target_behavior_write,
  emu.callbackType.write,
  0x05D5,
  0x05D5,
  emu.cpuType.snes,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_main_target_behavior_write,
  emu.callbackType.write,
  0x05D5,
  0x05D5,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_gsu_player_motion_write,
  emu.callbackType.write,
  0x03C9,
  0x03F4,
  emu.cpuType.gsu,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_main_player_motion_write,
  emu.callbackType.write,
  0x03C9,
  0x03F4,
  emu.cpuType.snes,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_main_player_motion_write,
  emu.callbackType.write,
  0x03C9,
  0x03F4,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_target_display_write,
  emu.callbackType.write,
  0xE871,
  0xE871,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_target_display_write,
  emu.callbackType.write,
  0xE961,
  0xE965,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_main_player_motion_write,
  emu.callbackType.write,
  0xB089,
  0xB260,
  emu.cpuType.snes,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_main_player_motion_write,
  emu.callbackType.write,
  0xB089,
  0xB260,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_fighter_move,
  emu.callbackType.exec,
  0x7F9DDE,
  0x7F9DDE,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_random_branch,
  emu.callbackType.exec,
  0x7F8EE5,
  0x7F8EE5,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_wait_for_angle,
  emu.callbackType.exec,
  0x7FA0C4,
  0x7FA0C4,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_wait,
  emu.callbackType.exec,
  0x7F84FB,
  0x7F84FB,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_random_value,
  emu.callbackType.exec,
  0x7F9A3C,
  0x7F9A3C,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_chase_angle,
  emu.callbackType.exec,
  0x7FA1B5,
  0x7FA1B5,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_divide_angle,
  emu.callbackType.exec,
  0x7FA5BF,
  0x7FA5BF,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_schedule,
  emu.callbackType.exec,
  0x7F97CA,
  0x7F97CA,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_face_player,
  emu.callbackType.exec,
  0x7F878B,
  0x7F878B,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_fire,
  emu.callbackType.exec,
  0x7F885E,
  0x7F885E,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fighter_vertical_step,
  emu.callbackType.exec,
  0x7F8925,
  0x7F8925,
  emu.cpuType.snes,
  emu.memType.snesMemory)
for _, range in ipairs({
  { 0x0600, 0x0605 },
  { 0x0626, 0x062B },
  { 0x063F, 0x0644 },
  { 0x0665, 0x066A },
}) do
  emu.addMemoryCallback(
    record_gsu_capital_state_write,
    emu.callbackType.write,
    range[1],
    range[2],
    emu.cpuType.snes,
    emu.memType.gsuWorkRam)
  emu.addMemoryCallback(
    record_main_capital_state_write,
    emu.callbackType.write,
    range[1],
    range[2],
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
emu.addMemoryCallback(
  record_fighter_pitch_target_write,
  emu.callbackType.write,
  0x2297,
  0x2297,
  emu.cpuType.snes,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_fighter_pitch_target_write,
  emu.callbackType.write,
  0x2297,
  0x2297,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_fighter_position_y_write,
  emu.callbackType.write,
  0x0584,
  0x0585,
  emu.cpuType.snes,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_fighter_position_y_write,
  emu.callbackType.write,
  0x0584,
  0x0585,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_fighter_position_y_write,
  emu.callbackType.write,
  0x05C3,
  0x05C4,
  emu.cpuType.snes,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_fighter_position_y_write,
  emu.callbackType.write,
  0x05C3,
  0x05C4,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_main_fighter_random_state_write,
  emu.callbackType.write,
  0x00E0,
  0x00E3,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_main_fighter_random_state_write,
  emu.callbackType.write,
  0x00E0,
  0x00E3,
  emu.cpuType.snes,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_gsu_fighter_random_state_write,
  emu.callbackType.write,
  0x00E0,
  0x00E3,
  emu.cpuType.gsu,
  emu.memType.gsuWorkRam)
emu.addMemoryCallback(
  record_mission_transition_write,
  emu.callbackType.write,
  0x1B6A,
  0x1B6A,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_mission_transition_write,
  emu.callbackType.write,
  0x1B68,
  0x1B68,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_mission_transition_write,
  emu.callbackType.write,
  0x1B78,
  0x1B7C,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_mission_transition_write,
  emu.callbackType.write,
  0x1BE0,
  0x1BE0,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
for service, address in pairs({
  walker_alternate = 0x07F7B3,
  walker_selected = 0x07F7BA,
  alternate_table_alternate = 0x07F7CB,
  alternate_table_selected = 0x07F7D2,
  craft_alternate = 0x07F7E3,
  craft_selected = 0x07F7EA,
}) do
  emu.addMemoryCallback(
    craft_form_service(service),
    emu.callbackType.exec,
    address,
    address,
    emu.cpuType.snes,
    emu.memType.snesMemory)
end
emu.addMemoryCallback(
  record_main_player_shape_write,
  emu.callbackType.write,
  0x03C1,
  0x03C2,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_main_player_shape_write,
  emu.callbackType.write,
  0x0400,
  0x0401,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(isolate_main_layer, emu.eventType.startFrame)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
