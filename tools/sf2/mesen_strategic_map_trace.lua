-- Clean-room trace of the retail strategic-map flow through the first sortie.
--
-- This is oracle-only tooling. Source addresses are deliberately confined to
-- this script; the native Rust game consumes the semantic transitions and
-- typed values recovered from the resulting trace.

local frame = 0
local armed = false
local armed_frame = -1
local stop_elapsed = 6900
local lines = {}
local last_state = ""
local input_label = "idle"
local last_captured_input_frame = -1

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function write_file(name, contents)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(contents)
  file:close()
end

local function capture_screen(label, elapsed)
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
  write_file(string.format("%04d_%s.ppm", elapsed, label), table.concat(output))
end

local function capture_state(label, elapsed)
  local output = {}
  -- The host-side globals and base/extension object records used by this flow
  -- are below $4000. Full snapshots make field discovery reproducible without
  -- logging every write.
  for address = 0, 0x3FFF do
    output[#output + 1] = string.char(work_byte(address))
  end
  write_file(string.format("%04d_%s.wram", elapsed, label), table.concat(output))
  capture_screen(label, elapsed)
end

local function state_key()
  return string.format(
    "%02X:%02X:%02X:%02X:%02X:%02X:%02X:%04X:%04X:%04X",
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_byte(0x1C20),
    work_byte(0x1BB5),
    work_byte(0x1BA5),
    work_byte(0xD7F2),
    work_word(0x12A8),
    work_word(0x12C3),
    work_word(0x12C5))
end

local function record(label)
  local elapsed = armed and frame - armed_frame or -1
  local line = string.format(
    "SF2_MAP frame=%d elapsed=%d event=%s input=%s " ..
      "mode=%d submode=%d phase=%d cursor=%d selection=%d mapmode=%d " ..
      "difficulty=%d " ..
      "active=%04X player1=%04X player2=%04X map=%02X:%04X",
    frame,
    elapsed,
    label,
    input_label,
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_byte(0x1C20),
    work_byte(0x1BB5),
    work_byte(0x1BA5),
    work_byte(0xD7F2),
    work_word(0x12A8),
    work_word(0x12C3),
    work_word(0x12C5),
    work_byte(0x192E),
    work_word(0x1657))
  lines[#lines + 1] = line
  emu.log(line)
end

local function pulse(value, period, offset)
  local phase = value % period
  return phase == offset or phase == offset + 1
end

local function provide_input()
  local elapsed = frame - armed_frame
  local start = pulse(frame, 180, 120) and (not armed or elapsed <= 600)
  local accept = armed and elapsed >= 210 and pulse(elapsed, 90, 30)
  local up = armed and elapsed >= 6000 and elapsed < 6045
  local right = armed and elapsed >= 6045 and elapsed < 6070
  input_label = "idle"
  if start then input_label = "start" end
  if accept then input_label = "accept" end
  if up then input_label = "up" end
  if right then input_label = "right" end
  emu.setInput({
    start = start,
    a = false,
    b = accept,
    x = false,
    y = false,
    l = false,
    r = false,
    select = false,
    up = up,
    down = false,
    left = false,
    right = right,
  }, 0)
end

local function arm_for_target_stream()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame
    record("armed")
    capture_state("armed", 0)
  end
end

local function end_frame()
  frame = frame + 1
  if not armed then return end

  local elapsed = frame - armed_frame
  local key = state_key()
  if key ~= last_state then
    record("state")
    capture_state("state", elapsed)
    last_state = key
  end
  if input_label ~= "idle" and frame ~= last_captured_input_frame then
    record("input")
    capture_state(input_label, elapsed)
    last_captured_input_frame = frame
  end
  if elapsed > 0 and elapsed % 240 == 0 then
    record("checkpoint")
    capture_state("checkpoint", elapsed)
  end

  if elapsed >= stop_elapsed then
    write_file("sf2_strategic_map_trace.txt", table.concat(lines, "\n") .. "\n")
    emu.stop(0)
  end
end

emu.addMemoryCallback(
  arm_for_target_stream,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
