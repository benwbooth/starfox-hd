-- Compare the three retail difficulty selections from one identical title
-- savestate and one identical confirmation frame.  This is oracle tooling:
-- source-machine storage is dumped only so the shipping port can recover the
-- underlying semantic difficulty state and its consumers.

local state_path = assert(
  os.getenv("SF2_TITLE_LOAD_STATE"),
  "SF2_TITLE_LOAD_STATE must name the stable title savestate")
local scenario = os.getenv("SF2_DIFFICULTY") or "normal"
assert(
  scenario == "normal" or scenario == "hard" or scenario == "expert",
  "SF2_DIFFICULTY must be normal, hard, or expert")

local state_file = assert(io.open(state_path, "r+b"))
local state_bytes = state_file:read("*a")
state_file:close()

local frame = 0
local loaded = false
local load_callback = nil
local lines = {}
local access_lines = {}
local stop_frame = tonumber(os.getenv("SF2_DIFFICULTY_STOP_FRAME")) or 720
local seen_reads = {}

assert(stop_frame >= 720, "SF2_DIFFICULTY_STOP_FRAME must be at least 720")

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

local function held_for_frame(target)
  return frame == target or frame == target + 1
end

local function pulse_from(first, period)
  if frame < first then return false end
  local phase = (frame - first) % period
  return phase == 0 or phase == 1
end

local function provide_input()
  if not loaded then return end
  local down = false
  if scenario == "hard" then
    down = held_for_frame(200)
  elseif scenario == "expert" then
    down = held_for_frame(200) or held_for_frame(380)
  end
  emu.setInput({
    start = held_for_frame(20) or held_for_frame(560),
    a = false,
    b = pulse_from(760, 90),
    x = false,
    y = false,
    l = false,
    r = false,
    up = false,
    down = down,
    left = false,
    right = false,
  }, 0)
end

local function record(label)
  lines[#lines + 1] = string.format(
    "%s frame=%d mode=%d submode=%d phase=%d cursor=%d prior=%d " ..
      "menu=%d selection=%d candidate=%d chosen=%04X",
    label,
    frame,
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_byte(0x1C20),
    work_byte(0x1BE2),
    work_byte(0x1C1F),
    work_byte(0x1BB5),
    work_byte(0x1BA5),
    work_word(0x1BA3))
end

local function dump_wram(label)
  local output = {}
  for address = 0, 0x1FFFF do
    output[#output + 1] = string.char(work_byte(address))
  end
  write_file(
    string.format("sf2_difficulty_%s_%s.wram", scenario, label),
    table.concat(output))
end

local function capture_screen(label)
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
    string.format("sf2_difficulty_%s_%s.ppm", scenario, label),
    table.concat(output))
end

local function record_write(address, value)
  if not loaded then return end
  local state = emu.getState()
  access_lines[#access_lines + 1] = string.format(
    "frame=%d address=%04X value=%02X cpu=%02X:%04X a=%04X x=%04X y=%04X",
    frame,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cpu.a"] or 0,
    state["cpu.x"] or 0,
    state["cpu.y"] or 0)
end

local function record_read(address, value)
  if not loaded or frame < 562 then return end
  local state = emu.getState()
  local key = string.format(
    "%04X:%02X:%04X",
    address,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
  if seen_reads[key] then return end
  seen_reads[key] = true
  access_lines[#access_lines + 1] = string.format(
    "frame=%d access=read address=%04X value=%02X cpu=%02X:%04X " ..
      "a=%04X x=%04X y=%04X",
    frame,
    address,
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cpu.a"] or 0,
    state["cpu.x"] or 0,
    state["cpu.y"] or 0)
end

local function load_state()
  if loaded then return end
  loaded = true
  emu.loadSavestate(state_bytes)
  if scenario == "expert" then
    local completion_flags = emu.read(0x703916, emu.memType.snesMemory, false)
    emu.write(0x703916, completion_flags | 0x10, emu.memType.snesMemory)
  end
end

local function end_frame()
  if not loaded then return end
  frame = frame + 1
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
  if frame == 540 then
    record("selected")
    dump_wram("selected")
  elseif frame == 600 then
    record("confirmed")
    dump_wram("confirmed")
  elseif frame == 720 then
    record("settled")
    dump_wram("settled")
  end
  if frame >= stop_frame then
    if frame ~= 720 then
      record("stopped")
      dump_wram("stopped")
    end
    capture_screen("stopped")
    write_file(
      string.format("sf2_difficulty_%s.txt", scenario),
      table.concat(lines, "\n") .. "\n")
    write_file(
      string.format("sf2_difficulty_%s_accesses.txt", scenario),
      table.concat(access_lines, "\n") .. "\n")
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
emu.addMemoryCallback(
  record_write,
  emu.callbackType.write,
  0x1BA3,
  0x1BA4,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_write,
  emu.callbackType.write,
  0xD7F2,
  0xD7F2,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_read,
  emu.callbackType.read,
  0x1BA3,
  0x1BA4,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_read,
  emu.callbackType.read,
  0xD7F2,
  0xD7F2,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
