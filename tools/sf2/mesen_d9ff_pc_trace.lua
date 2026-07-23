-- Capture the opcode-address sequence for the first $01:D9FF Super FX job.
-- This intentionally avoids emu.getState() so the 595k-instruction oracle is
-- fast enough for routine differential runs.

local active = false
local finished = false
local lines = {}

local function entry()
  if not finished then active = true end
end

local function trace(address)
  if active then
    lines[#lines + 1] = string.format("%06X\n", address)
  end
end

local function stop()
  if not active then return end
  active = false
  finished = true
  local file = assert(io.open(
    emu.getScriptDataFolder() .. "/d9ff-pc-trace.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.log("SF2_D9FF_PC_TRACE_DONE")
  emu.stop(0)
end

emu.addMemoryCallback(entry, emu.callbackType.exec, 0x01D9FF, 0x01D9FF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(trace, emu.callbackType.exec, 0x000000, 0x7FFFFF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(stop, emu.callbackType.exec, 0x01DAE2, 0x01DAE2,
  emu.cpuType.gsu, emu.memType.gsuMemory)
