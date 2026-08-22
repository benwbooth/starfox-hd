-- Watch retail newborn robot_0 worldx during Corneria gf851.
-- Load in Mesen Lua console after reaching Corneria gameplay.
-- Logs every write to slots 30/23/31 al_worldx ($7E098A etc).

local POOL_BASE = 0x0336
local STRIDE = 54
local GAME_FRAME = 0x1640

local function work_byte(addr)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function obj_worldx(slot)
  return emu.read(POOL_BASE + slot * STRIDE + 0x0C, emu.memType.snesWorkRam, true)
end

local function obj_shape(slot)
  return emu.read(POOL_BASE + slot * STRIDE + 0x04, emu.memType.snesWorkRam, false)
end

local function game_frame()
  return emu.read(GAME_FRAME, emu.memType.snesWorkRam, true)
end

-- Track robot_0 objects (shape word $BB9C stored as $9C,$BB)
local prev_positions = {}

local function check_frame()
  local gf = game_frame()
  if gf < 848 or gf > 855 then return end

  for slot = 5, 69 do
    local shape_lo = obj_shape(slot)
    -- robot_0 shape word low byte = $9C
    if shape_lo == 0x9C then
      local wx_lo = emu.read(POOL_BASE + slot * STRIDE + 0x0C, emu.memType.snesWorkRam, false)
      local wx_hi = emu.read(POOL_BASE + slot * STRIDE + 0x0D, emu.memType.snesWorkRam, false)
      local wx = wx_hi * 256 + wx_lo
      local wy_lo = emu.read(POOL_BASE + slot * STRIDE + 0x0E, emu.memType.snesWorkRam, false)
      local wy_hi = emu.read(POOL_BASE + slot * STRIDE + 0x0F, emu.memType.snesWorkRam, false)
      local wy = wy_hi * 256 + wy_lo
      local key = tostring(slot)
      local prev = prev_positions[key]
      print(string.format("[robot] gf=%d slot=%d shape_lo=%02X wx=%d wy=%d dx=%s",
        gf, slot, shape_lo, wx, wy,
        prev and tostring(wx - prev) or "new"))
      prev_positions[key] = wx
    end
  end
end

emu.addFrameCallback(check_frame)

print("[robot-watch] loaded — waiting for gf 848-855")
