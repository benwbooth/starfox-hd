-- Capture SF2's live bank-$7F WRAM in GSU battery RAM for byte-for-byte
-- comparison with the reset-time ROM copies.  This complements
-- mesen_wram_oracle.lua, which captures bank $7E.

local active_source_bank = -1
local active_source_address = -1
local capture_countdown = -1
local frames = 0

local function decompressor_entry()
  active_source_address = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  active_source_bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
end

local function decompressor_stop()
  if active_source_bank == 0x19 and active_source_address == 0x9F9C then
    capture_countdown = 120
  end
end

local function end_frame()
  if capture_countdown < 0 then
    return
  end
  if capture_countdown > 0 then
    capture_countdown = capture_countdown - 1
    return
  end

  for address = 0, 0xFFFF do
    local byte = emu.read(0x10000 + address, emu.memType.snesWorkRam, false)
    emu.write(address, byte, emu.memType.gsuWorkRam)
  end
  emu.stop(0)
end

local function provide_input()
  frames = frames + 1
  local phase = frames % 180
  emu.setInput({ start = phase == 120 or phase == 121 }, 0)
end

emu.addMemoryCallback(
  decompressor_entry,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

emu.addMemoryCallback(
  decompressor_stop,
  emu.callbackType.exec,
  0x01DAE2,
  0x01DAE2,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
