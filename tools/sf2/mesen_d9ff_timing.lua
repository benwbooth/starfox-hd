-- Exact master/GSU clock boundaries for the first $01:D9FF transform.

local entry_line = nil

local function record(kind, address)
  local state = emu.getState()
  return string.format(
    "%s pc=%06X frame=%d master=%d gsuCycles=%d cpuCycles=%d\n",
    kind, address, emu.getState()["ppu.frameCount"] or 0,
    state["memoryManager.masterClock"] or 0,
    state["cart.coprocessor.cycleCount"] or 0,
    state["cpu.cycleCount"] or 0)
end

local function entry(address)
  if entry_line == nil then entry_line = record("entry", address) end
end

local function stop(address)
  if entry_line == nil then return end
  local file = assert(io.open(
    emu.getScriptDataFolder() .. "/d9ff-timing.txt", "wb"))
  file:write(entry_line)
  file:write(record("stop", address))
  file:close()
  emu.log("SF2_D9FF_TIMING_DONE")
  emu.stop(0)
end

emu.addMemoryCallback(entry, emu.callbackType.exec, 0x01D9FF, 0x01D9FF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(stop, emu.callbackType.exec, 0x01DAE2, 0x01DAE2,
  emu.cpuType.gsu, emu.memType.gsuMemory)
