-- Capture the first 1,000 pre-instruction states of the first $01:D9FF job.

local active = false
local lines = {}

local function boolean_bit(value, bit)
  if value then return bit else return 0 end
end

local function finish()
  local file = assert(io.open(
    emu.getScriptDataFolder() .. "/d9ff-head-trace.txt", "wb"))
  file:write(table.concat(lines))
  file:close()
  emu.log("SF2_D9FF_HEAD_TRACE_DONE")
  emu.stop(0)
end

local function entry()
  active = true
end

local function trace(address)
  if not active then return end
  local state = emu.getState()
  local fields = { string.format("%06X", address) }
  for index = 0, 15 do
    fields[#fields + 1] = string.format(
      "%04X", state["cart.coprocessor.r" .. index] or 0)
  end
  local sfr = boolean_bit(state["cart.coprocessor.sfr.zero"], 0x02)
    | boolean_bit(state["cart.coprocessor.sfr.carry"], 0x04)
    | boolean_bit(state["cart.coprocessor.sfr.sign"], 0x08)
    | boolean_bit(state["cart.coprocessor.sfr.overflow"], 0x10)
    | boolean_bit(state["cart.coprocessor.sfr.running"], 0x20)
  fields[#fields + 1] = string.format("%04X", sfr)
  fields[#fields + 1] = string.format("%02X", state["cart.coprocessor.srcReg"] or 0)
  fields[#fields + 1] = string.format("%02X", state["cart.coprocessor.destReg"] or 0)
  fields[#fields + 1] = state["cart.coprocessor.sfr.alt1"] and "1" or "0"
  fields[#fields + 1] = state["cart.coprocessor.sfr.alt2"] and "1" or "0"
  fields[#fields + 1] = state["cart.coprocessor.sfr.prefix"] and "1" or "0"
  fields[#fields + 1] = string.format("%02X", state["cart.coprocessor.romBank"] or 0)
  fields[#fields + 1] = string.format("%04X", state["cart.coprocessor.ramAddress"] or 0)
  lines[#lines + 1] = table.concat(fields, " ") .. "\n"
  if #lines >= 1000 then finish() end
end

emu.addMemoryCallback(entry, emu.callbackType.exec, 0x01D9FF, 0x01D9FF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(trace, emu.callbackType.exec, 0x000000, 0x7FFFFF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
