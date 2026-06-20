-- Fence enable/toggle commands (Story 4.1). Shells out to the CLI; the CLI
-- owns all fence logic (FR22 — no logic here).

local M = {}

local function notify(res)
  vim.schedule(function()
    local level = res.code == 0 and vim.log.levels.INFO or vim.log.levels.ERROR
    vim.notify(res.stdout ~= "" and res.stdout or (res.stderr or "envforge fence"), level)
    require("envforge.status").refresh()
  end)
end

function M.enable(bin)
  vim.system({ bin, "fence" }, { text = true }, notify)
end

function M.disable(bin)
  vim.system({ bin, "fence", "--disable" }, { text = true }, notify)
end

--- Toggle: read current state, then flip. Confirms before disabling (disable
--- strips EnvForge-owned content, though user content is preserved).
function M.toggle(bin)
  vim.system({ bin, "fence", "--status", "--json" }, { text = true }, function(res)
    local blocked = false
    if res.code == 0 and res.stdout and #res.stdout > 0 then
      local ok, parsed = pcall(vim.json.decode, res.stdout)
      if ok and type(parsed) == "table" then blocked = parsed.all_fenced == true end
    end
    vim.schedule(function()
      if blocked then
        local ans = vim.fn.confirm("Disable EnvForge fence? (user content preserved)", "&Yes\n&No", 2)
        if ans == 1 then M.disable(bin) end
      else
        M.enable(bin)
      end
    end)
  end)
end

return M
