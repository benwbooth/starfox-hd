-- Clean-room trace of the retail command-map transition after the neutral
-- first sortie. Source-machine addresses are confined to this oracle helper;
-- the native port consumes only the recovered semantic states and timings.

local resume_state_path = os.getenv("SF2_ORACLE_LOAD_STATE")
local resume_elapsed = tonumber(os.getenv("SF2_ORACLE_RESUME_ELAPSED")) or 0
local input_script_text = os.getenv("SF2_ORACLE_INPUT_SCRIPT")
local combat_autopilot = os.getenv("SF2_ORACLE_COMBAT_AUTOPILOT") == "1"
combat_hold_fire = os.getenv("SF2_ORACLE_COMBAT_HOLD_FIRE") == "1"
local frame = resume_state_path and resume_elapsed or 0
local armed = resume_state_path ~= nil
local armed_frame = resume_state_path and 0 or -1
local stop_elapsed = tonumber(os.getenv("SF2_ORACLE_STOP_ELAPSED")) or 25000
local map_layer_mask_text = os.getenv("SF2_ORACLE_MAP_LAYER_MASK")
local map_layer_mask = map_layer_mask_text and tonumber(map_layer_mask_text) or nil
local capture_ppu = os.getenv("SF2_ORACLE_CAPTURE_PPU") == "1"
local capture_loaded_state =
  os.getenv("SF2_ORACLE_CAPTURE_LOADED_STATE") == "1"
local capture_full_work =
  os.getenv("SF2_ORACLE_CAPTURE_FULL_WRAM") == "1"
local continue_campaign = os.getenv("SF2_ORACLE_CONTINUE_CAMPAIGN") == "1"
local teleport_text = os.getenv("SF2_ORACLE_PLAYER_TELEPORT")
local lock_teleport_horizontal =
  os.getenv("SF2_ORACLE_LOCK_HORIZONTAL") == "1"
local lock_teleport_vertical =
  os.getenv("SF2_ORACLE_LOCK_VERTICAL") == "1"
local target_meteor_core_parent =
  os.getenv("SF2_ORACLE_TARGET_METEOR_CORE_PARENT") == "1"
local force_projectile_hit =
  os.getenv("SF2_ORACLE_FORCE_PROJECTILE_HIT") == "1"
-- Oracle-only allocation probe for the surface target created by Queen
-- Dragoon's retail death path. This intentionally exposes source addresses;
-- the native port consumes only the resulting semantic evidence.
meteor_switch_oracle = {
  enabled = os.getenv("SF2_ORACLE_TRACE_METEOR_SWITCH") == "1",
  shape_writes =
    os.getenv("SF2_ORACLE_METEOR_SWITCH_SHAPE_WRITES") ~= "0",
  minimum_elapsed = tonumber(
    os.getenv("SF2_ORACLE_METEOR_SWITCH_START_ELAPSED")) or 0,
  lines = {},
}
player_damage_oracle = {
  force = os.getenv("SF2_ORACLE_FORCE_HOSTILE_PROJECTILE_HIT") == "1",
  trace = os.getenv("SF2_ORACLE_TRACE_PLAYER_DAMAGE") == "1",
  probe = os.getenv("SF2_ORACLE_PROBE_PLAYER_DAMAGE") == "1",
  maximum_hits = tonumber(os.getenv("SF2_ORACLE_FORCE_HOSTILE_HITS")) or 1,
  impact_offset_x = tonumber(
    os.getenv("SF2_ORACLE_HOSTILE_IMPACT_OFFSET_X")) or 0,
  impact_offset_y = tonumber(
    os.getenv("SF2_ORACLE_HOSTILE_IMPACT_OFFSET_Y")) or 0,
  impact_offset_z = tonumber(
    os.getenv("SF2_ORACLE_HOSTILE_IMPACT_OFFSET_Z")) or 0,
  projectile = nil,
  locked_hits = 0,
  initial_health = nil,
  minimum_health = nil,
  hit_elapsed = nil,
  last_snapshot = nil,
  lines = {},
}
assert(
  player_damage_oracle.maximum_hits >= 1,
  "SF2_ORACLE_FORCE_HOSTILE_HITS must be positive")
for _, offset in ipairs({
  player_damage_oracle.impact_offset_x,
  player_damage_oracle.impact_offset_y,
  player_damage_oracle.impact_offset_z,
}) do
  assert(
    offset >= -32768 and offset <= 32767,
    "hostile impact offsets must be signed words")
end
local force_target_collision =
  os.getenv("SF2_ORACLE_FORCE_TARGET_COLLISION") == "1"
local force_meteor_core_trigger =
  os.getenv("SF2_ORACLE_FORCE_METEOR_CORE_TRIGGER") == "1"
force_macbeth_core_gate =
  os.getenv("SF2_ORACLE_FORCE_MACBETH_CORE_GATE") == "1"
force_macbeth_core_gate_applied = false
force_fortuna_boss_gate =
  os.getenv("SF2_ORACLE_FORCE_FORTUNA_BOSS_GATE") == "1"
force_fortuna_boss_gate_applied = false
force_fortuna_core_gate =
  os.getenv("SF2_ORACLE_FORCE_FORTUNA_CORE_GATE") == "1"
force_fortuna_core_gate_applied = false
-- ImportByteIndexed 0x2B reads the retail encounter latch at
-- INDEXED_VARIABLE_TABLE + 0x2B. Keep the named oracle address global because
-- this large script is at Lua's main-chunk local limit.
METEOR_CORE_TRIGGER_ADDRESS = 0xD787
METEOR_CORE_WAITING_PATH = 0x5F92
METEOR_CORE_RECHECK_PATH = 0x5F9A
local trace_stage_writes =
  os.getenv("SF2_ORACLE_TRACE_STAGE_WRITES") == "1"
trace_audio_programs =
  os.getenv("SF2_ORACLE_TRACE_AUDIO_PROGRAMS") == "1"
local trace_map_motion =
  os.getenv("SF2_ORACLE_TRACE_MAP_MOTION") == "1"
local trace_final_gate =
  os.getenv("SF2_ORACLE_TRACE_FINAL_GATE") == "1"
local trace_final_activation =
  os.getenv("SF2_ORACLE_TRACE_FINAL_ACTIVATION") == "1"
local trace_threat_retirement =
  os.getenv("SF2_ORACLE_TRACE_THREAT_RETIREMENT") == "1"
local trace_craft_transition =
  os.getenv("SF2_ORACLE_TRACE_CRAFT_TRANSITION") == "1"
local trace_walker_dynamics =
  os.getenv("SF2_ORACLE_TRACE_WALKER_DYNAMICS") == "1"
local trace_walker_writes =
  os.getenv("SF2_ORACLE_TRACE_WALKER_WRITES") == "1"
local traced_map_actor_text = os.getenv("SF2_ORACLE_TRACE_MAP_ACTOR")
local traced_map_actor = traced_map_actor_text
  and tonumber(traced_map_actor_text, 16) or nil
local trace_map_control =
  os.getenv("SF2_ORACLE_TRACE_MAP_CONTROL") == "1"
local trace_map_control_reads =
  os.getenv("SF2_ORACLE_TRACE_MAP_CONTROL_READS") == "1"
local trace_astropolis_gate =
  os.getenv("SF2_ORACLE_TRACE_ASTROPOLIS_GATE") == "1"
local ignore_pressure_encounters =
  os.getenv("SF2_ORACLE_IGNORE_PRESSURE") == "1"
local avoid_pressure_encounters =
  os.getenv("SF2_ORACLE_AVOID_PRESSURE") == "1"
local enable_final_target =
  os.getenv("SF2_ORACLE_ENABLE_FINAL_TARGET") == "1"
local repair_final_activation =
  os.getenv("SF2_ORACLE_REPAIR_FINAL_ACTIVATION") == "1"
local finish_strategic_threats =
  os.getenv("SF2_ORACLE_FINISH_STRATEGIC_THREATS") == "1"
local finished_strategic_threats = false
local repaired_final_activation = false
local preserve_shields =
  os.getenv("SF2_ORACLE_PRESERVE_SHIELDS") == "1"
local preserve_shields_until_elapsed = tonumber(
  os.getenv("SF2_ORACLE_PRESERVE_SHIELDS_UNTIL_ELAPSED"))
local forced_player_health = tonumber(os.getenv("SF2_ORACLE_PLAYER_HEALTH"))
local skip_surface_objectives =
  os.getenv("SF2_ORACLE_SKIP_SURFACE_OBJECTIVES") == "1"
local finish_current_mission =
  os.getenv("SF2_ORACLE_FINISH_CURRENT_MISSION") == "1"
local finish_each_mission =
  os.getenv("SF2_ORACLE_FINISH_EACH_MISSION") == "1"
forced_objective_remaining = tonumber(
  os.getenv("SF2_ORACLE_OBJECTIVE_REMAINING"))
forced_objective_remaining_applied = false
forced_base_destroyed_bits = tonumber(
  os.getenv("SF2_ORACLE_BASE_DESTROYED_BITS"))
forced_base_destroyed_bits_applied = false
forced_base_handshake_bits = tonumber(
  os.getenv("SF2_ORACLE_BASE_HANDSHAKE_BITS"))
forced_base_handshake_bits_applied = false
local skipped_surface_objectives = false
local finished_current_mission = false
local forced_projectile_hit_applied = false
local forced_target_collision_applied = false
local forced_target_collision_frame = nil
local forced_target_initial_path = nil
local forced_target_reaction_seen = false
local forced_target_reaction_complete = false
local forced_meteor_core_trigger_applied = false
local forced_projectile_address = nil
local forced_projectile_position = nil
local observed_rival_health = {}
local observed_astropolis_spike_health = {}
local observed_astropolis_cube_health = {}
local observed_astropolis_mask_health = {}
local observed_astropolis_final_core_health = {}
-- Deliberately global: this large oracle is at Lua's main-chunk local limit.
-- The table records verification evidence only and is never consumed by the
-- shipping port.
observed_eladard_barrier_health = {}
local forced_meteor_core_health = tonumber(os.getenv("SF2_ORACLE_METEOR_CORE_HEALTH"))
local forced_meteor_core_health_applied = false
local forced_meteor_core_parent_health = tonumber(
  os.getenv("SF2_ORACLE_METEOR_CORE_PARENT_HEALTH"))
local forced_rival_health = tonumber(os.getenv("SF2_ORACLE_RIVAL_HEALTH"))
local forced_mirage_health = tonumber(os.getenv("SF2_ORACLE_MIRAGE_HEALTH"))
local forced_fighter_health = tonumber(os.getenv("SF2_ORACLE_FIGHTER_HEALTH"))
preserve_fighter_attackers =
  os.getenv("SF2_ORACLE_PRESERVE_FIGHTER_ATTACKERS") == "1"
forced_reserve_pilot_index = tonumber(
  os.getenv("SF2_ORACLE_RESERVE_PILOT_INDEX"))
if forced_reserve_pilot_index then
  assert(
    forced_reserve_pilot_index >= 0 and forced_reserve_pilot_index <= 5,
    "SF2_ORACLE_RESERVE_PILOT_INDEX must identify one of six pilots")
end
local forced_target_shape_text = os.getenv("SF2_ORACLE_TARGET_SHAPE")
local forced_target_shape = forced_target_shape_text
  and tonumber(forced_target_shape_text, 16) or nil
-- Deliberately global: this large oracle has reached Lua's main-chunk local
-- limit. Exact source-object selection is oracle instrumentation only.
forced_target_object_text = os.getenv("SF2_ORACLE_TARGET_OBJECT")
forced_target_object = forced_target_object_text
  and tonumber(forced_target_object_text, 16) or nil
-- Bind an exact source-object request to the shape occupying that slot on the
-- first resumed frame. Retail recycles object addresses immediately after an
-- actor is removed; continuing to match the bare address would clamp and fire
-- at an unrelated replacement actor.
forced_target_object_shape = nil
forced_target_object_retired = false
local forced_target_health = tonumber(os.getenv("SF2_ORACLE_TARGET_HEALTH"))
forced_target_path_text = os.getenv("SF2_ORACLE_TARGET_PATH")
forced_target_path = forced_target_path_text
  and tonumber(forced_target_path_text, 16) or nil
forced_target_path_applied = false
if forced_target_shape_text then
  assert(
    forced_target_shape and forced_target_shape <= 0xFFFF,
    "SF2_ORACLE_TARGET_SHAPE must be a four-digit hexadecimal shape token")
end
if forced_target_object_text then
  assert(
    forced_target_object and forced_target_object <= 0xFFFF,
    "SF2_ORACLE_TARGET_OBJECT must be a four-digit hexadecimal object address")
end
if forced_target_health then
  assert(
    forced_target_health >= 0 and forced_target_health <= 255,
    "SF2_ORACLE_TARGET_HEALTH must be byte-sized")
end
if forced_target_path_text then
  assert(
    forced_target_path and forced_target_path <= 0xFFFF,
    "SF2_ORACLE_TARGET_PATH must be a four-digit hexadecimal path offset")
end
if forced_objective_remaining then
  assert(
    forced_objective_remaining >= 0 and forced_objective_remaining <= 255,
    "SF2_ORACLE_OBJECTIVE_REMAINING must be byte-sized")
end
if forced_base_destroyed_bits then
  assert(
    forced_base_destroyed_bits >= 0 and forced_base_destroyed_bits <= 65535,
    "SF2_ORACLE_BASE_DESTROYED_BITS must be word-sized")
end
if forced_base_handshake_bits then
  assert(
    forced_base_handshake_bits >= 0 and forced_base_handshake_bits <= 255,
    "SF2_ORACLE_BASE_HANDSHAKE_BITS must be byte-sized")
end
if forced_player_health then
  assert(
    forced_player_health >= 0 and forced_player_health <= 255,
    "SF2_ORACLE_PLAYER_HEALTH must be byte-sized")
end
local forced_corneria_damage = tonumber(
  os.getenv("SF2_ORACLE_CORNERIA_DAMAGE"))
local forced_stage_selection = tonumber(
  os.getenv("SF2_ORACLE_STAGE_SELECTION"))
forced_difficulty_selection = tonumber(
  os.getenv("SF2_ORACLE_DIFFICULTY_SELECTION"))
forced_active_difficulty_selection = tonumber(
  os.getenv("SF2_ORACLE_ACTIVE_DIFFICULTY_SELECTION"))
local forced_map_target_selection = tonumber(
  os.getenv("SF2_ORACLE_MAP_TARGET_SELECTION"))
-- Deliberately global: this large oracle has reached Lua's main-chunk local
-- limit. Shipping Rust never receives this source selection or object layout.
forced_occupied_selection = tonumber(
  os.getenv("SF2_ORACLE_FORCE_OCCUPIED_SELECTION"))
local chased_map_selection = tonumber(
  os.getenv("SF2_ORACLE_CHASE_MAP_SELECTION"))
local chase_map_once = os.getenv("SF2_ORACLE_CHASE_ONCE") == "1"
local requested_chased_map_actor_text =
  os.getenv("SF2_ORACLE_CHASE_MAP_ACTOR")
local requested_chased_map_actor = requested_chased_map_actor_text
  and tonumber(requested_chased_map_actor_text, 16) or nil
local chased_map_actor = requested_chased_map_actor
local map_chase_engaged = false
local forced_map_cursor_text = os.getenv("SF2_ORACLE_MAP_CURSOR")
local forced_map_cursor_x, forced_map_cursor_y = string.match(
  forced_map_cursor_text or "",
  "^(%d+),(%d+)$")
forced_map_cursor_x = tonumber(forced_map_cursor_x)
forced_map_cursor_y = tonumber(forced_map_cursor_y)
local parked_map_team_text = os.getenv("SF2_ORACLE_PARK_MAP_TEAM")
local evade_pressure = os.getenv("SF2_ORACLE_EVADE_PRESSURE") == "1"
local parked_map_team_x, parked_map_team_y = string.match(
  parked_map_team_text or "",
  "^(%d+),(%d+)$")
parked_map_team_x = tonumber(parked_map_team_x)
parked_map_team_y = tonumber(parked_map_team_y)
if forced_map_cursor_text then
  assert(
    forced_map_cursor_x and forced_map_cursor_x <= 255
      and forced_map_cursor_y and forced_map_cursor_y <= 255,
    "SF2_ORACLE_MAP_CURSOR must be x,y with byte-sized coordinates")
end
if parked_map_team_text then
  assert(
    parked_map_team_x and parked_map_team_x <= 255
      and parked_map_team_y and parked_map_team_y <= 255,
    "SF2_ORACLE_PARK_MAP_TEAM must be x,y with byte-sized coordinates")
end
if forced_corneria_damage then
  assert(
    forced_corneria_damage >= 0 and forced_corneria_damage <= 100,
    "SF2_ORACLE_CORNERIA_DAMAGE must be between 0 and 100")
end
if forced_stage_selection then
  assert(
    forced_stage_selection >= 0 and forced_stage_selection <= 255,
    "SF2_ORACLE_STAGE_SELECTION must be byte-sized")
end
if forced_difficulty_selection then
  assert(
    forced_difficulty_selection >= 0 and forced_difficulty_selection <= 2,
    "SF2_ORACLE_DIFFICULTY_SELECTION must identify a retail difficulty")
end
if forced_active_difficulty_selection then
  assert(
    forced_active_difficulty_selection >= 0
      and forced_active_difficulty_selection <= 2,
    "SF2_ORACLE_ACTIVE_DIFFICULTY_SELECTION must identify a retail difficulty")
end
if forced_map_target_selection then
  assert(
    forced_map_target_selection >= 0
      and forced_map_target_selection <= 255,
    "SF2_ORACLE_MAP_TARGET_SELECTION must be byte-sized")
end
if forced_occupied_selection then
  assert(
    forced_occupied_selection >= 0 and forced_occupied_selection <= 5,
    "SF2_ORACLE_FORCE_OCCUPIED_SELECTION must identify one of six planets")
end
if chased_map_selection then
  assert(
    chased_map_selection >= 0 and chased_map_selection <= 255,
    "SF2_ORACLE_CHASE_MAP_SELECTION must be byte-sized")
end
if requested_chased_map_actor_text then
  assert(
    requested_chased_map_actor
      and requested_chased_map_actor >= 0
      and requested_chased_map_actor <= 0xFFFF,
    "SF2_ORACLE_CHASE_MAP_ACTOR must be a four-digit hexadecimal address")
end
if traced_map_actor_text then
  assert(
    traced_map_actor
      and traced_map_actor >= 0
      and traced_map_actor <= 0xFFFF,
    "SF2_ORACLE_TRACE_MAP_ACTOR must be a four-digit hexadecimal address")
end
local teleport_x, teleport_y, teleport_z = string.match(
  teleport_text or "",
  "^(-?%d+),(-?%d+),(-?%d+)$")
teleport_x = tonumber(teleport_x)
teleport_y = tonumber(teleport_y)
teleport_z = tonumber(teleport_z)
local teleport_yaw = tonumber(os.getenv("SF2_ORACLE_PLAYER_YAW"))
local teleported_player = false
-- A resumed placement must span enough input callbacks for the retail player,
-- camera, and linked transform state to agree before the oracle releases it.
-- One callback only updates the visible object coordinates, which retail
-- movement immediately replaces from its restored state.
local teleport_frames_remaining = tonumber(os.getenv("SF2_ORACLE_TELEPORT_FRAMES")) or 120
local sortie_stride = tonumber(os.getenv("SF2_ORACLE_SORTIE_STRIDE")) or 4
-- Oracle-only detail for recovering Mirage Dragon's articulated-body
-- scheduler. Shipping Rust models these values as typed segment state.
TRACE_MIRAGE_SEGMENT_STATE =
  os.getenv("SF2_ORACLE_TRACE_MIRAGE_SEGMENTS") == "1"
local save_elapsed = tonumber(os.getenv("SF2_ORACLE_SAVE_ELAPSED"))
local pending_savestate = false
local saved_state = false
local save_callback_reference = nil
local loaded_state = resume_state_path == nil
-- Deliberately global: this oracle has reached Lua's main-chunk local limit.
restoring_state = false
local load_callback_reference = nil
local resume_state = nil
if resume_state_path then
  local file = assert(io.open(resume_state_path, "r+b"))
  resume_state = file:read("*a")
  file:close()
end
local scripted_inputs = {}
for action in string.gmatch(input_script_text or "", "[^;]+") do
  local first, last, button_text = string.match(action, "^(%d+)%-(%d+):(.+)$")
  assert(first and last and button_text, "invalid SF2_ORACLE_INPUT_SCRIPT action: " .. action)
  local buttons = {}
  for button in string.gmatch(button_text, "[^+]+") do
    buttons[button] = true
  end
  scripted_inputs[#scripted_inputs + 1] = {
    first = tonumber(first),
    last = tonumber(last),
    label = button_text,
    buttons = buttons,
  }
end
local requested_captures = {}
for value in string.gmatch(os.getenv("SF2_ORACLE_CAPTURE_ELAPSED") or "", "[^,]+") do
  local elapsed = tonumber(value)
  if elapsed then requested_captures[elapsed] = true end
end
capture_screen_range = nil
capture_range_text = os.getenv("SF2_ORACLE_CAPTURE_SCREEN_RANGE")
if capture_range_text then
  capture_range_first, capture_range_last, capture_range_step = string.match(
    capture_range_text,
    "^(%d+),(%d+),(%d+)$")
  assert(capture_range_first and capture_range_last and capture_range_step,
    "SF2_ORACLE_CAPTURE_SCREEN_RANGE must be first,last,step")
  capture_screen_range = {
    first = tonumber(capture_range_first),
    last = tonumber(capture_range_last),
    step = tonumber(capture_range_step),
  }
  assert(capture_screen_range.first <= capture_screen_range.last,
    "SF2_ORACLE_CAPTURE_SCREEN_RANGE must not be reversed")
  assert(capture_screen_range.step > 0,
    "SF2_ORACLE_CAPTURE_SCREEN_RANGE step must be positive")
