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
local scenario_stop_frames = {
  campaign = 4000,
  campaign_guided = 4200,
  campaign_prompted = 6000,
}
local default_stop_frame = scenario_stop_frames[scenario] or 1000
local stop_frame = tonumber(os.getenv("SF2_TITLE_STOP_FRAME")) or default_stop_frame
local lines = {}
local input_event_lines = {}
local prompt_first_frame = nil
local prompt_accept_end_frame = -1
local last_prompt_accept_frame = -1000

local map_mode = 7
local prompt_dwell_frames = 120
local prompt_accept_frames = 2
local prompt_accept_cooldown_frames = 30
local prompt_color = 0xEF8C39
local prompt_left = 189
local prompt_right = 196
local prompt_top = 216
local prompt_bottom = 222

assert(stop_frame > 0, "SF2_TITLE_STOP_FRAME must be positive")
assert(
  stop_frame % capture_step == 0,
  "SF2_TITLE_STOP_FRAME must align with the capture cadence")

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function write_file(name, data)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(data)
  file:close()
end

local function held_for_frame(target)
  return frame_count == target or frame_count == target + 1
end

local function pulses_from(first_frame, period)
  if frame_count < first_frame then return false end
  local phase = (frame_count - first_frame) % period
  return phase == 0 or phase == 1
end

local function prompt_visible()
  local size = emu.getScreenSize()
  local screen = emu.getScreenBuffer()
  for y = prompt_top, prompt_bottom do
    for x = prompt_left, prompt_right do
      local pixel = screen[y * size.width + x + 1] or 0
      if (pixel & 0xFFFFFF) == prompt_color then return true end
    end
  end
  return false
end

local function prompt_accept_requested()
  if work_byte(0x1B68) ~= map_mode then return false end
  if frame_count <= prompt_accept_end_frame then return true end
  if frame_count - last_prompt_accept_frame < prompt_accept_cooldown_frames then
    return false
  end
  if prompt_visible() and prompt_first_frame == nil then
    prompt_first_frame = frame_count
  end
  if prompt_first_frame == nil then return false end
  if frame_count - prompt_first_frame < prompt_dwell_frames then return false end

  last_prompt_accept_frame = frame_count
  prompt_accept_end_frame = frame_count + prompt_accept_frames - 1
  input_event_lines[#input_event_lines + 1] = string.format(
    "prompt=%d accept=%d",
    prompt_first_frame,
    frame_count)
  prompt_first_frame = nil
  return true
end

local function provide_input()
  if not loaded then return end
  local down = false
  local right = false
  local start = false
  local accept = false
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
  elseif scenario == "campaign" then
    start = held_for_frame(20) or held_for_frame(200)
  elseif scenario == "campaign_guided" then
    start = held_for_frame(20) or held_for_frame(200)
    accept = pulses_from(968, 90)
  elseif scenario == "campaign_prompted" then
    start = held_for_frame(20) or held_for_frame(200)
    accept = prompt_accept_requested()
  end
  emu.setInput({
    start = start,
    a = false,
    b = accept,
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
    lines[#lines + 1] = string.format(
      "frame=%d mode=%d submode=%d phase=%d cursor=%d menu=%d",
      frame_count,
      work_byte(0x1B68),
      work_byte(0x1B76),
      work_byte(0x1BE0),
      work_byte(0x1C20),
      work_byte(0x1C1F))
    capture_screen()
  end
  if frame_count >= stop_frame then
    write_file(
      string.format("sf2_title_%s_trace.txt", scenario),
      table.concat(lines, "\n") .. "\n")
    if #input_event_lines > 0 then
      write_file(
        string.format("sf2_title_%s_inputs.txt", scenario),
        table.concat(input_event_lines, "\n") .. "\n")
    end
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
