-- Capture SF2's generated bank-$7E WRAM through Mesen's independent emulator.
--
-- Mesen persists GSU work RAM as the game's 64 KiB .srm.  Once the retail
-- $19:9F9C stream has decompressed and the S-CPU has had two frames to install
-- it, this script copies all of WRAM bank $7E into GSU RAM and exits.  The .srm
-- is then an exact, externally generated bank-$7E oracle snapshot.

local active_source_bank = -1
local active_source_address = -1
local capture_countdown = -1
local frames = 0

local function read_gsu_word(address)
  return emu.read16(address, emu.memType.gsuWorkRam, false)
end

local function decompressor_entry()
  active_source_address = read_gsu_word(0x0068)
  active_source_bank = read_gsu_word(0x006A) & 0x7F
end

local function decompressor_stop()
  if active_source_bank == 0x19 and active_source_address == 0x9F9C then
    -- The decompressor stops before the S-CPU's long installation/copy pass.
    -- Give that host-side pass two seconds of NTSC frames to settle.
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
    local byte = emu.read(address, emu.memType.snesWorkRam, false)
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
