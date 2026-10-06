-- Independent, read-only opening entropy-order evidence. No input beyond a
-- neutral controller and no state injection. Counts are verification data,
-- never a production refresh schedule or a native-generated expectation.
local last_update = tonumber(os.getenv("SF2_ENTROPY_LAST_UPDATE")) or 440
assert(last_update > 0, "last update must be positive")
local update, visits, draws, actor_draws = 0, 0, 0, 0
local refreshing, drawing = false, nil
local intra_actor, total_refreshes = 0, 0
local lines = {}

local function w8(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function random_text()
  return string.format("%d,%d,%d,%d", w8(0xE0), w8(0xE1), w8(0xE2), w8(0xE3))
end

local function scene_text()
  local slots, poses, seen = {}, {}, {}
  local base = emu.read16(0x12A8, emu.memType.snesWorkRam, false)
  local function word(address)
    return emu.read16(address, emu.memType.snesWorkRam, false)
  end
  local function signed(address)
    local value = word(address)
    return value >= 0x8000 and value - 0x10000 or value
  end
  while base ~= 0 do
    local index = (base - 0x03BD) / 0x3F
    assert(index >= 0 and index < 60 and index == math.floor(index), "invalid actor")
    assert(not seen[base], "actor-list cycle")
    seen[base] = true
    slots[#slots + 1] = string.format("%d", index)
    -- Controller and inactive-player storage are not scene transforms.
    if index ~= 0 and index ~= 1 then
      poses[#poses + 1] = string.format("%d,%d,%d,%d,%d,%d,%d", index,
        signed(base + 12), signed(base + 14), signed(base + 16),
        w8(base + 18), w8(base + 20), w8(base + 22))
    end
    base = word(base)
  end
  return string.format("slots=%s poses=%s camera=%d,%d,%d,%d,%d,%d",
    table.concat(slots, ","), table.concat(poses, ";"),
    signed(0x34B), signed(0x34D), signed(0x34F),
    word(0x351), word(0x353), word(0x355))
end

local function finish()
  lines[#lines + 1] = string.format(
    "summary updates=%d refreshes=%d intra_actor_refreshes=%d",
    last_update, total_refreshes, intra_actor)
  local file = assert(io.open(
    emu.getScriptDataFolder() .. "/sf2_opening_entropy.txt", "wb"))
  file:write(table.concat(lines, "\n") .. "\n")
  file:close()
  assert(intra_actor > 0, "opening did not exercise an intra-actor refresh")
  emu.log("SF2_OPENING_ENTROPY_DONE")
  emu.stop(0)
end

local function controller()
  if update > 0 then
    assert(drawing == nil and not refreshing, "incomplete random draw at boundary")
    lines[#lines + 1] = string.format(
      "complete update=%d visits=%d draws=%d random=%s %s",
      update, visits, draws, random_text(), scene_text())
  else
    lines[#lines + 1] = "start random=" .. random_text()
  end
  if update == last_update then finish(); return end
  update = update + 1
  visits, draws, actor_draws = 0, 0, 0
end

local function next_actor()
  if update == 0 then return end
  visits = visits + 1
  actor_draws = 0
end

local function refresh()
  if update == 0 then return end
  -- A generator interrupted during its arithmetic is a distinct unsupported
  -- ordering contract; do not silently represent it as two atomic draws.
  assert(drawing == nil, "refresh interrupted generator arithmetic")
  assert(not refreshing, "refresh missing its generator call")
  total_refreshes = total_refreshes + 1
  if actor_draws > 0 then intra_actor = intra_actor + 1 end
  local actor = emu.read16(0x12C7, emu.memType.snesWorkRam, false)
  lines[#lines + 1] = string.format(
    "refresh update=%d visits=%d draws=%d actor=%04X actor_draws=%d random=%s",
    update, visits, draws, actor, actor_draws, random_text())
  refreshing = true
end

local function random_entry()
  if update == 0 then return end
  assert(drawing == nil, "nested random draw")
  drawing = refreshing
  refreshing = false
end

local function random_return()
  if update == 0 then return end
  assert(drawing ~= nil, "random return missing its entry")
  if not drawing then
    draws = draws + 1
    actor_draws = actor_draws + 1
  end
  drawing = nil
end

local function watch(address, callback)
  emu.addMemoryCallback(callback, emu.callbackType.exec, address, address,
    emu.cpuType.snes, emu.memType.snesMemory)
end

watch(0x0DBCCF, controller)
watch(0x7F3531, next_actor)
watch(0x7F357D, next_actor)
watch(0x7F058F, refresh)
watch(0x7F7BD4, random_entry)
watch(0x7F7BE7, random_return)
emu.addEventCallback(function()
  emu.setInput({start=false, select=false, a=false, b=false, x=false,
    y=false, l=false, r=false, up=false, down=false, left=false, right=false}, 0)
end, emu.eventType.inputPolled)
