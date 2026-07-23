-- Trace the retail host's semantic audio-program selections and sound-port
-- writes while unattended input advances through the presentation and menus.

local frame_count = 0
local last_mode = -1
local last_submode = -1
local lines = {}
local stop_frame = 1400

local function record(line)
  lines[#lines + 1] = line
  emu.log(line)
end

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function capture_screen(mode, submode)
  local size = emu.getScreenSize()
  local screen = emu.getScreenBuffer()
  local path = string.format(
    "%s/frame_%04d_mode_%02d_submode_%02d.ppm",
    emu.getScriptDataFolder(), frame_count, mode, submode)
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

local function audio_program_entry()
  record(string.format(
    "SF2_AUDIO_PROGRAM frame=%d mode=%d submode=%d record=%03X conditional=%d",
    frame_count, work_byte(0x1B68), work_byte(0x1B76),
    work_word(0x1B6E), work_byte(0x1BBB)))
end

local function pulse(value, period, offset)
  local phase = value % period
  return phase == offset or phase == offset + 1
end

local function provide_input()
  local accept = frame_count >= 300 and pulse(frame_count, 90, 30)
  emu.setInput({
    start = pulse(frame_count, 180, 120) and frame_count <= stop_frame,
    a = false,
    b = accept,
    x = false,
    y = false,
    l = false,
    r = false,
    up = false,
    down = false,
    left = false,
    right = false,
  }, 0)
end

local function end_frame()
  frame_count = frame_count + 1
  local mode = work_byte(0x1B68)
  local submode = work_byte(0x1B76)
  if mode ~= last_mode or submode ~= last_submode then
    record(string.format(
      "SF2_AUDIO_STATE frame=%d mode=%d submode=%d map=%02X:%04X",
      frame_count, mode, submode, work_byte(0x192E), work_word(0x1657)))
    last_mode = mode
    last_submode = submode
  end
  if frame_count == 100 or frame_count == 250
      or frame_count == 350 or frame_count == 450
      or frame_count == 600 or frame_count == 750 or frame_count == 850
      or frame_count == 950 or frame_count == 1050 or frame_count == 1150
      or frame_count == 1250 or frame_count == 1350 then
    capture_screen(mode, submode)
  end
  if frame_count >= stop_frame then
    local file = assert(io.open(emu.getScriptDataFolder() .. "/sf2_audio_trace.txt", "wb"))
    file:write(table.concat(lines, "\n") .. "\n")
    file:close()
    emu.stop(0)
  end
end

emu.addMemoryCallback(
  audio_program_entry,
  emu.callbackType.exec,
  0x03E1E5,
  0x03E1E5,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
record("SF2_AUDIO_TRACE_LOADED")
