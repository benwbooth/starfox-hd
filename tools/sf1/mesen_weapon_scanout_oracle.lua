-- Original-game laser scanouts and BG1 data, with source-bound controller
-- sampling. No machine state is injected and no native output is read.
local video = 0
local requested = false
local active = false
local next_fire = false
local first_scene = 310
local last_scene = 340
local observed_scenes = {}
local captured_scanouts = 0
local output = { "rom_sha1=" .. emu.getRomInfo().fileSha1Hash:lower() .. "\n" }
local folder = emu.getScriptDataFolder()

local function word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end
local function byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end
local function fire(scene)
  return scene >= 319 and scene <= 321
end
local function front_end_pad(tick)
  if tick >= 380 and tick < 382 then return { down = true } end
  if tick >= 420 and tick < 422 then return { start = true } end
  if tick <= 360 and tick % 60 < 2 then return { start = true } end
  if tick >= 500 and tick < 512 then return { start = true } end
  if tick >= 840 and tick < 900 and (tick - 840) % 2 == 0 then return { b = true } end
  return {}
end
local function write(name, data)
  local file = assert(io.open(folder .. "/" .. name, "wb"))
  file:write(data)
  file:close()
end
-- Mesen logs a Lua callback exception but may keep other callbacks running.
-- Make every failed invariant terminate the runner, not just that callback.
local function guard(callback)
  return function(...)
    local ok, message = pcall(callback, ...)
    if not ok then
      write("weapon_error.txt", tostring(message))
      emu.log(tostring(message))
      emu.stop(1)
    end
  end
end
local function finish(status)
  if status == 0 then
    assert(captured_scanouts > 0, "missing weapon scanouts")
    for scene = 0, last_scene do
      assert(observed_scenes[scene], "missing weapon strategy visit")
    end
  end
  write("weapon_display.txt", table.concat(output))
  emu.stop(status)
end
emu.addEventCallback(guard(function()
  local input = { a = false, b = false, x = false, y = false, l = false, r = false,
    start = false, select = false, up = false, down = false, left = false, right = false }
  local buttons = active and { y = next_fire } or front_end_pad(math.floor(video / 3))
  for button, pressed in pairs(buttons) do input[button] = pressed end
  emu.setInput(input, 0)
end), emu.eventType.inputPolled)
emu.addEventCallback(guard(function()
  video = video + 1
  if active then
    local scene = word(0x15BB)
    if scene >= first_scene and scene <= last_scene then
      local size = emu.getScreenSize()
      local screen = emu.getScreenBuffer()
      local image = { string.format("P6\n%d %d\n255\n", size.width, size.height) }
      for index = 1, size.width * size.height do
        local pixel = assert(screen[index], "missing Mesen screen pixel")
        image[#image + 1] = string.char((pixel >> 16) & 255, (pixel >> 8) & 255, pixel & 255)
      end
      local name = string.format("weapon_%04d.ppm", video)
      write(name, table.concat(image))
      local state = emu.getState()
      assert((state["ppu.bgMode"] == 1 or state["ppu.bgMode"] == 2)
        and not state["ppu.layers[0].largeTiles"])
      assert(not state["ppu.layers[0].doubleWidth"] and not state["ppu.layers[0].doubleHeight"])
      local vram = {}
      for address = 0, 65535 do
        vram[#vram + 1] = string.char(emu.read(address, emu.memType.snesVideoRam, false))
      end
      local vram_name = string.format("weapon_%04d.vram", video)
      write(vram_name, table.concat(vram))
      output[#output + 1] = string.format(
        "video=%d scene=%d image=%s vram=%s bg1_map=%d bg1_chr=%d bg1_hscroll=%d bg1_vscroll=%d\n",
        video, scene, name, vram_name,
        state["ppu.layers[0].tilemapAddress"] * 2, state["ppu.layers[0].chrAddress"] * 2,
        state["ppu.layers[0].hscroll"], state["ppu.layers[0].vscroll"])
      captured_scanouts = captured_scanouts + 1
    end
    if scene > last_scene then finish(0) end
  end
  if video > 6000 then finish(1) end
end), emu.eventType.endFrame)
emu.addMemoryCallback(guard(function() requested = true end), emu.callbackType.exec,
  0x03C437, 0x03C437, emu.cpuType.snes, emu.memType.snesMemory)
emu.addMemoryCallback(guard(function()
  local scene = word(0x15BB)
  if requested and scene == 0 then active = true end
  if not active then return end
  local sampled_pad = (byte(0x1202) << 8) | byte(0x1204)
  assert(sampled_pad == (fire(scene) and 0x4000 or 0),
    string.format("weapon controller mismatch at scene %d: %04x", scene, sampled_pad))
  -- The current strategy already owns the latched pad. Publish physical
  -- buttons for the IRQ/controller sample used by the next strategy visit.
  next_fire = fire(scene + 1)
  observed_scenes[scene] = true
  output[#output + 1] = string.format("kind=input video=%d scene=%d sampled_pad=%d\n",
    video, scene, sampled_pad)
end), emu.callbackType.exec, 0x02DAF2, 0x02DAF2, emu.cpuType.snes, emu.memType.snesMemory)
emu.log("SF1_WEAPON_SCANOUT_ORACLE_LOADED")
