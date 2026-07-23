-- Capture the unattended retail Star Fox 2 boot and title-intro presentation.
-- Source-machine state is oracle-only and remains confined to this script.

local frame_count = 0
local capture_step = 4
local stop_frame = tonumber(os.getenv("SF2_BOOT_STOP_FRAME")) or 1800
local start_frames = {}
local lines = {}

assert(stop_frame > 0, "SF2_BOOT_STOP_FRAME must be positive")
assert(
  stop_frame % capture_step == 0,
  "SF2_BOOT_STOP_FRAME must align with the capture cadence")

for value in string.gmatch(os.getenv("SF2_BOOT_START_FRAMES") or "", "[^,]+") do
  local frame = assert(tonumber(value), "SF2_BOOT_START_FRAMES must contain numbers")
  assert(frame >= 0, "SF2_BOOT_START_FRAMES must contain non-negative frames")
  start_frames[frame] = true
  start_frames[frame + 1] = true
end

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function write_file(name, data)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
  file:write(data)
  file:close()
end

local function capture_screen()
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
  write_file(string.format("sf2_boot_%04d.ppm", frame_count), table.concat(output))
end

local function provide_input()
  emu.setInput({
    start = start_frames[frame_count] or false,
    a = false,
    b = false,
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
  frame_count = frame_count + 1
  if frame_count % capture_step == 0 then
    lines[#lines + 1] = string.format(
      "frame=%d mode=%d submode=%d phase=%d",
      frame_count,
      work_byte(0x1B68),
      work_byte(0x1B76),
      work_byte(0x1BE0))
    capture_screen()
  end
  if frame_count >= stop_frame then
    write_file("sf2_boot_trace.txt", table.concat(lines, "\n") .. "\n")
    emu.stop(0)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
