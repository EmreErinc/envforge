-- EnvForge Neovim Welcome & CLI Installer Helper
local M = {}

local tips = {
  {
    desc = "You can run health checks to verify your environment setup and secret providers:",
    code = "envforge doctor"
  },
  {
    desc = "Fence writes ignore/rules for configured AI tools so they are less likely to ingest .env files. Not a sandbox:",
    code = "envforge fence"
  },
  {
    desc = "You can run commands with volatile secret access (secrets kept in memory only, never written to disk):",
    code = "envforge run --volatile -- npm start"
  },
  {
    desc = "Scan MCP config files for hardcoded credentials:",
    code = "envforge mcp status"
  },
  {
    desc = "Scan git commit history to audit and detect AI-assisted secret leaks across your repository:",
    code = "envforge audit --ai-leaks"
  },
  {
    desc = "Create honeypot canary credentials in your environment files to detect secret exfiltration:",
    code = "envforge canary create DB_CANARY_KEY"
  },
  {
    desc = "Auto-generate a type-safe .env.schema from your existing environment variables:",
    code = "envforge schema generate"
  },
  {
    desc = "Switch between development, staging, and production environment profiles instantly:",
    code = "envforge profile switch production"
  },
  {
    desc = "Redact secrets automatically in subprocess logs during command execution:",
    code = "envforge run --redact -- ./deploy.sh"
  }
}

function M.get_random_tip()
  math.randomseed(os.time())
  return tips[math.random(#tips)]
end

function M.show_welcome()
  local tip = M.get_random_tip()
  local is_mac = vim.fn.has("mac") == 1 or vim.fn.has("macunix") == 1
  local install_lines = is_mac and {
    "Get started via Homebrew or Cargo:",
    "  brew install emreerinc/tap/envforge",
    "  cargo install env-forge-tui",
  } or {
    "Get started by running in your terminal:",
    "  cargo install env-forge-tui",
  }

  local lines = {
    "==================================================",
    "             ENVFORGE: WELCOME",
    "==================================================",
    "",
    "Welcome to EnvForge!",
    "Profiles, .env.schema, and a TUI — this plugin adds LSP, signs, and fence commands.",
    "",
  }
  for _, l in ipairs(install_lines) do
    table.insert(lines, l)
  end
  table.insert(lines, "")
  table.insert(lines, "--------------------------------------------------")
  table.insert(lines, "Did you know?")
  table.insert(lines, tip.desc)
  table.insert(lines, "  Command: " .. tip.code)
  table.insert(lines, "--------------------------------------------------")
  table.insert(lines, "")
  table.insert(lines, "Available Neovim Commands:")
  table.insert(lines, "  :EnvForgeInstall  - Open terminal & install CLI")
  table.insert(lines, "  :EnvForgeTips     - Show another random CLI tip")
  table.insert(lines, "")
  table.insert(lines, "Press <Esc> or q to close this window.")
  }

  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].filetype = "envforge-welcome"
  vim.bo[buf].modifiable = false

  local width = 56
  local height = #lines + 2
  local win_width = vim.o.columns
  local win_height = vim.o.lines

  local row = math.floor((win_height - height) / 2)
  local col = math.floor((win_width - width) / 2)

  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = width,
    height = height,
    row = math.max(0, row),
    col = math.max(0, col),
    style = "minimal",
    border = "rounded",
    title = " EnvForge Welcome ",
    title_pos = "center",
  })

  vim.keymap.set("n", "q", function() vim.api.nvim_win_close(win, true) end, { buffer = buf, silent = true })
  vim.keymap.set("n", "<Esc>", function() vim.api.nvim_win_close(win, true) end, { buffer = buf, silent = true })
end

function M.show_tip()
  local tip = M.get_random_tip()
  local msg = "Did you know?\n" .. tip.desc .. "\nCommand: " .. tip.code
  vim.notify(msg, vim.log.levels.INFO, { title = "EnvForge Tip" })
end

function M.install_cli()
  local cmd = "cargo install env-forge-tui"
  vim.fn.setreg("+", cmd)
  vim.fn.setreg('"', cmd)
  
  if vim.fn.has("terminal") == 1 or vim.fn.exists(":terminal") == 2 then
    vim.cmd("split | terminal " .. cmd)
    vim.notify("Started 'cargo install env-forge-tui' in terminal.", vim.log.levels.INFO, { title = "EnvForge Installer" })
  else
    vim.notify("Copied '" .. cmd .. "' to clipboard!", vim.log.levels.INFO, { title = "EnvForge Installer" })
  end
end

return M
