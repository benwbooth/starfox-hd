-- Independent retail title-frame oracle for the strict native conformance
-- runner. The input cadence matches sf1_title_trace exactly and captures a
-- narrow frame window around its first visible title update. Source-machine
-- addresses and raw memories remain confined to this oracle helper.

local frame = 0
local first_capture_frame = tonumber(os.getenv("SF1_TITLE_FIRST_FRAME")) or 488
local last_capture_frame = tonumber(os.getenv("SF1_TITLE_LAST_FRAME")) or 500
local dump_video_memory = os.getenv("SF1_TITLE_DUMP_VIDEO_MEMORY") == "1"
local dump_graphics_memory = os.getenv("SF1_TITLE_DUMP_GRAPHICS_MEMORY") == "1"
local dump_shape_projection = os.getenv("SF1_TITLE_DUMP_SHAPE_PROJECTION") == "1"
local dump_shape_visibility = os.getenv("SF1_TITLE_DUMP_SHAPE_VISIBILITY") == "1"
local dump_face_state = os.getenv("SF1_TITLE_DUMP_FACE_STATE") == "1"
local dump_scanlines = os.getenv("SF1_TITLE_DUMP_SCANLINES") == "1"
local trace_shape_color_execution =
  os.getenv("SF1_TITLE_TRACE_SHAPE_COLOR_EXECUTION") == "1"
local video_frames_per_tick = 3
local input_cadence_ticks = 60
local input_hold_ticks = 2

local game_frame = 0x15BB
local dots_flag = 0x16F9
local current_background = 0x1741
local title_background = 249
local dust_points = 0x0B52
local dust_point_bytes = 120 * 6
local dust_random_state = 0x0140
local view_position = 0x00C6
local world_matrix = 0x00D2
local rotated_points = 0x05C2
local rotated_point_bytes = 120 * 6
local dust_init_count = 0
local presentation_state_lines = {}
local last_fixed_color = -1
local shape_projection_sequence = 0
local title_input_locked = false

local shape_projection_entry = 0x0191A0
local shape_bsp_initialization_entry = 0x018B81
local shape_bsp_output_entry = 0x018C08
local shape_position = 0x0026
local shape_matrix = 0x0116
local shape_point_count = 0x0132
local projected_points = 0x07A2
local shape_pointer = 0x001A
local shape_bsp_list_pointer = 0x0056
local shape_bsp_list = 0x0B02
local shape_visibility = 0x0E72
local shape_visibility_count = 48
local title_ship_second_face = 0x0CE113
local shape_color_ready_entry = 0x019191
local face_state_sequence = 0
local shape_color_execution_seen = {}
local scanline_entry = 0x01A752
local polygon_scan_entry = 0x01A66B
local polygon_points = 0x0982

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function signed_word(value)
  if value >= 0x8000 then
    return value - 0x10000
  end
  return value
end

local function signed_byte(value)
  if value >= 0x80 then
    return value - 0x100
  end
  return value
end

local function output_path(name)
  return emu.getScriptDataFolder() .. "/" .. name
end

local function write_binary(name, contents)
  local file = assert(io.open(output_path(name), "w+b"))
  file:write(contents)
  file:close()
end

