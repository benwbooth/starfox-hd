-- Clean-room camera/object trace for the first retail sortie transition.
-- Source addresses stay in this oracle script; native Rust receives typed
-- object poses and presentation phases only.

local frame = 0
local armed = false
local armed_frame = -1
local lines = {}
local stop_elapsed = 6800
local draw_record_size = 0x26
local draw_list_start = 0xB273
local screenshot_frames = {
  [6490] = true,
  [6500] = true,
  [6510] = true,
  [6520] = true,
  [6560] = true,
  [6730] = true,
  [6740] = true,
}

local function script_path(filename)
  return emu.getScriptDataFolder() .. "/" .. filename
end

local function write_binary(filename, contents)
  local file = assert(io.open(script_path(filename), "w+b"))
  file:write(contents)
  file:close()
end

local function capture_screen(elapsed)
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
  write_binary(string.format("sf2_first_sortie_%04d.ppm", elapsed), table.concat(output))
end

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

local function signed_word(address)
  local value = work_word(address)
  if value >= 0x8000 then return value - 0x10000 end
  return value
end

local function object_pose(address)
  return string.format(
    "%04X,%04X,%d,%d,%d,%d,%d,%d",
    address,
    work_word(address + 4),
    signed_word(address + 12),
    signed_word(address + 14),
    signed_word(address + 16),
    work_byte(address + 18),
    work_byte(address + 20),
    work_byte(address + 22))
end

local function draw_poses()
  local output = {}
  local count = work_word(0x18C6)
  for index = 0, math.min(count, 64) - 1 do
    local record = draw_list_start + index * draw_record_size
    output[#output + 1] = string.format(
      "%04X,%d,%d,%d",
      work_word(record + 8),
      signed_word(record + 32),
      signed_word(record + 34),
      signed_word(record + 36))
  end
  return table.concat(output, ";")
end

local function record(elapsed)
  lines[#lines + 1] = string.format(
    "elapsed=%d mode=%d submode=%d phase=%d " ..
      "camera=%d,%d,%d,%d,%d,%d player1=[%s] player2=[%s] draws=[%s]",
    elapsed,
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    signed_word(0x034B),
    signed_word(0x034D),
    signed_word(0x034F),
    work_byte(0x0351),
    work_byte(0x0353),
    work_byte(0x0355),
    object_pose(0x03BD),
    object_pose(0x03FC),
    draw_poses())
end

local function pulse(value, period, offset)
  local phase = value % period
  return phase == offset or phase == offset + 1
end

local function provide_input()
  local elapsed = frame - armed_frame
  local accept = armed and elapsed >= 210 and pulse(elapsed, 90, 30)
  emu.setInput({
    start = pulse(frame, 180, 120) and (not armed or elapsed <= 600),
    b = accept,
    up = armed and elapsed >= 6000 and elapsed < 6045,
    right = armed and elapsed >= 6045 and elapsed < 6070,
  }, 0)
end

local function arm_for_target_stream()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame
  end
end

local function end_frame()
  frame = frame + 1
  if not armed then return end
  local elapsed = frame - armed_frame
  if elapsed >= 6380 and elapsed <= stop_elapsed and elapsed % 10 == 0 then
    record(elapsed)
    if screenshot_frames[elapsed] then capture_screen(elapsed) end
  end
  if elapsed >= stop_elapsed then
    write_binary("sf2_first_sortie_trace.txt", table.concat(lines, "\n") .. "\n")
    emu.stop(0)
  end
end

emu.addMemoryCallback(
  arm_for_target_stream,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
