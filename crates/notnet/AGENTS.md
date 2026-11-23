# notnet

Gets the machine onto a tailnet so `notstrap` can reach a private dotfiles
remote. Small crate, single job.

## Entry point

`ensure_connected(&TailscaleOptions) -> Result<bool, NotnetError>`

The `bool` is **not** success — it's whether work was done:

- `Ok(false)` — already on the tailnet, nothing to do (skipped).
- `Ok(true)`  — joined the tailnet during this call.
- `Err(_)`    — could not connect.

`notstrap` maps this straight onto `StepStatus::Skipped` vs `Ok`, so flipping
the polarity silently mislabels bootstrap reports.

Sequence: fast-path status check → install if missing and `opts.install` →
resolve auth key → join → verify the target peer is reachable.

## Modules

| Module         | Holds                                                |
| -------------- | ---------------------------------------------------- |
| `lib.rs`       | `ensure_connected` — plus private `join`, `verify_peer`, and a private `status` mod |
| `auth.rs`      | `resolve_auth_key`                                   |
| `installer.rs` | `is_installed`, `install`                            |
| `ports.rs`     | `TailscaleOptions` (deserialized from notstrap.toml) |
| `error.rs`     | `NotnetError`                                        |

`ensure_connected` is the only public entry point; everything else in `lib.rs`
is private. Widen that surface only if a caller genuinely needs a step alone.

## Auth key chain

`resolve_auth_key` tries, in order:

1. `TS_AUTHKEY` environment variable (non-empty).
2. YubiKey PIV slot 9d — only when built with `--features yubikey`.
3. Interactive prompt.

Exhausting all three is `NotnetError::NoAuthKey`. The `yubikey` feature here is
*only* this slot read; it is unrelated to `notsecrets::YubikeySource`, which is
an age identity source. Two different features with the same name in two crates.

## Gotchas

- This crate shells out to the `tailscale` binary and parses its output; it is
  not a Tailscale API client. There are no unit tests for that reason — nearly
  everything here needs a real host.
- `install()` performs a privileged system change. Nothing should call it
  unless the caller set `opts.install`; without it a missing binary is
  `NotnetError::NotInstalled`.
- `notnet` is not covered by `scripts/check-dep-boundaries.py` (the script only
  tracks notcore/notfiles/notsecrets/nothooks/notstrap). Depending on a feature
  crate here wouldn't be caught by CI — don't.

## Testing

```bash
cargo test -p notnet
cargo check -p notnet --features yubikey   # PIV slot 9d read is feature-gated
```
