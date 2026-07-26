-- Independent Star Fox controller-screen visual oracle. This verification
-- helper drives the retail ROM through Intro and Title, captures the live
-- controller-layout frame, accepts the layout, then captures the authored
-- TRAINING/GAME selection frame. Source-machine addresses remain confined to
-- this oracle script.

local frame = 0
local briefing_frame = nil
local destination_input_frame = nil
local destination_requested = false
local captured_layout = false
local captured_destination = false

-- Rev 2 retail source-layout identities.
local GAME_FRAME = 0x1640
local CONTROLLER_TYPE = 0x1A13
local EXIT_DESTINATION = 0xA0E6
local PLAYER_OBJECT = 0x12C3
local PLAYER_VIEW_VELOCITY_Z = 0x157F
local PLAYER_VIEW_X = 0x1581
local PLAYER_VIEW_Y = 0x1583
local PLAYER_VIEW_Z = 0x1585
local CAMERA_VIEW_X = 0xB4
local CAMERA_VIEW_Y = 0xB6
local CAMERA_VIEW_Z = 0xB8
local VIEW_DISTANCE = 0x1622
local VIEW_ROTATION_X = 0x16B9
local VIEW_ROTATION_Y = 0x16BB
local VIEW_ROTATION_Z = 0x16BD
local OUTPUT_VELOCITY_X = 0x1944
local OUTPUT_VELOCITY_Y = 0x1946
local OUTPUT_VELOCITY_Z = 0x1948
local OUTPUT_DISTANCE = 0x194A
local OBJECT_SHAPE_POINTER = 0x04
local OBJECT_WORLD_X = 0x0C
local OBJECT_WORLD_Y = 0x0E
local OBJECT_WORLD_Z = 0x10
local OBJECT_ROTATION_X = 0x12
local OBJECT_ROTATION_Y = 0x13
local OBJECT_ROTATION_Z = 0x14
local VANISH_X = 0x700034
local VANISH_Y = 0x700036
local CLIP_LEFT = 0x2A
local CLIP_TOP = 0x2C
local CLIP_RIGHT = 0x2E
local CLIP_BOTTOM = 0x30

local EXPECTED_VANISH_X = 64
local EXPECTED_VANISH_Y = 48
local LAYOUT_CAPTURE_FRAME = 50
local DESTINATION_MINIMUM_GAME_FRAME = 20
local DESTINATION_CAPTURE_DELAY = 20
local ORACLE_TIMEOUT_FRAMES = 7000

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function signed_work_word(address)
  local value = work_word(address)
  if value >= 0x8000 then
    return value - 0x10000
  end
  return value
end

local function source_word(address)
  return emu.read16(address, emu.memType.snesMemory, false)
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
  for index = 1, size.width * size.height do
    local pixel = screen[index] or 0
    output[#output + 1] = string.char(
      (pixel >> 16) & 0xFF,
      (pixel >> 8) & 0xFF,
      pixel & 0xFF)
  end
  write_binary(name, table.concat(output))
end

local function in_controller_screen()
  return source_word(VANISH_X) == EXPECTED_VANISH_X
    and source_word(VANISH_Y) == EXPECTED_VANISH_Y
end

local function provide_input()
  local start = false
  if briefing_frame == nil then
    local phase = frame % 180
    start = phase == 120 or phase == 121
  elseif not destination_requested
    and work_word(GAME_FRAME) >= DESTINATION_MINIMUM_GAME_FRAME then
    destination_requested = true
    destination_input_frame = frame
    start = true
  elseif destination_requested and frame - destination_input_frame <= 3 then
    -- A short multi-frame press survives the retail edge-trigger update.
    -- The destination loop performs a transfer before it tests START again;
    -- the later release prevents that screen from being confirmed as well.
    start = true
  end
  emu.setInput({ start = start }, 0)
end

