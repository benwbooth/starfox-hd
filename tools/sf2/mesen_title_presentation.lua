-- Capture a stable retail Star Fox 2 title baseline and reusable savestate.
--
-- This is oracle tooling only.  The shipping port consumes generated image
-- tracks selected by typed title state; it does not expose source-machine
-- storage or execute the original program.

local frame_count = 0
local lines = {}
local first_capture_frame = 560
local last_capture_frame = 1800
local capture_step = 4
local pending_savestate = false
local saved_savestate = false

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function write_file(name, data)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(data)
  file:close()
end

local function record(label)
  local line = string.format(
    "%s frame=%d mode=%d submode=%d phase=%d cursor=%d prior=%d menu=%d " ..
      "selection=%d difficulty=%d audio=%d",
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
    work_byte(0x1BB4))
  lines[#lines + 1] = line
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
    string.format("sf2_title_%05d.ppm", frame_count),
    table.concat(output))
end

local function held_for_frame(target)
  return frame_count == target or frame_count == target + 1
end

local function provide_input()
  -- The first three presses advance the retail boot presentation to title.
  -- Subsequent events provide a broad diagnostic trace; exhaustive branches
  -- are captured from the saved title state by mesen_title_branch_capture.lua.
  emu.setInput({
    start = held_for_frame(120)
      or held_for_frame(300)
      or held_for_frame(480),
    a = false,
    b = held_for_frame(940)
      or held_for_frame(1280),
    x = false,
    y = held_for_frame(1120),
    l = false,
    r = false,
    up = held_for_frame(860)
      or held_for_frame(1200),
    down = held_for_frame(620)
      or held_for_frame(700)
      or held_for_frame(1360)
      or held_for_frame(1440),
    left = false,
    right = held_for_frame(780),
  }, 0)
end

local function end_frame()
  frame_count = frame_count + 1
  if frame_count == 589 then
    pending_savestate = true
  end
  if frame_count >= first_capture_frame
    and frame_count <= last_capture_frame
    and (frame_count - first_capture_frame) % capture_step == 0 then
    record("SF2_TITLE_PRESENTATION")
    capture_screen()
  end
  if frame_count >= last_capture_frame then
    write_file("sf2_title_presentation.txt", table.concat(lines, "\n") .. "\n")
    emu.stop(0)
  end
end

local function save_pending_state()
  if pending_savestate and not saved_savestate then
    write_file("sf2_title_base.mss", emu.createSavestate())
    pending_savestate = false
    saved_savestate = true
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(
  save_pending_state,
  emu.callbackType.exec,
  0,
  0xFFFFFF,
  emu.cpuType.snes,
  emu.memType.snesMemory)
