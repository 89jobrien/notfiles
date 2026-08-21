# notstrap

The new-machine bootstrap orchestrator. This is the one crate allowed to know
about all the others.

## Dependency boundary

`notstrap` is the **orchestrator** and is exempt from the sibling rule in
`scripts/check-dep-boundaries.py`: it may depend on `notcore` plus every
feature crate. That exemption exists so the feature crates never need to reach
sideways for each other — if `notfiles` seems to need something from
`notsecrets`, the composition belongs here instead.

## The run sequence

`run(BootstrapOptions) -> Result<Report>` executes in this order:

1. `prereqs::check_prerequisites`
2. tailscale — `notnet::ensure_connected`, only when `[tailscale]` is present
3. clone dotfiles — `repo::clone_if_missing`
4. age key — identity sources tried in order
5. secrets — SOPS decryption via `notsecrets`
6. link dotfiles — `notfiles::link`
7. hooks — `nothooks::run_phase`
8. final report

Order matters and is not incidental: the tailnet must be up before a private
remote can be cloned, the age key must resolve before secrets decrypt, and
secrets must land before hooks that read them.

## Failure model

Steps do **not** abort the process. Each appends to notcore's `Report` with
`StepStatus::Ok` / `Skipped` / `Failed(msg)`, the run continues, and `main.rs`
exits non-zero at the end if `report.has_failures()`. When adding a step,
follow that shape — an early `?` return that skips the report leaves the user
with no summary of what did happen.

`Skipped` is a real outcome, not a soft failure: `ensure_connected` returning
`Ok(false)` means "already connected", and `clone_if_missing` returning
`Ok(false)` means the repo was already there.

## Config

`notstrap.toml`:

- `[bootstrap]` — `dotfiles_repo` is required *unless* `[tailscale]` is
  present, in which case that section's `gitea_url` is the clone source.
  Also `dotfiles_dir`, `bw_age_item`, `sops_file`.
- `[tailscale]` — optional, deserializes into `notnet::TailscaleOptions`.
- `[[hooks]]` — a list of `notcore::HookSpec`.

## Testing

```bash
cargo test -p notstrap
cargo test -p integration          # tests/integration — bootstrap.rs, cross_crate.rs
```

The workspace-level `tests/integration` crate is where notstrap's end-to-end
coverage actually lives. Those tests run hooks, so **they need `nu` on `PATH`**
and fail wholesale without it.
