-- Independent Star Fox 2 decompressor oracle for Mesen 2's GSU core.
--
-- Run with:
--   python3 tools/sf2/run_mesen_oracle.py --timeout 35 \
--     tools/sf2/mesen_decompress_oracle.lua
--
-- The script advances the retail title screen autonomously, records the ABI at
-- every $01:D9FF decompressor entry, and hashes the GSU RAM output at STOP.

local active_source_bank = -1
local active_source_address = -1
local frames = 0
local decompression_index = 0
local expected = {
  { bank = 0x16, source = 0xBFB8, first = 0x3B50, last = 0x5B70, hash = 0x68444FCF },
  { bank = 0x16, source = 0xC4E4, first = 0x5B50, last = 0x6B50, hash = 0x9B84A6AD },
  { bank = 0x18, source = 0xAF24, first = 0x6E18, last = 0x92D8, hash = 0x3E4A3761 },
  { bank = 0x16, source = 0xEF70, first = 0x3B50, last = 0x4B50, hash = 0x76C12B24 },
  { bank = 0x16, source = 0xFD7C, first = 0x3B50, last = 0x4350, hash = 0xA2FFB62F },
  { bank = 0x19, source = 0x9F9C, first = 0x3B50, last = 0x5B70, hash = 0xC78AFF13 },
}

local function read_word(address)
  return emu.read16(address, emu.memType.gsuWorkRam, false)
end

local function fnv1a(start_address, length)
  local hash = 0x811C9DC5
  for offset = 0, length - 1 do
    hash = ((hash ~ emu.read(start_address + offset, emu.memType.gsuWorkRam, false))
      * 0x01000193) & 0xFFFFFFFF
  end
  return hash
end

local function decompressor_entry()
  active_source_address = read_word(0x0068)
  active_source_bank = read_word(0x006A) & 0x7F
end

local function decompressor_stop()
  local output_start = read_word(0x002C)
  local output_end = read_word(0x0060)
  local hash = fnv1a(output_start, output_end - output_start)
  local record = string.format(
    "GSU_DECOMP source=%02X:%04X output=%04X..%04X fnv1a=%08X",
    active_source_bank, active_source_address, output_start, output_end, hash)
  emu.log(record)
  print(record)

  decompression_index = decompression_index + 1
  local wanted = expected[decompression_index]
  if wanted == nil
      or wanted.bank ~= active_source_bank
      or wanted.source ~= active_source_address
      or wanted.first ~= output_start
      or wanted.last ~= output_end
      or wanted.hash ~= hash then
    print("GSU_DECOMP_MISMATCH index=" .. tostring(decompression_index))
    emu.stop(1)
    return
  end

  if active_source_bank == 0x19 and active_source_address == 0x9F9C then
    print("GSU_DECOMP_OK count=" .. tostring(decompression_index))
    emu.stop(0)
  end
end

local function provide_input()
  frames = frames + 1
  -- Repeated, short Start pulses safely leave the title/attract loop without
  -- requiring a human to know the exact frame on which the title accepts it.
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

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
