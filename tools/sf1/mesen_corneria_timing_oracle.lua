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
local input_segment_frames = 30
local direction_run_segments = 3
local oracle_timeout_video_frames = 12000
local input_mode = os.getenv("SF1_MESEN_CORNERIA_INPUT") or "route"
local first_scene_game_frame =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_FIRST_SCENE")) or 315
local last_scene_game_frame =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_LAST_SCENE")) or 322
local poll_trace_limit =
  tonumber(os.getenv("SF1_MESEN_CORNERIA_POLL_TRACE_LIMIT")) or 0
local capture_timeline = os.getenv("SF1_MESEN_CORNERIA_TIMELINE") ~= "0"
local capture_gsu_jobs = os.getenv("SF1_MESEN_CORNERIA_GSU_JOBS") == "1"

local game_frame_address = 0x15BB
local measured_motion_address = 0x14E3
local video_interrupt_count_address = 0x1200

local cadence_reset_entry = 0x02D960
local horizontal_poll_entry = 0x02DCC5
local horizontal_position_sample = 0x02DCC8
local horizontal_safe_window_ready = 0x02DCDA
local horizontal_dma_complete = 0x02DD04
local horizontal_transfer_complete = 0x02D9EB
local motion_sample_complete = 0x02DA7E
local corneria_game_start_entry = 0x03C437
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
local active_gsu_job = nil
local output_lines = {
  "input_mode scene_game_frame reset_game_frame sample_game_frame reset_video_frame sample_video_frame reset_master poll_begin_master safe_master dma_master transfer_complete_master sample_master reset_to_sample reset_cpu_cycle sample_cpu_cycle reset_to_sample_cpu_cycles safe_wait safe_to_transfer_complete polls video_interrupt_count measured_motion\n"
}

assert(input_mode == "route" or input_mode == "neutral",
  "SF1_MESEN_CORNERIA_INPUT must be route or neutral")
assert(first_scene_game_frame <= last_scene_game_frame,
  "Corneria timing scene range must be ordered")
assert(poll_trace_limit >= 0, "poll trace limit must not be negative")

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function master_clock()
  return emu.getState()["memoryManager.masterClock"] or 0
end

local function write_output()
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
  local direction_run = math.floor(segment / direction_run_segments)
  if direction_run == 0 then return { up = true, left = true } end
  if direction_run == 1 then return { up = true, right = true } end
  if direction_run == 2 then return { down = true, right = true } end
  if direction_run == 3 then return { down = true, left = true } end
  return {}
end

local function provide_input()
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
  }
end

local function arm_corneria_measurement()
  corneria_started = true
end

local function timeline_callback(name)
  return function()
    if active_measurement == nil or not capture_timeline then return end
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
    }
  end
  active_gsu_job.steps = active_gsu_job.steps + 1
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
  active_gsu_job = nil
end

local function record_video_interrupt_count(address, value)
  if active_measurement == nil or value == 0 then return end
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

  local sample_master = master_clock()
  local sample_cpu_cycle = emu.getState()["cpu.cycleCount"] or 0
  for _, line in ipairs(active_measurement.poll_trace) do
    output_lines[#output_lines + 1] = line
  end
  output_lines[#output_lines + 1] = string.format(
    "%s %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n",
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
    work_byte(measured_motion_address))
  completed_measurements = completed_measurements + 1
  active_measurement = nil

  if scene_game_frame == last_scene_game_frame then
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
emu.log("SF1_CORNERIA_TIMING_ORACLE_LOADED")
