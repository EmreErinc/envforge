-- Fence status for the statusline (Story 4.1). Mirrors the VS Code / IntelliJ
-- status item: "<N> vars · AI BLOCKED|AI ALLOWED". Data from the CLI only.

local M = {}

-- Cached statusline string, refreshed off a timer so the statusline render
-- (called very frequently) never blocks on a subprocess.
local cached = ""
local bin = "envforge"

local function refresh()
  -- Fence state (honest aggregate from Story 1.6).
  vim.system({ bin, "fence", "--status", "--json" }, { text = true }, function(res)
    local blocked = nil
    if res.code == 0 and res.stdout and #res.stdout > 0 then
      local ok, parsed = pcall(vim.json.decode, res.stdout)
      if ok and type(parsed) == "table" then
        blocked = parsed.all_fenced == true
      end
    end
    -- Var count (best-effort; keys only, never values).
    vim.system({ bin, "list", "--keys-only" }, { text = true }, function(lr)
      local n = 0
      if lr.code == 0 and lr.stdout then
        for _ in lr.stdout:gmatch("[^\r\n]+") do n = n + 1 end
      end
      local state
      if blocked == nil then
        state = ""
      elseif blocked then
        state = " · AI BLOCKED"
      else
        state = " · AI ALLOWED"
      end
      cached = string.format("%d vars%s", n, state)
      vim.schedule(function() vim.cmd("redrawstatus") end)
    end)
  end)
end

function M.setup(b)
  bin = b or "envforge"
  refresh()
  -- Refresh every 30s (matches the VS Code slow timer) + on fence changes.
  local timer = vim.uv.new_timer()
  timer:start(30000, 30000, vim.schedule_wrap(refresh))
  M._refresh = refresh
end

--- Current cached statusline string.
function M.line()
  return cached
end

--- Force a refresh now (call after a fence toggle).
function M.refresh()
  refresh()
end

--- Print full status to the user.
function M.show(b)
  vim.system({ b or bin, "fence", "--status" }, { text = true }, function(res)
    vim.schedule(function()
      vim.notify(res.stdout or res.stderr or "envforge: no output",
        res.code == 0 and vim.log.levels.INFO or vim.log.levels.WARN)
    end)
  end)
end

return M
