-- Read-only original-game display evidence for the first launch reveal.
local video = 0
local requested = false
local active = false
local output = {}
local folder = emu.getScriptDataFolder()

local function word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end
local function byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end
local function pad(tick)
  if tick >= 380 and tick < 382 then return { down = true } end
  if tick >= 420 and tick < 422 then return { start = true } end
  if tick <= 360 and tick % 60 < 2 then return { start = true } end
  if tick >= 500 and tick < 512 then return { start = true } end
  if tick >= 840 and tick < 900 and (tick - 840) % 2 == 0 then return { b = true } end
  return {}
end
local function finish(status)
  local file = assert(io.open(folder .. "/launch_display.txt", "wb"))
  file:write(table.concat(output))
  file:close()
  emu.stop(status)
end
local function end_frame()
  video = video + 1
  if active then
    local scene = word(0x15BB)
    if scene >= 4 and scene <= 22 then
      local size = emu.getScreenSize()
      local screen = emu.getScreenBuffer()
      local image = { string.format("P6\n%d %d\n255\n", size.width, size.height) }
      for index = 1, size.width * size.height do
        local pixel = screen[index] or 0
        image[#image + 1] = string.char((pixel >> 16) & 255, (pixel >> 8) & 255, pixel & 255)
      end
      local name = string.format("launch_%04d.ppm", video)
      local file = assert(io.open(folder .. "/" .. name, "wb"))
      file:write(table.concat(image))
      file:close()
      output[#output + 1] = string.format("video=%d scene=%d level=%d direction=%d display=%d image=%s\n",
        video, scene, byte(0x18B3), byte(0x18B2), byte(0x45F4), name)
    end
    if scene > 22 then finish(0) end
  end
  if video > 4500 then finish(1) end
end
emu.addEventCallback(function()
  emu.setInput(active and {} or pad(math.floor(video / 3)), 0)
end, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(function() requested = true end, emu.callbackType.exec,
  0x03C437, 0x03C437, emu.cpuType.snes, emu.memType.snesMemory)
emu.addMemoryCallback(function()
  if requested and word(0x15BB) == 0 then active = true end
end, emu.callbackType.exec, 0x02DAF2, 0x02DAF2, emu.cpuType.snes, emu.memType.snesMemory)
emu.log("SF1_LAUNCH_DISPLAY_ORACLE_LOADED")
