-- Capture retail Star Fox 2 audio commands from a saved campaign position.
--
-- This is verification-only tooling. Source-machine addresses and processor
-- state intentionally remain outside the shipping Rust port.

local state_path = assert(
  os.getenv("SF2_ORACLE_LOAD_STATE"),
  "SF2_ORACLE_LOAD_STATE must name a Mesen savestate")
local capture_frames = tonumber(os.getenv("SF2_ORACLE_CAPTURE_FRAMES")) or 600
local input_mode = os.getenv("SF2_ORACLE_INPUT") or "neutral"
local release_frame = tonumber(os.getenv("SF2_ORACLE_RELEASE_FRAME")) or 540
local trace_objects = os.getenv("SF2_ORACLE_TRACE_OBJECTS") == "1"
local trace_queue = os.getenv("SF2_ORACLE_TRACE_QUEUE") == "1"
local trace_messages = os.getenv("SF2_ORACLE_TRACE_MESSAGES") == "1"
local capture_screen_frames = {}
for value in string.gmatch(os.getenv("SF2_ORACLE_CAPTURE_SCREEN_FRAMES") or "", "[^,]+") do
  local capture_frame = tonumber(value)
  if capture_frame then capture_screen_frames[capture_frame] = true end
end

local state_file = assert(io.open(state_path, "r+b"))
local state_bytes = state_file:read("*a")
state_file:close()

local loaded = false
local loading = false
local frame = 0
local lines = {}
local load_callback = nil
local previous_loop_value = {}
local previous_message_state = nil

local function write_file(name, contents)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(contents)
  file:close()
end

local function capture_screen()
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
  write_file(string.format("sf2_audio_%04d.ppm", frame), table.concat(output))
end

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function bus_word(address)
  return emu.read16(address, emu.memType.snesMemory, false)
end

local function signed_word(address)
  local value = work_word(address)
  if value >= 0x8000 then return value - 0x10000 end
  return value
end

local function pose(object)
  return string.format(
    "%d,%d,%d,%d,%d,%d,%d",
    signed_word(object + 12),
    signed_word(object + 14),
    signed_word(object + 16),
    work_byte(object + 18),
    work_byte(object + 20),
    work_byte(object + 22),
    work_byte(object + 24))
end

