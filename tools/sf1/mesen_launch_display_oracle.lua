-- Read-only original-game display evidence for the first launch reveal.
local video = 0
local requested = false
local active = false
local output = { "rom_sha1=" .. emu.getRomInfo().fileSha1Hash:lower() .. "\n" }
local window_writes = {}
local folder = emu.getScriptDataFolder()
local captured_edges = {}
local first_edge_scene = 4
local last_edge_scene = 19
local window_buffer_address = 0x0EF2
local window_buffer_bytes = 224 * 2 * 2
local captured_state = false
local diagnostics = os.getenv("SF1_LAUNCH_DIAGNOSTIC") ~= nil

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
  if status == 0 then
    for scene = first_edge_scene, last_edge_scene do
      assert(captured_edges[scene], "missing original launch aperture record")
    end
  end
  local file = assert(io.open(folder .. "/launch_display.txt", "wb"))
  file:write(table.concat(output))
  file:close()
  file = assert(io.open(folder .. "/launch_window_writes.txt", "wb"))
  file:write(table.concat(window_writes))
  file:close()
  emu.stop(status)
end
local function end_frame()
  video = video + 1
  if active then
    local scene = word(0x15BB)
    if scene >= 4 and scene <= 22 then
      if diagnostics and not captured_state then
        local state = emu.getState()
        local names = {}
        for name, _ in pairs(state) do
          if name:match("^ppu%.") or name:match("^cart%.coprocessor%.") then
            names[#names + 1] = name
          end
        end
        table.sort(names)
        local file = assert(io.open(folder .. "/launch_ppu_state.txt", "wb"))
        for _, name in ipairs(names) do file:write(name .. "=" .. tostring(state[name]) .. "\n") end
        file:close()
        captured_state = true
      end
      local size = emu.getScreenSize()
      local screen = emu.getScreenBuffer()
      local image = { string.format("P6\n%d %d\n255\n", size.width, size.height) }
      for index = 1, size.width * size.height do
        local pixel = assert(screen[index], "missing Mesen screen pixel")
        image[#image + 1] = string.char((pixel >> 16) & 255, (pixel >> 8) & 255, pixel & 255)
      end
      local name = string.format("launch_%04d.ppm", video)
      local file = assert(io.open(folder .. "/" .. name, "wb"))
      file:write(table.concat(image))
      file:close()
      local state = emu.getState()
      assert(state["ppu.bgMode"] == 1 and not state["ppu.layers[0].largeTiles"])
      assert(not state["ppu.layers[0].doubleWidth"] and not state["ppu.layers[0].doubleHeight"])
      local vram = {}
      for address = 0, 65535 do
        vram[#vram + 1] = string.char(emu.read(address, emu.memType.snesVideoRam, false))
      end
      local vram_name = string.format("launch_%04d.vram", video)
      file = assert(io.open(folder .. "/" .. vram_name, "wb"))
      file:write(table.concat(vram))
      file:close()
      output[#output + 1] = string.format(
        "video=%d scene=%d level=%d direction=%d display=%d image=%s vram=%s bg1_map=%d bg1_chr=%d bg1_hscroll=%d bg1_vscroll=%d\n",
        video, scene, byte(0x18B3), byte(0x18B2), byte(0x45F4), name, vram_name,
        state["ppu.layers[0].tilemapAddress"] * 2, state["ppu.layers[0].chrAddress"] * 2,
        state["ppu.layers[0].hscroll"], state["ppu.layers[0].vscroll"])
    end
    if scene > 22 then finish(0) end
  end
  if video > 4500 then finish(1) end
end
emu.addEventCallback(function()
  local input = { a = false, b = false, x = false, y = false, l = false, r = false,
    start = false, select = false, up = false, down = false, left = false, right = false }
  for button, pressed in pairs(active and {} or pad(math.floor(video / 3))) do
    input[button] = pressed
  end
  emu.setInput(input, 0)
end, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(function() requested = true end, emu.callbackType.exec,
  0x03C437, 0x03C437, emu.cpuType.snes, emu.memType.snesMemory)
emu.addMemoryCallback(function()
  if requested and word(0x15BB) == 0 then active = true end
end, emu.callbackType.exec, 0x02DAF2, 0x02DAF2, emu.cpuType.snes, emu.memType.snesMemory)
-- TRANS has completed both the original GSU line walker and window-edge
-- normalization here. Read the real output before the next draw reuses it.
emu.addMemoryCallback(function()
  if not active or byte(0x1FD0) == 0 then return end
  local scene = word(0x15BB)
  if scene < first_edge_scene or scene > last_edge_scene or captured_edges[scene] then return end
  local bytes = {}
  for offset = 0, window_buffer_bytes - 1 do
    bytes[#bytes + 1] = string.char(emu.read(window_buffer_address + offset, emu.memType.gsuWorkRam, false))
  end
  local name = string.format("launch_edges_%04d.bin", scene)
  local file = assert(io.open(folder .. "/" .. name, "wb"))
  file:write(table.concat(bytes))
  file:close()
  captured_edges[scene] = true
  output[#output + 1] = string.format("kind=edges video=%d scene=%d next_record=%d image=%s\n",
    video, scene, word(0x1FC9), name)
end, emu.callbackType.exec, 0x02DA76, 0x02DA76, emu.cpuType.snes, emu.memType.snesMemory)
emu.addMemoryCallback(function(address, value)
  if not diagnostics or not active or word(0x15BB) ~= 7 then return end
  local state = emu.getState()
  window_writes[#window_writes + 1] = string.format(
    "video=%d line=%d address=%04X value=%d\n", video,
    assert(state["ppu.scanline"]), address, value)
end, emu.callbackType.write, 0x2126, 0x2127, emu.cpuType.snes, emu.memType.snesMemory)
emu.log("SF1_LAUNCH_DISPLAY_ORACLE_LOADED")
