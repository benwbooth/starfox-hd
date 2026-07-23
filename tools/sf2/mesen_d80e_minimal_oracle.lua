-- Minimal, non-invasive probe of the first $CD99 renderer's stack-sentinel
-- load.  Unlike the full trace, this performs no GSU RAM reads.

local frame = 0
local active = false
local lines = {}

local function entry()
  if not active and #lines == 0 then active = true end
end

local function point(address)
  if not active then return end
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "%06X r1=%04X r10=%04X r11=%04X S=%s RAMB=%s RAMA=%04X\n",
    address,
    state["cart.coprocessor.r1"] or 0,
    state["cart.coprocessor.r10"] or 0,
    state["cart.coprocessor.r11"] or 0,
    tostring(state["cart.coprocessor.sfr.sign"]),
    tostring(state["cart.coprocessor.ramBank"]),
    state["cart.coprocessor.ramAddress"] or 0)
end

local function stop()
  if not active then return end
  active = false
end

local function end_frame()
  frame = frame + 1
  if frame < 220 then return end
  local file = assert(io.open(emu.getScriptDataFolder() .. "/points.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.stop(0)
end

emu.addMemoryCallback(entry, emu.callbackType.exec, 0x01CD99, 0x01CD99,
  emu.cpuType.gsu, emu.memType.gsuMemory)
for _, address in ipairs({ 0x01D80E, 0x01D810, 0x01D812, 0x01D813, 0x01D818 }) do
  emu.addMemoryCallback(point, emu.callbackType.exec, address, address,
    emu.cpuType.gsu, emu.memType.gsuMemory)
end
emu.addMemoryCallback(stop, emu.callbackType.exec, 0x01CE35, 0x01CE35,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
