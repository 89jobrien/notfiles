# nothooks

Nushell hook runner for the notfiles workspace. Executes `.nu` scripts in two
distinct lifecycle phases — **dot** (always) and **setup** (once) — with
state persistence so setup hooks are not re-run across invocations.

## Concepts

### HookPhase

Each hook belongs to exactly one phase:

| Phase | When it runs | Typical use |
|-------|-------------|-------------|
| `dot` | Every time nothooks is invoked | Apply shell config, set env vars, refresh symlinks |
| `setup` | Once per machine (tracked in state) | Install packages, create dirs, first-time configuration |

### HookSpec

A hook is described by three fields (defined in `notcore`):

```toml
[[hooks]]
name   = "install-starship"
script = "hooks/setup/install-starship.nu"
phase  = "setup"

[[hooks]]
name   = "set-default-shell"
script = "hooks/dot/shell.nu"
phase  = "dot"
```

### State tracking

Setup hooks are tracked in `.nothooks-state.toml` in the state directory
(typically the dotfiles root). Once a setup hook completes successfully its
name is recorded and it will be **skipped** on all future runs.

```toml
# .nothooks-state.toml (auto-generated, do not edit manually)
completed_setup_hooks = ["install-starship", "configure-git"]
```

Dot hooks are **never** recorded in state — they run unconditionally on every
invocation.

### --force re-run

To re-run setup hooks (e.g. after a machine rebuild or to apply changes),
construct the runner with `HookRunner::with_force`. This bypasses the state
check so all setup hooks execute regardless of prior completion.

The notstrap CLI exposes this as `--force`.

## Hook script lifecycle

```
notstrap / nothooks invoked
│
├── dot phase
│   └── for each hook where phase == "dot"
│       └── nu <script>          # always runs
│
└── setup phase
    └── for each hook where phase == "setup"
        ├── state.is_done(name)?
        │   ├── yes (and !force) → skip
        │   └── no  (or force)   → nu <script>
        │                             └── success → state.mark_done(name)
        └── report collected
```

## Writing a hook script

Hook scripts are plain Nushell (`.nu`) files. They receive no arguments and
communicate success/failure via exit code (0 = success, non-zero = failure).

```nushell
#!/usr/bin/env nu
# hooks/setup/install-starship.nu
# Installs starship prompt if not already present.

if (which starship | is-empty) {
    print "Installing starship..."
    sh -c "curl -sS https://starship.rs/install.sh | sh -s -- -y"
} else {
    print "starship already installed, skipping"
}
```

```nushell
#!/usr/bin/env nu
# hooks/dot/xdg-dirs.nu
# Ensure XDG base directories exist on every login.

mkdir ~/.config ~/.cache ~/.local/share ~/.local/state
```

## Usage from Rust

```rust
use nothooks::{HookRunner, run_phase};
use notcore::{HookPhase, HookSpec};
use std::path::PathBuf;

let hooks: Vec<HookSpec> = /* load from notfiles.toml */;
let state_dir = PathBuf::from("/path/to/dotfiles");

// Normal run
let runner = HookRunner::new(state_dir.clone());

// Force re-run of setup hooks
let runner = HookRunner::with_force(state_dir.clone());

let dot_report   = run_phase(&hooks, &HookPhase::Dot,   &runner);
let setup_report = run_phase(&hooks, &HookPhase::Setup, &runner);
```

`run_phase` returns a `notcore::Report` containing a `StepStatus` (`Ok`,
`Skipped`, or `Failed(msg)`) for each hook in that phase.
