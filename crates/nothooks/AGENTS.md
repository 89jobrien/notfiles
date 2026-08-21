# nothooks

Runs Nushell hook scripts with two phases and remembers which setup hooks have
already run.

## Dependency boundary

May depend on `notcore` only. **Not** on `notfiles` or `notsecrets` —
`scripts/check-dep-boundaries.py` fails CI on sibling edges.

## Phases

| Phase              | When it runs                                            |
| ------------------ | ------------------------------------------------------- |
| `HookPhase::Dot`   | Every invocation. Idempotent by contract.               |
| `HookPhase::Setup` | Once per machine, then recorded and skipped thereafter. |

`--force` re-runs setup hooks that are already marked done.

## State

`.nothooks-state.toml` in the state dir records completed setup hook names.
`HookRunner::run_phase` loads state once and saves once **per phase**, not per
hook — so a hook that panics mid-phase can lose the record of earlier setup
hooks in that same run. Keep that batching in mind before adding early returns
to the loop.

## Running hooks

`HookRunner` shells out to `nu <script>`. Consequences worth knowing:

- **Nushell must be on `PATH`.** Without it every hook fails. This is why CI
  installs nushell before integration tests, and why `tests/integration` in the
  workspace root fails locally on a machine without `nu`.
- Hook scripts are `.nu` only. This repo has no `.sh` hooks and shouldn't grow
  any.

`HookResult` is `Ok` / `Skipped` / `Failed(String)`; results roll up into
notcore's `Report`, so `report.has_failures()` is how callers decide the exit
code rather than hooks aborting the process themselves.

## Testing

```bash
cargo test -p nothooks
```

`tests/integration.rs` needs `nu` on `PATH`. If tests fail with every hook
reporting failure, check `which nu` before debugging the runner.
