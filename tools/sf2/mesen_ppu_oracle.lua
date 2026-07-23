-- Capture independent Mesen PPU memories/register state at a fixed frame.

local frame = 0
local armed = false
local armed_frame = -1
local capture_elapsed = 3840
local dma_lines = {}

local function path(name)
  return emu.getScriptDataFolder() .. "/" .. name
end

local function write(name, data)
  local file = assert(io.open(path(name), "w+b"))
  file:write(data)
  file:close()
end

local function dump_memory(name, kind, length)
  local out = {}
  for address = 0, length - 1 do
    out[#out + 1] = string.char(emu.read(address, kind, false))
  end
  write(name, table.concat(out))
end

local function end_frame()
  frame = frame + 1
  if not armed or frame - armed_frame ~= capture_elapsed then
    return
  end
  dump_memory("vram.bin", emu.memType.snesVideoRam, 0x10000)
  dump_memory("cgram.bin", emu.memType.snesCgRam, 0x200)
  dump_memory("oam.bin", emu.memType.snesSpriteRam, 544)
  dump_memory("gsuram.bin", emu.memType.gsuWorkRam, 0x10000)
  write("dma.txt", table.concat(dma_lines))
  local state = emu.getState()
  local keys = {}
  for key, _ in pairs(state) do
    keys[#keys + 1] = key
  end
  table.sort(keys)
  local lines = {}
  for _, key in ipairs(keys) do
    local lower = string.lower(key)
    if string.find(lower, "ppu", 1, true)
      or string.find(lower, "bg", 1, true)
      or string.find(lower, "screen", 1, true)
      or string.find(lower, "brightness", 1, true) then
      lines[#lines + 1] = key .. "=" .. tostring(state[key]) .. "\n"
    end
  end
  write("state.txt", table.concat(lines))
  emu.log("SF2_PPU_ORACLE_DONE")
  emu.stop(0)
end

local function dma(_, enabled)
  for channel = 0, 7 do
    if (enabled & (1 << channel)) ~= 0 then
      local base = 0x4300 + channel * 0x10
      local dmap = emu.read(base, emu.memType.snesMemory, false)
      local bbad = emu.read(base + 1, emu.memType.snesMemory, false)
      local source = emu.read16(base + 2, emu.memType.snesMemory, false)
      local bank = emu.read(base + 4, emu.memType.snesMemory, false)
      local length = emu.read16(base + 5, emu.memType.snesMemory, false)
      if length == 0 then length = 0x10000 end
      if bbad == 0x18 then
        local nonzero = 0
        local hash = 0x811C9DC5
        local address = source
        for index = 0, length - 1 do
          local value = emu.read((bank << 16) | address, emu.memType.snesMemory, false)
          if value ~= 0 then nonzero = nonzero + 1 end
          hash = ((hash ~ value) * 0x01000193) & 0xFFFFFFFF
          if (dmap & 0x08) == 0 then
            if (dmap & 0x10) ~= 0 then
              address = (address - 1) & 0xFFFF
            else
              address = (address + 1) & 0xFFFF
            end
          end
        end
        dma_lines[#dma_lines + 1] = string.format(
          "%d source=%02X:%04X length=%04X nonzero=%d fnv=%08X\n",
          frame, bank, source, length, nonzero, hash)
      end
    end
  end
end

local function input()
  local elapsed = frame - armed_frame
  local start_phase = frame % 180
  local accept_phase = elapsed % 90
  emu.setInput({
    start = (start_phase == 120 or start_phase == 121) and (not armed or elapsed <= 600),
    b = armed and elapsed >= 210 and (accept_phase == 30 or accept_phase == 31),
  }, 0)
end

local function arm()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame
  end
end

emu.addEventCallback(input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(
  arm,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  dma,
  emu.callbackType.write,
  0x00420B,
  0x00420B,
  emu.cpuType.snes,
  emu.memType.snesMemory)
