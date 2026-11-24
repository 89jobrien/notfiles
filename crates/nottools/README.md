# nottools

Tool and alias reconciliation for notfiles: declared tools versus what is
installed.

Reads the tool manifests a dotfiles repo keeps (a mise config and
Brewfiles), scans shell config files for aliases, and reports:

- alias targets that do not resolve on `PATH`, with the install command
  when the tool is declared in mise or a Brewfile
- `cargo install` crates that no manifest declares, split by registry,
  git, and path source

## API

- `run(cfg, dotfiles_dir, home, fs, probe) -> Result<DoctorReport>` — the
  check. `DoctorReport { missing: Vec<MissingTool>, undeclared, git,
local }`; `has_failures` (missing alias targets); `render`.
- `apply_fixes(report, installer, dry_run) -> Vec<(FixAction, bool)>` —
  `FixAction::{MiseInstall, BrewInstall { formula }, Adopt { name,
version }}`.
- `DoctorConfig` — the `[doctor]` table of `notfiles.toml` (`mise`,
  `brewfiles`, `alias_files`).

## Modules

- `config` — `DoctorConfig`, TOML parsing
- `declared` — `mise_tools` / `brew_names` parsers, `tool_name`
- `aliases` — `parse_alias` (nu/zsh/bash/fish), `is_checkable`, `Scan`
- `doctor` — `run`, `DoctorReport`, `MissingTool`, `Backend`
- `fix` — `FixAction`, `apply_fixes`, `adopt` (plan a
  `cargo:<name> = <version>` declaration for an installed crate no
  manifest declares)
- `ports` — `Fs`, `ToolProbe`, `Installer`, `Source`, `InstalledCrate`
- `adapters` — `SystemFs`, `SystemProbe`, `SystemInstaller`

## Testing

`cargo test -p nottools`
