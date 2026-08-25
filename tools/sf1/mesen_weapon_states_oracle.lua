-- Capture one logic-aligned retail presentation for each requested weapon
-- scene in one uninterrupted run. The source renderer completes asynchronously:
-- TRANS.ASM marks a bitmap transfer complete before that bitmap reaches the
-- visible screen. Capturing three completed video frames after that marker
-- selects the first whole, repeatable presentation after the display swap.

local video_frame = 0
local video_frames_per_front_end_tick = 3
local front_end_end_video_frame = 2700
local front_end_confirm_cadence_ticks = 60
local front_end_confirm_hold_ticks = 2
local front_end_last_confirm_tick = 360
local game_destination_select_tick = 380
local game_destination_confirm_tick = 420
local route_selection_confirm_tick = 500
local route_selection_confirm_hold_ticks = 12
local planet_dismiss_start_tick = 840
local planet_dismiss_end_tick = 900
local planet_dismiss_cadence_ticks = 2
local fire_first_game_frame = 318
local fire_last_game_frame = 321
local first_game_frame =
  tonumber(os.getenv("SF1_WEAPON_CAPTURE_FIRST_GAME_FRAME")) or 312
local last_game_frame =
  tonumber(os.getenv("SF1_WEAPON_CAPTURE_LAST_GAME_FRAME")) or 337
local oracle_timeout_video_frames = 6000
local visible_frame_delay = 3

local game_frame_address = 0x15BB
local transfer_progress_address = 0x18BB
local object_render_entry = 0x018456
local capture_lines = {}
local pipeline_lines = {}
local completed_render_scenes = {}
local scheduled_captures = {}
local captured_scenes = {}
local active_render_scene = nil
local graphics_tracking_enabled = false
local capture_count = 0

assert(first_game_frame <= last_game_frame, "weapon state range must be ordered")

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
    string.format("sf1_weapon_scene_%03d.ppm", scene_game_frame),
    table.concat(output))
end

local function front_end_input(tick)
  if tick >= game_destination_select_tick
      and tick < game_destination_select_tick + front_end_confirm_hold_ticks then
    return { down = true }
  end
  if tick >= game_destination_confirm_tick
      and tick < game_destination_confirm_tick + front_end_confirm_hold_ticks then
    return { start = true }
  end
  if tick <= front_end_last_confirm_tick
      and tick % front_end_confirm_cadence_ticks < front_end_confirm_hold_ticks then
    return { start = true }
  end
  if tick >= route_selection_confirm_tick
      and tick < route_selection_confirm_tick + route_selection_confirm_hold_ticks then
    return { start = true }
  end
  if tick >= planet_dismiss_start_tick and tick < planet_dismiss_end_tick
      and (tick - planet_dismiss_start_tick) % planet_dismiss_cadence_ticks == 0 then
    return { b = true }
  end
  return {}
end

local function provide_input()
  if video_frame < front_end_end_video_frame then
    emu.setInput(
      front_end_input(math.floor(video_frame / video_frames_per_front_end_tick)),
      0)
    return
  end
  local game_frame = work_word(game_frame_address)
  emu.setInput({
    y = game_frame >= fire_first_game_frame and game_frame <= fire_last_game_frame
  }, 0)
end

local function flush_captures()
  for scene_game_frame = first_game_frame, last_game_frame do
    assert(captured_scenes[scene_game_frame], string.format(
      "scene game frame %d was not captured", scene_game_frame))
  end
  write_binary("sf1_weapon_captures.txt", table.concat(capture_lines))
  write_binary("sf1_weapon_pipeline.txt", table.concat(pipeline_lines))
  return true
end

local function track_graphics_execution(address, opcode)
  if address == object_render_entry and active_render_scene == nil then
    active_render_scene = work_word(game_frame_address)
    pipeline_lines[#pipeline_lines + 1] = string.format(
      "kind=render_start scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d\n",
      active_render_scene,
      work_word(game_frame_address),
      video_frame)
  end
  if opcode == 0 and active_render_scene ~= nil then
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
  assert(scene_game_frame ~= nil, "bitmap transfer completed without a rendered scene")
  pipeline_lines[#pipeline_lines + 1] = string.format(
    "kind=transfer_complete scene_game_frame=%s observed_game_frame=%d retail_video_frame=%d\n",
    tostring(scene_game_frame),
    work_word(game_frame_address),
    video_frame)
  if scene_game_frame >= first_game_frame and scene_game_frame <= last_game_frame then
    local capture_video_frame = video_frame + visible_frame_delay
    assert(scheduled_captures[capture_video_frame] == nil,
      "two requested scenes targeted the same visible video frame")
    scheduled_captures[capture_video_frame] = {
      scene_game_frame = scene_game_frame,
      transfer_complete_video_frame = video_frame,
    }
  end
end

local function end_frame()
  video_frame = video_frame + 1
  local game_frame = work_word(game_frame_address)
  if not graphics_tracking_enabled and game_frame >= first_game_frame - 6 then
    graphics_tracking_enabled = true
    emu.addMemoryCallback(
      track_graphics_execution,
      emu.callbackType.exec,
      0,
      0x7FFFFF,
      emu.cpuType.gsu,
      emu.memType.gsuMemory)
  end
  local capture = scheduled_captures[video_frame]
  if capture ~= nil then
    local scene_game_frame = capture.scene_game_frame
    assert(not captured_scenes[scene_game_frame], "scene was captured twice")
    capture_screen(scene_game_frame)
    captured_scenes[scene_game_frame] = true
    capture_count = capture_count + 1
    capture_lines[#capture_lines + 1] = string.format(
      "scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d transfer_complete_video_frame=%d\n",
      scene_game_frame,
      game_frame,
      video_frame,
      capture.transfer_complete_video_frame)
    pipeline_lines[#pipeline_lines + 1] = string.format(
      "kind=visible_capture scene_game_frame=%d observed_game_frame=%d retail_video_frame=%d\n",
      scene_game_frame,
      game_frame,
      video_frame)
  end
  if capture_count == last_game_frame - first_game_frame + 1 then
    if not flush_captures() then
      emu.stop(3)
      return
    end
    emu.log("SF1_WEAPON_STATES_ORACLE_DONE")
    emu.stop(0)
    return
  end
  if video_frame >= oracle_timeout_video_frames then
    emu.log("SF1_WEAPON_STATES_ORACLE_TIMEOUT")
    emu.stop(2)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.addMemoryCallback(
  record_transfer_progress,
  emu.callbackType.write,
  transfer_progress_address,
  transfer_progress_address,
  emu.cpuType.snes,
  emu.memType.snesWorkRam)
emu.log("SF1_WEAPON_STATES_ORACLE_LOADED")
