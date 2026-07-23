-- Record the early CPU-triggered Super FX jobs and their critical RAM inputs.
-- This gives the Rust retail machine an instruction-independent job boundary
-- oracle: the write to R15 high ($301F) starts a job on real hardware.

local frame = 0
local sequence = 0
local lines = {
  "sequence frame pbr pc ram003A ram24C2 ram24C4 ram0014 cpu_pc\n"
}

local function word(address)
  return emu.read16(address, emu.memType.gsuWorkRam, false)
end

local function kick(_, high)
  sequence = sequence + 1
  local low = emu.read(0x00301E, emu.memType.snesMemory, false)
  local pbr = emu.read(0x003034, emu.memType.snesMemory, false) & 0x7F
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "%d %d %02X %04X %04X %04X %04X %04X %06X\n",
    sequence,
    frame,
    pbr,
    ((high << 8) | low) & 0xFFFF,
    word(0x003A),
    word(0x24C2),
    word(0x24C4),
    word(0x0014),
    state["cpu.pc"] or 0)
end

local function finish()
  local path = emu.getScriptDataFolder() .. "/jobs.txt"
  local file = assert(io.open(path, "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.log("SF2_GSU_JOBS_ORACLE_DONE")
  emu.stop(0)
end

local function end_frame()
  frame = frame + 1
  if frame >= 120 then finish() end
end

emu.addMemoryCallback(
  kick,
  emu.callbackType.write,
  0x00301F,
  0x00301F,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
