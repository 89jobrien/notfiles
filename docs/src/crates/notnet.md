# notnet

Network utilities used during bootstrap: Tailscale connectivity, and
Yubikey as an identity source for `notsecrets`.

## Tailscale integration

`ensure_connected(opts: &TailscaleOptions)` is the entry point:

1. Fast path — if `tailscale status --peers=false` already reports a
   connection, return `Ok(false)` (skipped).
2. If Tailscale isn't installed and `opts.install` is set, install it
   (`installer` module); otherwise return `NotnetError::NotInstalled`.
3. Resolve an auth key (`auth::resolve_auth_key`) and run
   `tailscale up --authkey=...`.
4. Verify the target peer is reachable via `tailscale ping`.

Returns `Ok(true)` if it just connected, `Ok(false)` if already
connected, or a `NotnetError` (`NotInstalled`, `CommandFailed`,
`PeerUnreachable`) otherwise.

## YubikeySource

Provides Yubikey-backed identity resolution as another source
`notsecrets` can draw on, alongside the file/1Password/Bitwarden
sources.

## Modules

`auth`, `error`, `installer`, `ports` (`TailscaleOptions`).
