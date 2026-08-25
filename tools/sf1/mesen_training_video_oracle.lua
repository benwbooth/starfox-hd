-- Capture one logic-aligned retail presentation for every requested Training
-- scene. The source renderer completes asynchronously; three completed video
-- frames after TRANS.ASM's completed-transfer marker selects the first whole,
-- repeatable screen after the display swap.

local video_frame = 0
local video_frames_per_front_end_tick = 3
local front_end_confirm_cadence_ticks = 60
local front_end_confirm_hold_ticks = 2
local training_confirm_end_tick = 420
local graphics_tracking_start_video_frame = 1200
local first_game_frame =
  tonumber(os.getenv("SF1_TRAINING_CAPTURE_FIRST_GAME_FRAME")) or 1
local last_game_frame =
  tonumber(os.getenv("SF1_TRAINING_CAPTURE_LAST_GAME_FRAME")) or 1758
local oracle_timeout_video_frames = 16000
local visible_frame_delay =
  tonumber(os.getenv("SF1_TRAINING_VISIBLE_FRAME_DELAY")) or 3

local game_frame_address = 0x15BB
local stage_countdown_address = 0x15B9
local transfer_progress_address = 0x18BB
local background_vertical_offsets_address = 0x18CD
local background_vertical_scroll_address = 0x194D
local vertical_offset_enabled_address = 0x1954
local vertical_offset_enable_flag = 0x4000
local display_table_address = 0x45F4
local object_render_entry = 0x018456
local window_buffers_ready_entry = 0x01D726
local window_left_buffer_address = 0x0EF2
local window_right_buffer_address = 0x10B2
local window_buffer_size = 224 * 2
local capture_lines = {}
local pipeline_lines = {}
local completed_render_scenes = {}
local scheduled_captures = {}
local captured_scenes = {}
local active_render_scene = nil
local graphics_tracking_enabled = false
local capture_count = 0
local display_control = 0x80
local isolate_background = os.getenv("SF1_TRAINING_ISOLATE_BACKGROUND") ~= nil
local capture_window_buffers =
  os.getenv("SF1_TRAINING_CAPTURE_WINDOW_BUFFERS") ~= nil
local capture_source_bitmaps =
  os.getenv("SF1_TRAINING_CAPTURE_SOURCE_BITMAPS") ~= nil
local captured_window_buffers = {}

assert(first_game_frame <= last_game_frame, "Training video range must be ordered")

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function write_binary(name, contents)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(contents)
  file:close()
end

local function capture_screen(scene_game_frame)
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
    string.format("sf1_training_scene_%04d.ppm", scene_game_frame),
    table.concat(output))
end

