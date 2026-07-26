-- Independent Star Fox post-mission route-map oracle. This verification
-- helper drives the retail ROM to live gameplay, requests a normal clear,
-- lets the authored tally and map transition run, then records the Arwing
-- course movement, confirmation hold, and next General Pepper sequence.
-- Source-machine addresses remain confined to this oracle script.

local frame = 0
local controller_frame = nil
local controller_confirm_frame = nil
local route_frame = nil
local route_confirmed_frame = nil
local forced_clear = false
local forced_clear_frame = nil
local map_entry_frame = nil
local map_wait_frame = nil
local confirmation_frame = nil
local trace = {}
local captured = {}

local GAME_FRAME = 0x15BB
local PRESENTATION_GAME_FRAME = 0x1640
local SPECIAL_TOTAL = 0x173C
local SPECIALS_DEAD = 0x14D9
local LEVEL_FINISHED = 0x1AD6
local STAGE = 0x175B
local WHICH_ROUTE = 0x175D
local CURRENT_PLANET = 0x175E
local FLASH_SHIP = 0x34
local SHIP_XY = 0x32
local NEW_SHIP_XY = 0x38
local VANISH_X = 0x700034
local VANISH_Y = 0x700036

local CONTROLLER_VANISH_X = 64
local CONTROLLER_VANISH_Y = 48
local CONTROLLER_MINIMUM_GAME_FRAME = 20
local CONTROLLER_DESTINATION_DOWN_DELAY = 20
local CONTROLLER_DESTINATION_CONFIRM_DELAY = 40
local ROUTE_CHANGE_FRAME = 70
local ROUTE_CONFIRM_FRAME = 120
local BRIEFING_DISMISS_FRAME = 520
local NORMAL_CLEAR_FRAME = 700
local NEXT_MISSION_CONFIRM_DELAY = 90
local INPUT_HOLD_FRAMES = 3
local FINISH_AFTER_CONFIRMATION = 650
local TIMEOUT_FRAMES = 20000

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function source_word(address)
  return emu.read16(address, emu.memType.snesMemory, false)
end

local function write_work_byte(address, value)
  emu.write(address, value, emu.memType.snesWorkRam)
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
  if captured[name] then
    return
  end
  captured[name] = true
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
  write_binary("sf1_post_tally_" .. name .. ".ppm", table.concat(output))
end

local function held_at(elapsed, start_frame)
  return elapsed >= start_frame and elapsed <= start_frame + INPUT_HOLD_FRAMES
end

local function in_controller_screen()
  return source_word(VANISH_X) == CONTROLLER_VANISH_X
    and source_word(VANISH_Y) == CONTROLLER_VANISH_Y
end

local function provide_input()
  local input = {}
  if map_wait_frame ~= nil and confirmation_frame == nil then
    local elapsed = frame - map_wait_frame
    input.start = held_at(elapsed, NEXT_MISSION_CONFIRM_DELAY)
  elseif forced_clear then
    -- Tally and return to the route map are authored, noninteractive flows.
  elseif route_confirmed_frame ~= nil then
    input.start = held_at(frame - route_confirmed_frame, BRIEFING_DISMISS_FRAME)
  elseif route_frame ~= nil then
    local elapsed = frame - route_frame
    input.right = held_at(elapsed, ROUTE_CHANGE_FRAME)
    input.start = elapsed >= ROUTE_CONFIRM_FRAME
      and elapsed <= ROUTE_CONFIRM_FRAME + 90
  elseif controller_frame ~= nil then
    if controller_confirm_frame == nil
      and work_word(PRESENTATION_GAME_FRAME) >= CONTROLLER_MINIMUM_GAME_FRAME then
      controller_confirm_frame = frame
    end
    if controller_confirm_frame ~= nil then
      local elapsed = frame - controller_confirm_frame
      input.start = held_at(elapsed, 0)
        or held_at(elapsed, CONTROLLER_DESTINATION_CONFIRM_DELAY)
      input.down = held_at(elapsed, CONTROLLER_DESTINATION_DOWN_DELAY)
    end
  else
    local pulse = frame % 180
    input.start = pulse == 120 or pulse == 121
  end
  emu.setInput(input, 0)
end

local function append_trace(label)
  trace[#trace + 1] = string.format(
    "%s,%d,%d,%d,%d,%d,%d,%d,%d,%d\n",
    label,
    frame,
    map_entry_frame == nil and -1 or frame - map_entry_frame,
    work_word(GAME_FRAME),
    work_byte(STAGE),
    work_byte(WHICH_ROUTE),
    work_byte(CURRENT_PLANET),
    work_byte(FLASH_SHIP),
    work_word(SHIP_XY),
    work_word(NEW_SHIP_XY))
