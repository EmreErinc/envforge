-- AI-exposure heatmap in the sign column (Story 4.2 / FR20). Renders one
-- red/amber/green sign per env-var line, plus a shield glyph for canary
-- tripwires. Data comes from `envforge exposure <file>` (same classification
-- the LSP serves — byte-identical, parity per ide-behavior-contract.md).

local M = {}
local SIGN_GROUP = "envforge_exposure"

local function define_signs()
  vim.fn.sign_define("envforge_red", { text = "●", texthl = "DiagnosticError" })
  vim.fn.sign_define("envforge_amber", { text = "●", texthl = "DiagnosticWarn" })
  vim.fn.sign_define("envforge_green", { text = "●", texthl = "DiagnosticOk" })
  -- Canary tripwire: shield glyph, keeps the threat-tier color via hl.
  vim.fn.sign_define("envforge_canary_red", { text = "", texthl = "DiagnosticError" })
  vim.fn.sign_define("envforge_canary_amber", { text = "", texthl = "DiagnosticWarn" })
  vim.fn.sign_define("envforge_canary_green", { text = "", texthl = "DiagnosticOk" })
end

local function sign_name(level, canary)
  local lvl = (level == "red" or level == "amber" or level == "green") and level or "red"
  return canary and ("envforge_canary_" .. lvl) or ("envforge_" .. lvl)
end

local function render(bin, bufnr)
  if not vim.api.nvim_buf_is_loaded(bufnr) then return end
  local name = vim.api.nvim_buf_get_name(bufnr)
  if name == "" then return end
  vim.system({ bin, "exposure", name }, { text = true }, function(res)
    vim.schedule(function()
      if not vim.api.nvim_buf_is_loaded(bufnr) then return end
      vim.fn.sign_unplace(SIGN_GROUP, { buffer = bufnr })
      if res.code ~= 0 or not res.stdout or #res.stdout == 0 then return end
      local ok, parsed = pcall(vim.json.decode, res.stdout)
      if not ok or type(parsed) ~= "table" or type(parsed.entries) ~= "table" then return end
      for _, e in ipairs(parsed.entries) do
        -- CLI lines are 0-based; nvim signs are 1-based.
        local lnum = (tonumber(e.line) or 0) + 1
        vim.fn.sign_place(0, SIGN_GROUP, sign_name(e.level, e.canary == true), bufnr,
          { lnum = lnum, priority = 10 })
      end
    end)
  end)
end

function M.setup(bin, grp)
  define_signs()
  vim.api.nvim_create_autocmd({ "BufReadPost", "BufWritePost" }, {
    group = grp,
    pattern = { ".env", ".env.*", "*.env" },
    callback = function(args) render(bin, args.buf) end,
  })
  -- Clear signs when leaving / on non-env buffers handled implicitly (signs
  -- are per-buffer and only placed on .env* matches above).
  M._render = render
end

return M
