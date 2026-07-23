-- Trace every physical access to GSU RAM $03C6/$03C7 before and during the
-- first full-screen job.  The callback is armed from reset so prior jobs and
-- CPU initialization cannot escape the trace.

local lines = {}
local frame = 0
local first_job = false

local function state_pc(prefix, address, value)
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "%s frame=%d callback=%06X value=%02X cpu=%02X:%04X cpuCycles=%d master=%d gsu=%02X:%04X gsuCycles=%d r1=%04X r12=%04X running=%s ramAccess=%s delay=%d write=%04X:%02X direct=%04X\n",
    prefix, frame, address, value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cpu.cycleCount"] or 0,
    state["memoryManager.masterClock"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    state["cart.coprocessor.cycleCount"] or 0,
    state["cart.coprocessor.r1"] or 0,
    state["cart.coprocessor.r12"] or 0,
    tostring(state["cart.coprocessor.sfr.running"]),
    tostring(state["cart.coprocessor.gsuRamAccess"]),
    state["cart.coprocessor.ramDelay"] or 0,
    state["cart.coprocessor.ramWriteAddress"] or 0,
    state["cart.coprocessor.ramWriteValue"] or 0,
    emu.read16(0x03C6, emu.memType.gsuWorkRam, false))
end

local function entry(address, value)
  if first_job then return end
  first_job = true
  state_pc("ENTRY", address, value)
end

local function stop(address, value)
  if not first_job then return end
  state_pc("STOP", address, value)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/accesses.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.stop(0)
end

local function end_frame() frame = frame + 1 end

local function gsu_read(address, value) state_pc("GSU_READ", address, value) end
local function gsu_write(address, value) state_pc("GSU_WRITE", address, value) end
local function cpu_read(address, value) state_pc("CPU_READ", address, value) end
local function cpu_write(address, value) state_pc("CPU_WRITE", address, value) end

emu.addMemoryCallback(gsu_read, emu.callbackType.read, 0x03C6, 0x03C7,
  emu.cpuType.gsu, emu.memType.gsuWorkRam)
emu.addMemoryCallback(gsu_write, emu.callbackType.write, 0x03C6, 0x03C7,
  emu.cpuType.gsu, emu.memType.gsuWorkRam)
emu.addMemoryCallback(cpu_read, emu.callbackType.read, 0x03C6, 0x03C7,
  emu.cpuType.snes, emu.memType.gsuWorkRam)
emu.addMemoryCallback(cpu_write, emu.callbackType.write, 0x03C6, 0x03C7,
  emu.cpuType.snes, emu.memType.gsuWorkRam)
emu.addMemoryCallback(entry, emu.callbackType.exec, 0x01CD99, 0x01CD99,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(stop, emu.callbackType.exec, 0x01CE35, 0x01CE35,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
