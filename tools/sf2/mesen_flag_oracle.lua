-- Trace the retail frame-task flag at WRAM $1B92 while driving SF2 menus.

local frame = 0
local armed = false
local armed_frame = -1
local last_value = -1
local lines = { "frame elapsed value pc cycle\n" }

local function pulse(value, period, offset)
  local phase = value % period
  return phase == offset or phase == offset + 1
end

local function input()
  local elapsed = frame - armed_frame
  local accept = armed and elapsed >= 210 and pulse(elapsed, 90, 30)
  emu.setInput({
    start = pulse(frame, 180, 120) and (not armed or elapsed <= 600),
    b = accept,
    up = armed and elapsed >= 6000 and elapsed < 6045,
    right = armed and elapsed >= 6045 and elapsed < 6070,
  }, 0)
end

local function arm()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame
  end
end

local function flag_write(_, value)
  if value == last_value then return end
  last_value = value
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "%d %d %02X %04X %s\n",
    frame,
    armed and frame - armed_frame or -1,
    value,
    state["cpu.pc"] or 0,
    tostring(state["cpu.cycleCount"] or 0))
end

local function finish()
  local file = assert(io.open(emu.getScriptDataFolder() .. "/flag_writes.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.stop(0)
end

local function end_frame()
  frame = frame + 1
  if armed and frame - armed_frame >= 6500 then finish() end
end

emu.addMemoryCallback(
  arm, emu.callbackType.exec, 0x01D9FF, 0x01D9FF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(
  flag_write, emu.callbackType.write, 0x1B92, 0x1B92,
  emu.cpuType.snes, emu.memType.snesWorkRam)
emu.addEventCallback(input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
