-- Read-only retail Star Fox end-of-level tally oracle.  The helper drives a
-- fresh ROM through the menus, requests a normal clear only after gameplay is
-- live, and captures the original tally's semantic state and final pixels.
-- Source-machine storage is confined to this verification tool.

local frame = 0
local gameplay_frames = 0
local forced_clear = false
local tally_entry_frame = nil
local captured = {}
local lines = {}

local GAME_FRAME = 0x15BB
local SPECIAL_TOTAL = 0x173C
local SPECIALS_DEAD = 0x14D9
local LEVEL_FINISHED = 0x1FD2
local TEAM_PEPPY = 0x18A0
local TEAM_FALCO = 0x18A1
local TEAM_SLIPPY = 0x18A2
local SCORE_CURSOR = 0x1FBB
local SCORE_BUFFER = 0x1FBD
local CREDITS = 0x1898

local CURRENT_PERCENT = 0x16
local TARGET_PERCENT = 0x18
local DISPLAY_PHASE = 0x1A
local EXIT_DISPLAY = 0x1C
local COUNTDOWN = 0x20
local PREVIOUS_TOTAL = 0x22
local BONUS_TIMER = 0x24

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function write_work_byte(address, value)
  emu.write(address, value, emu.memType.snesWorkRam)
end

local function write_work_word(address, value)
  write_work_byte(address, value & 0xFF)
  write_work_byte(address + 1, (value >> 8) & 0xFF)
end

local function write_file(name, contents)
  local path = emu.getScriptDataFolder() .. "/" .. name
  local file = assert(io.open(path, "w+b"))
  file:write(contents)
  file:close()
end

local function record(label)
  local line = string.format(
    "%s frame=%d game=%d total=%d dead=%d level=%d current=%d target=%d " ..
      "phase=%d exit=%d countdown=%d old_total=%d bonus=%d cursor=%d " ..
      "scores=%d,%d,%d,%d,%d,%d,%d,%d,%d team=%d,%d,%d credits=%d",
    label,
    frame,
    work_word(GAME_FRAME),
    work_byte(SPECIAL_TOTAL),
    work_byte(SPECIALS_DEAD),
    work_word(LEVEL_FINISHED),
    work_word(CURRENT_PERCENT),
    work_word(TARGET_PERCENT),
    work_word(DISPLAY_PHASE),
    work_word(EXIT_DISPLAY),
    work_word(COUNTDOWN),
    work_word(PREVIOUS_TOTAL),
    work_word(BONUS_TIMER),
    work_word(SCORE_CURSOR),
    work_byte(SCORE_BUFFER + 0),
    work_byte(SCORE_BUFFER + 1),
    work_byte(SCORE_BUFFER + 2),
    work_byte(SCORE_BUFFER + 3),
    work_byte(SCORE_BUFFER + 4),
    work_byte(SCORE_BUFFER + 5),
    work_byte(SCORE_BUFFER + 6),
    work_byte(SCORE_BUFFER + 7),
    work_byte(SCORE_BUFFER + 8),
    work_byte(TEAM_PEPPY),
    work_byte(TEAM_FALCO),
    work_byte(TEAM_SLIPPY),
    work_byte(CREDITS))
  lines[#lines + 1] = line
  emu.log(line)
end

local function capture_screen(label)
  if captured[label] then return end
  captured[label] = true
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
  write_file("tally_" .. label .. ".ppm", table.concat(output))
  record("SF1_TALLY_CAPTURE_" .. label)
end

local function provide_input()
  local menu_pulse = frame % 180
  local accept = frame >= 2000 and menu_pulse >= 120 and menu_pulse <= 127
  emu.setInput({
    start = not forced_clear and frame < 2000
      and (menu_pulse == 120 or menu_pulse == 121),
    a = false,
    b = not forced_clear and accept,
    x = false,
    y = false,
    l = false,
    r = false,
    up = false,
    down = false,
    left = false,
    right = false,
  }, 0)
end

local function end_frame()
  frame = frame + 1

  if not forced_clear then
    if work_byte(SPECIAL_TOTAL) > 0 and work_word(GAME_FRAME) > 30 then
      gameplay_frames = gameplay_frames + 1
    else
      gameplay_frames = 0
    end
    if gameplay_frames >= 60 then
      -- Give the tally a deterministic nonzero target while preserving the
      -- retail teammate contribution and every downstream display operation.
      write_work_byte(SPECIAL_TOTAL, 10)
      write_work_byte(SPECIALS_DEAD, 5)
      write_work_word(CURRENT_PERCENT, 0)
      write_work_word(TARGET_PERCENT, 0)
      write_work_word(DISPLAY_PHASE, 0)
      write_work_word(EXIT_DISPLAY, 0)
      write_work_word(COUNTDOWN, 0)
      write_work_word(PREVIOUS_TOTAL, 0)
      write_work_word(BONUS_TIMER, 0)
      write_work_byte(LEVEL_FINISHED, 1)
      forced_clear = true
      record("SF1_TALLY_FORCED_NORMAL_CLEAR")
    elseif frame >= 6000 then
      record("SF1_TALLY_NO_GAMEPLAY")
      capture_screen("no_gameplay")
      write_file("tally_trace.txt", table.concat(lines, "\n") .. "\n")
      emu.stop(2)
    elseif frame % 600 == 0 then
      record("SF1_TALLY_MENU_PROBE")
      capture_screen(string.format("probe_%04d", frame))
    end
    return
  end

  local current = work_word(CURRENT_PERCENT)
  local target = work_word(TARGET_PERCENT)
  if tally_entry_frame == nil and target > 0 and target <= 100 then
    tally_entry_frame = frame
    capture_screen("entry")
  end
  if tally_entry_frame == nil then return end

  if current >= 3 then capture_screen("count_003") end
  if current >= 30 then capture_screen("count_030") end
  if current >= target and target > 0 then capture_screen("target") end
  if work_word(COUNTDOWN) == 1 then capture_screen("score_committed") end

  if frame - tally_entry_frame >= 180 then
    capture_screen("settled")
    record("SF1_TALLY_DONE")
    write_file("tally_trace.txt", table.concat(lines, "\n") .. "\n")
    emu.stop(0)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF1_TALLY_ORACLE_LOADED")
