# tests/integration

Cross-crate end-to-end tests. A workspace member (`integration`) that exists
only to hold tests — it has no `src/`.

Per-crate tests live with their crate; this is for behavior that only appears
when crates are composed, which in practice means `notstrap`'s bootstrap flow.

| File            | Covers                                                     |
| --------------- | ---------------------------------------------------------- |
| `bootstrap.rs`  | Full `notstrap::run` sequence, dot vs. setup hook phases   |
| `cross_crate.rs`| Interactions spanning more than one crate                  |

## Nushell is required

These tests execute real hooks, and `nothooks` shells out to `nu`. **Without
Nushell on `PATH` they fail wholesale** — `bootstrap.rs` and part of
`cross_crate.rs` — while every other crate's tests pass. That failure signature
means a missing binary, not broken code. CI installs nushell for this reason.

```bash
which nu || echo "install nushell first"
cargo test -p integration
```

## Writing tests here

Assert on the `Report` rather than on process exit: `notstrap` records
`StepStatus::Ok` / `Skipped` / `Failed(msg)` per step and keeps going, so a
failing step is visible in `report.steps`, not in a panic. `Skipped` is a
success outcome — "already connected", "already cloned" — so don't assert
`Ok` where `Skipped` is legitimate.
