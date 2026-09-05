-- Read-only retail verification of the Nintendo-logo clipping selectors.
-- Addresses and execution state remain confined to this oracle helper.

local frame = 0
local stop_frame = tonumber(os.getenv("SF2_LOGO_DRAW_STOP")) or 800
assert(stop_frame > 0, "SF2_LOGO_DRAW_STOP must be positive")
local draw_start = 0x0AD0
local record_size = 0x26
local capacity = 64
local policy_offset = 0x1E
local reads = {}
local expected_reads = { [4] = false, [5] = false }
local planes = {}
local installed = { [4] = false, [5] = false }

local function signed_word(address)
  local value = emu.read16(address, emu.memType.gsuWorkRam, false)
  return value >= 0x8000 and value - 0x10000 or value
end

local function plane_installed()
  local slot = emu.read16(0x2858, emu.memType.gsuWorkRam, false)
  if frame < 156 or installed[slot] == nil then return end
  installed[slot] = true
  local fields = {frame, slot}
  -- Object transform in row order, translation, then the installed plane.
  for _, address in ipairs({0x132, 0x138, 0x13E, 0x134, 0x13A, 0x140,
      0x136, 0x13C, 0x142, 0x26, 0x28, 0x2A}) do
    fields[#fields + 1] = signed_word(address)
  end
  local start = 0x2818 + (slot - 1) * 8
  for offset = 0, 6, 2 do fields[#fields + 1] = signed_word(start + offset) end
  planes[#planes + 1] = table.concat(fields, ",")
end

local function policy_read(address, value)
  -- Mesen supplies the mapped $70:xxxx bus address even for a callback
  -- registered against the physical gsuWorkRam range.
  address = address & 0xFFFF
  local delta = address - draw_start
  assert(delta >= 0 and delta < record_size * capacity, "read outside draw-list range")
  if frame < 156 or delta % record_size ~= policy_offset then return end
  local record = address - policy_offset
  local shape = emu.read16(record + 8, emu.memType.gsuWorkRam, false)
  if shape < 0xE530 or shape > 0xE610 then return end
  local state = emu.getState()
  local bank = assert(state["cart.coprocessor.programBank"])
  local pc = assert(state["cart.coprocessor.r15"])
  -- Mesen reports the instruction after the byte load at $01:D1BB.
  assert(bank == 1 and pc == 0xD1BC, "unexpected clipping-selector consumer")
  assert(value == 0 or value == 4 or value == 5, "unexpected logo clipping selector")
  if expected_reads[value] ~= nil then expected_reads[value] = true end
  local key = string.format("%02X:%04X policy=%d", bank, pc, value)
  local entry = reads[key]
  if entry then
    entry.count = entry.count + 1
  else
    reads[key] = { count = 1, frame = frame, shape = shape, record = record }
  end
end

local function end_frame()
  frame = frame + 1
  if frame < stop_frame then return end
  local keys = {}
  for key in pairs(reads) do keys[#keys + 1] = key end
  table.sort(keys)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/logo_draw_policy_reads.txt", "wb"))
  for _, key in ipairs(keys) do
    local entry = reads[key]
    file:write(string.format("%s count=%d first_frame=%d shape=%04X record=%04X\n",
      key, entry.count, entry.frame, entry.shape, entry.record))
  end
  file:close()
  local capture = assert(io.open(emu.getScriptDataFolder() .. "/logo_clipping_planes.csv", "wb"))
  capture:write("frame,slot,xx,xy,xz,yx,yy,yz,zx,zy,zz,x,y,z,nx,ny,nz,distance\n")
  capture:write(table.concat(planes, "\n") .. "\n")
  capture:close()
  assert(expected_reads[4] and expected_reads[5], "both live logo clipping selectors must be consumed")
  assert(installed[4] and installed[5], "both authored logo planes must be installed")
  emu.log("SF2_LOGO_DRAW_POLICY_ORACLE_DONE")
  emu.stop(0)
end

emu.addMemoryCallback(policy_read, emu.callbackType.read,
  draw_start, draw_start + record_size * capacity - 1,
  emu.cpuType.gsu, emu.memType.gsuWorkRam)
emu.addMemoryCallback(plane_installed, emu.callbackType.exec,
  0x01F1BB, 0x01F1BB, emu.cpuType.gsu, emu.memType.gsuMemory)
emu.addEventCallback(function() emu.setInput({}, 0) end, emu.eventType.inputPolled)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
