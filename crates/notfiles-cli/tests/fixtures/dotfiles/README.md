# Test Fixture — Dotfiles

This directory is a test fixture representing a realistic dotfiles repo.
It is used by notfiles integration tests and serves as an example of the
expected directory structure.

## Package directories (linked to ~)

- `git/` — `.gitconfig` at root
- `zsh/` — `.zshrc` at root
- `nushell/` — `.config/nushell/` tree
- `starship/` — `.config/starship.toml`
- `alacritty/` — `.config/alacritty/alacritty.toml`
- `mise/` — `.config/mise/config.toml`
- `fish/` — `.config/fish/config.fish`
- `nixos/` — platform-gated to Linux only
- `vscode/` — deep `Library/Application Support/` path (macOS)

## Non-package directories (excluded via include list)

- `scripts/` — shell scripts, not a stow target
- `docs/` — documentation
- `secrets/` — encrypted env files, copy-only with ignore