local function byte_range(address, length)
  local bytes = {}
  for offset = 0, length - 1 do
    bytes[#bytes + 1] = string.format("%02X", work_byte(address + offset))
  end
  return table.concat(bytes)
end

local function active_objects()
  local output = {}
  local seen = {}
  local object = work_word(0x12A8)
  while object ~= 0 and not seen[object] and #output < 60 do
    seen[object] = true
    output[#output + 1] = string.format(
      "%04X,%04X,%s,%04X,%d,%d,%d,%d,%d,%d,%d",
      object,
      work_word(object + 4),
      pose(object),
      work_word(object + 0x2B),
      work_byte(object + 0x28),
      work_byte(object + 0x2D),
      work_byte(object + 0x2E),
      work_byte(object + 0x2F),
      work_byte(object + 0x30),
      work_byte(object + 0x31),
      work_byte(object + 0x1CCC))
    object = work_word(object)
  end
  return table.concat(output, ";")
end

local function audio_write(address, value)
  if not loaded then return end
  local channel = address - 0x2140
  if (channel == 1 or channel == 2) and previous_loop_value[channel] == value then
    return
  end
  previous_loop_value[channel] = value
  local state = emu.getState()
  local selected_source = work_word(0x1CE8)
  lines[#lines + 1] = string.format(
    "frame=%d clock=%d channel=%d value=%d mode=%d submode=%d source=%02X:%04X prepared=%d sound=%d selected=%04X selected_shape=%04X selected_pose=%s listener_pose=%s",
    frame,
    state["memoryManager.masterClock"] or 0,
    channel,
    value,
    work_byte(0x1B68),
    work_byte(0x1B76),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    work_byte(0x1CE6),
    work_byte(0x1CE7),
    selected_source,
    selected_source ~= 0 and work_word(selected_source + 4) or 0,
    pose(selected_source),
    pose(0x033F))
  if trace_objects and (channel == 2 or channel == 3) then
    lines[#lines + 1] = string.format(
      "objects frame=%d channel=%d value=%d player=%04X active=[%s]",
      frame,
      channel,
      value,
      work_word(0x12C3),
      active_objects())
  end
end

local function positional_prepare(_, value)
  if not loaded or not trace_objects then return end
  local state = emu.getState()
  local selected_source = work_word(0x1CEE)
  lines[#lines + 1] = string.format(
    "position frame=%d value=%d sound=%d selected=%04X selected_shape=%04X selected_pose=%s selected_angles=%s listener_pose=%s listener_angles=%s player_angles=%s angle=%04X listener_yaw=%d source=%02X:%04X",
    frame,
    value,
    work_byte(0x1CED),
    selected_source,
    work_word(selected_source + 4),
    pose(selected_source),
    byte_range(selected_source + 0x12, 7),
    pose(0x033F),
    byte_range(0x033F + 0x12, 7),
    byte_range(work_word(0x12C3) + 0x12, 7),
    work_word(0x50),
    work_byte(0x033F + 0x15),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function queue_sound()
  if not loaded or not trace_queue then return end
  local state = emu.getState()
  local stack = {}
  local stack_pointer = state["cpu.s"] or state["cpu.sp"] or 0x01FF
  for offset = 1, 12 do
    stack[#stack + 1] = string.format(
      "%02X",
      emu.read(
        (stack_pointer + offset) % 0x10000,
        emu.memType.snesMemory,
        false))
  end
  lines[#lines + 1] = string.format(
    "queue frame=%d command=%04X object=%04X stack_pointer=%04X stack=%s source=%02X:%04X",
    frame,
    state["cpu.a"] or 0,
    state["cpu.x"] or 0,
    stack_pointer,
    table.concat(stack),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function queue_path_sound()
  if not loaded or not trace_queue then return end
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "path_queue frame=%d command=%04X object=%04X path=%04X source=%02X:%04X",
    frame,
    work_word(0x1C31),
    state["cpu.y"] or 0,
    work_word((state["cpu.y"] or 0) + 0x2B),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function start_message()
  if not loaded or not trace_messages then return end
  local state = emu.getState()
  local stack_pointer = state["cpu.s"] or state["cpu.sp"] or 0x01FF
  local stack = {}
  for offset = 1, 12 do
    stack[#stack + 1] = string.format(
      "%02X",
      emu.read(
        (stack_pointer + offset) % 0x10000,
        emu.memType.snesMemory,
        false))
  end
  lines[#lines + 1] = string.format(
    "message_start frame=%d request=%d stack_pointer=%04X stack=%s source=%02X:%04X",
    frame,
    (state["cpu.a"] or 0) % 0x100,
    stack_pointer,
    table.concat(stack),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function load_state()
  if loaded or loading then return end
  loading = true
  emu.loadSavestate(state_bytes)
  loaded = true
  loading = false
end

local function provide_input()
  if not loaded then return end
  if input_mode == "rapid" then
    emu.setInput({ b = true }, 0)
  elseif input_mode == "charge-release" then
    emu.setInput({ b = frame < release_frame }, 0)
  elseif input_mode == "charge" then
    emu.setInput({ b = true }, 0)
  elseif input_mode == "boost" then
    emu.setInput({ x = true }, 0)
  elseif input_mode == "left" then
    emu.setInput({ left = true }, 0)
  elseif input_mode == "right" then
    emu.setInput({ right = true }, 0)
  else
    emu.setInput({}, 0)
  end
end

local function finish()
  local path = emu.getScriptDataFolder() .. "/sf2_audio_events.txt"
  local output = assert(io.open(path, "wb"))
  output:write(string.format(
    "input=%s frames=%d events=%d\n",
    input_mode,
    frame,
    #lines))
  if #lines > 0 then
    output:write(table.concat(lines, "\n") .. "\n")
  end
  output:close()
  emu.log(string.format("SF2_AUDIO_EVENTS_DONE events=%d", #lines))
  emu.stop(0)
end

local function end_frame()
  if not loaded then return end
  if trace_messages then
    local message_id = work_byte(0xCF31)
    local animation = work_byte(0xCF32)
    local speaker = work_byte(0x17F6)
    local presentation = work_byte(0x1B48)
    local state = string.format("%02X,%02X,%02X,%02X", message_id, animation, speaker, presentation)
    if state ~= previous_message_state or animation ~= 0 then
      lines[#lines + 1] = string.format(
        "message frame=%d id=%d animation=%d speaker=%d presentation=%d camera=%s text_pointer=%04X text_cursor=%04X panel_top=%d panel_side=%d",
        frame,
        message_id,
        animation,
        speaker,
        presentation,
        pose(0x033F),
        bus_word(0x7000B8),
        bus_word(0x7000AA),
        bus_word(0x700068),
        bus_word(0x70002C))
      previous_message_state = state
    end
  end
  if capture_screen_frames[frame] then capture_screen() end
  frame = frame + 1
  if load_callback then
    emu.removeMemoryCallback(
      load_callback,
      emu.callbackType.exec,
      0x000000,
      0xFFFFFF,
      emu.cpuType.snes,
      emu.memType.snesMemory)
    load_callback = nil
  end
  if frame >= capture_frames then finish() end
end

load_callback = emu.addMemoryCallback(
  load_state,
  emu.callbackType.exec,
  0x000000,
  0xFFFFFF,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  audio_write,
  emu.callbackType.write,
  0x002140,
  0x002143,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  positional_prepare,
  emu.callbackType.write,
  0x7E1CEC,
  0x7E1CEC,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  queue_sound,
  emu.callbackType.exec,
  0x7F6E09,
  0x7F6E09,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  queue_path_sound,
  emu.callbackType.exec,
  0x7FA43E,
  0x7FA43E,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  start_message,
  emu.callbackType.exec,
  0x0ACF04,
  0x0ACF04,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF2_AUDIO_EVENTS_ORACLE_LOADED")
