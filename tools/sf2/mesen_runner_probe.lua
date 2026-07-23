local stopped = false

local function stop(reason, code)
  if stopped then
    return
  end
  stopped = true
  emu.stop(code)
  if io and io.open then
    local file = io.open(emu.getScriptDataFolder() .. "/runner_probe.txt", "wb")
    if file then
      file:write(reason .. "\n")
      file:close()
    end
  end
end

emu.addMemoryCallback(
  function(address)
    stop(string.format("exec:%06X", address), 8)
  end,
  emu.callbackType.exec,
  0,
  16777215,
  emu.cpuType.snes,
  emu.memType.snesMemory
)

emu.addEventCallback(function()
  stop("end-frame", 7)
end, emu.eventType.endFrame)
