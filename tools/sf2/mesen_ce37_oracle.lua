-- Capture complete cartridge-RAM state around the first $01:CE37 Super FX job.

local frame = 0
local active = false
local dumped_entry = false
local dumped_exit = false

local function write_file(name, data)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "wb"))
  file:write(data)
  file:close()
end

local function dump_ram(name)
  local bytes = {}
  for address = 0, 0xFFFF do
    bytes[#bytes + 1] = string.char(
      emu.read(address, emu.memType.gsuWorkRam, false))
  end
  write_file(name, table.concat(bytes))
end

local function entry()
  if dumped_entry then return end
  dump_ram("ce37-entry.bin")
  dumped_entry = true
  active = true
end

local function stop()
  if not active or dumped_exit then return end
  dump_ram("ce37-exit.bin")
  dumped_exit = true
  active = false
  emu.log("SF2_CE37_ORACLE_DONE frame=" .. frame)
  emu.stop(0)
end

local function end_frame()
  frame = frame + 1
  if frame >= 220 then
    emu.log("SF2_CE37_ORACLE_TIMEOUT")
    emu.stop(1)
  end
end

emu.addMemoryCallback(
  entry, emu.callbackType.exec, 0x01CE37, 0x01CE37,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addMemoryCallback(
  stop, emu.callbackType.exec, 0x01CE35, 0x01CE35,
  emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
