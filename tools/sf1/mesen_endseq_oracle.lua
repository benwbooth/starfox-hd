-- Independent Star Fox end-sequence visual oracle. This verification helper
-- drives the source build to live gameplay, requests its real end-game path,
-- installs one recorded encounter, and captures only the retail BG2/BG3 recap
-- layers. Source-machine storage stays confined to this oracle.

local frame = 0
local gameplay_frames = 0
local forced_ending = false
local captured = false
local layer_mask = tonumber(os.getenv("SF1_ENDSEQ_LAYER_MASK")) or 6
local encounter_entry = tonumber(os.getenv("SF1_ENDSEQ_ENTRY")) or 0
local capture_ticks = tonumber(os.getenv("SF1_ENDSEQ_CAPTURE_TICKS")) or 100

-- Rev 2 retail addresses. The boss/recap fields retain their source-layout
-- offsets relative to the independently established score cursor.
local GAME_FRAME = 0x15BB
local SPECIAL_TOTAL = 0x173C
local SPECIALS_DEAD = 0x14D9
local LEVEL_FINISHED = 0x1FD2
local BOSS_SEQUENCE_LENGTH = 0x1F69
local BOSS_SEQUENCE = 0x1F6B
local SCORE_COUNT = 0x1FBB
local SCORE_BUFFER = 0x1FBD
local DEMO_TICKS = 0x1F21

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

local function output_path(name)
  return emu.getScriptDataFolder() .. "/" .. name
end

local function write_binary(name, contents)
  local file = assert(io.open(output_path(name), "w+b"))
  file:write(contents)
  file:close()
end

local function dump_memory(name, kind, length)
  local bytes = {}
  for address = 0, length - 1 do
    bytes[#bytes + 1] = string.char(emu.read(address, kind, false))
  end
  write_binary(name, table.concat(bytes))
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
  write_binary(
    string.format("endseq_layers_%02d_ticks_%03d.ppm", layer_mask, capture_ticks),
    table.concat(output))
end

local function provide_input()
  local pulse = frame % 180
  emu.setInput({
    start = not forced_ending and frame < 2000 and (pulse == 120 or pulse == 121),
    b = not forced_ending and frame >= 2000 and pulse >= 120 and pulse <= 127,
  }, 0)
end

local function isolate_recap_layers()
  if work_word(DEMO_TICKS) > 0 then
    emu.write(0x212C, layer_mask, emu.memType.snesMemory)
    emu.write(0x212D, 0, emu.memType.snesMemory)
    emu.write(0x2131, 0, emu.memType.snesMemory)
  end
end

local function end_frame()
  frame = frame + 1
  if not forced_ending then
    if work_byte(SPECIAL_TOTAL) > 0 and work_word(GAME_FRAME) > 30 then
      gameplay_frames = gameplay_frames + 1
    else
      gameplay_frames = 0
    end
    if gameplay_frames >= 60 then
      write_work_byte(SPECIAL_TOTAL, 10)
      write_work_byte(SPECIALS_DEAD, 5)
      write_work_word(SCORE_COUNT, 0)
      write_work_byte(SCORE_BUFFER, 50)
      -- Entry is a source endseqboss-relative semantic-table offset. Zero is
      -- Route 1 stage 1; five is Route 1 stage 2.
      write_work_word(BOSS_SEQUENCE, encounter_entry)
      write_work_word(BOSS_SEQUENCE_LENGTH, 2)
      write_work_byte(LEVEL_FINISHED, 6)
      forced_ending = true
      emu.log("SF1_ENDSEQ_FORCED_ENDING")
    elseif frame >= 7000 then
      emu.log("SF1_ENDSEQ_NO_GAMEPLAY")
      emu.stop(2)
    elseif frame % 600 == 0 then
      emu.log(string.format(
        "SF1_ENDSEQ_PROBE frame=%d game=%d total=%d level=%d",
        frame,
        work_word(GAME_FRAME),
        work_byte(SPECIAL_TOTAL),
        work_word(LEVEL_FINISHED)))
    end
    return
  end

  local remaining = work_word(DEMO_TICKS)
  if not captured and remaining == capture_ticks then
    captured = true
    capture_screen()
    dump_memory("endseq_vram.bin", emu.memType.snesVideoRam, 0x10000)
    dump_memory("endseq_cgram.bin", emu.memType.snesCgRam, 0x200)
    local state = emu.getState()
    local keys = {}
    for key, _ in pairs(state) do
      local lower = string.lower(key)
      if string.find(lower, "ppu", 1, true)
        or string.find(lower, "bg", 1, true)
        or string.find(lower, "brightness", 1, true) then
        keys[#keys + 1] = key
      end
    end
    table.sort(keys)
    local lines = {}
    for _, key in ipairs(keys) do
      lines[#lines + 1] = key .. "=" .. tostring(state[key]) .. "\n"
    end
    write_binary("endseq_ppu_state.txt", table.concat(lines))
    emu.log(string.format(
      "SF1_ENDSEQ_ORACLE_DONE frame=%d remaining=%d mask=%d",
      frame,
      remaining,
      layer_mask))
    emu.stop(0)
  elseif frame >= 10000 then
    emu.log("SF1_ENDSEQ_NO_RECAP")
    emu.stop(3)
  end
end

emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(isolate_recap_layers, emu.eventType.startFrame)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
emu.log("SF1_ENDSEQ_ORACLE_LOADED")
