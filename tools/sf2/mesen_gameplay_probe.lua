-- Drive the retail Star Fox 2 ROM without human input and capture independent
-- framebuffer/WRAM evidence at fixed points after the main $19:9F9C asset
-- stream has been decompressed.
--
-- Run through `tools/sf2/run_mesen_oracle.py`.  The screenshots and the final
-- bank-$7E dump are written beneath its disposable LuaScriptData directory,
-- avoiding the user's emulator saves and configuration.

local armed = false
local armed_frame = -1
local frame_count = 0
local final_elapsed_frame = 7100
local capture_period = 480
local bootstrap_limit = 2400
local bootstrap_capture_period = 120
local shape_trace_start = 6800
local shape_trace_end = 7000
local shape_read_counts = {}
local bus_read_counts = {}
local ram_read_counts = {}
local geometry_read_trace = {}
local geometry_state_keys = nil
local shape_trace_written = false
-- Retail `$02:9201..$02:947D` builds a counted array of 0x26-byte render
-- records here.  `$18C6` is the authoritative live count; bytes after that
-- count are stale records from prior frames and must never be treated as
-- active objects.
local draw_list_base = 0xB273
local draw_record_size = 0x26
local draw_record_capacity = 64
local draw_count_address = 0x18C6
local draw_snapshots = {}
local draw_writes = {}
local draw_state_keys = nil
local decompressor_source = -1
local decompressor_bank = -1
local decompressions = {}
local dma_records = {}
local wram_address = 0
local wram_bank = 0
local map_samples = {}
local last_map_bank = -1
local last_map_cursor = -1

local function script_path(filename)
  local directory = emu.getScriptDataFolder()
  if directory == "" then
    emu.log("SF2_PROBE_ERROR file I/O is disabled")
    emu.stop(2)
    return filename
  end
  return directory .. "/" .. filename
end

local function write_binary(filename, contents)
  local file, message = io.open(script_path(filename), "w+b")
  if file == nil then
    emu.log("SF2_PROBE_ERROR " .. tostring(message))
    emu.stop(3)
    return
  end
  file:write(contents)
  file:close()
end

local function capture_screen(elapsed)
  local filename = string.format("sf2_frame_%04d.ppm", elapsed)
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
  write_binary(filename, table.concat(output))
  emu.log("SF2_PROBE_SCREENSHOT " .. filename)
end

local function capture_boot_screen(frame)
  capture_screen(frame)
end

