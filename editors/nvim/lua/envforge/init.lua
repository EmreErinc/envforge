-- EnvForge first-party Neovim plugin (Story 4.1 / FR20, FR22).
--
-- Thin LSP client + native UI on top of `envforge lsp` / the CLI. Contains NO
-- business logic — all decisions come from the EnvForge binary (parity, FR22).
-- Requires Neovim 0.10+ (vim.system, vim.fs.root) and `envforge` on PATH.

local M = {}

-- NOTE: EnvForge does NOT attach to whole source languages (typescript,
-- python, rust, java, …) or to shared config filetypes. Attaching the LSP to
-- every source buffer to provide one niche feature (goto from an env-var read
-- back to .env.schema) is broad and degrades the editor; the feature is
-- intentionally dropped. The LSP attaches only to EnvForge's own files: dotenv.
local default_filetypes = {
  "dotenv",
}

--- Setup the plugin. Call once from your config: require("envforge").setup()
--- opts: { bin = "envforge", filetypes = {...} }
function M.setup(opts)
  opts = opts or {}
  local bin = opts.bin or "envforge"
  M.bin = bin

  -- .env* filetype detection → `dotenv`.
  vim.filetype.add({
    filename = { [".env"] = "dotenv" },
    pattern = { ["%.env%..*"] = "dotenv", [".*%.env"] = "dotenv" },
  })

  local grp = vim.api.nvim_create_augroup("EnvForge", { clear = true })

  -- Auto-start the LSP for EnvForge's own files (dotenv).
  vim.api.nvim_create_autocmd("FileType", {
    group = grp,
    pattern = opts.filetypes or default_filetypes,
    callback = function(args)
      local root = vim.fs.root(args.buf, {
        ".env.schema.toml", ".env.schema", ".env", ".git",
      }) or vim.fn.getcwd()
      vim.lsp.start({ name = "envforge", cmd = { bin, "lsp" }, root_dir = root },
        { bufnr = args.buf })
    end,
  })

  -- Commands.
  vim.api.nvim_create_user_command("EnvForgeFence", function()
    require("envforge.fence").enable(bin)
  end, { desc = "Fence all detected AI tools" })
  vim.api.nvim_create_user_command("EnvForgeFenceToggle", function()
    require("envforge.fence").toggle(bin)
  end, { desc = "Toggle the EnvForge AI fence" })
  vim.api.nvim_create_user_command("EnvForgeStatus", function()
    require("envforge.status").show(bin)
  end, { desc = "Show fence status" })

  require("envforge.status").setup(bin)
  require("envforge.exposure").setup(bin, grp)
end

--- Statusline component: returns e.g. "12 vars · AI BLOCKED".
--- Use in lualine/heirline, or: vim.o.statusline = "%{v:lua.require'envforge'.statusline()}"
function M.statusline()
  return require("envforge.status").line()
end

return M