end

function capture_screen_range_contains(elapsed)
  return capture_screen_range
    and elapsed >= capture_screen_range.first
    and elapsed <= capture_screen_range.last
    and (elapsed - capture_screen_range.first) % capture_screen_range.step == 0
end
local temporarily_masked_pressure = {}
local lines = {}
lines[#lines + 1] = string.format(
  "elapsed=%d event=oracle-config target_object=%s target_health=%s " ..
    "target_collision=%s projectile_hit=%s hostile_projectile_hit=%s " ..
    "meteor_core_health=%s meteor_core_parent_health=%s meteor_core_trigger=%s " ..
    "objective_remaining=%s base_destroyed_bits=%s " ..
    "base_handshake_bits=%s teleport=%s preserve_shields=%s " ..
      "preserve_shields_until=%s",
  resume_elapsed,
  forced_target_object_text or "none",
  tostring(forced_target_health),
  tostring(force_target_collision),
  tostring(force_projectile_hit),
  tostring(player_damage_oracle.force),
  tostring(forced_meteor_core_health),
  tostring(forced_meteor_core_parent_health),
  tostring(force_meteor_core_trigger),
  tostring(forced_objective_remaining),
  tostring(forced_base_destroyed_bits),
  tostring(forced_base_handshake_bits),
  tostring(teleport_text ~= nil),
  tostring(preserve_shields),
  tostring(preserve_shields_until_elapsed))
audio_program_lines = {}
local craft_transition_lines = {}
local walker_dynamics_lines = {}
sortie_actor_oracle = {
  enabled = os.getenv("SF2_ORACLE_TRACE_SORTIE_ACTOR_LOGIC") == "1",
  projectiles_enabled =
    os.getenv("SF2_ORACLE_TRACE_SORTIE_PROJECTILE_LOGIC") == "1",
  lines = {},
  projectile_lines = {},
  objects = {
    [0x0633] = true,
    [0x05F4] = true,
    [0x05B5] = true,
    [0x0576] = true,
  },
}
explicit_trace_objects = {}
for trace_object_token in string.gmatch(
  os.getenv("SF2_ORACLE_TRACE_OBJECTS") or "", "[^,]+") do
  trace_object_address = tonumber(trace_object_token, 16)
  assert(
    trace_object_address and trace_object_address <= 0xFFFF,
    "SF2_ORACLE_TRACE_OBJECTS must contain comma-separated four-digit " ..
      "hexadecimal object addresses")
  sortie_actor_oracle.objects[trace_object_address] = true
  explicit_trace_objects[trace_object_address] = true
end
local walker_dynamics_before = nil
local last_state = ""
local input_label = "idle"
local final_gate_accesses = {}
local eladard_last_x = nil
local eladard_last_z = nil
local eladard_stuck_polls = 0
local eladard_progress_anchor_x = nil
local eladard_progress_anchor_z = nil
local eladard_no_progress_polls = 0
local eladard_flight_recovery_polls = 0
local eladard_centered_polls = 0
local eladard_recovery_count = 0
local eladard_transform_press_until = -1
local astropolis_transform_press_until = -1
local astropolis_transform_requested = false
local astropolis_walker_mode = false
macbeth_transform_press_until = -1
macbeth_next_transform_press_frame = -1
macbeth_knight_engaged = false
local eladard_next_recovery_frame = -1
local eladard_base_entered = false
-- Deliberately global: this large oracle has reached Lua's main-chunk local
-- limit. This is a semantic observation latch, not source memory exposed to
-- the shipping port.
titania_base_entered = false
local installation_core_encounter_seen = false

local function work_byte(address)
  return emu.read(address, emu.memType.snesWorkRam, false)
end

local function work_word(address)
  return emu.read16(address, emu.memType.snesWorkRam, false)
end

-- Deliberately global: avoid adding another main-chunk local to this large
-- oracle. The active core must have left both instructions in its retail
-- trigger loop before a forced hit is representative of combat.
function meteor_core_accepts_damage(object, shape)
  if not force_meteor_core_trigger or shape ~= 0xEB6C then return true end
  local path = work_word(object + 0x2B)
  return path ~= METEOR_CORE_WAITING_PATH
    and path ~= METEOR_CORE_RECHECK_PATH
end

local function work_long(address)
  return work_word(address) | (work_byte(address + 2) << 16)
end

local function write_work_word(address, value)
  local encoded = value % 65536
  emu.write(address, encoded & 0xFF, emu.memType.snesWorkRam)
  emu.write(address + 1, (encoded >> 8) & 0xFF, emu.memType.snesWorkRam)
end

function player_damage_oracle.is_hostile_projectile(object)
  return work_word(object + 4) == 0xE3A8
    and work_byte(object + 0x2D) == 10
    and work_byte(object + 0x2E) == 2
    and work_byte(object + 0x2F) == 0
    and work_byte(object + 0x30) == 0
    and work_byte(object + 0x31) == 90
end

function player_damage_oracle.pin_to_player(projectile, player)
  write_work_word(
    projectile + 12,
    signed_word(player + 12) + player_damage_oracle.impact_offset_x)
  write_work_word(
    projectile + 14,
    signed_word(player + 14) + player_damage_oracle.impact_offset_y)
  write_work_word(
    projectile + 16,
    signed_word(player + 16) + player_damage_oracle.impact_offset_z)
end

function player_damage_oracle.observe_probe()
  if not player_damage_oracle.probe or not armed then return end
  local player = work_word(0x12C3)
  if player == 0 or player_damage_oracle.locked_hits == 0 then return end
  local health = work_byte(0x1DD1)
  if player_damage_oracle.initial_health == nil then
    player_damage_oracle.initial_health = health
    player_damage_oracle.minimum_health = health
  end
  if health < player_damage_oracle.minimum_health then
    player_damage_oracle.minimum_health = health
  end
  if player_damage_oracle.hit_elapsed == nil
    and health < player_damage_oracle.initial_health then
    player_damage_oracle.hit_elapsed = frame - armed_frame
  end
end

