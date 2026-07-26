-- Independent Star Fox route-map and planet-briefing oracle. This helper
-- drives the retail ROM through the controller screen, chooses GAME, changes
-- route once, confirms it, then records the complete source presentation
-- timeline. Source-machine addresses remain confined to this oracle script.

local frame = 0
local controller_frame = nil
local controller_confirm_frame = nil
local route_frame = nil
local route_confirmed_frame = nil
local trace = {}
local debug_trace = {}
local route_capture_frames = {
  [40] = "route_initial",
  [90] = "route_changed",
}
local confirmation_capture_frames = {
  [0] = "route_confirm",
  [40] = "route_flash",
  [100] = "route_fade",
  [150] = "route_isolated",
  [200] = "route_scroll",
  [260] = "planet_zoom",
  [340] = "pepper_reveal",
  [430] = "pepper_briefing",
}

local GAME_FRAME = 0x1640
local STAGE = 0x175B
local WHICH_ROUTE = 0x175D
local CURRENT_PLANET = 0x175E
local FLASH_SHIP = 0x34
local SHIP_XY = 0x32
local NEW_SHIP_XY = 0x38
local MARIO_PALETTE = 0x70009A
local PLANET_RADIUS = 0x7001C6
local PEPPER_CHARACTERS = 0x7EF14F
local PEPPER_MESSAGE = 0x7EF150
local VANISH_X = 0x700034
local VANISH_Y = 0x700036

local CONTROLLER_VANISH_X = 64
local CONTROLLER_VANISH_Y = 48
local CONTROLLER_MINIMUM_GAME_FRAME = 20
local CONTROLLER_DESTINATION_DOWN_DELAY = 20
local CONTROLLER_DESTINATION_CONFIRM_DELAY = 40
local INPUT_HOLD_FRAMES = 3
local ROUTE_CHANGE_FRAME = 70
local ROUTE_CONFIRM_FRAME = 120
local ROUTE_CONFIRM_MAXIMUM_HOLD_FRAMES = 90
local ORACLE_FINISH_AFTER_CONFIRMATION = 650
local ORACLE_TIMEOUT_FRAMES = 8000

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
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
  write_binary(name .. ".ppm", table.concat(output))
end

local function in_controller_screen()
  return source_word(VANISH_X) == CONTROLLER_VANISH_X
    and source_word(VANISH_Y) == CONTROLLER_VANISH_Y
end

local function held_at(elapsed, start_frame)
  return elapsed >= start_frame
    and elapsed <= start_frame + INPUT_HOLD_FRAMES
end

local function provide_input()
  local input = {}
  if route_frame ~= nil then
    local elapsed = frame - route_frame
    input.right = held_at(elapsed, ROUTE_CHANGE_FRAME)
    input.start = route_confirmed_frame == nil
      and elapsed >= ROUTE_CONFIRM_FRAME
      and elapsed <= ROUTE_CONFIRM_FRAME + ROUTE_CONFIRM_MAXIMUM_HOLD_FRAMES
  elseif controller_frame ~= nil then
    if controller_confirm_frame == nil
      and work_word(GAME_FRAME) >= CONTROLLER_MINIMUM_GAME_FRAME then
      controller_confirm_frame = frame
    end
    if controller_confirm_frame ~= nil then
      local elapsed = frame - controller_confirm_frame
      input.start = held_at(elapsed, 0)
        or held_at(elapsed, CONTROLLER_DESTINATION_CONFIRM_DELAY)
      input.down = held_at(elapsed, CONTROLLER_DESTINATION_DOWN_DELAY)
    end
  else
    local phase = frame % 180
    input.start = phase == 120 or phase == 121
  end
  emu.setInput(input, 0)
end