local function capture_memory(name, memory_type, size)
  local bytes = {}
  for address = 0, size - 1 do
    bytes[#bytes + 1] = string.char(emu.read(address, memory_type, false))
  end
  write_binary(name, table.concat(bytes))
end

local function capture_memory_range(name, memory_type, address, size)
  local bytes = {}
  for offset = 0, size - 1 do
    bytes[#bytes + 1] = string.char(
      emu.read(address + offset, memory_type, false))
  end
  write_binary(name, table.concat(bytes))
end

local function provide_input()
  local tick = math.floor(video_frame / video_frames_per_front_end_tick)
  emu.setInput({
    start = tick <= training_confirm_end_tick
      and tick % front_end_confirm_cadence_ticks < front_end_confirm_hold_ticks
  }, 0)
end

local function flush_captures()
  for scene_game_frame = first_game_frame, last_game_frame do
    assert(captured_scenes[scene_game_frame], string.format(
      "Training scene game frame %d was not captured", scene_game_frame))
  end
  write_binary("sf1_training_captures.txt", table.concat(capture_lines))
  write_binary("sf1_training_pipeline.txt", table.concat(pipeline_lines))
end

local function track_graphics_execution(address, opcode)
  if address == window_buffers_ready_entry and capture_window_buffers then
    local scene_game_frame = work_word(game_frame_address)
    if scene_game_frame >= first_game_frame
      and scene_game_frame <= last_game_frame
      and not captured_window_buffers[scene_game_frame]
    then
      capture_memory_range(
        string.format("sf1_training_window_left_%04d.bin", scene_game_frame),
        emu.memType.gsuWorkRam,
        window_left_buffer_address,
        window_buffer_size)
      capture_memory_range(
        string.format("sf1_training_window_right_%04d.bin", scene_game_frame),
        emu.memType.gsuWorkRam,
        window_right_buffer_address,
        window_buffer_size)
      captured_window_buffers[scene_game_frame] = true
      pipeline_lines[#pipeline_lines + 1] = string.format(
        "kind=window_buffers scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d left=%04X right=%04X\n",
        scene_game_frame,
        work_word(game_frame_address),
        video_frame,
        window_left_buffer_address,
        window_right_buffer_address)
    end
  end
  if address == object_render_entry and active_render_scene == nil then
    active_render_scene = work_word(game_frame_address)
    pipeline_lines[#pipeline_lines + 1] = string.format(
      "kind=render_start scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d\n",
      active_render_scene,
      work_word(game_frame_address),
      video_frame)
  end
  if opcode == 0 and active_render_scene ~= nil then
    if capture_source_bitmaps
      and active_render_scene >= first_game_frame
      and active_render_scene <= last_game_frame
    then
      local state = emu.getState()
      capture_memory(
        string.format("sf1_training_bitmap_ram_%04d.bin", active_render_scene),
        emu.memType.gsuWorkRam,
        0x10000)
      pipeline_lines[#pipeline_lines + 1] = string.format(
        "kind=source_bitmap scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d screen_base=%d screen_mode=%d\n",
        active_render_scene,
        work_word(game_frame_address),
        video_frame,
        state["cart.coprocessor.screenBase"] or -1,
        state["cart.coprocessor.screenMode"] or -1)
    end
    completed_render_scenes[#completed_render_scenes + 1] = active_render_scene
    pipeline_lines[#pipeline_lines + 1] = string.format(
      "kind=render_stop scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d address=%06X\n",
      active_render_scene,
      work_word(game_frame_address),
      video_frame,
      address)
    active_render_scene = nil
  end
end

local function record_transfer_progress(address, value)
  if not graphics_tracking_enabled or value ~= 2 then return end
  local scene_game_frame = table.remove(completed_render_scenes, 1)
  assert(scene_game_frame ~= nil, "bitmap transfer completed without a rendered Training scene")
  pipeline_lines[#pipeline_lines + 1] = string.format(
    "kind=transfer_complete scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d\n",
    scene_game_frame,
    work_word(game_frame_address),
    video_frame)
  if scene_game_frame >= first_game_frame and scene_game_frame <= last_game_frame then
    local capture_video_frame = video_frame + visible_frame_delay
    assert(scheduled_captures[capture_video_frame] == nil,
      "two Training scenes targeted the same visible video frame")
    scheduled_captures[capture_video_frame] = {
      scene_game_frame = scene_game_frame,
      transfer_complete_video_frame = video_frame,
    }
  end
end

local function record_display_control(address, value)
  display_control = value
end

local function enable_graphics_tracking()
  graphics_tracking_enabled = true
  emu.addMemoryCallback(
    track_graphics_execution,
    emu.callbackType.exec,
    0,
    0x7FFFFF,
    emu.cpuType.gsu,
    emu.memType.gsuMemory)
end

local function capture_stable_screen()
  local game_frame = work_word(game_frame_address)
  local capture = scheduled_captures[video_frame]
  if capture ~= nil then
    local scene_game_frame = capture.scene_game_frame
    assert(not captured_scenes[scene_game_frame], "Training scene was captured twice")
    capture_screen(scene_game_frame)
    capture_memory(
      string.format("sf1_training_cgram_%04d.bin", scene_game_frame),
      emu.memType.snesCgRam,
      0x200)
    capture_memory(
      string.format("sf1_training_oam_%04d.bin", scene_game_frame),
      emu.memType.snesSpriteRam,
      0x220)
    captured_scenes[scene_game_frame] = true
    capture_count = capture_count + 1
    local background_vertical_scroll = work_word(background_vertical_scroll_address)
    local display_table_control =
      emu.read(display_table_address, emu.memType.snesWorkRam, false)
    local first_background_vertical_offset =
      (work_word(background_vertical_offsets_address)
        - background_vertical_scroll
        - vertical_offset_enable_flag) & 0xFFFF
    if first_background_vertical_offset >= 0x8000 then
      first_background_vertical_offset = first_background_vertical_offset - 0x10000
    end
    capture_lines[#capture_lines + 1] = string.format(
      "scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d transfer_complete_video_frame=%d display_control=%d display_table_control=%d stage_countdown=%d background_vertical_scroll=%d first_background_vertical_offset=%d vertical_offset_enabled=%d\n",
      scene_game_frame,
      game_frame,
      video_frame,
      capture.transfer_complete_video_frame,
      display_control,
      display_table_control,
      work_word(stage_countdown_address),
      background_vertical_scroll,
      first_background_vertical_offset,
      emu.read(vertical_offset_enabled_address, emu.memType.snesWorkRam, false))
    pipeline_lines[#pipeline_lines + 1] = string.format(
      "kind=visible_capture scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d\n",
      scene_game_frame,
      game_frame,
      video_frame)
  end

  if capture_count == last_game_frame - first_game_frame + 1 then
    flush_captures()
    emu.log("SF1_TRAINING_VIDEO_ORACLE_DONE")
    emu.stop(0)
    return
  end
end

local function isolate_background_layer()
  if not isolate_background or video_frame < graphics_tracking_start_video_frame then
    return
  end
  emu.write(0x212C, 2, emu.memType.snesMemory)
  emu.write(0x212D, 0, emu.memType.snesMemory)
  emu.write(0x212E, 0, emu.memType.snesMemory)
  emu.write(0x212F, 0, emu.memType.snesMemory)
  emu.write(0x2130, 0, emu.memType.snesMemory)
  emu.write(0x2131, 0, emu.memType.snesMemory)
end

local function end_frame()
  video_frame = video_frame + 1
  if not graphics_tracking_enabled and video_frame >= graphics_tracking_start_video_frame then
    enable_graphics_tracking()
  end
  if video_frame >= oracle_timeout_video_frames then
    emu.log("SF1_TRAINING_VIDEO_ORACLE_TIMEOUT")
    emu.stop(2)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(isolate_background_layer, emu.eventType.startFrame)
emu.addEventCallback(capture_stable_screen, emu.eventType.startFrame)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(
  record_transfer_progress,
  emu.callbackType.write,
  transfer_progress_address,
  transfer_progress_address,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.addMemoryCallback(
  record_display_control,
  emu.callbackType.write,
  0x2100,
  0x2100,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.log("SF1_TRAINING_VIDEO_ORACLE_LOADED")
