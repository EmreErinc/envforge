# EnvForge convenience aliases — source from your shell rc:
#   echo "source $HOME/projects/env-forge/scripts/envforge-aliases.sh" >> ~/.zshrc
# Then: source ~/.zshrc

# short alias
alias ef='envforge'

# rebuild + reinstall the CLI and all detected IDE plugins from the repo
alias ef-update='( cd "$HOME/projects/env-forge" && ./scripts/envforge-install.sh )'

# common day-to-day shortcuts (only env-affecting reads/checks)
alias ef-doctor='envforge doctor'
alias ef-ls='envforge list'
alias ef-version='envforge --version'
