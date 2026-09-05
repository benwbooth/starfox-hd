-- Bounded oracle trace for the native-reconstruction candidate SF2 boot scene.
-- Captures scene fields and source-writer evidence for verification only.
-- Neither machine state nor sampled animation tracks enter shipping Rust.

local frame = 0
local stop_frame = tonumber(os.getenv("SF2_INTRO_TRACE_STOP")) or 800
local step = tonumber(os.getenv("SF2_INTRO_TRACE_STEP")) or 4
assert(stop_frame > 0 and step > 0, "trace stop and step must be positive")
local lines = {}
local installations = {}
local draw_record_size = 0x26
local draw_list_start = 0xB273

local function w8(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function w16(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function s16(address)
  local value = w16(address)
  return value >= 0x8000 and value - 0x10000 or value
end

local function draw_list()
  local out = {}
  local count = w16(0x18C6)
  assert(count <= 64, "draw list exceeds the reviewed capacity")
  for index = 0, count - 1 do
    local record = draw_list_start + index * draw_record_size
    out[#out + 1] = string.format(
      "%04X,%d,%d,%d,%d,%d,%d",
      w16(record + 8),
      s16(record + 32), s16(record + 34), s16(record + 36),
      w8(record + 4), w8(record + 5), w8(record + 6))
  end
  return table.concat(out, ";")
end

local function objects()
  local out = {}
  local seen = {}
  local base = w16(0x12A8)
  for _ = 1, 60 do
    if base == 0 then break end
    local delta = base - 0x03BD
    if delta < 0 or delta % 0x3F ~= 0 or delta / 0x3F >= 60 then
      error(string.format("invalid active object pointer %04X", base))
    end
    if seen[base] then error(string.format("active object loop at %04X", base)) end
    seen[base] = true
    local shape = w16(base + 4)
    local strategy = w8(base + 0x19) | (w8(base + 0x1A) << 8) | (w8(base + 0x1B) << 16)
    if (strategy == 0x7F7E00 or strategy == 0x7F7E53) and (shape ~= 0 or strategy ~= 0) then
      out[#out + 1] = string.format(
        "%04X,%06X,%d,%d,%d,%d,%d,%d,%d",
        shape, strategy, s16(base + 0x0C), s16(base + 0x0E), s16(base + 0x10),
        w8(base + 0x12), w8(base + 0x14), w8(base + 0x16), w16(base + 0x2B))
    end
    base = w16(base)
  end
  assert(base == 0, "active object list exceeds the reviewed capacity")
  return table.concat(out, ";")
end

local function record()
  lines[#lines + 1] = string.format(
    "frame=%d mode=%d submode=%d phase=%d map=%02X:%04X cursor=%04X camera=%d,%d,%d,%d,%d,%d objects=%s draws=%s",
    frame, w8(0x1B68), w8(0x1B76), w8(0x1BE0), w8(0x192E), w16(0x1657), w16(0x1C20),
    s16(0x034B), s16(0x034D), s16(0x034F),
    w8(0x0351), w8(0x0353), w8(0x0355), objects(), draw_list())
end

local function input()
  emu.setInput({start=false, select=false, a=false, b=false, x=false,
    y=false, l=false, r=false, up=false, down=false, left=false, right=false}, 0)
end

local function path_write(address, value)
  address = address & 0xFFFF
  if address < 0x03BD or address >= 0x03BD + 60 * 0x3F then return end
  local field = (address - 0x03BD) % 0x3F
  if frame > 160 or value == 0 or (field ~= 0x2B and field ~= 0x2C) then return end
  local state = emu.getState()
  installations[#installations + 1] = string.format(
    "frame=%d object=%04X field=%02X value=%04X writer=%02X:%04X",
    frame, address - field, field, value, state["cpu.k"], state["cpu.pc"])
end

local function end_frame()
  frame = frame + 1
  if frame % step == 0 then record() end
  if frame >= stop_frame then
    local file = assert(io.open(emu.getScriptDataFolder() .. "/sf2_intro_scene_trace.txt", "wb"))
    file:write(table.concat(lines, "\n") .. "\n")
    file:close()
    local writers = assert(io.open(emu.getScriptDataFolder() .. "/sf2_intro_path_writers.txt", "wb"))
    writers:write(table.concat(installations, "\n") .. "\n")
    writers:close()
    emu.log("SF2_INTRO_SCENE_TRACE_DONE")
    emu.stop(0)
  end
end

emu.addEventCallback(input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(path_write, emu.callbackType.write, 0x03BD,
  0x03BD + 60 * 0x3F - 1, emu.cpuType.snes, emu.memType.snesWorkRam)
