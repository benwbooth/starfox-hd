-- Record master-clock boundaries for every early retail Star Fox 2 GSU job.
-- Opcode callbacks expose the prefetched instruction stream: the first one
-- after STOP is the entry and opcode $00 is the completion boundary.

local frame = 0
local sequence = 0
local active = false
local steps = 0
local entry_master = 0
local entry_gsu = 0
local entry_address = 0
local entry_frame = 0
local entry_cache = 0
local entry_clock_select = 0
local entry_high_speed = 0
local lines = {
  "sequence entry_frame stop_frame pbr pc steps entry_master stop_master duration entry_gsu stop_gsu cache_base clock_select high_speed\n"
}

local function execute(address, opcode)
  if not active then
    local state = emu.getState()
    active = true
    sequence = sequence + 1
    steps = 0
    entry_address = address
    entry_master = state["memoryManager.masterClock"] or 0
    entry_gsu = state["cart.coprocessor.cycleCount"] or 0
    entry_frame = state["ppu.frameCount"] or frame
    entry_cache = state["cart.coprocessor.cacheBase"] or 0
    entry_clock_select = state["cart.coprocessor.clockSelect"] and 1 or 0
    entry_high_speed = state["cart.coprocessor.highSpeedMode"] and 1 or 0
  end
  steps = steps + 1
  if opcode == 0 then
    local state = emu.getState()
    local stop_master = state["memoryManager.masterClock"] or 0
    lines[#lines + 1] = string.format(
      "%d %d %d %02X %04X %d %d %d %d %d %d %04X %d %d\n",
      sequence, entry_frame, state["ppu.frameCount"] or frame,
      (entry_address >> 16) & 0xFF, entry_address & 0xFFFF, steps,
      entry_master, stop_master, stop_master - entry_master,
      entry_gsu, state["cart.coprocessor.cycleCount"] or 0,
      entry_cache, entry_clock_select, entry_high_speed)
    active = false
  end
end

local function end_frame()
  frame = frame + 1
  if frame < 220 then return end
  local file = assert(io.open(
    emu.getScriptDataFolder() .. "/job-timing.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.log("SF2_GSU_JOB_TIMING_DONE")
  emu.stop(0)
end

emu.addMemoryCallback(execute, emu.callbackType.exec, 0x000000, 0x7FFFFF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