end

local function finish(status, label)
  append_trace(label)
  write_binary("sf1_post_tally_planet_trace.csv", table.concat(trace))
  emu.log(label)
  emu.stop(status)
end

local function end_frame()
  frame = frame + 1

  if controller_frame == nil and in_controller_screen() then
    controller_frame = frame
    emu.log(string.format("SF1_POST_TALLY_CONTROLLER frame=%d", frame))
  end

  if route_frame == nil
    and work_byte(STAGE) == 10
    and work_byte(CURRENT_PLANET) == 0xFE then
    route_frame = frame
    emu.log(string.format("SF1_POST_TALLY_FIRST_ROUTE frame=%d", frame))
  end

  if route_frame ~= nil
    and route_confirmed_frame == nil
    and (work_byte(STAGE) ~= 10 or work_byte(CURRENT_PLANET) ~= 0xFE) then
    route_confirmed_frame = frame
    emu.log(string.format("SF1_POST_TALLY_FIRST_ROUTE_CONFIRMED frame=%d", frame))
  end

  if not forced_clear then
    if route_confirmed_frame ~= nil
      and frame - route_confirmed_frame >= NORMAL_CLEAR_FRAME then
      write_work_byte(SPECIAL_TOTAL, 10)
      write_work_byte(SPECIALS_DEAD, 5)
      write_work_byte(LEVEL_FINISHED, 1)
      forced_clear = true
      forced_clear_frame = frame
      trace[#trace + 1] =
        "label,frame,map_elapsed,game_frame,stage,which_route,current_planet,"
        .. "flash_ship,ship_xy,new_ship_xy\n"
      append_trace("forced_clear")
      emu.log("SF1_POST_TALLY_FORCED_CLEAR")
    end
  else
    if forced_clear_frame ~= nil and work_byte(STAGE) == 0 then
      -- The requested frame can still be inside the final planet-screen
      -- handoff. Keep the clear asserted only until the retail dispatcher
      -- consumes it and increments the stage.
      write_work_byte(LEVEL_FINISHED, 1)
    end
    local stage = work_byte(STAGE)
    local planet = work_byte(CURRENT_PLANET)
    if map_entry_frame == nil and stage == 1 and (planet == 0xFF or planet == 2) then
      map_entry_frame = frame
      capture_screen("map_entry")
      append_trace("map_entry")
      emu.log(string.format("SF1_POST_TALLY_MAP_ENTER frame=%d", frame))
    end

    if map_entry_frame ~= nil then
      local elapsed = frame - map_entry_frame
      append_trace("map")
      if elapsed % 15 == 0 and elapsed <= 180 then
        capture_screen(string.format("travel_%03d", elapsed))
      end

      if map_wait_frame == nil and planet == 0xFE then
        map_wait_frame = frame
        capture_screen("awaiting_confirmation")
        append_trace("awaiting_confirmation")
        emu.log(string.format(
          "SF1_POST_TALLY_AWAITING_CONFIRMATION frame=%d elapsed=%d",
          frame,
          elapsed))
      end

      if map_wait_frame ~= nil
        and confirmation_frame == nil
        and work_byte(FLASH_SHIP) ~= 0 then
        confirmation_frame = frame
        capture_screen("confirmed")
        append_trace("confirmed")
        emu.log(string.format(
          "SF1_POST_TALLY_CONFIRMED frame=%d wait_elapsed=%d",
          frame,
          frame - map_wait_frame))
      end

      if confirmation_frame ~= nil then
        local confirmation_elapsed = frame - confirmation_frame
        if confirmation_elapsed == 100 then
          capture_screen("fade")
        elseif confirmation_elapsed == 260 then
          capture_screen("zoom")
        elseif confirmation_elapsed == 430 then
          capture_screen("briefing")
        elseif confirmation_elapsed >= FINISH_AFTER_CONFIRMATION then
          finish(0, "SF1_POST_TALLY_PLANET_ORACLE_DONE")
          return
        end
      end
    end
  end

  if frame >= TIMEOUT_FRAMES then
    capture_screen("timeout")
    finish(
      2,
      string.format(
        "SF1_POST_TALLY_PLANET_ORACLE_TIMEOUT_controller_%s_route_%s_confirm_%s"
          .. "_presentation_frame_%d_special_total_%d",
        tostring(controller_frame),
        tostring(route_frame),
        tostring(route_confirmed_frame),
        work_word(PRESENTATION_GAME_FRAME),
        work_byte(SPECIAL_TOTAL)))
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF1_POST_TALLY_PLANET_ORACLE_LOADED")