local function end_frame()
  frame = frame + 1
  if briefing_frame == nil and in_controller_screen() then
    briefing_frame = frame
    emu.log(string.format(
      "SF1_BRIEFING_ENTER frame=%d gameframe=%d",
      frame,
      work_word(GAME_FRAME)))
  end

  if briefing_frame ~= nil then
    local elapsed = frame - briefing_frame
    if not captured_layout and elapsed >= LAYOUT_CAPTURE_FRAME then
      captured_layout = true
      capture_screen("sf1_briefing_layout.ppm")
    end
    if destination_requested
      and not captured_destination
      and frame - destination_input_frame >= DESTINATION_CAPTURE_DELAY then
      captured_destination = true
      capture_screen("sf1_briefing_destination.ppm")
      local player = work_word(PLAYER_OBJECT)
      local metadata = string.format(
        "frame=%d\ngame_frame=%d\ncontroller_type=%d\nexit_destination=%d\n"
          .. "vanish_x=%d\nvanish_y=%d\n"
          .. "clip_left=%d\nclip_top=%d\nclip_right=%d\nclip_bottom=%d\n"
          .. "player_object=%d\nplayer_shape_pointer=0x%04X\n"
          .. "player_x=%d\nplayer_y=%d\nplayer_z=%d\n"
          .. "player_rx=%d\nplayer_ry=%d\nplayer_rz=%d\n"
          .. "player_view_velocity_z=%d\n"
          .. "player_view_x=%d\nplayer_view_y=%d\nplayer_view_z=%d\n"
          .. "camera_x=%d\ncamera_y=%d\ncamera_z=%d\n"
          .. "view_distance=%d\n"
          .. "view_rx=%d\nview_ry=%d\nview_rz=%d\n"
          .. "output_velocity_x=%d\noutput_velocity_y=%d\n"
          .. "output_velocity_z=%d\noutput_distance=%d\n",
        frame,
        work_word(GAME_FRAME),
        work_byte(CONTROLLER_TYPE),
        work_byte(EXIT_DESTINATION),
        source_word(VANISH_X),
        source_word(VANISH_Y),
        work_word(CLIP_LEFT),
        work_word(CLIP_TOP),
        work_word(CLIP_RIGHT),
        work_word(CLIP_BOTTOM),
        player,
        work_word(player + OBJECT_SHAPE_POINTER),
        signed_work_word(player + OBJECT_WORLD_X),
        signed_work_word(player + OBJECT_WORLD_Y),
        signed_work_word(player + OBJECT_WORLD_Z),
        work_byte(player + OBJECT_ROTATION_X),
        work_byte(player + OBJECT_ROTATION_Y),
        work_byte(player + OBJECT_ROTATION_Z),
        signed_work_word(PLAYER_VIEW_VELOCITY_Z),
        signed_work_word(PLAYER_VIEW_X),
        signed_work_word(PLAYER_VIEW_Y),
        signed_work_word(PLAYER_VIEW_Z),
        signed_work_word(CAMERA_VIEW_X),
        signed_work_word(CAMERA_VIEW_Y),
        signed_work_word(CAMERA_VIEW_Z),
        signed_work_word(VIEW_DISTANCE),
        signed_work_word(VIEW_ROTATION_X),
        signed_work_word(VIEW_ROTATION_Y),
        signed_work_word(VIEW_ROTATION_Z),
        signed_work_word(OUTPUT_VELOCITY_X),
        signed_work_word(OUTPUT_VELOCITY_Y),
        signed_work_word(OUTPUT_VELOCITY_Z),
        signed_work_word(OUTPUT_DISTANCE))
      write_binary("sf1_briefing_state.txt", metadata)
      emu.log("SF1_BRIEFING_ORACLE_DONE")
      emu.stop(0)
    end
  end

  if frame >= ORACLE_TIMEOUT_FRAMES then
    emu.log("SF1_BRIEFING_ORACLE_TIMEOUT")
    emu.stop(2)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF1_BRIEFING_ORACLE_LOADED")
