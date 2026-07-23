-- Trace initialization of the GSU RAM dimensions consumed by the $01:CD99
-- full-screen job.  The Rust host previously reached that job with zeros at
-- $003A/$24C2/$24C4, turning finite image loops into 16-bit wraparound loops.

local frame = 0
local lines = { "frame address value cpu_pc gsu_pc\n" }
local seen = {}

local function output_path()
  return emu.getScriptDataFolder() .. "/writes.txt"
end

local function trace_write(address, value)
  local key = string.format("%04X=%02X", address, value)
  if seen[key] then return end
  seen[key] = true
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "%d %04X %02X %04X %04X\n",
    frame,
    address,
    value,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.pc"] or state["gsu.pc"] or 0)
end

local function finish()
  local file = assert(io.open(output_path(), "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.log("SF2_GSURAM_INIT_ORACLE_DONE")
  emu.stop(0)
end

local function end_frame()
  frame = frame + 1
  if frame >= 220 then finish() end
end

for _, range in ipairs({
  { 0x003A, 0x003B },
  { 0x24C2, 0x24C5 },
  { 0x3A52, 0x3A53 },
}) do
  emu.addMemoryCallback(
    trace_write,
    emu.callbackType.write,
    range[1],
    range[2],
    emu.cpuType.gsu,
    emu.memType.gsuWorkRam)
end

emu.addEventCallback(end_frame, emu.eventType.endFrame)
