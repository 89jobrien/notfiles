# notnet

Tailscale network presence management for notstrap.

Drives the `tailscale` CLI (through the `CommandRunner` port, whose real
adapter bounds every invocation with a timeout) to bring the machine onto
the tailnet and verify the target peer is reachable. Auth keys come from
the `TS_AUTHKEY` env var, then the YubiKey PIV slot 9d (when compiled with
the `yubikey` feature), then an interactive prompt.

## API

- `ensure_connected(opts) -> Result<bool, NotnetError>` — `Ok(false)` if
  already on the tailnet; `Ok(true)` if it joined now. Peer verification
  retries briefly because `tailscale up` returns while the node may still
  report disconnected.
- `ensure_connected_with(...)` — the same over explicit adapters.
- `TailscaleOptions` — `peer_hostname`, `gitea_url`, `install`.
- Constants: `VERIFY_PEER_RETRIES` (3), `VERIFY_PEER_RETRY_DELAY` (250 ms).
- Errors: `NotnetError`.

## Ports

- `Tailscale` — `is_connected`, `is_installed`, `join(auth_key)`,
  `verify_peer(hostname)`
- `TailscaleInstaller` — `install()`
- `AuthKeySource` — `auth_key()`
- `CommandRunner` — `run`, `run_timeout`, `exists`

## Testing

`cargo test -p notnet`
