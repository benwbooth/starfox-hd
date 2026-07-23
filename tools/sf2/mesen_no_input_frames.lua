-- Capture fixed retail Star Fox 2 frames with an idle controller. These are
-- the visual/state oracle for the Rust retail machine's unattended boot path.

local frame = 0
local capture_frames = { [153] = true, [166] = true, [300] = true,
  [500] = true, [1000] = true }

local function write(name, data)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(data)
  file:close()
end

local function capture_screen(number)
  local size = emu.getScreenSize()
  local screen = emu.getScreenBuffer()
  local output = { string.format("P6\n%d %d\n255\n", size.width, size.height) }
  for index = 1, size.width * size.height do
    local pixel = screen[index] or 0
    output[#output + 1] = string.char(
      (pixel >> 16) & 0xFF, (pixel >> 8) & 0xFF, pixel & 0xFF)
  end
  write(string.format("frame-%04d.ppm", number), table.concat(output))
end

local function dump_memory(name, kind, length)
  local output = {}
  for address = 0, length - 1 do
    output[#output + 1] = string.char(emu.read(address, kind, false))
  end
  write(name, table.concat(output))
end

local function capture_state(number)
  local state = emu.getState()
  local keys = {}
  for key, _ in pairs(state) do keys[#keys + 1] = key end
  table.sort(keys)
  local lines = {}
  for _, key in ipairs(keys) do
    local lower = string.lower(key)
    if string.find(lower, "ppu", 1, true)
      or string.find(lower, "bg", 1, true)
      or string.find(lower, "screen", 1, true)
      or string.find(lower, "brightness", 1, true)
      or string.find(lower, "window", 1, true)
      or string.find(lower, "color", 1, true)
      or string.find(lower, "dma", 1, true)
      or string.find(lower, "cpu.pc", 1, true)
      or string.find(lower, "cpu.k", 1, true) then
      lines[#lines + 1] = key .. "=" .. tostring(state[key]) .. "\n"
    end
  end
  write(string.format("state-%04d.txt", number), table.concat(lines))
end

local function end_frame()
  frame = frame + 1
  if capture_frames[frame] then
    capture_screen(frame)
    capture_state(frame)
  end
  if frame < 1000 then return end

  dump_memory("wram.bin", emu.memType.snesWorkRam, 0x20000)
  dump_memory("gsuram.bin", emu.memType.gsuWorkRam, 0x10000)
  dump_memory("vram.bin", emu.memType.snesVideoRam, 0x10000)
  dump_memory("cgram.bin", emu.memType.snesCgRam, 0x200)
  dump_memory("oam.bin", emu.memType.snesSpriteRam, 544)
  local state = emu.getState()
  local ppu_lines = {}
  local keys = {}
  for key, _ in pairs(state) do keys[#keys + 1] = key end
  table.sort(keys)
  for _, key in ipairs(keys) do
    local lower = string.lower(key)
    if string.find(lower, "ppu", 1, true)
      or string.find(lower, "bg", 1, true)
      or string.find(lower, "screen", 1, true)
      or string.find(lower, "brightness", 1, true)
      or string.find(lower, "window", 1, true)
      or string.find(lower, "color", 1, true) then
      ppu_lines[#ppu_lines + 1] = key .. "=" .. tostring(state[key]) .. "\n"
    end
  end
  write("ppu-state.txt", table.concat(ppu_lines))
  write("summary.txt", string.format(
    "frame=%d cpu=%02X:%04X master=%d map=%02X:%04X active=%04X draw=%04X\n",
    frame, state["cpu.k"] or 0, state["cpu.pc"] or 0,
    state["memoryManager.masterClock"] or 0,
    emu.read(0x192E, emu.memType.snesWorkRam, false),
    emu.read16(0x1657, emu.memType.snesWorkRam, false),
    emu.read16(0x12A8, emu.memType.snesWorkRam, false),
    emu.read16(0x18C6, emu.memType.snesWorkRam, false)))
  emu.stop(0)
end

emu.addEventCallback(end_frame, emu.eventType.endFrame)
