-- Trace the retail title-screen selections with isolated, deterministic input.
-- This is verification tooling only; the native port consumes the resulting
-- semantic state transitions rather than exposing source-machine storage.

local frame_count = 0
local lines = {}
local stop_frame = 1100

local capture_frames = {
  [590] = true,
  [630] = true,
  [680] = true,
  [730] = true,
  [780] = true,
  [830] = true,
  [900] = true,
  [1000] = true,
}

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function record(label)
  local line = string.format(
    "%s frame=%d mode=%d submode=%d phase=%d cursor=%d prior=%d menu=%d " ..
      "selection=%d difficulty=%d chosen=%d map=%02X:%04X",
    label,
    frame_count,
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_byte(0x1C20),
    work_byte(0x1BE2),
    work_byte(0x1C1F),
    work_byte(0x1BB5),
    work_byte(0x1BA5),
    work_word(0x1BA3),
    work_byte(0x192E),
    work_word(0x1657))
  lines[#lines + 1] = line
  emu.log(line)
end

local function capture_screen()
  local size = emu.getScreenSize()
  local screen = emu.getScreenBuffer()
  local path = string.format(
    "%s/title_%04d.ppm", emu.getScriptDataFolder(), frame_count)
  local file = assert(io.open(path, "w+b"))
  file:write(string.format("P6\n%d %d\n255\n", size.width, size.height))
  for index = 1, size.width * size.height do
    local pixel = screen[index] or 0
    file:write(string.char(
      (pixel >> 16) & 0xFF,
      (pixel >> 8) & 0xFF,
      pixel & 0xFF))
  end
  file:close()
end

local function held_for_frame(target)
  return frame_count == target or frame_count == target + 1
end

local function provide_input()
  emu.setInput({
    start = held_for_frame(120)
      or held_for_frame(300)
      or held_for_frame(480)
      or held_for_frame(620),
    a = false,
    b = held_for_frame(720)
      or held_for_frame(820)
      or held_for_frame(920),
    x = false,
    y = false,
    l = false,
    r = false,
    up = false,
    down = held_for_frame(670),
    left = false,
    right = held_for_frame(770) or held_for_frame(870),
  }, 0)
end

local function end_frame()
  frame_count = frame_count + 1
  if capture_frames[frame_count] then
    record("SF2_TITLE_STATE")
    capture_screen()
  end
  if frame_count >= stop_frame then
    local file = assert(io.open(
      emu.getScriptDataFolder() .. "/sf2_title_menu_trace.txt", "wb"))
    file:write(table.concat(lines, "\n") .. "\n")
    file:close()
    emu.stop(0)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
record("SF2_TITLE_TRACE_LOADED")
