-- Trace the full-screen Super FX job at $01:CD99 through its normal STOP.

local frame = 0
local entry_count = 0
local stop_count = 0
local lines = { "kind count frame ram003A ram24C2 ram24C4 ram0014\n" }
local state_lines = {}
local dumped_entry_ram = false
local dumped_exit_ram = false
local trace_active = false
local pc_trace = {}
local write_lines = { "frame address value pc\n" }
local last_03c6 = 0
local last_24da = 0
local point_lines = {}
local last_ram_buffer = ""

local function dump_ram(name)
  local bytes = {}
  for address = 0, 0xFFFF do
    bytes[#bytes + 1] = string.char(emu.read(address, emu.memType.gsuWorkRam, false))
  end
  write_file(name, table.concat(bytes))
end

local function word(address)
  return emu.read16(address, emu.memType.gsuWorkRam, false)
end

local function probe(kind, count)
  lines[#lines + 1] = string.format(
    "%s %d %d %04X %04X %04X %04X\n",
    kind, count, frame, word(0x003A), word(0x24C2), word(0x24C4), word(0x0014))
end

local function entry()
  entry_count = entry_count + 1
  probe("entry", entry_count)
  if not dumped_entry_ram then
    dump_ram("cd99-entry.bin")
    dumped_entry_ram = true
    trace_active = true
    last_03c6 = word(0x03C6)
    last_24da = word(0x24DA)
  end
  if entry_count == 1 then
    local state = emu.getState()
    local keys = {}
    for key, _ in pairs(state) do keys[#keys + 1] = key end
    table.sort(keys)
    for _, key in ipairs(keys) do
      local lower = string.lower(key)
      if string.find(lower, "coprocessor", 1, true)
        or string.find(lower, "gsu", 1, true) then
        state_lines[#state_lines + 1] = key .. "=" .. tostring(state[key]) .. "\n"
      end
    end
  end
end

local function stop()
  stop_count = stop_count + 1
  probe("stop", stop_count)
  if not dumped_exit_ram then
    pc_trace[#pc_trace + 1] = string.pack("<I4", 0x01CE35)
    dump_ram("cd99-exit.bin")
    dumped_exit_ram = true
    trace_active = false
  end
end

local function trace(address)
  if not trace_active then return end
  pc_trace[#pc_trace + 1] = string.pack("<I4", address)
  local bus_state = emu.getState()
  local ram_address = bus_state["cart.coprocessor.ramWriteAddress"] or 0
  if ram_address == 0x03C6 or ram_address == 0x03C7 or ram_address == 0x24DA then
    local key = string.format(
      "%04X:%02X:%d", ram_address,
      bus_state["cart.coprocessor.ramWriteValue"] or 0,
      bus_state["cart.coprocessor.ramDelay"] or 0)
    if key ~= last_ram_buffer then
      write_lines[#write_lines + 1] = string.format(
        "%d BUFFER %s %06X\n", frame, key, address)
      last_ram_buffer = key
    end
  end
  if address == 0x01D805 or address == 0x01D807 or address == 0x01D80B
    or address == 0x01D80E or address == 0x01D810 or address == 0x01D812
    or address == 0x01D813 or address == 0x01D818 then
    local state = emu.getState()
    local regs = {}
    for index = 0, 15 do
      regs[#regs + 1] = string.format(
        "%04X", state["cart.coprocessor.r" .. index] or 0)
    end
    point_lines[#point_lines + 1] = string.format(
      "%06X %s S=%s Z=%s C=%s O=%s A1=%s A2=%s SRC=%s DST=%s RAMB=%s RAMA=%04X RAMD=%02X\n",
      address,
      table.concat(regs, ","),
      tostring(state["cart.coprocessor.sfr.sign"]),
      tostring(state["cart.coprocessor.sfr.zero"]),
      tostring(state["cart.coprocessor.sfr.carry"]),
      tostring(state["cart.coprocessor.sfr.overflow"]),
      tostring(state["cart.coprocessor.sfr.alt1"]),
      tostring(state["cart.coprocessor.sfr.alt2"]),
      tostring(state["cart.coprocessor.srcReg"]),
      tostring(state["cart.coprocessor.destReg"]),
      tostring(state["cart.coprocessor.ramBank"]),
      state["cart.coprocessor.ramAddress"] or 0,
      state["cart.coprocessor.ramWriteValue"] or 0)
  end
  local value_03c6 = word(0x03C6)
  local value_24da = word(0x24DA)
  if value_03c6 ~= last_03c6 then
    write_lines[#write_lines + 1] = string.format(
      "%d 03C6 %04X %06X\n", frame, value_03c6, address)
    last_03c6 = value_03c6
  end
  if value_24da ~= last_24da then
    write_lines[#write_lines + 1] = string.format(
      "%d 24DA %04X %06X\n", frame, value_24da, address)
    last_24da = value_24da
  end
end

local function trace_write(address, value)
  if not trace_active then return end
  local offset = address & 0xFFFF
  if offset ~= 0x03C6 and offset ~= 0x03C7 and offset ~= 0x24DA
    and offset ~= 0x4128 and offset ~= 0x41C8 then return end
  local state = emu.getState()
  write_lines[#write_lines + 1] = string.format(
    "%d %04X %02X %04X\n",
    frame, address, value, state["cart.coprocessor.r15"] or 0)
end

function write_file(name, data)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "wb"))
  file:write(data)
  file:close()
end

local function end_frame()
  frame = frame + 1
  if frame >= 500 then
    write_file("cd99.txt", table.concat(lines))
    write_file("state_keys.txt", table.concat(state_lines))
    write_file("cd99-pc-trace.bin", table.concat(pc_trace))
    write_file("cd99-writes.txt", table.concat(write_lines))
    write_file("cd99-point-states.txt", table.concat(point_lines))
    emu.log("SF2_CD99_ORACLE_DONE")
    emu.stop(0)
  end
end

emu.addMemoryCallback(
  entry, emu.callbackType.exec, 0x01CD99, 0x01CD99,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(
  stop, emu.callbackType.exec, 0x01CE35, 0x01CE35,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(
  trace, emu.callbackType.exec, 0x000000, 0x7FFFFF,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(
  trace_write, emu.callbackType.write, 0x0000, 0xFFFF,
  emu.cpuType.gsu, emu.memType.gsuWorkRam)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
