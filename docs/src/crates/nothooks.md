# nothooks

Runs Nushell hook scripts around linking and bootstrap. See
[ADR 0004](../adr/0004-nushell-only-hooks.md) for why Nushell is the
only supported hook language.

## HookRunner

Executes `.nu` scripts via `nu <script>`, in two phases:

- **`HookPhase::Dot`** — always runs, every time hooks are invoked.
- **`HookPhase::Setup`** — runs once per package; whether it has
  already run is tracked in `.nothooks-state.toml`. Pass `--force` to
  re-run setup hooks that have already fired.

## State

`.nothooks-state.toml` persists which setup hooks have completed, so
re-running `link` or bootstrap doesn't re-execute one-time setup
steps unless `--force` is given. State writes are batched rather than
flushed per-hook.
