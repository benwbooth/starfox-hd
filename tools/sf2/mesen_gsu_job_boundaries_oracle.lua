-- Record every Super FX job boundary through the early Star Fox 2 boot.  GSU
-- opcode callbacks are more reliable here than watching the CPU register bus:
-- the first callback after a STOP is the job entry and opcode $00 is its exit.

local frame = 0
local sequence = 0
local active = false
local steps = 0
local lines = {
  "kind sequence frame pbr pc steps ram003A ram24C2 ram24C4 ram0014 cpu_pc\n"
}

local function word(address)
  return emu.read16(address, emu.memType.gsuWorkRam, false)
end

local function record(kind, address)
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "%s %d %d %02X %04X %d %04X %04X %04X %04X %02X:%04X\n",
    kind, sequence, frame,
    state["cart.coprocessor.programBank"] or ((address >> 16) & 0xFF),
    address & 0xFFFF, steps,
    word(0x003A), word(0x24C2), word(0x24C4), word(0x0014),
    state["cpu.k"] or 0, state["cpu.pc"] or 0)
end

local function execute(address, opcode)
  if not active then
    active = true
    sequence = sequence + 1
    steps = 0
    record("entry", address)
  end
  steps = steps + 1
  if opcode == 0 then
    record("stop", address)
    active = false
  end
end

local function end_frame()
  frame = frame + 1
  if frame < 220 then return end
  local file = assert(io.open(
    emu.getScriptDataFolder() .. "/jobs.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.log("SF2_GSU_JOB_BOUNDARIES_DONE")
  emu.stop(0)
end

emu.addMemoryCallback(execute, emu.callbackType.exec, 0x000000, 0x7FFFFF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
