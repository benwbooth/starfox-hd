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

local state_file = assert(io.open(state_path, "r+b"))
local state_bytes = state_file:read("*a")
state_file:close()

local loaded = false
local frame = 0
local lines = {}
local load_callback = nil
local previous_loop_value = {}

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function audio_write(address, value)
  if not loaded then return end
  local channel = address - 0x2140
  if (channel == 1 or channel == 2) and previous_loop_value[channel] == value then
    return
  end
  previous_loop_value[channel] = value
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "frame=%d clock=%d channel=%d value=%d mode=%d submode=%d source=%02X:%04X",
    frame,
    state["memoryManager.masterClock"] or 0,
    channel,
    value,
    work_byte(0x1B68),
    work_byte(0x1B76),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function load_state()
  if loaded then return end
  loaded = true
  emu.loadSavestate(state_bytes)
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
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF2_AUDIO_EVENTS_ORACLE_LOADED")