local function record_craft_form_service(service)
  if not trace_craft_transition or not armed then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  craft_transition_lines[#craft_transition_lines + 1] = string.format(
    "elapsed=%d event=form-service service=%s object=%04X shape=%04X " ..
      "selected=%d alternate=%d source=%02X:%04X",
    frame - armed_frame,
    service,
    object,
    work_word(object + 4),
    work_word(0x1E14) & 7,
    work_word(0x1E70) & 7,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function craft_form_service(service)
  return function() record_craft_form_service(service) end
end

local function record_craft_transition_write(address, value)
  if not trace_craft_transition or not armed then return value end
  local player = work_word(0x12C3)
  if player == 0 then return value end
  local local_address = address & 0xFFFF
  local field
  if local_address == ((player + 4) & 0xFFFF) then
    field = "shape-low"
  elseif local_address == ((player + 5) & 0xFFFF) then
    field = "shape-high"
  elseif local_address == ((player + 0x1CC7) & 0xFFFF) then
    field = "color-frame"
  elseif local_address == ((player + 0x1CC8) & 0xFFFF) then
    field = "animation-frame"
  elseif local_address == ((player + 0x1CCB) & 0xFFFF) then
    field = "transformation-frame"
  else
    return value
  end
  local state = emu.getState()
  craft_transition_lines[#craft_transition_lines + 1] = string.format(
    "elapsed=%d event=form-write object=%04X field=%s value=%d " ..
      "shape=%04X source=%02X:%04X",
    frame - armed_frame,
    player,
    field,
    value or 0,
    work_word(player + 4),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
  return value
end

local function signed_word(address)
  local value = work_word(address)
  if value >= 0x8000 then return value - 0x10000 end
  return value
end

local function signed_byte(address)
  local value = work_byte(address)
  if value >= 0x80 then return value - 0x100 end
  return value
end

function meteor_switch_oracle.record_stage(stage)
  return function()
    if not meteor_switch_oracle.enabled or not armed then return end
    local elapsed = frame - armed_frame
    if elapsed < meteor_switch_oracle.minimum_elapsed then return end
    local state = emu.getState()
    local object = state["cpu.x"] or 0
    meteor_switch_oracle.lines[#meteor_switch_oracle.lines + 1] = string.format(
      "elapsed=%d event=%s object=%04X shape=%04X path=%04X " ..
        "active=%04X source=%02X:%04X",
      elapsed,
      stage,
      object,
      object ~= 0 and work_word(object + 4) or 0,
      object ~= 0 and work_word(object + 0x2B) or 0,
      work_word(0x12A8),
      state["cpu.k"] or 0,
      state["cpu.pc"] or 0)
  end
end

function meteor_switch_oracle.record_shape_write(address, value)
  if not meteor_switch_oracle.enabled or not meteor_switch_oracle.shape_writes
    or not armed then return value end
  local elapsed = frame - armed_frame
  if elapsed < meteor_switch_oracle.minimum_elapsed then return value end
  local local_address = address & 0xFFFF
  local pool_offset = local_address - 0x03BD
  if pool_offset < 0 then return value end
  local field = pool_offset % 0x3F
  if field ~= 4 and field ~= 5 then return value end
  local object = local_address - field
  local before = work_word(object + 4)
  local after
  if field == 4 then
    after = (before & 0xFF00) | (value or 0)
  else
    after = (before & 0x00FF) | ((value or 0) << 8)
  end
  local state = emu.getState()
  meteor_switch_oracle.lines[#meteor_switch_oracle.lines + 1] = string.format(
    "elapsed=%d event=shape-write object=%04X before=%04X after=%04X " ..
      "path=%04X active=%04X source=%02X:%04X",
    elapsed,
    object,
    before,
    after,
    work_word(object + 0x2B),
    work_word(0x12A8),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
  return value
end

function sortie_actor_oracle.bytes_hex(address, count)
  local output = {}
  for offset = 0, count - 1 do
    output[#output + 1] = string.format("%02X", work_byte(address + offset))
  end
  return table.concat(output)
end

function player_damage_oracle.snapshot(event)
  if not player_damage_oracle.trace or not armed then return end
  local player = work_word(0x12C3)
  local object_health = player ~= 0 and work_byte(player + 0x2D) or 0
  local object_state = player ~= 0
    and sortie_actor_oracle.bytes_hex(player + 0x20, 7)
      .. sortie_actor_oracle.bytes_hex(player + 0x2D, 5)
      .. sortie_actor_oracle.bytes_hex(player + 0x38, 1) or "-"
  local snapshot = string.format(
    "%d,%d,%d,%04X,%04X,%04X,%d,%d,%d,%d,%d,%d,%d,%d,%s",
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_word(0x12A8),
    player,
    work_word(0x12C5),
    object_health,
    work_byte(0x1DD1),
    work_byte(0x1DD5),
    work_byte(0x1DD7),
    work_byte(0x1DDB),
    work_byte(0xB228),
    work_byte(0xB232),
    work_byte(0xB260),
    object_state)
  if snapshot == player_damage_oracle.last_snapshot then return end
  player_damage_oracle.last_snapshot = snapshot
  player_damage_oracle.lines[#player_damage_oracle.lines + 1] = string.format(
    "elapsed=%d event=%s mode=%d submode=%d phase=%d active=%04X " ..
      "player=%04X wingmate=%04X object_health=%d hud_health=%d " ..
      "hud_max=%d reserve_health=%d reserve_max=%d buffers=%d,%d,%d " ..
      "object_state=%s",
    frame - armed_frame,
    event,
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_word(0x12A8),
    player,
    work_word(0x12C5),
    object_health,
    work_byte(0x1DD1),
    work_byte(0x1DD5),
    work_byte(0x1DD7),
    work_byte(0x1DDB),
    work_byte(0xB228),
    work_byte(0xB232),
    work_byte(0xB260),
    object_state)
end

function player_damage_oracle.record_write(address, value)
  if not player_damage_oracle.trace or not armed then return value end
  local player = work_word(0x12C3)
  local fixed_field = address == 0x1B68
    or address == 0x1B76
    or address == 0x1BE0
    or address == 0x12A8
    or address == 0x12A9
    or address == 0x12C3
    or address == 0x12C4
    or address == 0x12C5
    or address == 0x12C6
    or address == 0x1DD1
    or address == 0x1DD5
    or address == 0x1DD7
    or address == 0x1DDB
    or address == 0xB228
    or address == 0xB232
    or address == 0xB260
  local object_field = player ~= 0
    and ((address >= player + 0x20 and address <= player + 0x26)
      or (address >= player + 0x2D and address <= player + 0x31)
      or address == player + 0x38)
  if not fixed_field and not object_field then return value end
  local state = emu.getState()
  player_damage_oracle.lines[#player_damage_oracle.lines + 1] = string.format(
    "elapsed=%d event=write address=%05X value=%d player=%04X " ..
      "source=%02X:%04X",
    frame - armed_frame,
    address,
    value or 0,
    player,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
  return value
end

local function record_walker_dynamics(stage)
  if not trace_walker_dynamics or not armed then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  local player = work_word(0x12C3)
  if object ~= player or object == 0 then return end
  local slot = work_word(object + 0x2B)
  local function player_field(address)
    return (address + slot) & 0xFFFF
  end
  if stage == "before" then
    walker_dynamics_before = {}
    for offset = 0, 0x1D7 do
      walker_dynamics_before[offset] = work_byte(player_field(0x6A61 + offset))
    end
    return
  end
  local changed = {}
  if walker_dynamics_before then
    for offset = 0, 0x1D7 do
      local before = walker_dynamics_before[offset]
      local after = work_byte(player_field(0x6A61 + offset))
      if before ~= after then
        changed[#changed + 1] = string.format("%03X:%02X>%02X", offset, before, after)
      end
    end
  end
  walker_dynamics_lines[#walker_dynamics_lines + 1] = string.format(
    "elapsed=%d stage=%s input=%s pad=%04X trigger=%04X object=%04X slot=%04X " ..
      "pose=%d,%d,%d,%d,%d,%d,%d ground=%d floor=%d " ..
      "motion=%d,%d,%d,%d,%d,%d,%d,%d jump=%d,%d,%d " ..
      "vertical=%d,%d speed=%d flags=%02X,%02X,%02X changed=[%s]",
    frame - armed_frame,
    stage,
    input_label,
    work_word(0x1936),
    work_word(0x1938),
    object,
    slot,
    signed_word(object + 12),
    signed_word(object + 14),
    signed_word(object + 16),
    work_byte(object + 18),
    work_byte(object + 20),
    work_byte(object + 22),
    work_byte(object + 24),
    signed_word(player_field(0x6A6E)),
    signed_word(player_field(0x6A7D)),
    work_byte(player_field(0x6A82)),
    work_byte(player_field(0x6A83)),
    work_byte(player_field(0x6A84)),
    signed_byte(player_field(0x6ADD)),
    signed_word(player_field(0x6B8D)),
    work_byte(player_field(0x6B8F)),
    signed_word(player_field(0x6B90)),
    work_byte(player_field(0x6B97)),
    work_byte(player_field(0x6B8B)),
    signed_byte(player_field(0x6B98)),
    work_byte(player_field(0x6B99)),
    signed_word(object + 0x1CC3),
    signed_word(object + 0x34),
    work_byte(object + 0x18),
    work_byte(player_field(0x6B8C)),
    work_byte(player_field(0x6B92)),
    work_byte(player_field(0x6B94)),
    table.concat(changed, ","))
end

local function walker_dynamics_stage(stage)
  return function() record_walker_dynamics(stage) end
end

local function record_walker_motion_stage(stage)
  if not trace_walker_dynamics or not armed then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  local player = work_word(0x12C3)
  if object ~= player or object == 0 then return end
  local slot = work_word(object + 0x2B)
  local function player_field(address)
    return (address + slot) & 0xFFFF
  end
  walker_dynamics_lines[#walker_dynamics_lines + 1] = string.format(
    "elapsed=%d event=motion-stage stage=%s input=%s object=%04X " ..
      "position=%d,%d,%d velocity=%d,%d,%d transformed=%d,%d,%d " ..
      "impulse=%d,%d,%d terrain=%d,%d,%d,%d mode=%02X gravity=%d",
    frame - armed_frame,
    stage,
    input_label,
    object,
    signed_word(object + 0x0C),
    signed_word(object + 0x0E),
    signed_word(object + 0x10),
    signed_word(object + 0x32),
    signed_word(object + 0x34),
    signed_word(object + 0x36),
    signed_word(object + 0x39),
    signed_word(object + 0x3B),
    signed_word(object + 0x3D),
    signed_word(object + 0x1CC1),
    signed_word(object + 0x1CC3),
    signed_word(object + 0x1CC5),
    signed_word(player_field(0x6A73)),
    signed_word(player_field(0x6AE2)),
    signed_byte(player_field(0x6A7A)),
    signed_byte(player_field(0x6A7B)),
    work_byte(player_field(0x6B94)),
    signed_word(0x1DAB))
end

local function walker_motion_stage(stage)
  return function() record_walker_motion_stage(stage) end
end

local walker_auxiliary_offsets = {
  [0x022] = true, [0x023] = true,
  [0x041] = true, [0x042] = true, [0x043] = true,
  [0x044] = true, [0x045] = true, [0x046] = true,
  [0x047] = true, [0x048] = true, [0x049] = true,
  [0x04A] = true, [0x04B] = true, [0x05B] = true,
  [0x06C] = true, [0x06F] = true, [0x070] = true,
  [0x074] = true, [0x07A] = true,
  [0x081] = true, [0x082] = true,
  [0x0AC] = true, [0x0AE] = true, [0x0AF] = true,
  [0x101] = true,
  [0x12A] = true, [0x12C] = true, [0x12D] = true,
  [0x12F] = true, [0x130] = true, [0x131] = true,
  [0x133] = true, [0x135] = true, [0x138] = true,
  [0x0DA] = true,
}

local function record_walker_write(address, value)
  if not trace_walker_writes or not armed then return value end
  local player = work_word(0x12C3)
  if player == 0 then return value end
  local local_address = address & 0xFFFF
  local slot = work_word(player + 0x2B)
  local auxiliary_start = (0x6A61 + slot) & 0xFFFF
  local auxiliary_offset = (local_address - auxiliary_start) & 0xFFFF
  local target = nil
  if walker_auxiliary_offsets[auxiliary_offset] then
    target = string.format("state+%03X", auxiliary_offset)
  else
    for _, offset in ipairs({
      0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11,
      0x12, 0x14, 0x16, 0x18,
      0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
      0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E,
      0x1CC3, 0x1CC4,
    }) do
      if local_address == ((player + offset) & 0xFFFF) then
        target = string.format("object+%04X", offset)
        break
      end
    end
  end
  if not target then return value end
  local state = emu.getState()
  walker_dynamics_lines[#walker_dynamics_lines + 1] = string.format(
    "elapsed=%d event=write input=%s target=%s value=%02X source=%02X:%04X",
    frame - armed_frame,
    input_label,
    target,
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
  return value
end

local function trace_stage_write(address, value)
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "elapsed=%d event=stage-write address=%04X value=%d host=%02X:%04X",
    frame - armed_frame,
    address,
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

function trace_audio_program_entry()
  if not trace_audio_programs or not armed then return end
  local state = emu.getState()
  audio_program_lines[#audio_program_lines + 1] = string.format(
    "elapsed=%d mode=%d submode=%d mission=%d record=%03X conditional=%d " ..
      "host=%02X:%04X",
    frame - armed_frame,
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BB5),
    work_word(0x1B6E),
    work_byte(0x1BBB),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_map_write(address, value)
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "elapsed=%d event=map-write address=%04X value=%d host=%02X:%04X",
    frame - armed_frame,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_final_gate_access(kind, address, value)
  local state = emu.getState()
  local host_bank = state["cpu.k"] or 0
  local host_pc = state["cpu.pc"] or 0
  local key = string.format(
    "%s:%06X:%02X:%04X",
    kind,
    address,
    host_bank,
    host_pc)
  if final_gate_accesses[key] then return end
  final_gate_accesses[key] = true
  lines[#lines + 1] = string.format(
    "elapsed=%d event=final-gate-%s address=%06X value=%d host=%02X:%04X",
    frame - armed_frame,
    kind,
    address,
    value or 0,
    host_bank,
    host_pc)
end

local function trace_final_gate_read(address, value)
  trace_final_gate_access("read", address, value)
end

local function trace_final_gate_write(address, value)
  trace_final_gate_access("write", address, value)
end

local function trace_objective_completion_execute(address, value)
  trace_final_gate_access("execute", address, value)
end

local function trace_final_activation_write(address, value)
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "elapsed=%d event=final-activation-write address=%04X value=%d " ..
      "host=%02X:%04X",
    frame - armed_frame,
    address,
    value,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_final_activation_execute(address, value)
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "elapsed=%d event=final-activation-execute address=%06X " ..
      "host=%02X:%04X",
    frame - armed_frame,
    address,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_threat_retirement_execute(address, value)
  local state = emu.getState()
  local actor = state["cpu.y"] or 0
  lines[#lines + 1] = string.format(
    "elapsed=%d event=threat-retirement-step address=%06X actor=%04X " ..
      "selection=%d flags=%04X position=%d,%d remaining=%d " ..
      "host=%02X:%04X",
    frame - armed_frame,
    address,
    actor,
    work_byte(actor + 0x32),
    work_word(actor + 0x2E),
    work_byte(actor + 0x1C),
    work_byte(actor + 0x1F),
    work_word(0xDA43),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_map_actor_write(address, value)
  local state = emu.getState()
  local normalized_address = address & 0x1FFFF
  lines[#lines + 1] = string.format(
    "elapsed=%d event=map-actor-write actor=%04X offset=%02X value=%d " ..
      "mode=%d remaining=%d host=%02X:%04X",
    frame - armed_frame,
    traced_map_actor,
    normalized_address - traced_map_actor,
    value,
    work_byte(0x1B68),
    work_word(0xDA43),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_map_control_write(address, value)
  local state = emu.getState()
  local normalized_address = address & 0x1FFFF
  if normalized_address > 0xFFFF then return end
  local current_object = work_word(0x1651)
  lines[#lines + 1] = string.format(
    "elapsed=%d event=map-control-write address=%04X value=%d mode=%d " ..
      "remaining=%d map=%02X:%04X objectives=%d,%d basebits=%04X " ..
      "required=%d scheduled=%04X shape=%04X path=%04X context=%04X,%04X " ..
      "host=%02X:%04X",
    frame - armed_frame,
    normalized_address,
    value or 0,
    work_byte(0x1B68),
    work_word(0xDA43),
    work_byte(0x192E),
    work_word(0x1657),
    work_byte(0xD7A1),
    work_byte(0xD7F4),
    work_word(0xD7F6),
    traced_map_actor and work_byte(traced_map_actor + 0x27) or 0,
    current_object,
    current_object ~= 0 and work_word(current_object + 4) or 0,
    current_object ~= 0 and work_word(current_object + 0x2B) or 0,
    state["cpu.x"] or 0,
    state["cpu.y"] or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local map_control_reads = {}
local function trace_map_control_read(address, value)
  local state = emu.getState()
  local normalized_address = address & 0x1FFFF
  if normalized_address > 0xFFFF then return end
  local host = ((state["cpu.k"] or 0) << 16) | (state["cpu.pc"] or 0)
  local key = string.format("%04X:%06X", normalized_address, host)
  if map_control_reads[key] then return end
  map_control_reads[key] = true
  local current_object = work_word(0x1651)
  lines[#lines + 1] = string.format(
    "elapsed=%d event=map-control-read address=%04X value=%d mode=%d " ..
      "remaining=%d map=%02X:%04X objectives=%d,%d basebits=%04X " ..
      "required=%d scheduled=%04X shape=%04X path=%04X context=%04X,%04X " ..
      "host=%02X:%04X",
    frame - armed_frame,
    normalized_address,
    value,
    work_byte(0x1B68),
    work_word(0xDA43),
    work_byte(0x192E),
    work_word(0x1657),
    work_byte(0xD7A1),
    work_byte(0xD7F4),
    work_word(0xD7F6),
    traced_map_actor and work_byte(traced_map_actor + 0x27) or 0,
    current_object,
    current_object ~= 0 and work_word(current_object + 4) or 0,
    current_object ~= 0 and work_word(current_object + 0x2B) or 0,
    state["cpu.x"] or 0,
    state["cpu.y"] or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local astropolis_gate_accesses = {}
local astropolis_gate_global_values = {}
local function trace_astropolis_gate_global_write(address, value)
  if not armed or work_byte(0x1BB5) ~= 11 then return end
  local state = emu.getState()
  local normalized_address = address & 0x1FFFF
  if astropolis_gate_global_values[normalized_address] == value then return end
  astropolis_gate_global_values[normalized_address] = value
  lines[#lines + 1] = string.format(
    "elapsed=%d event=astropolis-gate-global-write address=%04X value=%d " ..
      "trigger=%04X delay=%d kind=%d host=%02X:%04X",
    frame - armed_frame,
    normalized_address,
    value or 0,
    work_word(0xD777),
    work_byte(0xD779),
    work_byte(0xD77A),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_astropolis_mask_aux_write(address, value)
  if not armed or work_byte(0x1BB5) ~= 11 then return end
  local normalized_address = address & 0x1FFFF
  local first_object = 0x03BD
  local object_size = 0x3F
  local first_counter = first_object + 0x1CDA
  if normalized_address < first_counter then return end
  local object = first_object
    + math.floor((normalized_address - first_counter) / object_size)
      * object_size
  local variable = normalized_address - object - 0x1C41
  if variable ~= 0x99 and variable ~= 0x9A then return end
  if work_word(object + 4) ~= 0xDF48 then return end
  local side = variable == 0x99 and "left" or "right"
  local state = emu.getState()
  lines[#lines + 1] = string.format(
    "elapsed=%d event=astropolis-mask-eye object=%04X side=%s " ..
      "durability=%d path=%04X host=%02X:%04X",
    frame - armed_frame,
    object,
    side,
    value or 0,
    work_word(object + 0x2B),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_astropolis_gate_object_write(address, value)
  if not armed or work_byte(0x1BB5) ~= 11 then return end
  local normalized_address = address & 0x1FFFF
  local first_object = 0x03BD
  local object_size = 0x3F
  if normalized_address < first_object then return end
  local object = first_object
    + math.floor((normalized_address - first_object) / object_size) * object_size
  local offset = normalized_address - object
  if offset < 0x20 or offset > 0x31 then return end
  local shape = work_word(object + 4)
  if shape ~= 0xF6CC and shape ~= 0xD20C and shape ~= 0xD228
    and shape ~= 0xD340 and shape ~= 0xF65C and shape ~= 0xD1D4
    and shape ~= 0xD634 and shape ~= 0xD66C and shape ~= 0xCD20
    and shape ~= 0xEF5C and shape ~= 0xD260 and shape ~= 0xF3F4
    and shape ~= 0xD308 and shape ~= 0xD324 and shape ~= 0xD19C
    and shape ~= 0xD1F0 and shape ~= 0xDF48 and shape ~= 0xC1DC
    and shape ~= 0xC1F8 then
    return
  end
  local state = emu.getState()
  local host = ((state["cpu.k"] or 0) << 16) | (state["cpu.pc"] or 0)
  local key = string.format(
    "%04X:%02X:%02X:%06X", object, offset, value or 0, host)
  if astropolis_gate_accesses[key] then return end
  astropolis_gate_accesses[key] = true
  lines[#lines + 1] = string.format(
    "elapsed=%d event=astropolis-gate-object-write object=%04X " ..
      "shape=%04X offset=%02X value=%d path=%04X flag26=%02X " ..
      "trigger=%04X host=%02X:%04X",
    frame - armed_frame,
    object,
    shape,
    offset,
    value or 0,
    work_word(object + 0x2B),
    work_byte(object + 0x26),
    work_word(0xD777),
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

local function trace_astropolis_gate_execute(address, value)
  if not armed or work_byte(0x1BB5) ~= 11 then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  lines[#lines + 1] = string.format(
    "elapsed=%d event=astropolis-gate-execute address=%06X object=%04X " ..
      "shape=%04X path=%04X flag26=%02X trigger=%04X delay=%d kind=%d",
    frame - armed_frame,
    address,
    object,
    work_word(object + 4),
    work_word(object + 0x2B),
    work_byte(object + 0x26),
    work_word(0xD777),
    work_byte(0xD779),
    work_byte(0xD77A))
end

local function write_file(name, contents)
  local file = assert(io.open(emu.getScriptDataFolder() .. "/" .. name, "w+b"))
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
  write_file(string.format("sf2_post_sortie_%05d.ppm", elapsed), table.concat(output))
end

local function capture_work(elapsed)
  local output = {}
  for address = 0, 0x3FFF do
    output[#output + 1] = string.char(work_byte(address))
  end
  write_file(string.format("sf2_post_sortie_%05d.wram", elapsed), table.concat(output))
  if capture_full_work then
    local full_output = {}
    for address = 0, 0x1FFFF do
      full_output[#full_output + 1] = string.char(work_byte(address))
    end
    write_file(
      string.format("sf2_post_sortie_%05d_full.wram", elapsed),
      table.concat(full_output))
  end
end

local function capture_memory(elapsed, suffix, kind, length)
  local output = {}
  for address = 0, length - 1 do
    output[#output + 1] = string.char(emu.read(address, kind, false))
  end
  write_file(
    string.format("sf2_post_sortie_%05d_%s.bin", elapsed, suffix),
    table.concat(output))
end

local function capture_ppu_state(elapsed)
  capture_memory(elapsed, "vram", emu.memType.snesVideoRam, 0x10000)
  capture_memory(elapsed, "cgram", emu.memType.snesCgRam, 0x200)
  capture_memory(elapsed, "oam", emu.memType.snesSpriteRam, 544)
  local state = emu.getState()
  local keys = {}
  for key, _ in pairs(state) do
    local lower = string.lower(key)
    if string.find(lower, "ppu", 1, true)
      or string.find(lower, "sprite", 1, true)
      or string.find(lower, "brightness", 1, true) then
      keys[#keys + 1] = key
    end
  end
  table.sort(keys)
  local output = {}
  for _, key in ipairs(keys) do
    output[#output + 1] = key .. "=" .. tostring(state[key]) .. "\n"
  end
  write_file(
    string.format("sf2_post_sortie_%05d_ppu_state.txt", elapsed),
    table.concat(output))
end

local function pose(address)
  if address == 0 then return "-" end
  return string.format(
    "%d,%d,%d,%d,%d,%d,%d",
    signed_word(address + 12),
    signed_word(address + 14),
    signed_word(address + 16),
    work_byte(address + 18),
    work_byte(address + 20),
    work_byte(address + 22),
    work_byte(address + 24))
end

local function active_objects()
  local output = {}
  local seen = {}
  local object = work_word(0x12A8)
  while object ~= 0 and not seen[object] and #output < 60 do
    seen[object] = true
    output[#output + 1] = string.format(
      "%04X,%04X,%s,%06X,%04X,%04X,%d,%d,%d,%d,%d,%d,%d,%d",
      object,
      work_word(object + 4),
      pose(object),
      work_long(object + 25),
      work_word(object + 0x2B),
      work_word(object + 0x1CCD),
      signed_word(object + 0x32),
      signed_word(object + 0x34),
      signed_word(object + 0x36),
      work_byte(object + 0x2D),
      work_byte(object + 0x2E),
      work_byte(object + 0x2F),
      work_byte(object + 0x30),
      work_byte(object + 0x31))
    object = work_word(object)
  end
  return table.concat(output, ";")
end

function mirage_segment_states()
  if not TRACE_MIRAGE_SEGMENT_STATE then return "-" end
  local output = {}
  local seen = {}
  local object = work_word(0x12A8)
  while object ~= 0 and not seen[object] and #output < 9 do
    seen[object] = true
    local shape = work_word(object + 4)
    if shape == 0xE1E8 or shape == 0xE220 then
      output[#output + 1] = string.format(
        "%04X,%04X,%04X,%04X,%d,%d,%d,%d",
        object,
        shape,
        work_word(object + 6),
        work_word(object + 0x1C),
        work_byte(object + 0x1CE2),
        signed_word(object + 0x1CD3),
        work_byte(object + 0x1CD5),
        work_byte(object + 0x1CC8))
    end
    object = work_word(object)
  end
  return table.concat(output, ";")
end

-- Operation-level evidence for the four reusable combat slots.  This is
-- intentionally source-machine-facing oracle instrumentation; the importer
-- reduces it to typed movement, steering, wave, firing, and scheduling events
-- before anything reaches the Rust port.
function sortie_actor_oracle.record(event)
  if not sortie_actor_oracle.enabled
    and not sortie_actor_oracle.projectiles_enabled then return end
  if not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14900 or work_byte(0x1B68) ~= 1 then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  local shape = work_word(object + 4)
  local actor = sortie_actor_oracle.enabled and sortie_actor_oracle.objects[object]
  local projectile = sortie_actor_oracle.projectiles_enabled and shape == 0xE3A8
  if not actor and not projectile then return end
  local trigger_list = work_word(object + 0x1CE0)
  local selected = work_word(0xCF1F)
  -- A departed fighter slot can be reused by a hostile projectile. Classify
  -- by the current projectile shape before the slot's former actor identity
  -- so both semantic streams remain complete.
  local output = projectile
    and sortie_actor_oracle.projectile_lines or sortie_actor_oracle.lines
  output[#output + 1] = string.format(
    "elapsed=%d strategy_frame=%d event=%s object=%04X shape=%04X path=%04X pose=%s " ..
      "velocity=%d,%d,%d rng=%s relative_motion=%d,%d base=%s " ..
      "extension=%s selected=%04X selected_pose=%s triggers=%s",
    elapsed,
    work_byte(0x00C4),
    event,
    object,
    shape,
    work_word(object + 0x2B),
    pose(object),
    signed_word(object + 0x32),
    signed_word(object + 0x34),
    signed_word(object + 0x36),
    sortie_actor_oracle.bytes_hex(0x00E0, 4),
    signed_word(0x1E1C),
    signed_word(0x1E20),
    sortie_actor_oracle.bytes_hex(object, 0x39),
    sortie_actor_oracle.bytes_hex(object + 0x1CC1, 0x3F),
    selected,
    pose(selected),
    trigger_list ~= 0
      and sortie_actor_oracle.bytes_hex(0x6A61 + trigger_list, 0x40) or "-")
end

function sortie_actor_oracle.callback(event)
  return function() sortie_actor_oracle.record(event) end
end

function sortie_actor_oracle.capital_for_state_address(address)
  if (address >= 0x0600 and address <= 0x0605)
    or (address >= 0x0626 and address <= 0x062B) then
    return 0x05F4
  end
  if (address >= 0x063F and address <= 0x0644)
    or (address >= 0x0665 and address <= 0x066A) then
    return 0x0633
  end
  return nil
end

function sortie_actor_oracle.record_capital_state_write(source, address, value)
  if not sortie_actor_oracle.enabled or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14900 or work_byte(0x1B68) ~= 1 then return end
  local object = sortie_actor_oracle.capital_for_state_address(address)
  if not object then return end
  local state = emu.getState()
  sortie_actor_oracle.lines[#sortie_actor_oracle.lines + 1] = string.format(
    "elapsed=%d event=capital-state-write object=%04X shape=%04X " ..
      "source=%s address=%04X value=%d host=%02X:%04X " ..
      "coprocessor=%02X:%04X pose=%s velocity=%d,%d,%d",
    elapsed,
    object,
    work_word(object + 4),
    source,
    address,
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    pose(object),
    signed_word(object + 0x32),
    signed_word(object + 0x34),
    signed_word(object + 0x36))
end

function sortie_actor_oracle.record_main_capital_state_write(address, value)
  sortie_actor_oracle.record_capital_state_write("main-work", address, value)
end

function sortie_actor_oracle.record_gsu_capital_state_write(address, value)
  sortie_actor_oracle.record_capital_state_write(
    "coprocessor-work", address, value)
end

function sortie_actor_oracle.record_pitch_target_write(address, value)
  if not sortie_actor_oracle.enabled or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14900 or work_byte(0x1B68) ~= 1 then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  if not sortie_actor_oracle.objects[object] then return end
  sortie_actor_oracle.lines[#sortie_actor_oracle.lines + 1] = string.format(
    "elapsed=%d event=pitch-target-write object=%04X shape=%04X " ..
      "value=%d source=%02X:%04X",
    elapsed,
    object,
    work_word(object + 4),
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0)
end

function sortie_actor_oracle.record_position_y_write(address, value)
  if not sortie_actor_oracle.enabled or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14900 or work_byte(0x1B68) ~= 1 then return end
  local state = emu.getState()
  local object = state["cpu.x"] or 0
  if not sortie_actor_oracle.objects[object] then return end
  sortie_actor_oracle.lines[#sortie_actor_oracle.lines + 1] = string.format(
    "elapsed=%d event=position-y-write object=%04X shape=%04X " ..
      "address=%04X value=%d source=%02X:%04X pose=%s",
    elapsed,
    object,
    work_word(object + 4),
    address,
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    pose(object))
end

function sortie_actor_oracle.actor_for_position_address(address)
  for object, _ in pairs(sortie_actor_oracle.objects) do
    if address >= object + 12 and address <= object + 17 then
      return object
    end
  end
  return nil
end

function sortie_actor_oracle.record_coprocessor_position_write(address, value)
  if not sortie_actor_oracle.enabled or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14900 or work_byte(0x1B68) ~= 1 then return end
  local object = sortie_actor_oracle.actor_for_position_address(address)
  if not object then return end
  local state = emu.getState()
  sortie_actor_oracle.lines[#sortie_actor_oracle.lines + 1] = string.format(
    "elapsed=%d event=coprocessor-position-write object=%04X shape=%04X " ..
      "address=%04X value=%d coprocessor=%02X:%04X pose=%s",
    elapsed,
    object,
    work_word(object + 4),
    address,
    value or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    pose(object))
end

function sortie_actor_oracle.record_main_position_write(address, value)
  if not sortie_actor_oracle.enabled or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14900 or work_byte(0x1B68) ~= 1 then return end
  local object = sortie_actor_oracle.actor_for_position_address(address)
  if not object then return end
  local state = emu.getState()
  sortie_actor_oracle.lines[#sortie_actor_oracle.lines + 1] = string.format(
    "elapsed=%d event=main-position-write object=%04X shape=%04X " ..
      "address=%04X value=%d source=%02X:%04X pose=%s",
    elapsed,
    object,
    work_word(object + 4),
    address,
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    pose(object))
end

function sortie_actor_oracle.record_random_state_write(source, address, value)
  if not sortie_actor_oracle.enabled or not armed then return end
  local elapsed = frame - armed_frame
  if elapsed < 14900 or work_byte(0x1B68) ~= 1 then return end
  local state = emu.getState()
  sortie_actor_oracle.lines[#sortie_actor_oracle.lines + 1] = string.format(
    "elapsed=%d event=random-state-write source=%s address=%04X value=%d " ..
      "host=%02X:%04X coprocessor=%02X:%04X rng=%s stack_pointer=%04X stack=%s",
    elapsed,
    source,
    address,
    value or 0,
    state["cpu.k"] or 0,
    state["cpu.pc"] or 0,
    state["cart.coprocessor.programBank"] or 0,
    state["cart.coprocessor.r15"] or 0,
    sortie_actor_oracle.bytes_hex(0x00E0, 4),
    state["cpu.s"] or state["cpu.sp"] or 0x01FF,
    sortie_actor_oracle.cpu_stack_hex(state, 12))
end

function sortie_actor_oracle.cpu_stack_hex(state, count)
  local stack = {}
  local stack_pointer = state["cpu.s"] or state["cpu.sp"] or 0x01FF
  for offset = 1, count do
    stack[#stack + 1] = string.format(
      "%02X",
      emu.read(
        (stack_pointer + offset) & 0xFFFF,
        emu.memType.snesMemory,
        false))
  end
  return table.concat(stack)
end

function sortie_actor_oracle.record_main_random_state_write(address, value)
  sortie_actor_oracle.record_random_state_write("main-work", address, value)
end

function sortie_actor_oracle.record_gsu_random_state_write(address, value)
  sortie_actor_oracle.record_random_state_write(
    "coprocessor-work", address, value)
end

function sortie_actor_oracle.record_explicit_object_state_write(source)
  return function(address, value)
    if not sortie_actor_oracle.enabled or not armed then return end
    local object = nil
    for candidate, _ in pairs(explicit_trace_objects) do
      if (address >= candidate + 4 and address <= candidate + 5)
        or (address >= candidate + 0x12 and address <= candidate + 0x16)
        or (address >= candidate + 0x20 and address <= candidate + 0x31) then
        object = candidate
        break
      end
    end
    if not object then return end
    local state = emu.getState()
    sortie_actor_oracle.lines[#sortie_actor_oracle.lines + 1] = string.format(
      "elapsed=%d event=object-state-write object=%04X shape=%04X " ..
        "offset=%02X value=%d source=%s host=%02X:%04X coprocessor=%02X:%04X",
      frame - armed_frame,
      object,
      work_word(object + 4),
      address - object,
      value or 0,
      source,
      state["cpu.k"] or 0,
      state["cpu.pc"] or 0,
      state["cart.coprocessor.programBank"] or 0,
      state["cart.coprocessor.r15"] or 0)
  end
end

local function state_key()
  return string.format(
    "%02X:%02X:%02X:%02X:%02X:%02X:%02X:%04X:%04X:%04X:%02X:%04X:%04X:%04X",
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_byte(0x1C20),
    work_byte(0x1BB5),
    work_byte(0x1BA5),
    work_byte(0xD7F2),
    work_word(0x12A8),
    work_word(0x12C3),
    work_word(0x12C5),
    work_byte(0x192E),
    work_word(0x1657),
    work_word(0xDB47),
    work_word(0xDB49))
end

local function record(event, elapsed)
  local player = work_word(0x12C3)
  local wingmate = work_word(0x12C5)
  local player_slot = player ~= 0 and work_word(player + 0x2B) or 0
  local navigation_target = player_slot ~= 0
    and work_word((0x6BB8 + player_slot) % 0x10000) or 0
  lines[#lines + 1] = string.format(
    "elapsed=%d event=%s input=%s mode=%d submode=%d phase=%d " ..
      "cursor=%d selection=%d mapmode=%d difficulty=%d " ..
      "active=%04X player=%04X " ..
      "wingmate=%04X camera=%d,%d,%d,%d,%d,%d playerpose=%s " ..
      "wingpose=%s objects=[%s] mirage=[%s] encounterflags=%04X selected=%04X " ..
      "navtarget=%04X navpose=%d,%d,%d navdistance=%d navdisplay=%d,%d " ..
      "targetdigits=%d,%d,%d " ..
      "coretrigger=%d map=%02X:%04X mapcursor=%d,%d mapposition=%d,%d " ..
      "objectives=%d,%d basehandshake=%02X " ..
      "corneria_remaining=%d " ..
      "corneria_damage=%d",
    elapsed,
    event,
    input_label,
    work_byte(0x1B68),
    work_byte(0x1B76),
    work_byte(0x1BE0),
    work_byte(0x1C20),
    work_byte(0x1BB5),
    work_byte(0x1BA5),
    work_byte(0xD7F2),
    work_word(0x12A8),
    player,
    wingmate,
    signed_word(0x034B),
    signed_word(0x034D),
    signed_word(0x034F),
    work_byte(0x0351),
    work_byte(0x0353),
    work_byte(0x0355),
    pose(player),
    pose(wingmate),
    active_objects(),
    mirage_segment_states(),
    work_word(0xD77D),
    work_word(0x12C1),
    navigation_target,
    player_slot ~= 0
      and signed_word((0x6BCC + player_slot) % 0x10000) or 0,
    player_slot ~= 0
      and signed_word((0x6BCE + player_slot) % 0x10000) or 0,
    player_slot ~= 0
      and signed_word((0x6BD0 + player_slot) % 0x10000) or 0,
    player_slot ~= 0
      and work_word((0x6BBC + player_slot) % 0x10000) or 0,
    player_slot ~= 0
      and work_byte((0x6BAD + player_slot) % 0x10000) or 0,
    player_slot ~= 0
      and work_byte((0x6BAF + player_slot) % 0x10000) or 0,
    work_byte(0xE961),
    work_byte(0xE965),
    work_byte(0xE871),
    work_byte(0xD787),
    work_byte(0x192E),
    work_word(0x1657),
    signed_word(0xDA90),
    signed_word(0xDA92),
    work_byte(0xDAF3),
    work_byte(0xDAF6),
    work_byte(0xD7A1),
    work_byte(0xD7F4),
    work_byte(0xD78B),
    work_word(0xDB47),
    work_word(0xDB49))
end

local function pulse(value, period, offset)
  local phase = value % period
  return phase == offset or phase == offset + 1
end

local function wrapped_map_delta(first, second)
  local difference = math.abs(first - second)
  return math.min(difference, 256 - difference)
end

local function closest_collision_actor(selection)
  local player_x = work_byte(0xDAF3)
  local player_y = work_byte(0xDAF6)
  local closest_actor = nil
  local closest_distance_squared = math.huge
  local actor = work_word(0xE0A3)
  local seen = {}
  while actor ~= 0 and not seen[actor] do
    seen[actor] = true
    if work_byte(actor + 0x32) == selection then
      local delta_x = wrapped_map_delta(
        work_byte(actor + 0x1C), player_x)
      local delta_y = wrapped_map_delta(
        work_byte(actor + 0x1F), player_y)
      local distance_squared = delta_x * delta_x + delta_y * delta_y
      if distance_squared < closest_distance_squared then
        closest_actor = actor
        closest_distance_squared = distance_squared
      end
    end
    actor = work_word(actor)
  end
  return closest_actor
end

local function overlap_player_team_with_map_actor(target_actor)
  if not target_actor then return false end
  -- The team's strategic position is stored independently from the enemy
  -- collision list. Earlier versions of this helper incorrectly moved the
  -- nearest type-seven enemy as well, corrupting later dispatch evidence.
  emu.write(
    0xDAF3,
    work_byte(target_actor + 0x1C),
    emu.memType.snesWorkRam)
  emu.write(
    0xDAF6,
    work_byte(target_actor + 0x1F),
    emu.memType.snesWorkRam)
  return true
end

local function safest_pressure_evasion_position()
  local attackers = {}
  local actor = work_word(0xE0A3)
  local seen = {}
  while actor ~= 0 and not seen[actor] do
    seen[actor] = true
    local mission_selection = work_byte(actor + 0x32)
    local flags = work_word(actor + 0x2E)
    if (mission_selection == 6 or mission_selection == 7)
      and (flags & 0x3000) == 0 then
      attackers[#attackers + 1] = {
        x = work_byte(actor + 0x1C),
        y = work_byte(actor + 0x1F),
      }
    end
    actor = work_word(actor)
  end
  if #attackers == 0 then return (frame - armed_frame) % 256, 64 end

  -- Search the toroidal command map for the coarse point whose nearest live
  -- attacker is farthest away. Re-evaluating every frame prevents the oracle
  -- route itself from sweeping through a moving interceptor.
  local safest_x = 0
  local safest_y = 0
  local safest_distance_squared = -1
  for candidate_x = 0, 255, 16 do
    for candidate_y = 0, 255, 16 do
      local nearest_distance_squared = math.huge
      for _, attacker in ipairs(attackers) do
        local delta_x = wrapped_map_delta(candidate_x, attacker.x)
        local delta_y = wrapped_map_delta(candidate_y, attacker.y)
        local distance_squared = delta_x * delta_x + delta_y * delta_y
        nearest_distance_squared = math.min(
          nearest_distance_squared,
          distance_squared)
      end
      if nearest_distance_squared > safest_distance_squared then
        safest_x = candidate_x
        safest_y = candidate_y
        safest_distance_squared = nearest_distance_squared
      end
    end
  end
  return safest_x, safest_y
end

local function evade_pressure_on_map_entry(_, value)
  if not evade_pressure or not armed or value ~= 7 then return end
  -- Combat can hand control back to the command map midway through a frame.
  -- Seed the safe team position at that write boundary so the first retail
  -- collision pass cannot observe the sortie's old, nearby map coordinates.
  local safe_x, safe_y = safest_pressure_evasion_position()
  emu.write(0xDAF3, safe_x, emu.memType.snesWorkRam)
  emu.write(0xDAF6, safe_y, emu.memType.snesWorkRam)
end

local function evade_forced_pressure_snap()
  if not evade_pressure or not armed or work_byte(0x1B68) ~= 7 then return end
  -- A forced interceptor copies its own map coordinates over the team's
  -- global position after startFrame has already supplied the evasion route.
  -- Restore only that global route at the end of the retail copy routine;
  -- the attacker, its flags, and its lifetime bookkeeping remain untouched.
  local safe_x, safe_y = safest_pressure_evasion_position()
  emu.write(0xDAF3, safe_x, emu.memType.snesWorkRam)
  emu.write(0xDAF6, safe_y, emu.memType.snesWorkRam)
end

local function angle_difference(target, current)
  local difference = (target - current + 128) % 256 - 128
  return difference
end

-- Global because this oracle is at Lua's main-chunk local limit. The three
-- retail craft classes each have their own stable Arwing and Walker meshes.
function is_player_walker_shape(shape)
  return shape == 0xC914 or shape == 0xC930 or shape == 0xC94C
end

function is_player_flight_shape(shape)
  return shape == 0xC24C or shape == 0xC268 or shape == 0xC5E8
end

local function eladard_route_direction()
  -- The planetary base maze exposes its next turn as a white edge marker.
  -- Reading that presentation cue keeps this oracle pilot independent of the
  -- game's internal route bookkeeping.
  local screen = emu.getScreenBuffer()
  local size = emu.getScreenSize()
  local left_count = 0
  local right_count = 0
  for y = 100, 125 do
    for x = 24, 45 do
      local pixel = screen[y * size.width + x + 1] or 0
      if ((pixel >> 16) & 0xFF) >= 240
        and ((pixel >> 8) & 0xFF) >= 240
        and (pixel & 0xFF) >= 240 then
        left_count = left_count + 1
      end
    end
    for x = 214, 235 do
      local pixel = screen[y * size.width + x + 1] or 0
      if ((pixel >> 16) & 0xFF) >= 240
        and ((pixel >> 8) & 0xFF) >= 240
        and (pixel & 0xFF) >= 240 then
        right_count = right_count + 1
      end
    end
  end
  if left_count >= 6 and left_count > right_count then return "left" end
  if right_count >= 6 and right_count > left_count then return "right" end
  return nil
end

local function is_player_projectile(object, shape)
  -- Corridor shots use their own unambiguous shapes. All-range shots morph
  -- through four shapes shared by one actor, so also require the retail
  -- player-shot ownership/lifetime fields. Hostile E3A8 shots instead carry
  -- 10,2,0,0,90 and must never be accelerated onto another enemy.
  if shape == 0xBF58 or shape == 0xBF74 then return true end
  local all_range_shape = shape == 0xE3E0
    or shape == 0xE3C4
    or shape == 0xCBB4
    or shape == 0xE904
    or shape == 0xE920
  return all_range_shape
    and work_byte(object + 0x2D) == 120
    and work_byte(object + 0x2E) == 1
    and work_byte(object + 0x2F) == 0
    and work_byte(object + 0x30) == 0
    and work_byte(object + 0x31) == 138
end

function player_damage_oracle.force_impact()
  if not player_damage_oracle.force or work_byte(0x1B68) ~= 1 then return end
  local player = work_word(0x12C3)
  if player == 0 then return end

  if player_damage_oracle.projectile then
    if not player_damage_oracle.is_hostile_projectile(
      player_damage_oracle.projectile) then
      if player_damage_oracle.trace then
        player_damage_oracle.lines[#player_damage_oracle.lines + 1] = string.format(
          "elapsed=%d event=forced-hostile-projectile-consumed projectile=%04X",
          frame - armed_frame,
          player_damage_oracle.projectile)
      end
      player_damage_oracle.projectile = nil
    else
      player_damage_oracle.pin_to_player(
        player_damage_oracle.projectile,
        player)
    end
    return
  end

  if player_damage_oracle.locked_hits >= player_damage_oracle.maximum_hits then
    return
  end
  local object = work_word(0x12A8)
  local seen = {}
  while object ~= 0 and not seen[object] do
    seen[object] = true
    if object ~= player and player_damage_oracle.is_hostile_projectile(object) then
      player_damage_oracle.pin_to_player(object, player)
      player_damage_oracle.projectile = object
      player_damage_oracle.locked_hits = player_damage_oracle.locked_hits + 1
      if player_damage_oracle.probe then
        player_damage_oracle.initial_health = work_byte(0x1DD1)
        player_damage_oracle.minimum_health = player_damage_oracle.initial_health
      end
      if player_damage_oracle.trace then
        player_damage_oracle.lines[#player_damage_oracle.lines + 1] = string.format(
          "elapsed=%d event=forced-hostile-projectile-locked " ..
            "hit=%d projectile=%04X player=%04X health=%d pose=%d,%d,%d",
          frame - armed_frame,
          player_damage_oracle.locked_hits,
          object,
          player,
          work_byte(player + 0x2D),
          signed_word(player + 12),
          signed_word(player + 14),
          signed_word(player + 16))
      end
      break
    end
    object = work_word(object)
  end
end

-- Deliberately global: this large oracle has reached Lua's main-chunk local
-- limit. Shipping Rust never receives this source-state positioning helper.
function oracle_apply_player_teleport()
  if not teleport_x or work_byte(0x1B68) ~= 1 then return end
  -- Mesen can request controller input while `loadSavestate` is restoring
  -- memory. A one-shot placement made from that callback is overwritten by
  -- the remainder of the restore, but its frame budget would already be
  -- consumed. Wait for the first complete resumed frame so the requested
  -- placement becomes the authoritative retail state.
  if restoring_state or (resume_state_path and frame <= resume_elapsed) then return end
  local player = work_word(0x12C3)
  if player == 0 then return end
  if not teleported_player or lock_teleport_horizontal then
    write_work_word(player + 12, teleport_x)
    write_work_word(player + 16, teleport_z)
    if not teleported_player or lock_teleport_vertical then
      write_work_word(player + 14, teleport_y)
    end
    if teleport_yaw then
      emu.write(player + 20, teleport_yaw % 256, emu.memType.snesWorkRam)
    end
    if not teleported_player then
      teleport_frames_remaining = teleport_frames_remaining - 1
      teleported_player = teleport_frames_remaining <= 0
    end
  end
end

local function provide_combat_autopilot()
  if not combat_autopilot or work_byte(0x1B68) ~= 1 then return false end
  local player = work_word(0x12C3)
  if player == 0 then return false end
  local player_x = signed_word(player + 12)
  local player_y = signed_word(player + 14)
  local player_z = signed_word(player + 16)
  local player_shape = work_word(player + 4)
  if work_byte(0x1BB5) == 11 then
    if player_shape == 0xC310 or player_shape == 0xC2F4 then
      astropolis_transform_requested = true
    elseif is_player_walker_shape(player_shape) then
      astropolis_transform_requested = false
      astropolis_walker_mode = true
    elseif is_player_flight_shape(player_shape) then
      astropolis_transform_requested = false
      astropolis_walker_mode = false
    end
  end
  local target = 0
  local target_shape = 0
  local approaching_carrier = false
  local approaching_macbeth_base = false
  local approaching_fortuna_base = false
  local approaching_venom_base = false
  local target_distance_squared = math.huge
  local meteor_map = work_word(0x1657)
  local eladard_interior = work_byte(0x1BB5) == 3
    and work_byte(0x192E) == 0x05
    and (meteor_map == 0x33EC
      or meteor_map == 0x3490
      or meteor_map == 0x34DF)
  if eladard_interior then
    -- Savestates reload retail state but not this Lua controller's globals.
    -- Reconstruct the interior classification from the observed map gates so
    -- a strict replay can be verified in independent chunks.
    eladard_base_entered = true
    if meteor_map == 0x34DF then
      installation_core_encounter_seen = true
    end
  end
  local meteor_surface = work_byte(0x1BB5) == 4
    and work_byte(0x192E) == 0x05
    and meteor_map == 0x4012
  local venom_surface = work_byte(0x1BB5) == 0
    and work_byte(0x192E) == 0x05
    and meteor_map == 0x03AE
  local venom_interior = work_byte(0x1BB5) == 0
    and work_byte(0x192E) == 0x05
    and (meteor_map == 0x0A53
      or meteor_map == 0x0ADE
      or meteor_map == 0x0B45)
  local macbeth_surface = work_byte(0x1BB5) == 2
    and work_byte(0x192E) == 0x05
    and meteor_map == 0x24D8
  local fortuna_surface = work_byte(0x1BB5) == 5
    and work_byte(0x192E) == 0x05
    and meteor_map == 0x4B40
  local titania_surface = work_byte(0x1BB5) == 1
    and work_byte(0xD7A1) > 1
  if work_byte(0x1BB5) == 1
    and work_byte(0xD7A1) == 1
    and player_y <= -110
    and math.abs(player_x) <= 512
    and math.abs(player_z) <= 512 then
    -- Titania's east doorway lowers the Walker below the surface before
    -- the interior route returns to ordinary ground height. Remember that
    -- observed transition rather than classifying every objective-one pose
    -- on the installation's positive-Z half as being inside the base.
    titania_base_entered = true
  end
  local titania_entrance = work_byte(0x1BB5) == 1
    and work_byte(0xD7A1) == 1
    and not titania_base_entered
  local meteor_interior = work_byte(0x1BB5) == 4
    and work_byte(0x192E) == 0x05
    and (meteor_map == 0x45B9
      or meteor_map == 0x4512
      or meteor_map == 0x4893)
  local meteor_core_room = meteor_interior
    and meteor_map == 0x4893
  local macbeth_interior = work_byte(0x1BB5) == 2
    and work_byte(0x192E) == 0x05
    and meteor_map == 0x2889
  local fortuna_interior = work_byte(0x1BB5) == 5
    and work_byte(0x192E) == 0x05
    and (meteor_map == 0x4F5B or meteor_map == 0x4FD7)
  local inside_planetary_base = eladard_base_entered
    or meteor_interior
    or macbeth_interior
    or fortuna_interior
    or venom_interior
    or titania_base_entered
  local requested_object_seen = false
  local allow_ordinary_targets = not forced_target_shape
    and (not forced_target_object or forced_target_object_retired)
  local eladard_open_door = 0
  local object = work_word(0x12A8)
  local seen = {}
  while object ~= 0 and not seen[object] do
    seen[object] = true
    -- Certified campaign branches use either the ordinary interceptor craft
    -- or a Star Wolf rival. Source-machine tokens stay confined to this
    -- oracle controller.
    local shape = work_word(object + 4)
    if force_macbeth_core_gate and not force_macbeth_core_gate_applied
      and work_byte(0x1BB5) == 2 and shape == 0xF2F8 then
      -- Oracle-only fast-forward after the two retail corner guns have been
      -- independently retired. The controller waits for its linked phase to
      -- exceed one, then uses the original path to lower the shield and arm
      -- the core; leave those downstream transitions entirely to retail.
      emu.write(object + 0x1CE3, 2, emu.memType.snesWorkRam)
      force_macbeth_core_gate_applied = true
      lines[#lines + 1] = string.format(
        "elapsed=%d event=macbeth-core-gate-released controller=%04X phase=2",
        frame - armed_frame,
        object)
    end
    if force_fortuna_core_gate and not force_fortuna_core_gate_applied
      and work_byte(0x1BB5) == 5 and work_byte(0x192E) == 0x05
      and work_word(0x1657) == 0x4FD7 and shape == 0xF2F8
      and work_word(object + 0x2B) == 0x5E4A then
      -- Oracle-only fast-forward after the retail defensive emplacements are
      -- retired. The controller compares this completed-defense count with
      -- its required count, then performs the original shield and core
      -- activation paths; those downstream transitions remain retail code.
      emu.write(object + 0x1CE3, 2, emu.memType.snesWorkRam)
      force_fortuna_core_gate_applied = true
      lines[#lines + 1] = string.format(
        "elapsed=%d event=fortuna-core-gate-released " ..
          "controller=%04X completed_defenses=2",
        frame - armed_frame,
        object)
    end
    if (work_byte(0x1BB5) == 3 or venom_interior) and shape == 0xEC30 then
      eladard_open_door = object
    end
    if shape == 0xC348 and forced_rival_health
      and work_byte(object + 0x2E) == 4
      and work_byte(object + 0x2D) > forced_rival_health then
      -- Oracle-only acceleration for observing the retail post-rival path.
      -- The rival initializes its battle health after its object first
      -- appears, so clamp after that initialization marker is present.
      emu.write(
        object + 0x2D,
        forced_rival_health % 256,
        emu.memType.snesWorkRam)
      lines[#lines + 1] = string.format(
        "elapsed=%d event=rival-health-clamped object=%04X health=%d",
        frame - armed_frame,
        object,
        forced_rival_health)
    end
    if shape == 0xE1B0 and forced_mirage_health
      and work_byte(object + 0x2D) > forced_mirage_health then
      -- Oracle-only state forcing used to observe the retail post-boss path
      -- independently of the mouth-open collision window.
      emu.write(
        object + 0x2D,
        forced_mirage_health % 256,
        emu.memType.snesWorkRam)
    end
    if (shape == 0xF1C4 or shape == 0xEA00)
      and forced_fighter_health
      and work_byte(object + 0x2D) > forced_fighter_health then
      -- Oracle-only acceleration for observing the post-formation branch.
      emu.write(
        object + 0x2D,
        forced_fighter_health % 256,
        emu.memType.snesWorkRam)
    end
    if preserve_fighter_attackers and (shape == 0xF1C4 or shape == 0xEA00) then
      -- Oracle-only survivability used to retain ordinary retail attackers
      -- long enough to sample their hostile projectile and player-hit path.
      emu.write(object + 0x2D, 120, emu.memType.snesWorkRam)
    end
    local requested_shape = forced_target_shape and shape == forced_target_shape
    local requested_object_address =
      forced_target_object and object == forced_target_object
    if requested_object_address and forced_target_object_shape == nil then
      forced_target_object_shape = shape
      lines[#lines + 1] = string.format(
        "elapsed=%d event=forced-target-object-bound object=%04X shape=%04X",
        frame - armed_frame,
        object,
        shape)
    end
    local requested_object = requested_object_address
      and shape == forced_target_object_shape
    if requested_object then requested_object_seen = true end
    local requested_target = requested_object or requested_shape
    if requested_target and forced_target_path
      and not forced_target_path_applied then
      forced_target_initial_path = work_word(object + 0x2B)
      write_work_word(object + 0x2B, forced_target_path)
      forced_target_path_applied = true
      lines[#lines + 1] = string.format(
        "elapsed=%d event=forced-target-path object=%04X shape=%04X " ..
          "from=%04X to=%04X",
        frame - armed_frame,
        object,
        shape,
        forced_target_initial_path,
        forced_target_path)
    end
    if requested_target and forced_target_collision_applied then
      local current_path = work_word(object + 0x2B)
      if current_path ~= forced_target_initial_path then
        forced_target_reaction_seen = true
      elseif forced_target_reaction_seen then
        forced_target_reaction_complete = true
      end
    end
    local explicitly_requested_target = requested_target
      and not forced_target_reaction_complete
      and (not forced_target_collision_frame
        or frame - forced_target_collision_frame < 600)
    local target_health_ready = meteor_core_accepts_damage(object, shape)
      and (not force_target_collision
        or forced_target_health ~= 0
        or (forced_target_collision_frame
          and work_word(object + 0x2B) ~= forced_target_initial_path
          and work_word(object + 0x2B) ~= 0x0C21))
    local generic_target_health_probe = requested_target
      and forced_target_health ~= nil
    if requested_target
      and (target_health_ready or generic_target_health_probe)
      and forced_target_health
      and work_byte(object + 0x2D) > forced_target_health then
      -- Arbitrary source actor targeting is deliberately oracle-only. It is
      -- used to identify an unknown encounter's vulnerable actor before that
      -- actor receives a semantic name in the native port.
      emu.write(
        object + 0x2D,
        forced_target_health % 256,
        emu.memType.snesWorkRam)
    end
    if (meteor_core_room
      or (work_byte(0x1BB5) == 3 and work_word(0x1657) == 0x34DF))
      and (shape == 0xEB50 or shape == 0xEB6C) then
      installation_core_encounter_seen = true
    end
    if meteor_core_room and shape == 0xEB50
      and forced_meteor_core_parent_health then
      emu.write(
        object + 0x2D,
        forced_meteor_core_parent_health % 256,
        emu.memType.snesWorkRam)
    end
    if (work_byte(0x1BB5) == 3 and shape == 0xD27C)
      or (meteor_core_room and shape == 0xEB6C) then
      eladard_base_entered = true
      inside_planetary_base = true
    end
    local eladard_barrier = work_byte(0x1BB5) == 3 and shape == 0xD74C
    local eladard_final_room = work_byte(0x1BB5) == 3
      and work_word(0x1657) == 0x34DF
    local meteor_core_target = meteor_core_room
      and ((target_meteor_core_parent and shape == 0xEB50)
        or (not target_meteor_core_parent and shape == 0xEB6C))
      and (target_meteor_core_parent or work_byte(object + 0x2D) > 1)
    local eladard_core_target = eladard_final_room
      and shape == 0xEB6C and work_byte(object + 0x2D) > 1
    if meteor_core_target and forced_meteor_core_health
      and not forced_meteor_core_health_applied then
      -- Oracle-only state forcing used to observe the retail post-defeat path
      -- independently of the navigation controller.
      emu.write(
        object + 0x2D,
        forced_meteor_core_health % 256,
        emu.memType.snesWorkRam)
      forced_meteor_core_health_applied = true
    end
    local eladard_room_defender = work_byte(0x1BB5) == 3
      and (shape == 0xEE0C or (eladard_final_room
        and shape == 0xBECC))
    local planetary_base_defender = eladard_room_defender
      or meteor_core_target or eladard_core_target
    local eladard_target_switch = work_byte(0x1BB5) == 3
      and inside_planetary_base and shape == 0xEF5C
      and (work_byte(object + 0x26) & 0x02) == 0
    local meteor_surface_target = meteor_surface and shape == 0xEF5C
    local macbeth_switch_target = macbeth_surface and shape == 0xEF5C
      and (work_byte(object + 0x26) & 0x02) == 0
    local venom_interior_switch = venom_interior and shape == 0xEF5C
      and (work_byte(object + 0x26) & 0x02) == 0
    local macbeth_base_entrance = macbeth_surface
      and work_byte(0xD7A1) == 1 and shape == 0xD6C0
    local fortuna_base_entrance = fortuna_surface
      and work_byte(0xD7A1) == 1 and work_byte(0xD78B) == 3
      and shape == 0xD6C0
    local venom_base_entrance = venom_surface
      and work_byte(0xD7A1) == 1 and shape == 0xD6C0
    local titania_switch_target = titania_surface and shape == 0xEF5C
    local titania_route_landmark = titania_surface and shape == 0xEDD4
    local carrier_exterior_anchor = work_byte(0x1BB5) == 8
      and shape == 0xBC9C
      and work_byte(object + 0x2D) == 100
      and work_byte(object + 0x2E) == 4
    local carrier_core = work_byte(0x1BB5) == 8
      and work_byte(0x1BA5) == 3
      and shape == 0xBECC
    local astropolis_security_turret = work_byte(0x1BB5) == 11
      and shape == 0xF65C
    local astropolis_target_switch = work_byte(0x1BB5) == 11
      and work_byte(0xD78A) ~= 0 and shape == 0xEF5C
      and (work_byte(object + 0x26) & 0x02) == 0
    local astropolis_core_spike = work_byte(0x1BB5) == 11
      and shape == 0xC40C and work_byte(object + 0x2E) == 1
      and work_byte(object + 0x2D) > 1
    local astropolis_exposed_cube = work_byte(0x1BB5) == 11
      and shape == 0xE808 and work_word(object + 0x2B) == 0xF186
      and work_byte(object + 0x2D) > 1
    local astropolis_mask = work_byte(0x1BB5) == 11
      and shape == 0xDF48 and work_byte(object + 0x2E) == 0
      and work_byte(object + 0x2D) > 1
    local astropolis_final_core = work_byte(0x1BB5) == 11
      and shape == 0xDED8 and work_byte(object + 0x2E) == 1
      and work_byte(object + 0x2D) > 1
    -- A formation craft at forced low health must remain the targeting
    -- priority.  Its own nearby collision actor otherwise looks closer and
    -- can make the oracle pilot chase the projectile forever.
    local fighter_collision = not forced_fighter_health and shape == 0xBEB0
      and (work_byte(object + 0x2E) == 66
        or work_byte(object + 0x2E) == 80)
    local ordinary_candidate = allow_ordinary_targets
      and (shape == 0xF1C4 or shape == 0xEA00
        or shape == 0xC348 or shape == 0xE1B0
        or eladard_barrier or planetary_base_defender
        or eladard_target_switch
        or meteor_surface_target or macbeth_switch_target
        or venom_interior_switch
        or macbeth_base_entrance or fortuna_base_entrance
        or venom_base_entrance
        or titania_switch_target
        or titania_route_landmark
        or carrier_exterior_anchor or carrier_core or fighter_collision
        or astropolis_security_turret or astropolis_target_switch
        or astropolis_core_spike or astropolis_exposed_cube
        or astropolis_mask or astropolis_final_core)
    if explicitly_requested_target or ordinary_candidate then
      local delta_x = signed_word(object + 12) - player_x
      local delta_y = signed_word(object + 14) - player_y
      local delta_z = signed_word(object + 16) - player_z
      local distance_squared =
        delta_x * delta_x + delta_y * delta_y + delta_z * delta_z
      local prefer_left_eladard_barrier = eladard_barrier
        and (target == 0
          or target_shape ~= 0xD74C
          or signed_word(object + 12) < signed_word(target + 12))
      local prefer_planetary_core = (meteor_core_target or eladard_core_target)
        and target_shape ~= 0xEB6C
      local prefer_eladard_target_switch = eladard_target_switch
        and target_shape ~= 0xEF5C
      local prefer_meteor_surface_target = meteor_surface_target
        and target_shape ~= 0xEF5C
      local prefer_macbeth_switch_target = macbeth_switch_target
        and target_shape ~= 0xEF5C
      local prefer_venom_interior_switch = venom_interior_switch
        and target_shape ~= 0xEF5C
      local prefer_macbeth_base_entrance = macbeth_base_entrance
        and target_shape ~= 0xD6C0
      local prefer_fortuna_base_entrance = fortuna_base_entrance
        and target_shape ~= 0xD6C0
      local prefer_venom_base_entrance = venom_base_entrance
        and target_shape ~= 0xD6C0
      local prefer_astropolis_security_turret = astropolis_security_turret
        and target_shape ~= 0xF65C
      local prefer_astropolis_target_switch = astropolis_target_switch
        and target_shape ~= 0xEF5C
      local prefer_explicit_target = explicitly_requested_target
        and distance_squared < target_distance_squared
      if prefer_explicit_target
        or (allow_ordinary_targets
          and (prefer_left_eladard_barrier
            or prefer_planetary_core
            or prefer_eladard_target_switch
            or prefer_meteor_surface_target
            or prefer_macbeth_switch_target
            or prefer_venom_interior_switch
            or prefer_macbeth_base_entrance
            or prefer_fortuna_base_entrance
            or prefer_venom_base_entrance
            or prefer_astropolis_security_turret
            or prefer_astropolis_target_switch
            or carrier_exterior_anchor
            or (target_shape ~= 0xEB6C and not eladard_barrier
              and distance_squared < target_distance_squared))) then
        target = object
        target_shape = shape
        approaching_carrier = carrier_exterior_anchor
        approaching_macbeth_base = macbeth_base_entrance
        approaching_fortuna_base = fortuna_base_entrance
        approaching_venom_base = venom_base_entrance
        target_distance_squared = distance_squared
      end
    end
    object = work_word(object)
  end
  if forced_target_object and not requested_object_seen
    and not forced_target_object_retired then
    -- Linked children are live retail objects but are not members of the
    -- top-level object chain rooted at $12A8. An exact oracle request may
    -- therefore name a rotating turret, nested boss part, or similar child
    -- which the ordinary target walk cannot discover. Bind only the requested
    -- slot while it contains a valid shape header; this source-layout escape
    -- hatch remains strictly inside the oracle.
    forced_target_object_shape = forced_target_object_shape
      or work_word(forced_target_object + 4)
    if forced_target_object_shape >= 0xBC9C
      and forced_target_object_shape < 0xFB9C
      and work_word(forced_target_object + 4) == forced_target_object_shape then
      requested_object_seen = true
      target = forced_target_object
      target_shape = forced_target_object_shape
      target_distance_squared = 0
      if forced_target_health ~= nil
        and work_byte(forced_target_object + 0x2D) > forced_target_health then
        emu.write(
          forced_target_object + 0x2D,
          forced_target_health % 256,
          emu.memType.snesWorkRam)
      end
      if not forced_target_initial_path then
        forced_target_initial_path = work_word(forced_target_object + 0x2B)
        lines[#lines + 1] = string.format(
          "elapsed=%d event=forced-target-child-bound object=%04X shape=%04X",
          frame - armed_frame,
          forced_target_object,
          forced_target_object_shape)
      end
      if forced_target_path and not forced_target_path_applied then
        write_work_word(forced_target_object + 0x2B, forced_target_path)
        forced_target_path_applied = true
        lines[#lines + 1] = string.format(
          "elapsed=%d event=forced-target-child-path object=%04X " ..
            "shape=%04X from=%04X to=%04X",
          frame - armed_frame,
          forced_target_object,
          forced_target_object_shape,
          forced_target_initial_path,
          forced_target_path)
      end
    end
  end
  if forced_target_object and forced_target_object_shape
    and not requested_object_seen and not forced_target_object_retired then
    -- Once an exact oracle target leaves the active list, continue with the
    -- semantic mission targets discovered in the same retail run. Never bind
    -- the recycled address to a different actor.
    forced_target_object_retired = true
    lines[#lines + 1] = string.format(
      "elapsed=%d event=forced-target-object-retired object=%04X shape=%04X",
      frame - armed_frame,
      forced_target_object,
      forced_target_object_shape)
  end

  if titania_entrance and not forced_target_shape and not forced_target_object then
    -- Once both exterior switches are pressed, retail opens a doorway on the
    -- installation's east rim. Nearby fighters are incidental to that route;
    -- use the fixed architectural opening until the Walker crosses its
    -- interior floor transition.
    target = 0
    target_shape = 0
    target_distance_squared = math.huge
  end

  local target_x = target ~= 0 and signed_word(target + 12) or 0
  local target_y = target ~= 0 and signed_word(target + 14) or 0
  local target_z = target ~= 0 and signed_word(target + 16) or 0
  if approaching_fortuna_base
    and (player_x < -1700 or math.abs(player_z) > 96) then
    -- The opened tunnel is on the installation's west face. Align with that
    -- face before advancing on the centre marker; aiming at the marker from
    -- the northwest beach intersects the cylinder's intact outer wall.
    target_x = -1700
    target_z = 0
  end
  local behind_macbeth_knight = target_shape == 0xE974
    and target ~= 0 and player_z > target_z + 400
  if target_shape == 0xE974 and target ~= 0
    and not behind_macbeth_knight then
    -- The direct centre ray crosses Macbeth's shield and the paired gate
    -- panels. Aim inside the boss's retail body profile on the same side as
    -- the Walker; this is the narrow line of sight opened by the flank.
    target_x = target_x + (player_x >= 0 and 448 or -448)
  end
  if target_shape == 0xDF48 then
    -- The visible eye polygons are not the vulnerable volumes. The original
    -- Andross face defines two unrotated collision boxes below the broad head
    -- box. Aim at the retail collision centers so the oracle distinguishes
    -- eye damage from the mask's generic one-point armour response.
    local player_relative_x = player_x - target_x
    local local_x = player_relative_x >= 0 and 320 or -320
    target_x = target_x + local_x
    target_y = target_y - 480
    local delta_x = target_x - player_x
    local delta_y = target_y - player_y
    local delta_z = target_z - player_z
    target_distance_squared =
      delta_x * delta_x + delta_y * delta_y + delta_z * delta_z
  end
  if target_shape == 0xC348 and target ~= 0 then
    local health = work_byte(target + 0x2D)
    if observed_rival_health[target] ~= health then
      observed_rival_health[target] = health
      lines[#lines + 1] = string.format(
        "elapsed=%d event=rival-health object=%04X health=%d flags=%04X",
        frame - armed_frame,
        target,
        health,
        work_word(target + 0x20))
    end
  end

  if target_shape == 0xD74C and target ~= 0 then
    local health = work_byte(target + 0x2D)
    if observed_eladard_barrier_health[target] ~= health then
      observed_eladard_barrier_health[target] = health
      lines[#lines + 1] = string.format(
        "elapsed=%d event=eladard-barrier-health object=%04X health=%d path=%04X",
        frame - armed_frame,
        target,
        health,
        work_word(target + 0x2B))
    end
  end

  if target_shape == 0xC40C and target ~= 0 then
    local health = work_byte(target + 0x2D)
    if observed_astropolis_spike_health[target] ~= health then
      observed_astropolis_spike_health[target] = health
      lines[#lines + 1] = string.format(
        "elapsed=%d event=astropolis-spike-health object=%04X health=%d path=%04X",
        frame - armed_frame,
        target,
        health,
        work_word(target + 0x2B))
    end
  end

  if target_shape == 0xE808 and target ~= 0 then
    local health = work_byte(target + 0x2D)
    if observed_astropolis_cube_health[target] ~= health then
      observed_astropolis_cube_health[target] = health
      lines[#lines + 1] = string.format(
        "elapsed=%d event=astropolis-cube-health object=%04X health=%d path=%04X",
        frame - armed_frame,
        target,
        health,
        work_word(target + 0x2B))
    end
  end

  if target_shape == 0xDF48 and target ~= 0 then
    local health = work_byte(target + 0x2D)
    if observed_astropolis_mask_health[target] ~= health then
      observed_astropolis_mask_health[target] = health
      lines[#lines + 1] = string.format(
        "elapsed=%d event=astropolis-mask-health object=%04X health=%d path=%04X",
        frame - armed_frame,
        target,
        health,
        work_word(target + 0x2B))
    end
  end

  if target_shape == 0xDED8 and target ~= 0 then
    local health = work_byte(target + 0x2D)
    if observed_astropolis_final_core_health[target] ~= health then
      observed_astropolis_final_core_health[target] = health
      lines[#lines + 1] = string.format(
        "elapsed=%d event=astropolis-final-core-health object=%04X " ..
          "health=%d path=%04X",
        frame - armed_frame,
        target,
        health,
        work_word(target + 0x2B))
    end
  end

  local generic_target_damage_probe = forced_target_health
    and forced_target_health > 0
    and target == forced_target_object
    and target_shape == forced_target_object_shape
  if force_projectile_hit and forced_projectile_address
    and (meteor_core_accepts_damage(target, target_shape)
      or generic_target_damage_probe) then
    local projectile_shape = work_word(forced_projectile_address + 4)
    if not is_player_projectile(
      forced_projectile_address,
      projectile_shape) then
      forced_projectile_address = nil
      forced_projectile_position = nil
      forced_projectile_hit_applied = false
      lines[#lines + 1] = string.format(
        "elapsed=%d event=forced-projectile-consumed shape=%04X",
        frame - armed_frame,
        projectile_shape)
    elseif target ~= 0 then
      -- Keep following a moving target until retail collision handling
      -- consumes the shot. A static pin is insufficient for mobile rivals
      -- and Mirage Dragon's mouth.
      forced_projectile_position = {
        target_x % 65536,
        target_y % 65536,
        target_z % 65536,
      }
      write_work_word(
        forced_projectile_address + 12,
        forced_projectile_position[1])
      write_work_word(
        forced_projectile_address + 14,
        forced_projectile_position[2])
      write_work_word(
        forced_projectile_address + 16,
        forced_projectile_position[3])
    end
  end

  if force_projectile_hit and not forced_projectile_hit_applied and target ~= 0
    and (meteor_core_accepts_damage(target, target_shape)
      or generic_target_damage_probe) then
    local projectile = work_word(0x12A8)
    local projectile_seen = {}
    while projectile ~= 0 and not projectile_seen[projectile] do
      projectile_seen[projectile] = true
      local projectile_shape = work_word(projectile + 4)
      if projectile ~= player
        and is_player_projectile(projectile, projectile_shape) then
        write_work_word(projectile + 12, target_x)
        write_work_word(projectile + 14, target_y)
        write_work_word(projectile + 16, target_z)
        forced_projectile_address = projectile
        forced_projectile_position = {
          target_x % 65536,
          target_y % 65536,
          target_z % 65536,
        }
        forced_projectile_hit_applied = true
        lines[#lines + 1] = string.format(
          "elapsed=%d event=forced-projectile-locked projectile=%04X " ..
            "target=%04X target_shape=%04X",
          frame - armed_frame,
          projectile,
          target,
          target_shape)
        break
      end
      projectile = work_word(projectile)
    end
  end

  if force_target_collision and not forced_target_collision_applied
    and target ~= 0 and work_word(target + 0x1CCD) ~= 0 then
    forced_target_initial_path = work_word(target + 0x2B)
    emu.write(
      target + 0x20,
      work_byte(target + 0x20) | 0x80,
      emu.memType.snesWorkRam)
    forced_target_collision_applied = true
    forced_target_collision_frame = frame
  end

  if force_meteor_core_trigger and not forced_meteor_core_trigger_applied
    and target_shape == 0xEB6C then
    emu.write(
      METEOR_CORE_TRIGGER_ADDRESS,
      0xFF,
      emu.memType.snesWorkRam)
    forced_meteor_core_trigger_applied = true
    lines[#lines + 1] = string.format(
      "elapsed=%d event=forced-meteor-core-trigger target=%04X address=%04X",
      frame - armed_frame,
      target,
      METEOR_CORE_TRIGGER_ADDRESS)
  end

  -- Short pulses produce repeated direct shots instead of holding one charge
  -- forever.
  local fighting_rival = target_shape == 0xC348
  local fighting_mirage_dragon = target_shape == 0xE1B0
  local fighting_eladard_barrier = work_byte(0x1BB5) == 3
    and target_shape == 0xD74C
  local fighting_meteor_core = target_shape == 0xEB6C or target_shape == 0xEB50
  local fighting_carrier_core = work_byte(0x1BB5) == 8
    and target_shape == 0xBECC
  local fighting_macbeth_knight = work_byte(0x1BB5) == 2
    and work_word(0x1657) == 0x2889
    and target_shape == 0xE974
  local fighting_venom_knight = work_byte(0x1BB5) == 0
    and work_word(0x1657) == 0x0ADE
    and target_shape == 0xE974
  fighting_macbeth_knight = fighting_macbeth_knight or fighting_venom_knight
  if fighting_macbeth_knight
    and ((fighting_venom_knight and player_z >= 4800)
      or (not fighting_venom_knight
        and math.abs(player_x) <= 128 and player_z >= 3750)) then
    macbeth_knight_engaged = true
  end
  local fighting_astropolis_core = work_byte(0x1BB5) == 11
    and (target_shape == 0xC40C or target_shape == 0xE808)
  local fighting_astropolis_spike = work_byte(0x1BB5) == 11
    and target_shape == 0xC40C
  local fighting_astropolis_cube = work_byte(0x1BB5) == 11
    and target_shape == 0xE808
  if (approaching_macbeth_base or approaching_venom_base)
    and is_player_walker_shape(player_shape)
    and macbeth_transform_press_until < frame
    and macbeth_next_transform_press_frame <= frame then
    -- Retail admits an Arwing automatically at the opened installation.
    -- Surface switches require the Walker, so explicitly return to flight
    -- form before the verification pilot approaches the entrance.
    macbeth_transform_press_until = frame + 3
    macbeth_next_transform_press_frame = frame + 30
  end
  if fighting_astropolis_spike
    and is_player_walker_shape(player_shape)
    and not astropolis_transform_requested
    and astropolis_transform_press_until < frame then
    -- The core spikes are elevated around the chamber. Return to Arwing form
    -- so the retail pitch controls can line the blaster up with each one.
    astropolis_transform_press_until = frame + 3
    astropolis_transform_requested = true
  elseif fighting_astropolis_cube
    and is_player_flight_shape(player_shape)
    and not astropolis_transform_requested
    and astropolis_transform_press_until < frame then
    -- The exposed cube is below the flight line over the chasm. Walker form
    -- keeps a stable firing position while the retail blaster reaches it.
    astropolis_transform_press_until = frame + 3
    astropolis_transform_requested = true
  end
  -- Always replace the restored controller state. Passing nil when an
  -- explicitly requested target has not spawned yet lets Mesen retain the
  -- savestate's last input, which can steer an oracle placement away from a
  -- mission entrance before the requested actor exists.
  local buttons = {}
  if approaching_carrier then
    -- A battle carrier admits the craft automatically at close range. Its
    -- exterior shell is not the objective, so boost toward its stable centre
    -- anchor without wasting time firing into the armour.
    buttons = {}
  elseif approaching_macbeth_base then
    buttons = {}
  elseif approaching_fortuna_base then
    buttons = {}
  elseif approaching_venom_base then
    buttons = {}
  elseif fighting_rival or fighting_mirage_dragon then
    buttons = {
      -- Mobile rivals and Mirage Dragon's mouth are most reliably hit with
      -- charged shots; the short release window repeatedly launches them.
      b = frame % 32 < 26,
      l = pulse(frame, 80, 0),
      r = pulse(frame, 80, 40),
    }
    if fighting_rival then
      -- The campaign's stocked special items materially reduce exposure time
      -- during the much longer rival pursuit.
      buttons.x = pulse(frame, 600, 0)
    end
  elseif fighting_meteor_core then
    -- Fire continuously through the retail jump arc so at least one Walker
    -- laser is born at the elevated weak point's height.
    buttons = { b = pulse(frame, 12, 0) }
    if target_distance_squared <= 360000 then
      local strafe_phase = frame % 480
      buttons.left = strafe_phase < 240
      buttons.right = strafe_phase >= 240
    end
  elseif fighting_macbeth_knight then
    -- Knight Nack's separate shield fills the direct centre lane and reflects
    -- frontal shots. Establish a retail Walker-space flank before firing
    -- through the boss's ordinary collision path.
    buttons = {}
    if behind_macbeth_knight
      or (macbeth_knight_engaged
        and math.abs(player_x) >= 2000 and player_z >= 3750) then
      buttons.b = pulse(frame, 12, 0)
    end
  else
    buttons = { b = pulse(frame, 12, 0) }
  end
  if target ~= 0 then
    local delta_x = target_x - player_x
    local delta_y = target_y - player_y
    local delta_z = target_z - player_z
    local horizontal_distance = math.sqrt(delta_x * delta_x + delta_z * delta_z)
    local desired_yaw = math.floor(math.atan(delta_x, delta_z) * 128 / math.pi + 0.5) % 256
    local desired_pitch = math.floor(math.atan(delta_y, horizontal_distance) * 128 / math.pi + 0.5) % 256
    local planetary_base_walker =
      (inside_planetary_base or meteor_surface or macbeth_surface
        or titania_surface or fortuna_surface or venom_surface)
      and is_player_walker_shape(player_shape)
    local fortuna_base_walker = approaching_fortuna_base
      and is_player_walker_shape(player_shape)
    local carrier_core_walker = fighting_carrier_core
      or fighting_astropolis_core
    local activation_target = target_shape == 0xEF5C
    if activation_target
      and target_distance_squared < 160000
      and is_player_flight_shape(player_shape)
      and not astropolis_walker_mode
      and astropolis_transform_press_until < frame then
      -- Reach the fire-floor target in Walker form and retry the retail
      -- transform input until the transition shape is observed; a requested
      -- form is not a confirmed form. The target's own path owns activation.
      astropolis_transform_press_until = frame + 3
      astropolis_transform_requested = true
      lines[#lines + 1] = string.format(
        "elapsed=%d event=astropolis-transform-request distance2=%d player=%04X",
        frame - armed_frame,
        target_distance_squared,
        player_shape)
    end
    local eladard_surface_walker = fighting_eladard_barrier
      and is_player_walker_shape(player_shape)
    local ground_walker = planetary_base_walker or carrier_core_walker
      or eladard_surface_walker
      or astropolis_transform_requested or astropolis_walker_mode
    local astropolis_interior = work_byte(0x1BB5) == 11 and player_z > 1000
    if ground_walker or approaching_carrier or astropolis_interior then
      -- Walker-space lateral motion is mirrored relative to the flight-space
      -- convention used by the other oracle encounters. The carrier approach
      -- and Astropolis interior use the same all-range orientation convention.
      desired_yaw = math.floor(math.atan(-delta_x, delta_z) * 128 / math.pi + 0.5) % 256
    end
    if fighting_macbeth_knight then
      -- This boss is approached side-on, so aim the Walker's blasters through
      -- the ordinary world-space bearing while its D-pad continues to use the
      -- room-space translation convention above.
      desired_yaw = math.floor(
        math.atan(delta_x, delta_z) * 128 / math.pi + 0.5) % 256
    end
    local yaw_difference = angle_difference(desired_yaw, work_byte(player + 20))
    local pitch_difference = angle_difference(desired_pitch, work_byte(player + 18))
    if ground_walker then
      -- In Walker form the D-pad translates along the two ground-plane axes,
      -- while the shoulder buttons turn the aim. Keep those controls
      -- independent so the oracle can approach an off-axis retail target
      -- without mistaking a sidestep for flight steering.
      local approach_distance_squared
      if fighting_carrier_core then
        approach_distance_squared = math.huge
      elseif fighting_meteor_core then
        approach_distance_squared = 360000
      elseif activation_target then
        -- Walk fully onto a switch. Its retail path owns activation, so an
        -- artificial stand-off distance can only strand the oracle just
        -- outside the relevant contact volume.
        approach_distance_squared = 0
      else
        approach_distance_squared = 1440000
      end
      if fighting_macbeth_knight then
        if behind_macbeth_knight then
          -- A rear-line oracle placement already cleared every shield and
          -- fire-barrier volume; hold position so only retail damage remains.
        elseif not macbeth_knight_engaged then
          if player_z < 3800 then
            buttons.up = true
          end
        elseif math.abs(player_x) < 2000 then
          buttons.right = true
        else
          -- Clear the centre fire barrier from the outside lane, then keep
          -- circling into the arena while the boss turns toward the Walker.
          buttons.up = true
        end
      elseif fortuna_base_walker then
        -- Fortuna uses normal Walker steering underwater: the shoulder
        -- buttons turn and Up advances. Its opened installation is entered
        -- through the submerged side, so continue toward the retail shell
        -- instead of trying to descend through the solid roof marker.
        buttons.up = math.abs(yaw_difference) < 16
      elseif target_distance_squared > approach_distance_squared then
        local axis_tolerance = activation_target and 0 or 128
        if fighting_eladard_barrier then
          -- Surface Walker movement is heading-relative: the shoulder
          -- buttons turn and Up advances. Driving the two room-space axes
          -- directly makes the landed craft run sideways away from a fixed
          -- barrier while its aim continues to turn toward that barrier.
          buttons.up = math.abs(yaw_difference) < 16
        else
          if delta_x > axis_tolerance then
            buttons.right = true
          elseif delta_x < -axis_tolerance then
            buttons.left = true
          end
          if delta_z > axis_tolerance then
            buttons.up = true
          elseif delta_z < -axis_tolerance then
            buttons.down = true
          end
        end
      end
      if planetary_base_walker and player_z > 0 then
        buttons.x = pulse(frame, 90, 0)
      end
      if fortuna_base_walker then
        -- Y raises the Walker and A accelerates it. Use the retail altitude as
        -- feedback so the verification pilot stays centered on the submerged
        -- tunnel rather than surfacing over it or sinking below it.
        buttons.a = true
        buttons.y = player_y < -480
      elseif activation_target and not macbeth_surface then
        -- Activation targets can sit beyond raised terrain. Forward motion
        -- alone can stop the Walker just outside the contact volume, so use
        -- the retail jump controls while advancing onto the target. Let its
        -- retail path decide when activation has completed.
        buttons.a = pulse(frame, 120, 0)
        local jump_phase = frame % 120
        buttons.y = jump_phase >= 6 and jump_phase < 50
      elseif inside_planetary_base then
        buttons.a = pulse(frame, 120, 0)
        local jump_phase = frame % 120
        local acceleration_end = fighting_meteor_core and 80 or 22
        buttons.y = jump_phase >= 6 and jump_phase < acceleration_end
      elseif carrier_core_walker then
        -- The reactor weak points sit above the Walker's standing muzzle.
        -- Sustain the retail jump long enough for shots to reach their
        -- collision height while continuing to fire.
        buttons.a = pulse(frame, 120, 0)
        local jump_phase = frame % 120
        local acceleration_end = fighting_astropolis_core and 80 or 50
        buttons.y = jump_phase >= 6 and jump_phase < acceleration_end
      end
      local preserve_macbeth_flank_heading = fighting_macbeth_knight
        and macbeth_knight_engaged and math.abs(player_x) < 2000
      if not preserve_macbeth_flank_heading then
        if yaw_difference > 3 then
          buttons.l = true
        elseif yaw_difference < -3 then
          buttons.r = true
        end
      end
    else
      if yaw_difference > 3 then
        buttons.left = true
      elseif yaw_difference < -3 then
        buttons.right = true
      end
      if pitch_difference > 3 then
        buttons.up = true
      elseif pitch_difference < -3 then
        buttons.down = true
      end
      if approaching_carrier and math.abs(yaw_difference) < 24 then
        buttons.y = true
      elseif (approaching_macbeth_base or approaching_venom_base)
        and math.abs(yaw_difference) < 24 then
        buttons.y = true
      elseif target_distance_squared > 16000000
        and math.abs(yaw_difference) < 24 then
        buttons.y = true
      elseif fighting_eladard_barrier then
        -- The surface barriers are fixed all-range targets. Brake inside the
        -- ordinary boost threshold so the verification pilot establishes a
        -- firing orbit instead of coasting past the installation until the
        -- retail signed world coordinates wrap.
        buttons.a = true
      end
    end
    if frame <= astropolis_transform_press_until then
      buttons.select = true
    end
    if frame <= macbeth_transform_press_until then
      buttons.select = true
    end
    input_label = string.format(
      "combat-autopilot-%s-%04X-flags%02X-yaw%d-pitch%d",
      approaching_carrier and "carrier-approach"
        or (approaching_fortuna_base and "fortuna-entrance"
          or (approaching_venom_base and "venom-entrance" or "target")),
      target,
      work_byte(target + 0x26),
      yaw_difference,
      pitch_difference)
  else
    if work_byte(0x1BB5) == 11 and player_z > 1000 then
      -- The first Astropolis junction branches through doors in its side
      -- walls, rather than openings beyond the two panels on the rear wall.
      -- Commit to the left route with a true quarter turn and continue firing
      -- while the retail proximity/door path owns the transition.
      local opening_x
      local opening_y
      local opening_z
      if work_byte(0xD78A) == 0 then
        opening_x = -4000
        opening_y = -146
        opening_z = 7424
      elseif player_z < 10800 then
        -- The activation target unlocks the north door centred between the
        -- surrounding wall panels. Aim for the doorway, not the adjacent
        -- solid panel, while its retail path owns the opening transition.
        opening_x = -3840
        opening_y = -146
        opening_z = 11200
      elseif player_z < 14500 then
        -- The next chamber is divided by a paired rotating-panel doorway.
        -- Approach its centre square-on so blaster fire reaches the panels.
        opening_x = -3840
        opening_y = -146
        opening_z = 14848
      elseif player_z < 15550 and player_x < -3400 then
        -- The apparent east-side obstruction is a static L-shaped corridor
        -- wall, not a door. Clear its west end before taking the bend.
        opening_x = -3904
        opening_y = -146
        opening_z = 15616
      elseif player_x < 0 then
        -- Enter the centre of the four-way junction before turning north;
        -- cutting the corner intersects the next pair of static L walls.
        opening_x = 256
        opening_y = -146
        opening_z = 15616
      else
        -- The north corridor leads to the base objective. Its final door uses
        -- the same retail proximity path as the preceding corridor doors.
        opening_x = 256
        opening_y = -146
        opening_z = 19828
      end
      local delta_x = opening_x - player_x
      local delta_y = opening_y - player_y
      local delta_z = opening_z - player_z
      local horizontal_distance = math.sqrt(
        delta_x * delta_x + delta_z * delta_z)
      local desired_yaw = math.floor(
        math.atan(-delta_x, delta_z) * 128 / math.pi + 0.5) % 256
      local desired_pitch = math.floor(
        math.atan(delta_y, horizontal_distance) * 128 / math.pi + 0.5) % 256
      local yaw_difference = angle_difference(
        desired_yaw,
        work_byte(player + 20))
      local pitch_difference = angle_difference(
        desired_pitch,
        work_byte(player + 18))
      buttons.b = pulse(frame, 12, 0)
      if frame <= astropolis_transform_press_until then
        buttons.select = true
      elseif astropolis_walker_mode then
        buttons.up = math.abs(yaw_difference) < 16
        buttons.a = pulse(frame, 120, 0)
        local jump_phase = frame % 120
        buttons.y = jump_phase >= 6 and jump_phase < 22
        if yaw_difference > 3 then
          buttons.l = true
        elseif yaw_difference < -3 then
          buttons.r = true
        end
      else
        if yaw_difference > 3 then
          buttons.left = true
        elseif yaw_difference < -3 then
          buttons.right = true
        end
        if pitch_difference > 3 then
          buttons.up = true
        elseif pitch_difference < -3 then
          buttons.down = true
        end
        if math.abs(yaw_difference) < 20 then
          buttons.y = true
        end
      end
      input_label = string.format(
        "combat-autopilot-astropolis-opening-yaw%d-pitch%d",
        yaw_difference,
        pitch_difference)
    elseif meteor_surface and work_byte(0xD78B) ~= 0 then
      -- The pressed Queen Dragoon switch opens the installation at the north
      -- end of the surface. Walk to the centre of that retail entrance only
      -- after its controller handshake changes; the retail door path owns the
      -- actual transition into the base.
      local entrance_delta_x = -player_x
      local entrance_delta_z = 2800 - player_z
      local axis_tolerance = 96
      if entrance_delta_x > axis_tolerance then
        buttons.right = true
      elseif entrance_delta_x < -axis_tolerance then
        buttons.left = true
      end
      if entrance_delta_z > axis_tolerance then
        buttons.up = true
      elseif entrance_delta_z < -axis_tolerance then
        buttons.down = true
      end
      buttons.a = pulse(frame, 120, 0)
      local jump_phase = frame % 120
      buttons.y = jump_phase >= 6 and jump_phase < 22
      input_label = "combat-autopilot-meteor-entrance"
    elseif titania_entrance then
      -- The opened Titania base is not entered through its solid centre.
      -- Retail exposes a ramp and doorway on the east rim (x=700, z=0), then
      -- lowers the Walker onto the interior floor. Align with the doorway
      -- before continuing west so the original collision and transition
      -- paths remain authoritative.
      local doorway_x = 700
      local doorway_z = 0
      local doorway_tolerance = 80
      local doorway_aligned = math.abs(player_z - doorway_z) <= doorway_tolerance
      if player_z > doorway_z + doorway_tolerance then
        buttons.down = true
      elseif player_z < doorway_z - doorway_tolerance then
        buttons.up = true
      end
      if doorway_aligned then
        buttons.left = true
      elseif player_x > doorway_x + doorway_tolerance then
        buttons.left = true
      elseif player_x < doorway_x - doorway_tolerance then
        buttons.right = true
      else
        buttons.left = true
      end
      buttons.b = pulse(frame, 12, 0)
      input_label = "combat-autopilot-titania-entrance"
    elseif inside_planetary_base then
      local route_direction = eladard_route_direction()
      if ((work_byte(0x1BB5) == 3
          and work_word(0x1657) == 0x33EC)
        or venom_interior)
        and eladard_open_door ~= 0 then
        -- The first interior switch changes the north door from EC14 to
        -- EC30. Align with that retail object's centre before advancing;
        -- driving straight from the switch intersects the wall beside it.
        local doorway_delta_x = signed_word(eladard_open_door + 12) - player_x
        if doorway_delta_x > 80 then
          buttons.right = true
        elseif doorway_delta_x < -80 then
          buttons.left = true
        else
          buttons.up = true
        end
        route_direction = "first-door"
      elseif work_byte(0x1BB5) == 3
        and work_word(0x1657) == 0x3490
        and eladard_open_door ~= 0 then
        -- The next room's open door is rotated and offset from the approach
        -- lane. Follow its live centre on both room axes so the Walker
        -- crosses the opening instead of pressing against the adjacent wall.
        local doorway_delta_x = signed_word(eladard_open_door + 12) - player_x
        local doorway_delta_z = signed_word(eladard_open_door + 16) - player_z
        if doorway_delta_x > 48 then
          buttons.right = true
        elseif doorway_delta_x < -48 then
          buttons.left = true
        end
        if doorway_delta_z > 48 then
          buttons.up = true
        elseif doorway_delta_z < -48 then
          buttons.down = true
        end
        route_direction = "second-door"
      elseif installation_core_encounter_seen then
        local exit_delta_x = 6500 - player_x
        local exit_delta_z = 5120 - player_z
        local exit_yaw = math.floor(
          math.atan(-exit_delta_x, exit_delta_z) * 128 / math.pi + 0.5) % 256
        local exit_yaw_difference = angle_difference(
          exit_yaw,
          work_byte(player + 20))
        buttons.up = math.abs(exit_yaw_difference) < 8
        if exit_yaw_difference > 3 then
          buttons.l = true
        elseif exit_yaw_difference < -3 then
          buttons.r = true
        end
        route_direction = "post-core"
      else
        buttons.up = true
      end
      buttons.x = pulse(frame, 600, 0)
      buttons.a = pulse(frame, 120, 0)
      local jump_phase = frame % 120
      buttons.y = jump_phase >= 6 and jump_phase < 22
      if not installation_core_encounter_seen and route_direction == "left" then
        buttons.l = true
      elseif not installation_core_encounter_seen and route_direction == "right" then
        buttons.r = true
      end
      input_label = "combat-autopilot-base-" .. (route_direction or "forward")
    elseif work_byte(0x1BB5) == 3
      and work_byte(0x192E) == 0x05
      and work_word(0x1657) == 0x2E87 then
      -- Once the two surface barriers are gone, the remaining objective is
      -- the base entrance at the centre of the installation. The entrance
      -- itself stops participating in the object list while its door opens.
      local route_direction = eladard_route_direction()
      buttons.b = pulse(frame, 12, 0)
      local in_flight_recovery = player_y < -100
      if in_flight_recovery then
        eladard_flight_recovery_polls = eladard_flight_recovery_polls + 1
        if route_direction then
          buttons[route_direction] = true
          eladard_centered_polls = 0
        else
          eladard_centered_polls = eladard_centered_polls + 1
        end
        buttons.y = true
        if eladard_flight_recovery_polls > 240
          and (eladard_centered_polls > 30
            or eladard_flight_recovery_polls > 500)
          and eladard_transform_press_until < frame then
          eladard_transform_press_until = frame + 3
          eladard_next_recovery_frame = frame + 240
          eladard_flight_recovery_polls = -10000
        end
      else
        local barely_moved = eladard_last_x ~= nil
          and math.abs(player_x - eladard_last_x) <= 3
          and math.abs(player_z - eladard_last_z) <= 3
        eladard_stuck_polls = barely_moved and (eladard_stuck_polls + 1) or 0
        eladard_last_x = player_x
        eladard_last_z = player_z
        if eladard_progress_anchor_x == nil
          or math.abs(player_x - eladard_progress_anchor_x) > 64
          or math.abs(player_z - eladard_progress_anchor_z) > 64 then
          eladard_progress_anchor_x = player_x
          eladard_progress_anchor_z = player_z
          eladard_no_progress_polls = 0
        else
          eladard_no_progress_polls = eladard_no_progress_polls + 1
        end
        if route_direction then
          eladard_centered_polls = 0
        else
          eladard_centered_polls = eladard_centered_polls + 1
        end
        buttons.up = true
        if route_direction == "left" then
          buttons.l = true
        elseif route_direction == "right" then
          buttons.r = true
        end
        if (eladard_stuck_polls > 90
          or eladard_centered_polls > 120
          or eladard_no_progress_polls > 90)
          and eladard_recovery_count < 3
          and frame >= eladard_next_recovery_frame then
          eladard_recovery_count = eladard_recovery_count + 1
          eladard_transform_press_until = frame + 3
          eladard_next_recovery_frame = frame + 240
          eladard_stuck_polls = 0
          eladard_flight_recovery_polls = 0
          eladard_centered_polls = 0
          eladard_no_progress_polls = 0
        end
      end
      if frame <= eladard_transform_press_until then buttons.select = true end
      input_label = string.format(
        "combat-autopilot-entrance-%s-r%d-f%d",
        route_direction or "forward",
        eladard_recovery_count,
        eladard_flight_recovery_polls)
    elseif work_byte(0x1BB5) == 8 and work_byte(0x1BA5) == 3 then
      buttons.b = pulse(frame, 12, 0)
      if work_byte(0xD787) ~= 0 then
        -- Stop at the reactor, as the retail room changes the craft to Walker
        -- form and rotates the vulnerable panels around the fixed core.
        local sweep_phase = frame % 480
        buttons.l = sweep_phase < 240
        buttons.r = sweep_phase >= 240
        buttons.a = pulse(frame, 120, 0)
        local jump_phase = frame % 120
        buttons.y = jump_phase >= 6 and jump_phase < 50
        input_label = "combat-autopilot-carrier-core-sweep"
      else
        -- Before the reactor, retail constrains the craft to its central rail.
        -- Boost through the corridors and clear switches or doors with
        -- repeated direct fire.
        buttons.y = true
        input_label = "combat-autopilot-carrier-corridor"
      end
    else
      input_label = "combat-autopilot-search"
    end
  end
  if combat_hold_fire then buttons.b = nil end
  emu.setInput(buttons, 0)
  return true
end

local function keep_forced_projectile_on_target(address, value)
  if not forced_projectile_address or not forced_projectile_position then
    return value
  end
  local offset = address - forced_projectile_address - 12
  if offset < 0 or offset >= 6 then
    return value
  end
  local axis = math.floor(offset / 2) + 1
  local shift = (offset % 2) * 8
  return (forced_projectile_position[axis] >> shift) & 0xFF
end

-- Deliberately global: keep the object-relative encounter byte stable across
-- the retail parallel-state refresh which runs after the input callback.
function keep_forced_meteor_core_trigger(_, value)
  if forced_target_object
    and forced_target_object_shape == 0xEB6C
    and work_word(forced_target_object + 4) == forced_target_object_shape then
    return 0xFF
  end
  return value
end

local function provide_input()
  if not loaded_state then
    input_label = "loading-state"
    emu.setInput({}, 0)
    return
  end
  if not armed then
    input_label = "front-end"
    emu.setInput({ start = pulse(frame, 180, 120) }, 0)
    return
  end

  local elapsed = frame - armed_frame
  -- Oracle positioning is independent of the input source. This permits a
  -- one-time retail-state placement followed by an exact scripted control
  -- sequence, instead of requiring the general combat pilot to remain active.
  oracle_apply_player_teleport()
  local awaiting_macbeth_interior = combat_autopilot
    and (forced_target_shape ~= nil or forced_target_object ~= nil)
    and teleport_text ~= nil
    and work_byte(0x1BB5) == 2
    and work_byte(0x192E) == 0x05
    and work_word(0x1657) == 0x24D8
    and work_byte(0xD7A1) == 1
  if not awaiting_macbeth_interior and provide_combat_autopilot() then return end
  if input_script_text and elapsed >= resume_elapsed then
    for _, action in ipairs(scripted_inputs) do
      if elapsed >= action.first and elapsed < action.last then
        input_label = "script-" .. action.label
        emu.setInput(action.buttons, 0)
        return
      end
    end
    input_label = "script-idle"
    emu.setInput({}, 0)
    return
  end
  if elapsed < 6800 then
    input_label = "front-end"
    emu.setInput({
      start = pulse(frame, 180, 120) and elapsed <= 600,
      b = elapsed >= 210 and elapsed < 6450 and pulse(elapsed, 90, 30),
      up = elapsed >= 6000 and elapsed < 6045,
      right = elapsed >= 6045 and elapsed < 6070,
    }, 0)
  elseif elapsed < 14480 then
    input_label = "laser"
    emu.setInput({ b = true }, 0)
  elseif continue_campaign and elapsed >= 21240 and elapsed < 21400
    and work_byte(0x1B68) == 7 then
    input_label = "third-target-up"
    emu.setInput({ up = true, b = pulse(elapsed, 90, 30) }, 0)
  elseif continue_campaign and elapsed >= 21400 and elapsed < 21440
    and work_byte(0x1B68) == 7 then
    input_label = "third-target-left"
    emu.setInput({ left = true, b = pulse(elapsed, 90, 30) }, 0)
  elseif continue_campaign and elapsed >= 21500 and elapsed < 21700
    and work_byte(0x1B68) == 7 then
    input_label = "third-target-right"
    emu.setInput({ right = true, b = pulse(elapsed, 90, 30) }, 0)
  elseif elapsed >= 14900 and work_byte(0x1B68) == 1 then
    input_label = "second-sortie-laser"
    emu.setInput({ b = true }, 0)
  elseif elapsed < 14560 then
    input_label = "release"
    emu.setInput({}, 0)
  elseif elapsed < 14720 then
    input_label = "up"
    emu.setInput({ up = true }, 0)
  elseif elapsed < 14760 then
    input_label = "left"
    emu.setInput({ left = true }, 0)
  else
    local accept = pulse(elapsed, 90, 30)
    input_label = accept and "accept" or "idle"
    emu.setInput({ b = accept }, 0)
  end
end

local function save_pending_state()
  if pending_savestate then
    local elapsed = frame - armed_frame
    write_file(
      string.format("sf2_post_sortie_%05d.mss", elapsed),
      emu.createSavestate())
    pending_savestate = false
    saved_state = true
  end
end

local function save_on_next_main_instruction()
  save_pending_state()
end

local function remove_save_callback()
  if save_callback_reference and saved_state then
    emu.removeMemoryCallback(
      save_callback_reference,
      emu.callbackType.exec,
      0x000000,
      0xFFFFFF,
      emu.cpuType.snes,
      emu.memType.snesMemory)
    save_callback_reference = nil
  end
end

local function load_resume_state()
  if loaded_state then return end
  loaded_state = true
  restoring_state = true
  emu.loadSavestate(resume_state)
  restoring_state = false
  if capture_loaded_state then
    capture_screen(resume_elapsed)
    capture_work(resume_elapsed)
    capture_ppu_state(resume_elapsed)
  end
end

local function remove_load_callback()
  if load_callback_reference and loaded_state then
    emu.removeMemoryCallback(
      load_callback_reference,
      emu.callbackType.exec,
      0x000000,
      0xFFFFFF,
      emu.cpuType.snes,
      emu.memType.snesMemory)
    load_callback_reference = nil
  end
end

local function arm_for_target_stream()
  save_pending_state()
  local source = emu.read16(0x0068, emu.memType.gsuWorkRam, false)
  local bank = emu.read16(0x006A, emu.memType.gsuWorkRam, false) & 0x7F
  if not armed and bank == 0x19 and source == 0x9F9C then
    armed = true
    armed_frame = frame
    record("armed", 0)
  end
end

local function end_frame()
  if not loaded_state then return end
  frame = frame + 1
  remove_save_callback()
  remove_load_callback()
  -- Restore the verification-only collision masks before recording or saving
  -- any state.  The masks exist for one retail map update only, so campaign
  -- actors retain their original identity, flags, counters, and positions.
  for actor, flags in pairs(temporarily_masked_pressure) do
    local mission_selection = work_byte(actor + 0x32)
    if mission_selection == 6 or mission_selection == 7 then
      write_work_word(actor + 0x2E, flags)
    end
  end
  temporarily_masked_pressure = {}
  if chase_map_once and map_chase_engaged
    and chased_map_actor and work_byte(0x1B68) ~= 7 then
    lines[#lines + 1] = string.format(
      "elapsed=%d event=map-chase-complete actor=%04X kind=%d",
      frame - armed_frame,
      chased_map_actor,
      chased_map_selection or 0)
    chased_map_actor = nil
    chased_map_selection = nil
    map_chase_engaged = false
  end
  if not armed then return end
  if forced_reserve_pilot_index then
    -- Oracle-only presentation enumeration. These two observed selection
    -- fields identify the reserve pilot; retail still loads the corresponding
    -- portrait and constructs every prompt layer itself.
    emu.write(0x1E15, forced_reserve_pilot_index, emu.memType.snesWorkRam)
    emu.write(0x1E70, forced_reserve_pilot_index, emu.memType.snesWorkRam)
  end
  if forced_corneria_damage then
    -- Oracle-only campaign continuation. These complementary retail fields
    -- were identified by observing the 89 -> 91 -> 100 damage sequence.
    write_work_word(0xDB47, 100 - forced_corneria_damage)
    write_work_word(0xDB49, forced_corneria_damage)
  end
  local elapsed = frame - armed_frame
  local shield_preservation_active = preserve_shields
    and (not preserve_shields_until_elapsed
      or elapsed <= preserve_shields_until_elapsed)
  if shield_preservation_active and work_byte(0x1B68) == 1 then
    -- Oracle-only survivability for long autonomous route capture.  Restore
    -- each pilot's current shield from the retail maximum; mission scripts,
    -- enemies, collision, scoring, and completion remain retail-controlled.
    emu.write(0x1DD1, work_byte(0x1DD5), emu.memType.snesWorkRam)
    emu.write(0x1DD7, work_byte(0x1DDB), emu.memType.snesWorkRam)
    -- The active craft's three rotating Super FX object buffers carry the
    -- collision-side copy consumed before the host refreshes its HUD mirror.
    -- Keep those oracle buffers at the same retail maximum as well.
    emu.write(0xB228, work_byte(0x1DD5), emu.memType.snesWorkRam)
    emu.write(0xB232, work_byte(0x1DD5), emu.memType.snesWorkRam)
    emu.write(0xB260, work_byte(0x1DD5), emu.memType.snesWorkRam)
  end
  if forced_player_health and work_byte(0x1B68) == 1 then
    -- Oracle-only outcome isolation. Keep every retail mirror of the active
    -- craft's shield at the requested low value, then let an ordinary hostile
    -- hit drive the game's own loss and post-sortie state machines.
    local player = work_word(0x12C3)
    if player ~= 0 and work_byte(player + 0x2D) > forced_player_health then
      emu.write(
        player + 0x2D,
        forced_player_health % 256,
        emu.memType.snesWorkRam)
      lines[#lines + 1] = string.format(
        "elapsed=%d event=player-health-clamped object=%04X health=%d",
        frame - armed_frame,
        player,
        forced_player_health)
    end
    if work_byte(0x1DD1) > forced_player_health then
      emu.write(0x1DD1, forced_player_health, emu.memType.snesWorkRam)
    end
    -- Keep the reserve pilot available so the isolated active-craft loss
    -- follows the ordinary campaign fallback instead of ending the run merely
    -- because an earlier long oracle capture exhausted the reserve shield.
    emu.write(0x1DD7, work_byte(0x1DDB), emu.memType.snesWorkRam)
    for _, address in ipairs({0xB228, 0xB232, 0xB260}) do
      if work_byte(address) > forced_player_health then
        emu.write(address, forced_player_health, emu.memType.snesWorkRam)
      end
    end
  end
  player_damage_oracle.force_impact()
  player_damage_oracle.observe_probe()
  player_damage_oracle.snapshot("snapshot")
  if forced_stage_selection and work_byte(0x1B68) == 7 then
    -- Oracle-only route isolation.  Mobile pressure encounters can otherwise
    -- preempt every command-map path while identifying a newly discovered
    -- strategic actor.  The selected retail mission still performs its own
    -- ordinary transition and initialization.
    emu.write(
      0x1BB5,
      forced_stage_selection % 256,
      emu.memType.snesWorkRam)
  end
  if forced_difficulty_selection and work_byte(0x1B68) == 7 then
    -- Oracle-only campaign isolation. The retail setup and mission loaders
    -- continue to consume their own difficulty field; this merely permits an
    -- existing command-map state to reach Hard/Expert-only planet variants.
    write_work_word(0xD7F2, forced_difficulty_selection)
  end
  if finish_strategic_threats and not finished_strategic_threats
    and work_byte(0x1B68) == 7 then
    -- Oracle-only campaign fast-forward.  Earlier route isolation could leave
    -- map attackers allocated after their independently verified combat
    -- sorties.  Retire the two retail gate totals and let the original state
    -- machine perform the Wolf/Astropolis unlock sequence itself.
    write_work_word(0xDA43, 0)
    write_work_word(0xDA53, 0)
    finished_strategic_threats = true
  end
  if forced_map_target_selection and work_byte(0x1B68) == 7 then
    -- Oracle-only strategic collision isolation.  Follow the retail map's
    -- actor list by semantic mission identifiers, then overlap PlayerTeam
    -- with the requested destination.  Retail still owns collision,
    -- transition, map selection, and mission initialization.
    local player_map_actor = nil
    local target_map_actor = nil
    for actor = 0xE0F9, 0xE3DB, 0x52 do
      local mission_selection = work_byte(actor + 0x32)
      if mission_selection == 9 then player_map_actor = actor end
      if mission_selection == forced_map_target_selection then
        target_map_actor = actor
      end
    end
    if player_map_actor and target_map_actor then
      for offset = 0x1A, 0x1E, 2 do
        write_work_word(
          player_map_actor + offset,
          work_word(target_map_actor + offset))
      end
    end

    player_map_actor = nil
    target_map_actor = nil
    local map_actor = work_word(0xDB6B)
    local seen_map_actors = {}
    while map_actor ~= 0 and not seen_map_actors[map_actor] do
      seen_map_actors[map_actor] = true
      local mission_selection = work_word(map_actor + 6)
      if mission_selection == 9 then player_map_actor = map_actor end
      if mission_selection == forced_map_target_selection then
        target_map_actor = map_actor
      end
      map_actor = work_word(map_actor + 2)
    end
    if player_map_actor and target_map_actor then
      local target_x = work_word(target_map_actor + 12)
      local target_y = work_word(target_map_actor + 14)
      write_work_word(player_map_actor + 12, target_x)
      write_work_word(player_map_actor + 14, target_y)
      write_work_word(player_map_actor + 16, target_x)
      write_work_word(player_map_actor + 18, target_y)
    end
  end
  if forced_occupied_selection and work_byte(0x1B68) == 7 then
    -- Oracle-only destination enumeration.  Apply the ordinary occupied-world
    -- status to the selected semantic planet so retail itself renders the
    -- destination label and owns any subsequent mission transition.
    local map_object = work_word(0xDB67)
    local seen_map_objects = {}
    while map_object ~= 0 and not seen_map_objects[map_object] do
      seen_map_objects[map_object] = true
      if work_word(map_object + 4) == forced_occupied_selection then
        local flags = work_word(map_object + 0x1C)
        write_work_word(map_object + 0x1C, (flags | 0x2800) & 0xEFFF)
        write_work_word(map_object + 0x20, 0x8000)
        break
      end
      map_object = work_word(map_object)
    end
  end
  if chased_map_selection and work_byte(0x1B68) == 7 then
    -- Oracle-only pursuit of one moving strategic actor.  Locking the
    -- collision-list entry avoids switching among formations of the same
    -- semantic kind while the retail map advances them independently.
    if chased_map_actor
      and work_byte(chased_map_actor + 0x32) ~= chased_map_selection then
      chased_map_actor = nil
    end
    if not chased_map_actor then
      chased_map_actor = closest_collision_actor(chased_map_selection)
      if chased_map_actor then
        lines[#lines + 1] = string.format(
          "elapsed=%d event=map-chase-lock actor=%04X kind=%d",
          frame - armed_frame,
          chased_map_actor,
          chased_map_selection)
      end
    end
    if chased_map_actor then
      map_chase_engaged = true
      local target_x = work_byte(chased_map_actor + 0x1C)
      local target_y = work_byte(chased_map_actor + 0x1F)
      write_work_word(0xDA90, target_x * 256 + work_byte(0xDA90))
      write_work_word(0xDA92, work_byte(0xDA93) * 256 + target_y)
    end
  end
  if forced_map_cursor_x and work_byte(0x1B68) == 7
    and work_byte(0x1BE0) == 10 then
    -- Oracle-only direct command-map placement used to enumerate selectable
    -- destinations without allowing intervening mobile threats to intercept.
    -- Retail stores the horizontal integer in the high byte of its fixed-point
    -- word and the vertical integer in the low byte of the following word.
    -- Preserve both fractional bytes so this helper changes only the semantic
    -- destination selected by the verification pilot.
    write_work_word(
      0xDA90,
      forced_map_cursor_x * 256 + work_byte(0xDA90))
    write_work_word(
      0xDA92,
      work_byte(0xDA93) * 256 + forced_map_cursor_y)
  end
  local elapsed = frame - armed_frame
  local key = state_key()
  if key ~= last_state then
    record("state", elapsed)
    last_state = key
  end
  if elapsed >= 14000 and elapsed % 120 == 0 then
    record("checkpoint", elapsed)
  end
  if elapsed >= 14900 and work_byte(0x1B68) == 1 and elapsed % sortie_stride == 0 then
    record("sortie", elapsed)
  end
  local capture_screen_only = capture_screen_range_contains(elapsed)
  if requested_captures[elapsed] or capture_screen_only
    or elapsed == 14400 or elapsed == 14460 or elapsed == 14468
    or elapsed == 14520 or elapsed == 14640 or elapsed == 14880
    or elapsed == 15120 or elapsed == 15600 or elapsed == 16560
    or elapsed == 18000 or elapsed == 20000 or elapsed == 23000
    or elapsed == 25000 then
    capture_screen(elapsed)
    if not capture_screen_only or requested_captures[elapsed] then
      capture_work(elapsed)
    end
    if capture_ppu and requested_captures[elapsed] then
      capture_ppu_state(elapsed)
    end
  end
  if save_elapsed and elapsed >= save_elapsed and not saved_state
    and not pending_savestate then
    pending_savestate = true
    save_callback_reference = emu.addMemoryCallback(
      save_on_next_main_instruction,
      emu.callbackType.exec,
      0x000000,
      0xFFFFFF,
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
  if elapsed >= stop_elapsed then
    write_file("sf2_post_sortie_trace.txt", table.concat(lines, "\n") .. "\n")
    if trace_craft_transition then
      write_file(
        "sf2_craft_transition_trace.txt",
        table.concat(craft_transition_lines, "\n") .. "\n")
    end
    if trace_audio_programs then
      write_file(
        "sf2_audio_program_trace.txt",
        table.concat(audio_program_lines, "\n") .. "\n")
    end
    if sortie_actor_oracle.enabled then
      write_file(
        "sf2_sortie_actor_logic.txt",
        table.concat(sortie_actor_oracle.lines, "\n") .. "\n")
    end
    if sortie_actor_oracle.projectiles_enabled then
      write_file(
        "sf2_sortie_projectile_logic.txt",
        table.concat(sortie_actor_oracle.projectile_lines, "\n") .. "\n")
    end
    if trace_walker_dynamics or trace_walker_writes then
      write_file(
        "sf2_walker_dynamics_trace.txt",
        table.concat(walker_dynamics_lines, "\n") .. "\n")
    end
    if player_damage_oracle.trace then
      write_file(
        "sf2_player_damage_trace.txt",
        table.concat(player_damage_oracle.lines, "\n") .. "\n")
    end
    if player_damage_oracle.probe then
      write_file(
        "sf2_player_damage_probe.txt",
        string.format(
          "offset=%d,%d,%d locked=%d initial_health=%s " ..
            "minimum_health=%s hit_elapsed=%s\n",
          player_damage_oracle.impact_offset_x,
          player_damage_oracle.impact_offset_y,
          player_damage_oracle.impact_offset_z,
          player_damage_oracle.locked_hits,
          tostring(player_damage_oracle.initial_health),
          tostring(player_damage_oracle.minimum_health),
          tostring(player_damage_oracle.hit_elapsed)))
    end
    if meteor_switch_oracle.enabled then
      write_file(
        "sf2_meteor_switch_trace.txt",
        table.concat(meteor_switch_oracle.lines, "\n") .. "\n")
    end
    emu.stop(0)
  end
end

local function isolate_map_layers()
  if forced_active_difficulty_selection and loaded_state
    and work_byte(0x1B68) == 1 then
    -- Oracle-only difficulty isolation for a mission savestate. This permits
    -- the immediately following retail phase loader to expose a difficulty
    -- variant without reproducing the already-verified campaign approach.
    write_work_word(0xD7F2, forced_active_difficulty_selection)
  end
  if force_fortuna_boss_gate and not force_fortuna_boss_gate_applied
    and loaded_state and work_byte(0x1B68) == 1
    and work_byte(0x1BB5) == 5 and work_byte(0x192E) == 0x05
    and work_word(0x1657) == 0x4D4B then
    -- Oracle-only phase isolation. The extracted retail map graph proves
    -- this persistent jump resumes at the immediately following command.
    -- Advancing only that gate permits retail to create and run the Fortuna
    -- interior encounter while the missing outer-stage release is recovered.
    write_work_word(0x1655, 0)
    write_work_word(0x1657, 0x4D4F)
    force_fortuna_boss_gate_applied = true
    lines[#lines + 1] = string.format(
      "elapsed=%d event=fortuna-boss-gate-released",
      frame - armed_frame)
  end
  if forced_base_destroyed_bits and not forced_base_destroyed_bits_applied
    and loaded_state and work_byte(0x1B68) == 1 then
    -- Oracle-only base-flow isolation. Retail base controllers consume this
    -- destruction bitfield; forcing it here proves the resulting retail map
    -- transition without leaking its storage representation into the port.
    write_work_word(0xD7F6, forced_base_destroyed_bits)
    forced_base_destroyed_bits_applied = true
    lines[#lines + 1] = string.format(
      "elapsed=%d event=base-destroyed-bits-forced bits=%04X",
      frame - armed_frame,
      forced_base_destroyed_bits)
  end
  if forced_base_handshake_bits and not forced_base_handshake_bits_applied
    and forced_base_destroyed_bits_applied and loaded_state
    and work_byte(0x1B68) == 1 then
    local controller_waiting = false
    local object = work_word(0x12A8)
    local seen = {}
    while object ~= 0 and not seen[object] do
      seen[object] = true
      if work_word(object + 4) == 0xD6A4
        and work_word(object + 0x2B) == 0x8B75 then
        controller_waiting = true
        break
      end
      object = work_word(object)
    end
    -- Oracle-only downstream isolation. The linked approach path normally
    -- acknowledges the completed surface controllers through this bitset.
    -- Apply the requested acknowledgement once, after those controllers have
    -- consumed the forced destruction state, then leave every transition and
    -- mission-phase write under retail control.
    if controller_waiting then
      emu.write(
        0xD78B,
        work_byte(0xD78B) | forced_base_handshake_bits,
        emu.memType.snesWorkRam)
      forced_base_handshake_bits_applied = true
      lines[#lines + 1] = string.format(
        "elapsed=%d event=base-handshake-bits-forced bits=%02X",
        frame - armed_frame,
        forced_base_handshake_bits)
    end
  end
  if forced_objective_remaining and not forced_objective_remaining_applied
    and loaded_state and work_byte(0x1B68) == 1 then
    -- Oracle-only mission-flow isolation. This changes the two mirrored
    -- retail objective counters once, then leaves all downstream host and map
    -- transitions under retail control.
    emu.write(
      0xD7A1,
      forced_objective_remaining,
      emu.memType.snesWorkRam)
    emu.write(
      0xD7F4,
      forced_objective_remaining,
      emu.memType.snesWorkRam)
    forced_objective_remaining_applied = true
    lines[#lines + 1] = string.format(
      "elapsed=%d event=objective-remaining-forced remaining=%d",
      frame - armed_frame,
      forced_objective_remaining)
  end
  if finish_each_mission and work_byte(0x1B68) == 7 then
    finished_current_mission = false
  end
  if repair_final_activation and not repaired_final_activation
    and work_byte(0x1B68) == 7 then
    -- Oracle-only repair for a campaign trace that previously retired mobile
    -- pressure actors by changing their flags.  That experiment also removed
    -- the dormant marker from Astropolis.  Restore only that marker and let
    -- the original Wolf-to-final-target state machine perform every unlock,
    -- identity, collision, and mission-initialization write itself.
    local collision_actor = work_word(0xE0A3)
    local seen_collision_actors = {}
    while collision_actor ~= 0
      and not seen_collision_actors[collision_actor] do
      seen_collision_actors[collision_actor] = true
      if work_byte(collision_actor + 0x32) == 11
        and (work_word(collision_actor + 0x2E) & 0x2000) ~= 0 then
        write_work_word(
          collision_actor + 0x30,
          work_word(collision_actor + 0x30) | 0x0400)
        repaired_final_activation = true
        break
      end
      collision_actor = work_word(collision_actor)
    end
  end
  if skip_surface_objectives and not skipped_surface_objectives
    and work_byte(0x1B68) == 1 and work_byte(0x1BB5) == 1 then
    -- Oracle-only fast-forward through Titania's two surface switches. Keep
    -- the reactor itself as the sole remaining retail objective so the base
    -- entrance and all subsequent mission logic remain under retail control.
    emu.write(0xD7A1, 1, emu.memType.snesWorkRam)
    emu.write(0xD7F4, 1, emu.memType.snesWorkRam)
    skipped_surface_objectives = true
  end
  if (finish_current_mission or finish_each_mission)
    and not finished_current_mission
    and work_byte(0x1B68) == 1 and work_byte(0xD7A1) <= 1 then
    -- Oracle-only campaign fast-forward.  This is used after independently
    -- observing a mission's retail objective sequence, solely to reach and
    -- inspect later strategic branches without coupling the native port to
    -- source-machine state.
    emu.write(0xD7A1, 0, emu.memType.snesWorkRam)
    emu.write(0xD7F4, 0, emu.memType.snesWorkRam)
    finished_current_mission = true
  end
  if ignore_pressure_encounters and work_byte(0x1B68) == 7 then
    for actor = 0xE0F9, 0xE3DB, 0x52 do
      local mission_selection = work_byte(actor + 0x32)
      if mission_selection == 6 or mission_selection == 7 then
        write_work_word(
          actor + 0x2E,
          work_word(actor + 0x2E) | 0x3000)
      end
    end
  end
  if avoid_pressure_encounters and work_byte(0x1B68) == 7 then
    -- Oracle-only route isolation that leaves no mutation in a captured
    -- campaign state.  Mobile attackers are non-colliding for this single
    -- map update, then end_frame restores their exact original flags.  This
    -- permits retail cursor selection of a planet while preserving all live
    -- attackers for later, ordinary sorties and retirement bookkeeping.
    local collision_actor = work_word(0xE0A3)
    local seen_collision_actors = {}
    while collision_actor ~= 0
      and not seen_collision_actors[collision_actor] do
      seen_collision_actors[collision_actor] = true
      local mission_selection = work_byte(collision_actor + 0x32)
      if mission_selection == 6 or mission_selection == 7 then
        local flags = work_word(collision_actor + 0x2E)
        temporarily_masked_pressure[collision_actor] = flags
        write_work_word(collision_actor + 0x2E, flags | 0x3000)
      end
      collision_actor = work_word(collision_actor)
    end
  end
  if (parked_map_team_x or evade_pressure) and work_byte(0x1B68) == 7 then
    -- Oracle-only strategic isolation. Hold only the player team's rendered
    -- and global position away from mobile enemies; all enemy actors retain
    -- their retail flags, motion, collision, and retirement behavior.
    local player_x, player_y
    if parked_map_team_x then
      player_x = parked_map_team_x
      player_y = parked_map_team_y
    else
      player_x, player_y = safest_pressure_evasion_position()
    end
    emu.write(0xDAF3, player_x, emu.memType.snesWorkRam)
    emu.write(0xDAF6, player_y, emu.memType.snesWorkRam)
  end
  if forced_map_target_selection and work_byte(0x1B68) == 7 then
    local collision_actor = work_word(0xE0A3)
    local seen_collision_actors = {}
    while collision_actor ~= 0
      and not seen_collision_actors[collision_actor] do
      seen_collision_actors[collision_actor] = true
      if work_byte(collision_actor + 0x32)
        == forced_map_target_selection then
        emu.write(
          0xDAF3,
          work_byte(collision_actor + 0x1C),
          emu.memType.snesWorkRam)
        emu.write(
          0xDAF6,
          work_byte(collision_actor + 0x1F),
          emu.memType.snesWorkRam)
        break
      end
      collision_actor = work_word(collision_actor)
    end

    overlap_player_team_with_map_actor(
      closest_collision_actor(forced_map_target_selection))
  end
  if chased_map_actor and work_byte(0x1B68) == 7 then
    -- Isolate the requested target for one retail collision update. Nearby
    -- mobile enemies are restored at end_frame before any trace or state is
    -- captured, so their identity and strategic motion remain authoritative.
    local collision_actor = work_word(0xE0A3)
    local seen_collision_actors = {}
    while collision_actor ~= 0
      and not seen_collision_actors[collision_actor] do
      seen_collision_actors[collision_actor] = true
      local mission_selection = work_byte(collision_actor + 0x32)
      if collision_actor ~= chased_map_actor
        and (mission_selection == 6 or mission_selection == 7) then
        local flags = work_word(collision_actor + 0x2E)
        temporarily_masked_pressure[collision_actor] = flags
        write_work_word(collision_actor + 0x2E, flags | 0x3000)
      end
      collision_actor = work_word(collision_actor)
    end
    overlap_player_team_with_map_actor(chased_map_actor)
  end
  if enable_final_target and work_byte(0x1B68) == 7 then
    -- Oracle-only campaign-gate bypass. The retail command map keeps the
    -- Astropolis collision actor in its ordinary linked list with its high
    -- inactive flags set until the strategic prerequisites are satisfied.
    -- Clearing only those flags lets the retail collision/mission dispatch
    -- choose the final target normally; no shipping-port state is involved.
    local collision_actor = work_word(0xE0A3)
    local seen_collision_actors = {}
    while collision_actor ~= 0
      and not seen_collision_actors[collision_actor] do
      seen_collision_actors[collision_actor] = true
      if work_byte(collision_actor + 0x32) == 11 then
        write_work_word(
          collision_actor + 0x2E,
          (work_word(collision_actor + 0x2E) & 0xDFFF) | 0x0008)
      end
      collision_actor = work_word(collision_actor)
    end
  end
  if not armed or not map_layer_mask then return end
  local elapsed = frame - armed_frame + 1
  if elapsed < 14520 or elapsed > 14900 then return end
  emu.write(0x212C, map_layer_mask, emu.memType.snesMemory)
  emu.write(0x212D, 0, emu.memType.snesMemory)
  emu.write(0x2131, 0, emu.memType.snesMemory)
end

emu.addMemoryCallback(
  arm_for_target_stream,
  emu.callbackType.exec,
  0x01D9FF,
  0x01D9FF,
  emu.cpuType.gsu,
  emu.memType.gsuMemory)
if resume_state_path then
  load_callback_reference = emu.addMemoryCallback(
    load_resume_state,
    emu.callbackType.exec,
    0x000000,
    0xFFFFFF,
    emu.cpuType.snes,
    emu.memType.snesMemory)
end
if evade_pressure then
  emu.addMemoryCallback(
    evade_pressure_on_map_entry,
    emu.callbackType.write,
    0x1B68,
    0x1B68,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    evade_forced_pressure_snap,
    emu.callbackType.exec,
    0x04B5B6,
    0x04B5B6,
    emu.cpuType.snes,
    emu.memType.snesMemory)
end
emu.addEventCallback(provide_input, emu.eventType.inputPolled)
emu.addEventCallback(isolate_map_layers, emu.eventType.startFrame)
emu.addEventCallback(end_frame, emu.eventType.endFrame)
function sortie_actor_oracle.register_callbacks()
  for _, service in ipairs({
    { "move", 0x7F9DDE },
    { "random-branch", 0x7F8EE5 },
    { "wait-for-angle", 0x7FA0C4 },
    { "wait", 0x7F84FB },
    { "random-value", 0x7F9A3C },
    { "chase-angle", 0x7FA1B5 },
    { "chase-word", 0x7FA209 },
    { "divide-angle", 0x7FA5BF },
    { "indexed-byte-step", 0x7FA690 },
    { "schedule", 0x7F97CA },
    { "face-player", 0x7F878B },
    { "fire", 0x7F885E },
    { "vertical-step", 0x7F8925 },
    -- Projectile-only detail probes. These bracket the semantic operations
    -- used by the first re-engagement laser path so its native Rust lift can
    -- retain the fixed-point behavior without retaining object-memory state.
    { "projectile-distance-test", 0x7F8BEB },
    { "projectile-orbit-pitch", 0x7FADC7 },
    { "projectile-orbit-pitch-complete", 0x7FB05F },
    { "projectile-face-immediate", 0x7F872C },
    { "projectile-face-immediate-complete", 0x7F8952 },
    { "projectile-face-smooth", 0x7F87CA },
    { "projectile-face-smooth-complete", 0x7F8A3C },
    { "projectile-set-speed", 0x7F854A },
    { "projectile-set-speed-complete", 0x7F875C },
  }) do
    emu.addMemoryCallback(
      sortie_actor_oracle.callback(service[1]),
      emu.callbackType.exec,
      service[2],
      service[2],
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
  for _, range in ipairs({
    { 0x0600, 0x0605 },
    { 0x0626, 0x062B },
    { 0x063F, 0x0644 },
    { 0x0665, 0x066A },
  }) do
    emu.addMemoryCallback(
      sortie_actor_oracle.record_gsu_capital_state_write,
      emu.callbackType.write,
      range[1],
      range[2],
      emu.cpuType.snes,
      emu.memType.gsuWorkRam)
    emu.addMemoryCallback(
      sortie_actor_oracle.record_main_capital_state_write,
      emu.callbackType.write,
      range[1],
      range[2],
      emu.cpuType.snes,
      emu.memType.snesWorkRam)
  end
  for _, memory_type in ipairs({
    emu.memType.gsuWorkRam,
    emu.memType.snesWorkRam,
  }) do
    emu.addMemoryCallback(
      sortie_actor_oracle.record_pitch_target_write,
      emu.callbackType.write,
      0x2297,
      0x2297,
      emu.cpuType.snes,
      memory_type)
    for _, range in ipairs({
      { 0x0584, 0x0585 },
      { 0x05C3, 0x05C4 },
    }) do
      emu.addMemoryCallback(
        sortie_actor_oracle.record_position_y_write,
        emu.callbackType.write,
        range[1],
        range[2],
        emu.cpuType.snes,
        memory_type)
    end
  end
  for object, _ in pairs(sortie_actor_oracle.objects) do
    emu.addMemoryCallback(
      sortie_actor_oracle.record_coprocessor_position_write,
      emu.callbackType.write,
      object + 12,
      object + 17,
      emu.cpuType.gsu,
      emu.memType.gsuWorkRam)
    for _, memory_type in ipairs({
      emu.memType.gsuWorkRam,
      emu.memType.snesWorkRam,
    }) do
      emu.addMemoryCallback(
        sortie_actor_oracle.record_main_position_write,
        emu.callbackType.write,
        object + 12,
        object + 17,
        emu.cpuType.snes,
        memory_type)
    end
  end
  for object, _ in pairs(explicit_trace_objects) do
    for _, range in ipairs({
      { object + 4, object + 5 },
      -- Rotation writes expose the semantic Achase/AddRotation work which can
      -- run after a segment's relative-position operation but before the next
      -- presentation frame. Shipping Rust retains only the recovered typed
      -- angle actions, never these source-machine addresses.
      { object + 0x12, object + 0x16 },
      { object + 0x20, object + 0x31 },
    }) do
      emu.addMemoryCallback(
        sortie_actor_oracle.record_explicit_object_state_write("main-work"),
        emu.callbackType.write,
        range[1],
        range[2],
        emu.cpuType.snes,
        emu.memType.snesWorkRam)
      emu.addMemoryCallback(
        sortie_actor_oracle.record_explicit_object_state_write("coprocessor-work"),
        emu.callbackType.write,
        range[1],
        range[2],
        emu.cpuType.gsu,
        emu.memType.gsuWorkRam)
    end
  end
  emu.addMemoryCallback(
    sortie_actor_oracle.record_main_random_state_write,
    emu.callbackType.write,
    0x00E0,
    0x00E3,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    sortie_actor_oracle.record_main_random_state_write,
    emu.callbackType.write,
    0x00E0,
    0x00E3,
    emu.cpuType.snes,
    emu.memType.gsuWorkRam)
  emu.addMemoryCallback(
    sortie_actor_oracle.record_gsu_random_state_write,
    emu.callbackType.write,
    0x00E0,
    0x00E3,
    emu.cpuType.gsu,
    emu.memType.gsuWorkRam)
end
if sortie_actor_oracle.enabled or sortie_actor_oracle.projectiles_enabled then
  sortie_actor_oracle.register_callbacks()
end
if trace_craft_transition then
  for service, address in pairs({
    walker_alternate = 0x07F7B3,
    walker_selected = 0x07F7BA,
    alternate_table_alternate = 0x07F7CB,
    alternate_table_selected = 0x07F7D2,
    craft_alternate = 0x07F7E3,
    craft_selected = 0x07F7EA,
  }) do
    emu.addMemoryCallback(
      craft_form_service(service),
      emu.callbackType.exec,
      address,
      address,
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
  emu.addMemoryCallback(
    record_craft_transition_write,
    emu.callbackType.write,
    0,
    0x3FFF,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if trace_walker_dynamics then
  emu.addMemoryCallback(
    walker_dynamics_stage("before"),
    emu.callbackType.exec,
    0x06AC05,
    0x06AC05,
    emu.cpuType.snes,
    emu.memType.snesMemory)
  emu.addMemoryCallback(
    walker_dynamics_stage("after"),
    emu.callbackType.exec,
    0x06AD5F,
    0x06AD5F,
    emu.cpuType.snes,
    emu.memType.snesMemory)
  for stage, address in pairs({
    integrate_begin = 0x0DB2CE,
    gravity_position_applied = 0x0DB2FC,
    transformed_position_applied = 0x0DB311,
    gravity_accumulated = 0x0DB321,
    vectors_scaled = 0x0DB351,
    ascent_transformed = 0x0DB524,
    collision_transform_complete = 0x0DB613,
    vectors_restored = 0x0DB670,
    terrain_begin = 0x06D935,
    terrain_base_applied = 0x06D93D,
    terrain_mode_applied = 0x06D95C,
    terrain_lower_slope_applied = 0x06D977,
    terrain_upper_slope_applied = 0x06D98C,
  }) do
    emu.addMemoryCallback(
      walker_motion_stage(stage),
      emu.callbackType.exec,
      address,
      address,
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
end
if trace_walker_writes then
  emu.addMemoryCallback(
    record_walker_write,
    emu.callbackType.write,
    0,
    0xFFFF,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if player_damage_oracle.trace then
  emu.addMemoryCallback(
    player_damage_oracle.record_write,
    emu.callbackType.write,
    0,
    0xFFFF,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if force_projectile_hit then
  emu.addMemoryCallback(
    keep_forced_projectile_on_target,
    emu.callbackType.write,
    0,
    0x3FFF,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if meteor_switch_oracle.enabled then
  for stage, address in pairs({
    quick_spawn_entry = 0x7F91A3,
    quick_spawn_probe_91CB = 0x7F91CB,
    quick_spawn_probe_91D1 = 0x7F91D1,
    quick_spawn_probe_9224 = 0x7F9224,
  }) do
    emu.addMemoryCallback(
      meteor_switch_oracle.record_stage(stage),
      emu.callbackType.exec,
      address,
      address,
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
  emu.addMemoryCallback(
    meteor_switch_oracle.record_shape_write,
    emu.callbackType.write,
    0x03BD,
    0x0FFF,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if force_meteor_core_trigger and forced_target_object then
  emu.addMemoryCallback(
    keep_forced_meteor_core_trigger,
    emu.callbackType.write,
    METEOR_CORE_TRIGGER_ADDRESS,
    METEOR_CORE_TRIGGER_ADDRESS,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if trace_stage_writes then
  emu.addMemoryCallback(
    trace_stage_write,
    emu.callbackType.write,
    0x1BB5,
    0x1BB7,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_stage_write,
    emu.callbackType.write,
    0xE097,
    0xE098,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if trace_audio_programs then
  -- Oracle-only hook at the retail audio-program dispatcher. Shipping code
  -- receives semantic cue names, never this program record or host state.
  emu.addMemoryCallback(
    trace_audio_program_entry,
    emu.callbackType.exec,
    0x03E1E5,
    0x03E1E5,
    emu.cpuType.snes,
    emu.memType.snesMemory)
end
if trace_final_gate then
  -- Oracle-only access trace.  The first range is the static Astropolis map
  -- record; the second contains the four retail objective counters and their
  -- remaining-objective totals.  Deduplication by instruction gives a small
  -- list of routines that decide whether to materialize the final collision
  -- actor, while the native port remains a typed flat state model.
  emu.addMemoryCallback(
    trace_final_gate_read,
    emu.callbackType.read,
    0xDDC3,
    0xDE18,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_final_gate_read,
    emu.callbackType.read,
    0xDA29,
    0xDA4F,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_final_gate_write,
    emu.callbackType.write,
    0xDA29,
    0xDA4F,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_objective_completion_execute,
    emu.callbackType.exec,
    0x04C6AF,
    0x04C6CD,
    emu.cpuType.snes,
    emu.memType.snesMemory)
end
if trace_final_activation then
  -- Oracle-only trace around the retail Wolf-to-Astropolis unlock.  The
  -- ranges cover the dynamic final-map actor and the small set of semantic
  -- gate/timer fields touched by the activation state machine.
  local activation_write_ranges = {
    { 0xE0A3, 0xE0F8 },
    { 0xD7F8, 0xD7FF },
    { 0x1B88, 0x1B96 },
    { 0xD9B0, 0xD9B1 },
    { 0xDA05, 0xDA06 },
    { 0xDA57, 0xDA58 },
    { 0xDA7F, 0xDA80 },
    { 0xDB37, 0xDB38 },
  }
  for _, range in ipairs(activation_write_ranges) do
    emu.addMemoryCallback(
      trace_final_activation_write,
      emu.callbackType.write,
      range[1],
      range[2],
      emu.cpuType.snes,
      emu.memType.snesWorkRam)
  end
  local activation_steps = {
    0x04C517,
    0x04C523,
    0x04C565,
    0x04C5AC,
    0x04C5FF,
    0x04C61B,
    0x04C633,
    0x04C672,
  }
  for _, address in ipairs(activation_steps) do
    emu.addMemoryCallback(
      trace_final_activation_execute,
      emu.callbackType.exec,
      address,
      address,
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
end
if trace_threat_retirement then
  -- The strategic-actor lifecycle helper is relocated into the retail
  -- runtime block. Trace its semantic entry and counter-decrement step so a
  -- campaign fixture can prove which map actor a completed sortie retired.
  local retirement_steps = { 0x7F6181, 0x7F6209 }
  for _, address in ipairs(retirement_steps) do
    emu.addMemoryCallback(
      trace_threat_retirement_execute,
      emu.callbackType.exec,
      address,
      address,
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
end
if traced_map_actor then
  -- Oracle-only per-field lifecycle trace for one strategic actor. This is
  -- intentionally expressed in source-machine terms here; generated native
  -- fixtures retain only the recovered semantic transitions.
  emu.addMemoryCallback(
    trace_map_actor_write,
    emu.callbackType.write,
    traced_map_actor,
    traced_map_actor + 0x3E,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
if trace_map_control then
  local map_control_ranges = {
    { 0x1655, 0x1658 },
    { 0x1B68, 0x1B68 },
    { 0x1B88, 0x1B96 },
    -- The stage-wide encounter state is copied by path 7BA0 into indexed
    -- weak-point latches.  Trace its writer so the native fixture records
    -- the semantic activation event instead of merely observing the copy.
    { 0x1BBB, 0x1BBB },
    { 0x1BE0, 0x1BF2 },
    { 0x1D72, 0x1D72 },
    { 0x1D77, 0x1D79 },
    -- Indexed path variable 47 is the retail base-controller handshake.
    { 0xD78B, 0xD78B },
    -- Indexed path variable 43 gates installation weak-point activation.
    -- Trace its producer as well as the consumers already visible in object
    -- path snapshots; oracle output keeps this source address out of Rust.
    { 0xD787, 0xD787 },
    -- Fortuna's installation controller counts retired defenses in its
    -- extended state.  This oracle-only address exposes the exact retail
    -- handshake after a legitimate projectile retirement.
    { 0x27C3, 0x27C3 },
    { 0xD7A1, 0xD7A1 },
    { 0xD7F4, 0xD7F7 },
    { 0xDA0F, 0xDA10 },
    { 0xDA43, 0xDA44 },
    { 0xDA57, 0xDA58 },
    { 0xDA7F, 0xDA86 },
    { 0xE087, 0xE0A5 },
  }
  for _, range in ipairs(map_control_ranges) do
    emu.addMemoryCallback(
      trace_map_control_write,
      emu.callbackType.write,
      range[1],
      range[2],
      emu.cpuType.snes,
      emu.memType.snesWorkRam)
  end
end
if trace_map_control_reads then
  -- Read tracing is opt-in because the map polls these fields every frame.
  -- Deduplication by source instruction exposes the retail state handlers
  -- without flooding the compact oracle trace.
  local map_control_read_ranges = {
    { 0x1655, 0x1658 },
    { 0x1BBB, 0x1BBB },
    { 0x1BF2, 0x1BF2 },
    { 0x1D72, 0x1D72 },
    { 0x1D77, 0x1D79 },
    { 0xD78B, 0xD78B },
    { 0xD787, 0xD787 },
    { 0x27C3, 0x27C3 },
    { 0xD7A1, 0xD7A1 },
    { 0xD7F4, 0xD7F7 },
    { 0xDA0F, 0xDA10 },
  }
  for _, range in ipairs(map_control_read_ranges) do
    emu.addMemoryCallback(
      trace_map_control_read,
      emu.callbackType.read,
      range[1],
      range[2],
      emu.cpuType.snes,
      emu.memType.snesWorkRam)
  end
end
if trace_astropolis_gate then
  -- Oracle-only instrumentation for the first Astropolis security junction.
  -- The native port consumes the recovered semantic event, never these
  -- source-machine locations or processor-state diagnostics.
  emu.addMemoryCallback(
    trace_astropolis_gate_global_write,
    emu.callbackType.write,
    0xD76F,
    0xD77F,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_astropolis_gate_global_write,
    emu.callbackType.write,
    0xD780,
    0xD78F,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_astropolis_gate_object_write,
    emu.callbackType.write,
    0x03BD,
    0x12A7,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_astropolis_mask_aux_write,
    emu.callbackType.write,
    0x2097,
    0x2F81,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  for _, address in ipairs({ 0x7F99A9, 0x7FBC80 }) do
    emu.addMemoryCallback(
      trace_astropolis_gate_execute,
      emu.callbackType.exec,
      address,
      address,
      emu.cpuType.snes,
      emu.memType.snesMemory)
  end
end
if trace_map_motion then
  emu.addMemoryCallback(
    trace_map_write,
    emu.callbackType.write,
    0xDA90,
    0xDA93,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_map_write,
    emu.callbackType.write,
    0xDAF3,
    0xDAF3,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
  emu.addMemoryCallback(
    trace_map_write,
    emu.callbackType.write,
    0xDAF6,
    0xDAF6,
    emu.cpuType.snes,
    emu.memType.snesWorkRam)
end
