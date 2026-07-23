-- Capture the S-CPU stack/register state around SF2's bank-$7F strategy
-- dispatcher with Mesen 2 as an independent execution oracle.
--
-- The trace is buffered in Lua so it cannot perturb the game.  Once the
-- dispatcher returns, the records are copied to GSU work RAM and Mesen stops;
-- its isolated .srm is therefore a compact binary trace file.

local records = {}
local armed = false
local armed_frame = -1
local saw_strategy = false
local frame_count = 0
local capture_after_armed_frames = 204
local trace_base = 0xE000

local function byte(value)
  return (value or 0) & 0xFF
end

local function word(value)
  value = value or 0
  return { value & 0xFF, (value >> 8) & 0xFF }
end

local function append_word(record, value)
  local bytes = word(value)
  record[#record + 1] = bytes[1]
  record[#record + 1] = bytes[2]
end

local function write_trace_and_stop(status)
  -- Header: magic, version, record size, record count.
  local output = { 0x53, 0x46, 0x32, 0x54, 1, 36, #records & 0xFF,
    (#records >> 8) & 0xFF }
  for _, item in ipairs(records) do
    for _, value in ipairs(item) do
      output[#output + 1] = value
    end
  end
  for address, value in ipairs(output) do
    emu.write(trace_base + address - 1, value, emu.memType.gsuWorkRam)
  end
  emu.stop(status)
end

local function capture(pc)
  if not armed or frame_count - armed_frame < capture_after_armed_frames then
    return
  end

  local state = emu.getState()
  local sp = state["cpu.sp"] or 0
  local record = {
    pc & 0xFF, (pc >> 8) & 0xFF, (pc >> 16) & 0xFF,
    byte(state["cpu.ps"]),
  }
  append_word(record, sp)
  append_word(record, state["cpu.a"])
  append_word(record, state["cpu.x"])
  append_word(record, state["cpu.y"])
  append_word(record, state["cpu.d"])
  record[#record + 1] = byte(state["cpu.dbr"])
  record[#record + 1] = byte(state["cpu.irqSource"])
  record[#record + 1] = byte(state["cpu.prevIrqSource"])
  record[#record + 1] = byte(state["cpu.nmiFlagCounter"])
  record[#record + 1] = state["cpu.irqLock"] and 1 or 0
  record[#record + 1] = state["cpu.needNmi"] and 1 or 0

  for offset = 0, 15 do
    record[#record + 1] = emu.read(
      (sp + offset) & 0xFFFF, emu.memType.snesWorkRam, false)
  end
  records[#records + 1] = record

  if pc == 0x7FC4BC then
    saw_strategy = true
  end

  if saw_strategy and pc == 0x7F3680 then
    write_trace_and_stop(0)
  end
end

local function arm_for_target_stage()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame_count
  end
end

local function end_frame()
  frame_count = frame_count + 1
  -- Always leave a diagnostic trace instead of relying on the frontend's
  -- wall-clock timeout.  Ten seconds after the target decompression is long
  -- enough to establish whether this dispatcher is part of retail execution.
  if armed and frame_count - armed_frame >= 600 then
    write_trace_and_stop(1)
  end
end

local function provide_input()
  -- Mirror sf2_boot_capture's late, single Start pulse closely enough to
  -- exercise the same title/attract transition in the independent emulator.
  local elapsed = frame_count - armed_frame
  emu.setInput({ start = armed and elapsed >= 200 and elapsed <= 203 }, 0)
end

local trace_addresses = {
  0x7F3596,
  0x7F363B,
  0x7F3650,
  0x7F3680,
  0x7FC4BC,
  0x7FC4BD,
  0x7FC4DF,
  0x7F7EA7,
  0x7F7EE2,
  0x00FBBF,
}

for _, address in ipairs(trace_addresses) do
  emu.addMemoryCallback(
    function() capture(address) end,
    emu.callbackType.exec,
    address,
    address,
    emu.cpuType.snes,
    emu.memType.snesMemory)
end

emu.addMemoryCallback(
  arm_for_target_stage,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
