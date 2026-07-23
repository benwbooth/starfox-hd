-- Read-only retail oracle for isolating the PPU background behind an SF2
-- mission. Source-machine registers and savestate loading stay in this helper;
-- the shipping Rust renderer consumes only semantic, native textures.

local state_path = assert(
  os.getenv("SF2_ORACLE_LOAD_STATE"),
  "SF2_ORACLE_LOAD_STATE must name a Mesen savestate")
local layer_mask = tonumber(os.getenv("SF2_ORACLE_BACKGROUND_MASK")) or 15
local capture_frames = tonumber(os.getenv("SF2_ORACLE_CAPTURE_FRAMES")) or 4
local label = os.getenv("SF2_ORACLE_LABEL") or "mission"

assert(layer_mask >= 0 and layer_mask <= 15, "background mask must be 0..15")
assert(capture_frames >= 2, "capture needs at least two frames after loading")

local state_file = assert(io.open(state_path, "r+b"))
local state_bytes = state_file:read("*a")
state_file:close()

local loaded = false
local frame = 0
local load_callback = nil

local function script_path(filename)
  return emu.getScriptDataFolder() .. "/" .. filename
end

local function write_binary(filename, contents)
  local file = assert(io.open(script_path(filename), "w+b"))
  file:write(contents)
  file:close()
end

local function dump_memory(filename, kind, length)
  local contents = {}
  for address = 0, length - 1 do
    contents[#contents + 1] = string.char(emu.read(address, kind, false))
  end
  write_binary(filename, table.concat(contents))
end

local function capture_ppu_state()
  dump_memory(
    string.format("%s_vram.bin", label),
    emu.memType.snesVideoRam,
    0x10000)
  dump_memory(
    string.format("%s_cgram.bin", label),
    emu.memType.snesCgRam,
    0x200)
  local state = emu.getState()
  local keys = {}
  for key, _ in pairs(state) do
    local lower = string.lower(key)
    if string.find(lower, "ppu", 1, true)
      or string.find(lower, "brightness", 1, true) then
      keys[#keys + 1] = key
    end
  end
  table.sort(keys)
  local lines = {}
  for _, key in ipairs(keys) do
    lines[#lines + 1] = key .. "=" .. tostring(state[key]) .. "\n"
  end
  write_binary(
    string.format("%s_ppu_state.txt", label),
    table.concat(lines))
end

local function load_state()
  if loaded then return end
  loaded = true
  emu.loadSavestate(state_bytes)
end

local function isolate_background()
  if not loaded then return end
  -- startFrame runs after the retail vblank register setup and directly before
  -- the visible raster. Keep only BG1..BG4 on the main screen and disable the
  -- sub-screen/color math so OBJ and 3D output cannot contaminate the capture.
  emu.write(0x212C, layer_mask, emu.memType.snesMemory)
  emu.write(0x212D, 0, emu.memType.snesMemory)
  emu.write(0x2131, 0, emu.memType.snesMemory)
end

local function capture_screen()
  local size = emu.getScreenSize()
  local screen = emu.getScreenBuffer()
  local output = { string.format("P6\n%d %d\n255\n", size.width, size.height) }
  for index = 1, size.width * size.height do
    local pixel = screen[index] or 0
    output[#output + 1] = string.char(
      (pixel >> 16) & 0xFF,
      (pixel >> 8) & 0xFF,
      pixel & 0xFF)
  end
  write_binary(
    string.format("%s_background_mask_%02d.ppm", label, layer_mask),
    table.concat(output))
end

local function neutral_input()
  if loaded then emu.setInput({}, 0) end
end

local function end_frame()
  if not loaded then return end
  frame = frame + 1
  if load_callback then
    emu.removeMemoryCallback(
      load_callback,
      emu.callbackType.exec,
      0x000000,
      0xFFFFFF,
      emu.cpuType.snes,
      emu.memType.snesMemory)
    load_callback = nil
  end
  if frame >= capture_frames then
    capture_screen()
    capture_ppu_state()
    emu.log(string.format(
      "SF2_MISSION_BACKGROUND_ORACLE_DONE %s mask=%d frames=%d",
      label,
      layer_mask,
      frame))
    emu.stop(0)
  end
end

load_callback = emu.addMemoryCallback(
  load_state,
  emu.callbackType.exec,
  0x000000,
  0xFFFFFF,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addEventCallback(neutral_input, emu.eventType.inputPolled)
emu.addEventCallback(isolate_background, emu.eventType.startFrame)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF2_MISSION_BACKGROUND_ORACLE_LOADED")
