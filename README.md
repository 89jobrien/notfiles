# notfiles

A pure-Rust dotfiles manager — and eventually, a complete new-machine bootstrap system.

`notfiles` started as a Rust replacement for [GNU Stow](https://www.gnu.org/software/stow/). It's growing into a **Cargo workspace** of focused crates that together replace an entire shell-script-based dotfiles ecosystem.

---

## Quick Start

```bash
# Symlink your dotfiles
notfiles link

# Check status
notfiles status

# Validate config without changes (CI-friendly)
notfiles check

# Show copy-method divergence
notfiles diff

# Move an existing file into a package
notfiles adopt git .gitconfig

# Find the source package behind a managed path
notfiles which ~/.gitconfig

# Remove symlinks
notfiles unlink

# Generate shell completions
notfiles completions bash > ~/.bash_completion.d/notfiles
```

All commands support `--dry-run`, `--verbose`, and `--json` flags.

On a new machine (once `notstrap` ships):

```bash
cargo install notstrap
notstrap run
```

---

## Workspace Architecture

```
notfiles/
├── crates/
│   ├── notcore/        # shared types, config, paths, errors, Reporter trait
│   ├── notfiles/       # symlink engine (stow replacement)  ← you are here
│   ├── notsecrets/     # multi-provider secret resolution + age crypto
│   ├── nothooks/       # hook execution engine
│   ├── notnet/         # network utilities (Tailscale, Yubikey)
│   ├── notstrap/       # new-machine bootstrap orchestrator
│   └── notgraph/       # Rust dependency graph visualizer
├── notfiles.toml       # symlink package config
└── notstrap.toml       # bootstrap hook/phase config
```

### Crate responsibilities

| Crate | Type | Does |
|-------|------|------|
| `notcore` | lib | Shared types, config, paths, errors, Reporter trait — no deps on other crates |
| `notfiles` | lib + bin | Symlink/copy packages into `$HOME`, track state, adopt/diff/check |
| `notsecrets` | lib | Multi-provider secret resolution, native age encryption/decryption |
| `nothooks` | lib + bin | Run bootstrap hooks in phases, skip already-run setup hooks |
| `notnet` | lib | Tailscale integration, Yubikey identity source |
| `notstrap` | bin | Orchestrate everything on a fresh machine |
| `notgraph` | lib + bin | Rust file dependency/import graph with HTML/Mermaid output |

### Dependency graph

```
notstrap
  ├── notfiles
  │     └── notcore
  ├── notsecrets
  │     └── notcore
  ├── notnet
  └── nothooks
        └── notcore
```

`notcore` is the only shared dependency. No circular deps.

---

## How `notfiles` Works

Each subdirectory of your dotfiles repo is a **package**. `notfiles link` walks each package and symlinks its contents into a target directory (default: `$HOME`), mirroring the directory structure.

```
dotfiles/
└── zsh/
    └── .zshrc          →  symlink  →  ~/.zshrc

dotfiles/
└── git/
    └── .config/
        └── git/
            └── config  →  symlink  →  ~/.config/git/config
```

State is tracked in `.notfiles-state.toml` so `unlink` and `status` know exactly what was linked, when, and how.

### Link flow

```
notfiles link
  │
  ├─ config.validate()                check include/exclude mutual exclusion
  │
  ├─ resolve_packages_filtered()      include/exclude + platform filtering
  │
  ├─ collect_files()                  recursive walk, apply ignore patterns
  │
  ├─ conflict_check()                 existing file? symlink to wrong target?
  │
  └─ linker::link_package()           create symlinks (or copies), write state,
                                      return LinkResult with counters
```

### State file

`.notfiles-state.toml` records every linked file:

```toml
[[entries]]
source = "/Users/joe/dotfiles/zsh/.zshrc"
target = "/Users/joe/.zshrc"
method = "symlink"
package = "zsh"
linked_at = "2026-03-31T10:00:00Z"
```

This powers `status` (diff expected vs actual) and `unlink` (clean removal with empty-parent cleanup).

---

## New Machine Bootstrap (notstrap)

The hardest part of a new machine is the chicken-and-egg problem: you need secrets to set up the machine, but secrets live in an encrypted file that requires a key you haven't retrieved yet.

`notstrap` solves this with a staged bootstrap:

```
notstrap run
  │
  ├─ 1. Prerequisites check
  │      Is bw/sops/age available? Print exactly what's missing and stop.
  │
  ├─ 2. notsecrets — retrieve age key
  │      ├─ try: Bitwarden CLI (bw unlock)
  │      ├─ fallback: --key-file <path>  (USB drive)
  │      └─ fallback: interactive prompt (paste key)
  │          └─ sops decrypt secrets.sops.env → env injected
  │             (now op, bw, github, openai, anthropic tokens are live)
  │
  ├─ 3. Clone dotfiles repo (if not present)
  │
  ├─ 4. notfiles link — stow all packages
  │
  ├─ 5. nothooks --phase dot
  │      shell config, git config, AI tool configs (~seconds, re-runnable)
  │
  ├─ 6. nothooks --phase setup
  │      Homebrew/Nix packages, mise runtimes, dev tools, op install (~minutes, once)
  │
  └─ 7. Report
         ✓ linked 142 files  ✓ 3 dot hooks  ✓ 7 setup hooks
```

Note: 1Password (`op`) is installed as a **hook** in phase `setup` — after secrets are already available via sops. It takes over secret management for day-to-day use once the machine is live.

---

## Secrets Bootstrap Detail

`notsecrets` implements a `SecretResolver` with pluggable provider sources:

```
SecretResolver
  ├── EnvSource         read from environment variables
  ├── OpSource          1Password CLI (op read)
  ├── BitwardenSource   bw CLI
  ├── FileSource        read from file path
  ├── DotenvxSource     dotenvx encrypted .env files
  ├── SopsSource        SOPS-encrypted files (native age decryption)
  └── ...               (extensible via SecretSource trait)
```

Age encryption/decryption is handled natively (no external `age` or `sops`
binaries). Supports x25519, SSH ed25519/RSA, and scrypt identities.

---

## Hook Phases

`nothooks` runs hooks in two phases defined in `notstrap.toml`:

| Phase | Speed | Re-runnable | Examples |
|-------|-------|-------------|---------|
| `dot` | ~seconds | Yes | shell config, git config, AI tool configs |
| `setup` | ~minutes | No (tracked) | Homebrew packages, mise runtimes, op install |

`setup` hooks are tracked in `.nothooks-state.toml` — already-run hooks are skipped unless `--force` is passed. `dot` hooks always re-run (they're idempotent by design).

---

## Configuration

### `notfiles.toml` — symlink config

```toml
[defaults]
method = "symlink"
target = "~"
include = ["git", "zsh", "nushell", "starship"]  # allowlist (optional)
# exclude = ["scratch"]                           # or blocklist (mutually exclusive)

[packages.secrets]
method = "copy"    # copy instead of symlink for sensitive files

[packages.nixos]
platforms = ["linux"]  # only link on Linux

[packages.work]
target = "~/work"  # different target dir
ignore = ["*.local"]
```

### `notstrap.toml` — bootstrap config

```toml
[bootstrap]
dotfiles_repo = "git@github.com:you/dotfiles.git"
dotfiles_dir = "~/dotfiles"

[[hooks]]
name = "shell-config"
script = "scripts/setup-git-config.sh"
phase = "dot"

[[hooks]]
name = "homebrew-packages"
script = "scripts/setup-packages.sh"
phase = "setup"
```

---

## Migration from dotfiles/

Migration is gradual — shell scripts are replaced as Rust equivalents ship:

| Phase | Ships | Replaces |
|-------|-------|---------|
| 1 | `notfiles` workspace (now) | GNU Stow |
| 2 | `notsecrets` + `notstrap` skeleton | `setup-secrets.sh`, `install.sh` |
| 3 | `nothooks` | `bootstrap.sh` hook runner |
| 4 | Hook-by-hook Rust rewrites | Individual `setup-*.sh` scripts |
| 5 | `dotfiles/` archived | — |

---

## Absorbing `pj`

`pj` (89jobrien/pj) was a standalone portable dev-environment bootstrap CLI. Its functionality overlaps directly with this workspace:

| `pj` command | `notfiles` equivalent |
|---|---|
| `pj dot` | `notfiles` (symlink engine) |
| `pj doctor` | `notstrap` prerequisites check |
| `pj sync` | `notstrap run` |
| `pj secret` | `notsecrets` |
| `pj cache` | planned `notcache` or Cargo workspace tooling |
| `pj tui` | planned TUI layer on `notstrap` |

`pj` is archived. Its remaining functionality (cache management, TUI dashboard, git-config helper) will be folded in as those areas of the workspace mature.

---

## Development

```bash
cargo build                      # build all crates
cargo test                       # run all tests
cargo test -p notfiles           # test one crate
cargo clippy --workspace         # lint everything
cargo fmt --check                # format check
```

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
