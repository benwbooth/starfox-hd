-- Diagnose and prove unattended controller injection before using input-driven
-- retail gameplay captures as an oracle.  A fresh Mesen profile can otherwise
-- silently have no controller assigned, leaving a long capture parked on the
-- title screen while appearing successful.

local frames = 0
local lines = {}

local function record(line)
  lines[#lines + 1] = line
  emu.log(line)
end

local function describe_input(port)
  local input = emu.getInput(port)
  local fields = {}
  for key, value in pairs(input) do
    fields[#fields + 1] = tostring(key) .. "=" .. tostring(value)
  end
  table.sort(fields)
  return table.concat(fields, ",")
end

local function provide_input()
  frames = frames + 1
  local pressed = frames >= 300 and frames <= 305
  emu.setInput({ start = pressed }, 0)
  if frames == 1 or frames == 299 or frames == 302 or frames == 306 then
    record(string.format(
      "SF2_INPUT frame=%d injected_start=%s port0={%s} port1={%s}",
      frames, tostring(pressed), describe_input(0), describe_input(1)))
  end
end

local function trace_autojoy(address, value)
  if frames >= 295 and frames <= 310 then
    record(string.format(
      "SF2_AUTOJOY frame=%d address=%06X value=%02X", frames, address, value))
  end
end

local function end_frame()
  if frames == 900 then
    record(string.format(
      "SF2_INPUT_FINAL map=%02X:%04X counter=%04X active=%04X",
      emu.read(0x192E, emu.memType.snesWorkRam, false),
      emu.read16(0x1657, emu.memType.snesWorkRam, false),
      emu.read16(0x1655, emu.memType.snesWorkRam, false),
      emu.read16(0x12A8, emu.memType.snesWorkRam, false)))
    local file = assert(io.open(emu.getScriptDataFolder() .. "/sf2_input_probe.txt", "wb"))
    file:write(table.concat(lines, "\n") .. "\n")
    file:close()
    emu.stop(0)
  end
end

emu.addMemoryCallback(
  trace_autojoy,
  emu.callbackType.read,
  0x004218,
  0x00421B,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
record("SF2_INPUT_PROBE_LOADED")