local function capture_memory()
  local bytes = {}
  for address = 0, 0x1FFFF do
    bytes[#bytes + 1] = string.char(
      emu.read(address, emu.memType.snesWorkRam, false))
  end
  write_binary("sf2_wram.bin", table.concat(bytes))

  bytes = {}
  for address = 0, 0xFFFF do
    bytes[#bytes + 1] = string.char(
      emu.read(address, emu.memType.gsuWorkRam, false))
  end
  write_binary("sf2_gsu_ram.bin", table.concat(bytes))

  local lines = {}
  for _, item in ipairs(decompressions) do
    lines[#lines + 1] = string.format(
      "%d %02X:%04X %04X-%04X\n",
      item.elapsed, item.bank, item.source, item.output_start, item.output_end)
  end
  write_binary("sf2_decompressions.txt", table.concat(lines))

  lines = {}
  for _, item in ipairs(dma_records) do
    lines[#lines + 1] = string.format(
      "%d ch=%d source=%02X:%04X length=%04X dmap=%02X bbad=%02X wram=%02X:%04X\n",
      item.elapsed, item.channel, item.source_bank, item.source,
      item.length, item.dmap, item.bbad, item.wram_bank, item.wram_address)
  end
  write_binary("sf2_dma.txt", table.concat(lines))

  lines = {}
  for _, snapshot in ipairs(draw_snapshots) do
    lines[#lines + 1] = string.format(
      "frame %d count %d\n", snapshot.elapsed, snapshot.count)
    for index = 0, snapshot.count - 1 do
      local start = index * draw_record_size + 1
      local fields = {}
      for offset = 0, draw_record_size - 1 do
        fields[#fields + 1] = string.format("%02X", snapshot.bytes[start + offset])
      end
      lines[#lines + 1] = string.format("%02d %s\n", index, table.concat(fields, ""))
    end
  end
  write_binary("sf2_draw_snapshots.txt", table.concat(lines))

  lines = {}
  if draw_state_keys ~= nil then
    lines[#lines + 1] = "elapsed address value " .. table.concat(draw_state_keys, " ") .. "\n"
  end
  for _, item in ipairs(draw_writes) do
    local fields = {
      tostring(item.elapsed), string.format("%04X", item.address),
      string.format("%02X", item.value),
    }
    for _, key in ipairs(draw_state_keys or {}) do
      fields[#fields + 1] = tostring(item[key])
    end
    lines[#lines + 1] = table.concat(fields, " ") .. "\n"
  end
  write_binary("sf2_draw_writes.txt", table.concat(lines))

  lines = { "elapsed bank cursor counter active\n" }
  for _, item in ipairs(map_samples) do
    lines[#lines + 1] = string.format(
      "%d %02X %04X %04X %04X\n",
      item.elapsed, item.bank, item.cursor, item.counter, item.active)
  end
  write_binary("sf2_map_samples.txt", table.concat(lines))
  emu.log("SF2_PROBE_MEMORY sf2_wram.bin sf2_gsu_ram.bin")
end

local function capture_draw_list(elapsed)
  local count = emu.read16(draw_count_address, emu.memType.snesWorkRam, false)
  if count > draw_record_capacity then
    emu.log(string.format(
      "SF2_PROBE_ERROR invalid draw count %d at elapsed frame %d", count, elapsed))
    emu.stop(4)
    return
  end
  local bytes = {}
  for offset = 0, draw_record_size * count - 1 do
    bytes[#bytes + 1] = emu.read(
      draw_list_base + offset, emu.memType.snesWorkRam, false)
  end
  draw_snapshots[#draw_snapshots + 1] = {
    elapsed = elapsed,
    count = count,
    bytes = bytes,
  }
end

local function trace_draw_write(address, value)
  if not armed then
    return
  end
  local elapsed = frame_count - armed_frame
  if elapsed < 6440 or elapsed >= 6510 or #draw_writes >= 20000 then
    return
  end
  local state = emu.getState()
  if draw_state_keys == nil then
    draw_state_keys = {}
    for key, _ in pairs(state) do
      local lower = string.lower(key)
      if string.find(lower, "cpu.", 1, true)
        and (string.find(lower, "pc", 1, true)
          or string.find(lower, "pbr", 1, true)
          or string.find(lower, "dbr", 1, true)
          or string.find(lower, "a", 1, true)
          or string.find(lower, "x", 1, true)
          or string.find(lower, "y", 1, true)) then
        draw_state_keys[#draw_state_keys + 1] = key
      end
    end
    table.sort(draw_state_keys)
  end
  local item = { elapsed = elapsed, address = address, value = value }
  for _, key in ipairs(draw_state_keys) do
    item[key] = state[key]
  end
  draw_writes[#draw_writes + 1] = item
end

local function count_shape_read(address)
  if armed then
    local elapsed = frame_count - armed_frame
    if elapsed >= shape_trace_start and elapsed < shape_trace_end then
      shape_read_counts[address] = (shape_read_counts[address] or 0) + 1
    end
  end
end

local function count_ram_read(address)
  if armed then
    local elapsed = frame_count - armed_frame
    if elapsed >= shape_trace_start and elapsed < shape_trace_start + 5 then
      ram_read_counts[address] = (ram_read_counts[address] or 0) + 1
    end
  end
end

local function count_bus_read(address)
  if armed then
    local elapsed = frame_count - armed_frame
    if elapsed >= shape_trace_start and elapsed < shape_trace_start + 5 then
      bus_read_counts[address] = (bus_read_counts[address] or 0) + 1
    end
  end
end

local function trace_geometry_read(address)
  if not armed then
    return
  end
  local elapsed = frame_count - armed_frame
  if elapsed < shape_trace_start or elapsed >= shape_trace_start + 5 then
    return
  end
  local bank = address >> 16
  if bank ~= 0x07 and bank ~= 0x0D and bank ~= 0x0F then
    return
  end
  if (address & 0xFFFF) < 0x8000 then
    return
  end

  local state = emu.getState()
  if geometry_state_keys == nil then
    geometry_state_keys = {}
    for key, _ in pairs(state) do
      local lower = string.lower(key)
      if #key < 100 and string.find(lower, "coprocessor", 1, true)
        and not string.find(lower, "cache", 1, true)
        and not string.find(lower, "pixels", 1, true)
        and not string.find(lower, "gsuram", 1, true) then
        geometry_state_keys[#geometry_state_keys + 1] = key
      end
    end
    table.sort(geometry_state_keys)
  end

  if #geometry_read_trace < 5000 then
    local item = { address = address }
    for _, key in ipairs(geometry_state_keys) do
      item[key] = state[key]
    end
    geometry_read_trace[#geometry_read_trace + 1] = item
  end
end

local function finish_shape_trace()
  if shape_trace_written then
    return
  end
  shape_trace_written = true

  local addresses = {}
  for address, _ in pairs(shape_read_counts) do
    addresses[#addresses + 1] = address
  end
  table.sort(addresses)
  local lines = {}
  for _, address in ipairs(addresses) do
    lines[#lines + 1] = string.format(
      "%06X %d\n", address, shape_read_counts[address])
  end
  write_binary("sf2_gsu_shape_reads.txt", table.concat(lines))

  addresses = {}
  for address, _ in pairs(ram_read_counts) do
    addresses[#addresses + 1] = address
  end
  table.sort(addresses)
  lines = {}
  for _, address in ipairs(addresses) do
    lines[#lines + 1] = string.format(
      "%04X %d\n", address, ram_read_counts[address])
  end
  write_binary("sf2_gsu_ram_reads.txt", table.concat(lines))

  addresses = {}
  for address, _ in pairs(bus_read_counts) do
    addresses[#addresses + 1] = address
  end
  table.sort(addresses)
  lines = {}
  for _, address in ipairs(addresses) do
    lines[#lines + 1] = string.format(
      "%06X %d\n", address, bus_read_counts[address])
  end
  write_binary("sf2_gsu_bus_reads.txt", table.concat(lines))

  lines = {}
  if geometry_state_keys ~= nil then
    lines[#lines + 1] = "address " .. table.concat(geometry_state_keys, " ") .. "\n"
    for _, item in ipairs(geometry_read_trace) do
      local fields = { string.format("%06X", item.address) }
      for _, key in ipairs(geometry_state_keys) do
        fields[#fields + 1] = tostring(item[key])
      end
      lines[#lines + 1] = table.concat(fields, " ") .. "\n"
    end
  end
  write_binary("sf2_geometry_read_trace.txt", table.concat(lines))
end

local function arm_for_target_stream()
  decompressor_source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  decompressor_bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and decompressor_bank == 0x19 and decompressor_source == 0x9F9C then
    armed = true
    armed_frame = frame_count
    emu.log(string.format("SF2_PROBE_ARM frame=%d", armed_frame))
  end
end

local function record_decompressor_stop()
  decompressions[#decompressions + 1] = {
    elapsed = armed and (frame_count - armed_frame) or -1,
    bank = decompressor_bank,
    source = decompressor_source,
    output_start = emu.read16(0x002C, emu.memType.gsuWorkRam, false),
    output_end = emu.read16(0x0060, emu.memType.gsuWorkRam, false),
  }
end

local function record_dma(_, value)
  for channel = 0, 7 do
    if (value & (1 << channel)) ~= 0 then
      local base = 0x004300 + channel * 0x10
      dma_records[#dma_records + 1] = {
        elapsed = armed and (frame_count - armed_frame) or -1,
        channel = channel,
        dmap = emu.read(base, emu.memType.snesMemory, false),
        bbad = emu.read(base + 1, emu.memType.snesMemory, false),
        source = emu.read16(base + 2, emu.memType.snesMemory, false),
        source_bank = emu.read(base + 4, emu.memType.snesMemory, false),
        length = emu.read16(base + 5, emu.memType.snesMemory, false),
        wram_address = wram_address,
        wram_bank = wram_bank,
      }
      local bbad = emu.read(base + 1, emu.memType.snesMemory, false)
      if bbad == 0x80 then
        local length = emu.read16(base + 5, emu.memType.snesMemory, false)
        if length == 0 then
          length = 0x10000
        end
        local linear = ((wram_bank & 1) << 16) | wram_address
        linear = (linear + length) & 0x1FFFF
        wram_address = linear & 0xFFFF
        wram_bank = (linear >> 16) & 1
      end
    end
  end
end

local function set_wram_address_low(_, value)
  wram_address = (wram_address & 0xFF00) | value
end

local function set_wram_address_high(_, value)
  wram_address = (wram_address & 0x00FF) | (value << 8)
end

local function set_wram_bank(_, value)
  wram_bank = value & 1
end

local function end_frame()
  frame_count = frame_count + 1
  local map_bank = emu.read(0x192E, emu.memType.snesWorkRam, false)
  local map_cursor = emu.read16(0x1657, emu.memType.snesWorkRam, false)
  if map_bank ~= last_map_bank or map_cursor ~= last_map_cursor then
    map_samples[#map_samples + 1] = {
      elapsed = armed and (frame_count - armed_frame) or -frame_count,
      bank = map_bank,
      cursor = map_cursor,
      counter = emu.read16(0x1655, emu.memType.snesWorkRam, false),
      active = emu.read16(0x12A8, emu.memType.snesWorkRam, false),
    }
    last_map_bank = map_bank
    last_map_cursor = map_cursor
  end
  if not armed then
    if frame_count % bootstrap_capture_period == 0 then
      capture_boot_screen(frame_count)
    end
    if frame_count >= bootstrap_limit then
      capture_memory()
      emu.stop(1)
    end
    return
  end

  local elapsed = frame_count - armed_frame
  if elapsed == shape_trace_end then
    finish_shape_trace()
  end
  if elapsed >= 0 and elapsed % capture_period == 0 then
    capture_screen(elapsed)
  end
  if elapsed >= 5940 and elapsed <= 7020 and elapsed % 30 == 0 then
    capture_draw_list(elapsed)
  end
  if elapsed >= final_elapsed_frame then
    finish_shape_trace()
    capture_memory()
    emu.stop(0)
  end
end

local function pulse(elapsed, period, offset)
  local phase = elapsed % period
  return phase == offset or phase == offset + 1
end

local function provide_input()
  local elapsed = frame_count - armed_frame
  local accept = armed and elapsed >= 210 and pulse(elapsed, 90, 30)
  -- Start leaves the title/attract loop.  Subsequent, separated B pulses
  -- accept the default difficulty, pilot, and map choices without steering a
  -- live Arwing.  Repeating Start also recovers if the first pulse lands on a
  -- transition frame that ignores input.
  emu.setInput({
    start = pulse(frame_count, 180, 120) and (not armed or elapsed <= 600),
    a = false,
    b = accept,
    x = false,
    y = false,
    l = false,
    r = false,
    select = false,
    up = armed and elapsed >= 6000 and elapsed < 6045,
    down = false,
    left = false,
    right = armed and elapsed >= 6045 and elapsed < 6070,
  }, 0)
end

emu.addMemoryCallback(
  arm_for_target_stream,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

emu.addMemoryCallback(
  record_decompressor_stop,
  emu.callbackType.exec,
  0x01DAE2,
  0x01DAE2,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

emu.addMemoryCallback(
  record_dma,
  emu.callbackType.write,
  0x00420B,
  0x00420B,
  emu.cpuType.snes,
  emu.memType.snesMemory)

emu.addMemoryCallback(
  set_wram_address_low,
  emu.callbackType.write,
  0x002181,
  0x002181,
  emu.cpuType.snes,
  emu.memType.snesMemory)

emu.addMemoryCallback(
  set_wram_address_high,
  emu.callbackType.write,
  0x002182,
  0x002182,
  emu.cpuType.snes,
  emu.memType.snesMemory)

emu.addMemoryCallback(
  set_wram_bank,
  emu.callbackType.write,
  0x002183,
  0x002183,
  emu.cpuType.snes,
  emu.memType.snesMemory)

emu.addMemoryCallback(
  count_shape_read,
  emu.callbackType.read,
  0x120000,
  0x17FFFF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

emu.addMemoryCallback(
  count_bus_read,
  emu.callbackType.read,
  0x000000,
  0x3FFFFF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

for _, range in ipairs({
  { 0x070000, 0x07FFFF },
  { 0x0D0000, 0x0DFFFF },
  { 0x0F0000, 0x0FFFFF },
}) do
  emu.addMemoryCallback(
    trace_geometry_read,
    emu.callbackType.read,
    range[1],
    range[2],
    emu.cpuType.gsu,
    emu.memType.gsuMemory)
end

emu.addMemoryCallback(
  count_ram_read,
  emu.callbackType.read,
  0x0000,
  0xFFFF,
  emu.cpuType.gsu,
  emu.memType.gsuWorkRam)

emu.addMemoryCallback(
  trace_draw_write,
  emu.callbackType.write,
  draw_list_base,
  draw_list_base + draw_record_size * draw_record_capacity - 1,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF2_PROBE_LOADED")
