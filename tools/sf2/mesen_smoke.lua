-- Minimal Mesen 2 test-runner health check. Stop on the first emulated frame;
-- calling emu.stop while the test runner is still paused can be overwritten
-- by its unconditional resume after all scripts load.
emu.log("STARFOX_HD_MESEN_SMOKE")
print("STARFOX_HD_MESEN_PRINT")
print("STARFOX_HD_MESEN_DATA=" .. emu.getScriptDataFolder())
emu.addEventCallback(function()
  emu.stop(0)
end, emu.eventType.startFrame)
