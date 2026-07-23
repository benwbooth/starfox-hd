-- Trace every retail write to the map bank/cursor/counter while driving the
-- menus unattended.  This proves dynamically selected map roots that cannot
-- be found by scanning only immediate LDA/LDX root installs.

local frame = 0
local armed = false
local armed_frame = -1
local writes = {}
local state_keys = nil
local cursor_samples = {}
local last_cursor = -1
local last_bank = -1

local function pulse(value, period, offset)
  local phase = value % period
  return phase == offset or phase == offset + 1
end

local function provide_input()
  local elapsed = frame - armed_frame
  local accept = armed and elapsed >= 210 and pulse(elapsed, 90, 30)
  emu.setInput({
    start = pulse(frame, 180, 120) and (not armed or elapsed <= 600),
    a = false,
    b = accept,
    x = false,
    y = false,
    l = false,
    r = false,
    select = false,
    up = armed and elapsed >= 6000 and elapsed < 6045,
    down = false,
    left = armed and elapsed >= 6045 and elapsed < 6070,
    right = false,
  }, 0)
end

local function arm_for_target_stream()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame
  end
end

local function trace_write(address, value)
  local state = emu.getState()
  if state_keys == nil then
    state_keys = {}
    for key, _ in pairs(state) do
      if string.find(string.lower(key), "cpu.", 1, true) then
        state_keys[#state_keys + 1] = key
      end
    end
    table.sort(state_keys)
  end
  local item = { frame = frame, address = address, value = value }
  for _, key in ipairs(state_keys) do
    item[key] = state[key]
  end
  writes[#writes + 1] = item
end

local function finish()
  local lines = {}
  lines[#lines + 1] = "frame address value " .. table.concat(state_keys or {}, " ") .. "\n"
  for _, item in ipairs(writes) do
    local fields = {
      tostring(item.frame), string.format("%04X", item.address),
      string.format("%02X", item.value),
    }
    for _, key in ipairs(state_keys or {}) do
      fields[#fields + 1] = tostring(item[key])
    end
    lines[#lines + 1] = table.concat(fields, " ") .. "\n"
  end
  local file = assert(io.open(emu.getScriptDataFolder() .. "/sf2_map_writes.txt", "wb"))
  file:write(table.concat(lines))
  file:close()

  lines = { "frame bank cursor counter active\n" }
  for _, item in ipairs(cursor_samples) do
    lines[#lines + 1] = string.format(
      "%d %02X %04X %04X %04X\n",
      item.frame, item.bank, item.cursor, item.counter, item.active)
  end
  file = assert(io.open(emu.getScriptDataFolder() .. "/sf2_map_samples.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.stop(0)
end

local function end_frame()
  frame = frame + 1
  local bank = emu.read(0x192E, emu.memType.snesWorkRam, false)
  local cursor = emu.read16(0x1657, emu.memType.snesWorkRam, false)
  if bank ~= last_bank or cursor ~= last_cursor then
    cursor_samples[#cursor_samples + 1] = {
      frame = frame,
      bank = bank,
      cursor = cursor,
      counter = emu.read16(0x1655, emu.memType.snesWorkRam, false),
      active = emu.read16(0x12A8, emu.memType.snesWorkRam, false),
    }
    last_bank = bank
    last_cursor = cursor
  end
  if armed and frame - armed_frame >= 7100 then
    finish()
  end
end


emu.addMemoryCallback(
  arm_for_target_stream,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

for _, range in ipairs({
  { 0x1655, 0x1658 },
  { 0x192E, 0x192E },
}) do
  emu.addMemoryCallback(
    trace_write,
    emu.callbackType.write,
    range[1],
    range[2],
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