local function append_trace(elapsed)
  trace[#trace + 1] = string.format(
    "%d,%d,%d,%d,%d,%d,%d,%d,%d,%d,%d,%d\n",
    elapsed,
    work_word(GAME_FRAME),
    work_byte(STAGE),
    work_byte(WHICH_ROUTE),
    work_byte(CURRENT_PLANET),
    source_word(MARIO_PALETTE),
    source_word(PLANET_RADIUS),
    work_byte(FLASH_SHIP),
    work_word(SHIP_XY),
    work_word(NEW_SHIP_XY),
    work_byte(PEPPER_CHARACTERS),
    work_byte(PEPPER_MESSAGE))
end

local function end_frame()
  frame = frame + 1
  if frame % 60 == 0 then
    debug_trace[#debug_trace + 1] = string.format(
      "frame=%d controller=%s route=%s game_frame=%d stage=%d which_route=%d "
        .. "current_planet=%d vanish_x=%d vanish_y=%d\n",
      frame,
      tostring(controller_frame),
      tostring(route_frame),
      work_word(GAME_FRAME),
      work_byte(STAGE),
      work_byte(WHICH_ROUTE),
      work_byte(CURRENT_PLANET),
      source_word(VANISH_X),
      source_word(VANISH_Y))
  end
  if controller_frame == nil and in_controller_screen() then
    controller_frame = frame
    emu.log(string.format("SF1_PLANETS_CONTROLLER frame=%d", frame))
  end

  if route_frame == nil
    and work_byte(STAGE) == 10
    and work_byte(CURRENT_PLANET) == 0xFE then
    route_frame = frame
    trace[#trace + 1] =
      "elapsed,game_frame,stage,which_route,current_planet,mario_palette,"
      .. "planet_radius,flash_ship,ship_xy,new_ship_xy,pepper_characters,"
      .. "pepper_message\n"
    emu.log(string.format("SF1_PLANETS_ENTER frame=%d", frame))
  end

  if route_frame ~= nil then
    local elapsed = frame - route_frame
    append_trace(elapsed)
    local capture = route_capture_frames[elapsed]
    if capture ~= nil then
      capture_screen("sf1_planets_" .. capture)
    end

    if route_confirmed_frame == nil
      and (work_byte(STAGE) ~= 10 or work_byte(CURRENT_PLANET) ~= 0xFE) then
      route_confirmed_frame = frame
      capture_screen("sf1_planets_route_confirm")
      emu.log(string.format(
        "SF1_PLANETS_CONFIRMED frame=%d elapsed=%d",
        frame,
        elapsed))
    end

    if route_confirmed_frame ~= nil then
      local confirmation_elapsed = frame - route_confirmed_frame
      local confirmation_capture = confirmation_capture_frames[confirmation_elapsed]
      if confirmation_capture ~= nil and confirmation_elapsed ~= 0 then
        capture_screen("sf1_planets_" .. confirmation_capture)
      end
      if confirmation_elapsed >= ORACLE_FINISH_AFTER_CONFIRMATION then
        write_binary("sf1_planet_sequence_trace.csv", table.concat(trace))
        write_binary("sf1_planet_sequence_debug.txt", table.concat(debug_trace))
        emu.log("SF1_PLANET_SEQUENCE_ORACLE_DONE")
        emu.stop(0)
      end
    elseif elapsed > ROUTE_CONFIRM_FRAME + ROUTE_CONFIRM_MAXIMUM_HOLD_FRAMES then
      write_binary("sf1_planet_sequence_trace.csv", table.concat(trace))
      write_binary("sf1_planet_sequence_debug.txt", table.concat(debug_trace))
      emu.log("SF1_PLANET_SEQUENCE_ORACLE_CONFIRMATION_FAILED")
      emu.stop(3)
    end
  end

  if frame >= ORACLE_TIMEOUT_FRAMES then
    write_binary("sf1_planet_sequence_trace.csv", table.concat(trace))
    write_binary("sf1_planet_sequence_debug.txt", table.concat(debug_trace))
    emu.log("SF1_PLANET_SEQUENCE_ORACLE_TIMEOUT")
    emu.stop(2)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF1_PLANET_SEQUENCE_ORACLE_LOADED")
