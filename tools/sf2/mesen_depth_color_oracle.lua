-- Read-only retail oracle for polygon depth-colour and light-shade table use.
-- Source-machine addresses stay in this verification helper.  The shipping
-- Rust renderer consumes only typed palette-pair tables recovered from it.

local state_path = assert(
  os.getenv("SF2_ORACLE_LOAD_STATE"),
  "SF2_ORACLE_LOAD_STATE must name a Mesen savestate")
local capture_frames = tonumber(os.getenv("SF2_ORACLE_CAPTURE_FRAMES")) or 180
local label = os.getenv("SF2_ORACLE_LABEL") or "mission"

local state_file = assert(io.open(state_path, "r+b"))
local state_bytes = state_file:read("*a")
state_file:close()

local loaded = false
local frame = 0
local reads = {}
local first_frame = {}
local load_callback = nil

local ranges = {
  { "standard", 0x018B0C, 0x018B8B },
  { "mist", 0x018B8C, 0x018C0B },
  { "desert", 0x018C0C, 0x018C8B },
  { "marine", 0x018C8C, 0x018D0B },
  { "red", 0x018D0C, 0x018D8B },
  { "light", 0x018D8C, 0x018F1B },
}

local function script_path(filename)
  return emu.getScriptDataFolder() .. "/" .. filename
end

local function load_state()
  if loaded then return end
  loaded = true
  emu.loadSavestate(state_bytes)
end

local function trace_read(address)
  if not loaded then return end
  reads[address] = (reads[address] or 0) + 1
  first_frame[address] = first_frame[address] or frame
end

local function family_count(first, last)
  local total = 0
  for address = first, last do
    total = total + (reads[address] or 0)
  end
  return total
end

local function finish()
  local lines = {
    string.format("label=%s frames=%d\n", label, frame),
    "family reads\n",
  }
  for _, range in ipairs(ranges) do
    lines[#lines + 1] = string.format(
      "%s %d\n", range[1], family_count(range[2], range[3]))
  end
  lines[#lines + 1] = "address reads first_frame\n"
  for address = ranges[1][2], ranges[#ranges][3] do
    if reads[address] then
      lines[#lines + 1] = string.format(
        "%06X %d %d\n", address, reads[address], first_frame[address])
    end
  end
  lines[#lines + 1] = "work_address depth_family\n"
  for address = 0, 0xFFFE do
    local value = emu.read16(address, emu.memType.gsuWorkRam, false)
    for family = 1, 5 do
      if value == (ranges[family][2] & 0xFFFF) then
        lines[#lines + 1] = string.format(
          "%04X %s\n", address, ranges[family][1])
      end
    end
  end
  local output = assert(io.open(script_path("depth_color_reads.txt"), "wb"))
  output:write(table.concat(lines))
  output:close()
  emu.log("SF2_DEPTH_COLOR_ORACLE_DONE " .. label)
  emu.stop(0)
end

local function end_frame()
  if not loaded then return end
  frame = frame + 1
  if load_callback then
    emu.removeMemoryCallback(
      load_callback,
      emu.callbackType.exec,
      0x000000,
      0xFFFFFF,
      emu.cpuType.snes,
      emu.memType.snesMemory)
    load_callback = nil
  end
  if frame >= capture_frames then finish() end
end

local function neutral_input()
  if loaded then emu.setInput({}, 0) end
end

load_callback = emu.addMemoryCallback(
  load_state,
  emu.callbackType.exec,
  0x000000,
  0xFFFFFF,
  emu.cpuType.snes,
  emu.memType.snesMemory)
emu.addMemoryCallback(
  trace_read,
  emu.callbackType.read,
  ranges[1][2],
  ranges[#ranges][3],
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
emu.addEventCallback(neutral_input, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF2_DEPTH_COLOR_ORACLE_LOADED")
