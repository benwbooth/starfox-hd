-- Capture a deterministic Star Fox 2 title/menu branch from a stable retail
-- title savestate.  Oracle-only source-machine state remains confined here.

local state_path = assert(
  os.getenv("SF2_TITLE_LOAD_STATE"),
  "SF2_TITLE_LOAD_STATE must name the stable title savestate")
local scenario = os.getenv("SF2_TITLE_SCENARIO") or "main"
local state_file = assert(io.open(state_path, "r+b"))
local state_bytes = state_file:read("*a")
state_file:close()

local frame_count = 0
local loaded = false
local load_callback = nil
local capture_step = 4
local stop_frame = 1000

local function write_file(name, data)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(data)
  file:close()
end

local function held_for_frame(target)
  return frame_count == target or frame_count == target + 1
end

local function provide_input()
  if not loaded then return end
  local down = false
  local right = false
  local start = false
  if scenario == "record" then
    down = held_for_frame(20)
  elseif scenario == "stereo" then
    down = held_for_frame(20) or held_for_frame(200)
  elseif scenario == "sound" then
    down = held_for_frame(20) or held_for_frame(200)
    right = held_for_frame(380)
  elseif scenario == "difficulty_normal" then
    start = held_for_frame(20)
  elseif scenario == "difficulty_hard" then
    down = held_for_frame(200)
    start = held_for_frame(20)
  elseif scenario == "difficulty" then
    down = held_for_frame(200) or held_for_frame(380)
    start = held_for_frame(20)
  elseif scenario == "records" then
    down = held_for_frame(20)
    start = held_for_frame(200)
  end
  emu.setInput({
    start = start,
    a = false,
    b = false,
    x = false,
    y = false,
    l = false,
    r = false,
    up = false,
    down = down,
    left = false,
    right = right,
  }, 0)
end

local function load_state()
  if loaded then return end
  loaded = true
  emu.loadSavestate(state_bytes)
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
  write_file(
    string.format("sf2_title_%s_%04d.ppm", scenario, frame_count),
    table.concat(output))
end

local function end_frame()
  if not loaded then return end
  frame_count = frame_count + 1
  if load_callback then
    emu.removeMemoryCallback(
      load_callback,
      emu.callbackType.exec,
      0,
      0xFFFFFF,
      emu.cpuType.snes,
      emu.memType.snesMemory)
    load_callback = nil
  end
  if frame_count % capture_step == 0 then
    capture_screen()
  end
  if frame_count >= stop_frame then
    emu.stop(0)
  end
end

load_callback = emu.addMemoryCallback(
  load_state,
  emu.callbackType.exec,
  0,
  0xFFFFFF,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