local function capture_screen(name)
  local size = emu.getScreenSize()
  local screen = emu.getScreenBuffer()
  local output = { string.format("P6\n%d %d\n255\n", size.width, size.height) }
  local sample = {}
  local combined = 0
  local maximum = 0
  for index = 1, size.width * size.height do
    local pixel = screen[index] or 0
    combined = combined | pixel
    maximum = math.max(maximum, pixel)
    if pixel ~= 0 and #sample < 12 then
      sample[#sample + 1] = string.format("%08X", pixel)
    end
    output[#output + 1] = string.char(
      (pixel >> 16) & 0xFF,
      (pixel >> 8) & 0xFF,
      pixel & 0xFF)
  end
  presentation_state_lines[#presentation_state_lines + 1] = string.format(
    "screen_buffer_frame=%d combined=%08X maximum=%08X sample=%s\n",
    frame,
    combined,
    maximum,
    table.concat(sample, ","))
  write_binary(name, table.concat(output))
end

local function dump_graphics_state(name)
  local bytes = {}
  for address = 0, 0xFFFF do
    bytes[#bytes + 1] = string.char(
      emu.read(address, emu.memType.gsuWorkRam, false))
  end
  write_binary(name, table.concat(bytes))
end

local function dump_memory(name, memory_type, byte_count)
  local bytes = {}
  for address = 0, byte_count - 1 do
    bytes[#bytes + 1] = string.char(emu.read(address, memory_type, false))
  end
  write_binary(name, table.concat(bytes))
end

local function record_display_write(address, value)
  if frame + 1 < first_capture_frame or frame > last_capture_frame then
    return
  end
  presentation_state_lines[#presentation_state_lines + 1] = string.format(
    "display_write_frame=%d game_frame=%d value=%02X\n",
    frame,
    work_word(game_frame),
    value)
end

local function record_fixed_color_write(address, value)
  if frame + 1 < first_capture_frame or frame > last_capture_frame
    or value == last_fixed_color then
    return
  end
  last_fixed_color = value
  presentation_state_lines[#presentation_state_lines + 1] = string.format(
    "fixed_color_write_frame=%d game_frame=%d value=%02X\n",
    frame,
    work_word(game_frame),
    value)
end

local function capture_dust_initialization()
  dust_init_count = dust_init_count + 1
  local bytes = {}
  for address = dust_points, dust_points + dust_point_bytes - 1 do
    bytes[#bytes + 1] = string.char(
      emu.read(address, emu.memType.gsuWorkRam, false))
  end
  write_binary(
    string.format(
      "sf1_dust_init_%02d_frame_%03d_random_%04X.bin",
      dust_init_count,
      frame,
      emu.read16(dust_random_state, emu.memType.gsuWorkRam, false)),
    table.concat(bytes))
end

local function capture_dust_points(name)
  local bytes = {}
  for address = dust_points, dust_points + dust_point_bytes - 1 do
    bytes[#bytes + 1] = string.char(
      emu.read(address, emu.memType.gsuWorkRam, false))
  end
  write_binary(name, table.concat(bytes))
end

local function capture_dust_projection()
  if frame < first_capture_frame or frame > last_capture_frame then
    return
  end
  local bytes = {}
  for address = rotated_points, rotated_points + rotated_point_bytes - 1 do
    bytes[#bytes + 1] = string.char(
      emu.read(address, emu.memType.gsuWorkRam, false))
  end
  write_binary(string.format(
    "sf1_title_rotated_frame_%03d.bin", frame), table.concat(bytes))
  presentation_state_lines[#presentation_state_lines + 1] = string.format(
    "projection_frame=%d game_frame=%d camera=%d,%d,%d player_view_depth=%d view=%d,%d,%d matrix=%d,%d,%d,%d,%d,%d,%d,%d,%d random=%04X\n",
    frame,
    work_word(game_frame),
    signed_word(work_word(0x00C1)),
    signed_word(work_word(0x00C3)),
    signed_word(work_word(0x00C5)),
    signed_word(work_word(0x14FA)),
    signed_word(emu.read16(view_position, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(view_position + 2, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(view_position + 4, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 2, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 4, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 6, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 8, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 10, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 12, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 14, emu.memType.gsuWorkRam, false)),
    signed_word(emu.read16(world_matrix + 16, emu.memType.gsuWorkRam, false)),
    emu.read16(dust_random_state, emu.memType.gsuWorkRam, false))
end

local function capture_shape_projection()
  if not dump_shape_projection or frame < first_capture_frame
      or frame > last_capture_frame then
    return
  end
  shape_projection_sequence = shape_projection_sequence + 1
  local count = emu.read16(shape_point_count, emu.memType.gsuWorkRam, false)
  local values = {
    string.format(
      "video_frame=%d game_frame=%d sequence=%d count=%d position=%d,%d,%d light=%d,%d,%d matrix=",
      frame,
      work_word(game_frame),
      shape_projection_sequence,
      count,
      signed_word(emu.read16(shape_position, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(shape_position + 2, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(shape_position + 4, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(0x00F4, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(0x00F6, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(0x00F8, emu.memType.gsuWorkRam, false)))
  }
  for offset = 0, 8 do
    values[#values + 1] = string.format(
      "%s%d", offset == 0 and "" or ",",
      signed_byte(emu.read(shape_matrix + offset, emu.memType.gsuWorkRam, false)))
  end
  values[#values + 1] = " points="
  for point = 0, count - 1 do
    local address = projected_points + point * 6
    values[#values + 1] = string.format(
      "%s%d,%d,%d", point == 0 and "" or ";",
      signed_word(emu.read16(address, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(address + 2, emu.memType.gsuWorkRam, false)),
      emu.read16(address + 4, emu.memType.gsuWorkRam, false))
  end
  values[#values + 1] = " rotated="
  for point = 0, count - 1 do
    local address = rotated_points + point * 6
    values[#values + 1] = string.format(
      "%s%d,%d,%d", point == 0 and "" or ";",
      signed_word(emu.read16(address, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(address + 2, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(address + 4, emu.memType.gsuWorkRam, false)))
  end
  values[#values + 1] = "\n"
  local file = assert(io.open(output_path("sf1_title_shape_projection.txt"), "ab"))
  file:write(table.concat(values))
  file:close()
end

local function append_shape_visibility(phase)
  if not dump_shape_visibility or frame < first_capture_frame
      or frame > last_capture_frame then
    return
  end
  local values = {
    string.format(
      "video_frame=%d game_frame=%d sequence=%d phase=%s shape=%04X visibility=",
      frame,
      work_word(game_frame),
      shape_projection_sequence,
      phase,
      emu.read16(shape_pointer, emu.memType.gsuWorkRam, false))
  }
  for index = 0, shape_visibility_count - 1 do
    values[#values + 1] = string.format(
      "%s%d", index == 0 and "" or ",",
      signed_byte(emu.read(
        shape_visibility + index, emu.memType.gsuWorkRam, false)))
  end
  if phase == "output" then
    values[#values + 1] = " faces="
    local end_pointer = emu.read16(
      shape_bsp_list_pointer, emu.memType.gsuWorkRam, false)
    for address = shape_bsp_list, end_pointer - 2, 2 do
      values[#values + 1] = string.format(
        "%s%04X", address == shape_bsp_list and "" or ",",
        emu.read16(address, emu.memType.gsuWorkRam, false))
    end
  end
  values[#values + 1] = "\n"
  local file = assert(io.open(output_path("sf1_title_shape_visibility.txt"), "ab"))
  file:write(table.concat(values))
  file:close()
end

local function capture_shape_visibility_initialization()
  append_shape_visibility("initialization")
end

local function capture_shape_visibility_output()
  append_shape_visibility("output")
end

local function append_face_state(phase)
  if not dump_face_state or frame < first_capture_frame
      or frame > last_capture_frame then
    return
  end
  face_state_sequence = face_state_sequence + 1
  local fields = {}
  for key, value in pairs(emu.getState()) do
    fields[#fields + 1] = tostring(key) .. "=" .. tostring(value)
  end
  table.sort(fields)
  local file = assert(io.open(output_path("sf1_title_face_state.txt"), "ab"))
  file:write(string.format(
    "video_frame=%d game_frame=%d sequence=%d phase=%s shape=%04X %s\n",
    frame,
    work_word(game_frame),
    face_state_sequence,
    phase,
    emu.read16(shape_pointer, emu.memType.gsuWorkRam, false),
    table.concat(fields, " ")))
  file:close()
end

local function capture_title_ship_second_face()
  append_face_state("face")
end

local function capture_shape_color_ready()
  append_face_state("color")
end

local function record_shape_color_execution(address)
  if not trace_shape_color_execution or frame < first_capture_frame
      or frame > last_capture_frame or shape_color_execution_seen[address] then
    return
  end
  shape_color_execution_seen[address] = true
  local file = assert(io.open(output_path("sf1_title_shape_color_execution.txt"), "ab"))
  file:write(string.format("%06X\n", address))
  file:close()
end

local function capture_shape_scanline()
  if not dump_scanlines or frame < first_capture_frame
      or frame > last_capture_frame then
    return
  end
  local state = emu.getState()
  local source_y = state["cart.coprocessor.r2"] or -1
  local first_edge = state["cart.coprocessor.r7"] or 0
  local second_edge = state["cart.coprocessor.r9"] or 0
  local first_x = (first_edge >> 8) & 0xFF
  local second_x = (second_edge >> 8) & 0xFF
  if source_y < 140 or source_y > 160 then
    return
  end
  local file = assert(io.open(output_path("sf1_title_shape_scanlines.txt"), "ab"))
  file:write(string.format(
    "video_frame=%d game_frame=%d shape=%04X y=%d edge_fixed=%d,%d edge_x=%d,%d color=%d\n",
    frame,
    work_word(game_frame),
    emu.read16(shape_pointer, emu.memType.gsuWorkRam, false),
    source_y,
    first_edge,
    second_edge,
    first_x,
    second_x,
    state["cart.coprocessor.colorReg"] or -1))
  file:close()
end

local function capture_shape_polygon()
  if not dump_scanlines or frame < first_capture_frame
      or frame > last_capture_frame then
    return
  end
  local state = emu.getState()
  local count = state["cart.coprocessor.r0"] or 0
  if count < 3 or count > 12 then
    return
  end
  local values = {}
  for index = 0, count - 1 do
    local address = polygon_points + index * 4
    values[#values + 1] = string.format(
      "%s%d,%d", index == 0 and "" or ";",
      signed_word(emu.read16(address, emu.memType.gsuWorkRam, false)),
      signed_word(emu.read16(address + 2, emu.memType.gsuWorkRam, false)))
  end
  local file = assert(io.open(output_path("sf1_title_shape_polygons.txt"), "ab"))
  file:write(string.format(
    "video_frame=%d game_frame=%d shape=%04X count=%d points=%s\n",
    frame,
    work_word(game_frame),
    emu.read16(shape_pointer, emu.memType.gsuWorkRam, false),
    count,
    table.concat(values)))
  file:close()
end

local function provide_input()
  if work_word(current_background) == title_background then
    title_input_locked = true
  end
  local tick = math.floor(frame / video_frames_per_tick)
  local phase = tick % input_cadence_ticks
  emu.setInput({ start = not title_input_locked and phase < input_hold_ticks }, 0)
end

local function end_frame()
  frame = frame + 1
  if frame >= first_capture_frame and frame <= last_capture_frame then
    local background = work_word(current_background)
    local source_frame = work_word(game_frame)
    local dots = signed_word(work_word(dots_flag))
    local state = emu.getState()
    local presentation_fields = {}
    for key, value in pairs(state) do
      local lower = string.lower(key)
      if string.find(lower, "brightness", 1, true)
        or string.find(lower, "inidisp", 1, true)
        or string.find(lower, "forcedblank", 1, true) then
        presentation_fields[#presentation_fields + 1] =
          key .. "=" .. tostring(value)
      end
    end
    table.sort(presentation_fields)
    presentation_state_lines[#presentation_state_lines + 1] = string.format(
      "frame=%d game_frame=%d camera=%d,%d,%d slow=%d slow_depth=%d %s\n",
      frame,
      source_frame,
      signed_word(work_word(0x00C1)),
      signed_word(work_word(0x00C3)),
      signed_word(work_word(0x00C5)),
      emu.read(0x14C2, emu.memType.snesWorkRam, false),
      signed_word(work_word(0x14CD)),
      table.concat(presentation_fields, " "))
    capture_screen(string.format(
      "sf1_title_frame_%03d_bg_%03d_game_%03d_dots_%d.ppm",
      frame,
      background,
      source_frame,
      dots))
    capture_dust_points(string.format(
      "sf1_title_dust_frame_%03d_game_%03d.bin",
      frame,
      source_frame))
    if dump_video_memory then
      dump_memory(string.format("sf1_title_vram_frame_%03d.bin", frame),
        emu.memType.snesVideoRam, 0x10000)
      dump_memory(string.format("sf1_title_cgram_frame_%03d.bin", frame),
        emu.memType.snesCgRam, 0x200)
    end
    if dump_graphics_memory and frame == last_capture_frame then
      dump_memory("sf1_title_graphics_memory.bin", emu.memType.gsuMemory, 0x20000)
    end
    if frame == last_capture_frame then
      dump_graphics_state("sf1_title_graphics_state.bin")
      write_binary(
        "sf1_title_state.txt",
        string.format(
          "frame=%d\nbackground=%d\ngame_frame=%d\ndots=%d\n",
          frame,
          background,
          source_frame,
          dots))
      write_binary("sf1_title_presentation_state.txt", table.concat(presentation_state_lines))
      if background ~= title_background then
        emu.log("SF1_TITLE_ORACLE_WRONG_PHASE")
        emu.stop(2)
        return
      end
      emu.log("SF1_TITLE_ORACLE_DONE")
      emu.stop(0)
    end
  elseif frame > last_capture_frame then
    emu.log("SF1_TITLE_ORACLE_MISSED_CAPTURE")
    emu.stop(3)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(
  record_display_write,
  emu.callbackType.write,
  0x2100,
  0x2100,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  record_fixed_color_write,
  emu.callbackType.write,
  0x2132,
  0x2132,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  capture_dust_initialization,
  emu.callbackType.exec,
  0x01A89B,
  0x01A89B,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_dust_projection,
  emu.callbackType.exec,
  0x01AA09,
  0x01AA09,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_shape_projection,
  emu.callbackType.exec,
  shape_projection_entry,
  shape_projection_entry,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_shape_visibility_initialization,
  emu.callbackType.exec,
  shape_bsp_initialization_entry,
  shape_bsp_initialization_entry,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_shape_visibility_output,
  emu.callbackType.exec,
  shape_bsp_output_entry,
  shape_bsp_output_entry,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_title_ship_second_face,
  emu.callbackType.exec,
  title_ship_second_face,
  title_ship_second_face,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_shape_color_ready,
  emu.callbackType.exec,
  shape_color_ready_entry,
  shape_color_ready_entry,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  record_shape_color_execution,
  emu.callbackType.exec,
  0x019000,
  0x0191A0,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_shape_scanline,
  emu.callbackType.exec,
  scanline_entry,
  scanline_entry,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addMemoryCallback(
  capture_shape_polygon,
  emu.callbackType.exec,
  polygon_scan_entry,
  polygon_scan_entry,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.log("SF1_TITLE_ORACLE_LOADED")
