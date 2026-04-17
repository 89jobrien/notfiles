# Tailscale Integration Design

**Date:** 2026-04-16
**Status:** Approved

## Goal

Make notfiles environments ephemeral but reliable. A fresh machine runs `notstrap` and gets a
fully configured environment — no public GitHub exposure for dotfiles, no manual Tailscale setup,
no cloud dependency for secrets on bootstrap day.

## Overview

Add a `notnet` crate that owns Tailscale network presence, extend `notsecrets` with a
`YubikeySource`, and thread both into `notstrap`'s orchestration sequence. The dotfiles repo lives
on a private Gitea instance on minibox, reachable only over Tailscale.

## Crate Responsibilities

### `notnet` (new)

Owns all Tailscale concerns:

- Detect if Tailscale is already running (`tailscale status`)
- Install Tailscale if missing (package manager detection: `apt`/`brew`/`pacman`, fallback to
  official install script)
- Resolve the auth key via priority chain (see below)
- Join the tailnet (`tailscale up --authkey=<key>`)
- Verify the target peer is reachable (ping by Tailscale hostname)

**Auth key resolution chain:**

1. `TS_AUTHKEY` env var
2. YubiKey PIV slot 9d (via `yubikey` crate)
3. Interactive prompt

**Public API:**

```rust
pub struct TailscaleOptions {
    pub peer_hostname: String,
    pub gitea_url: String,
    pub install: bool,
}

/// Ensure this machine is connected to the tailnet and the target peer is reachable.
/// Returns immediately if Tailscale is already running.
pub fn ensure_connected(opts: &TailscaleOptions) -> Result<()>;
```

### `notsecrets` (extended)

New `YubikeySource` implementing `IdentitySource`:

- Uses the `yubikey` crate (pure Rust, no `ykman` dependency)
- Opens the first connected YubiKey
- Reads age private key material from PIV slot 9c
- Returns `Box<dyn Identity>`

Updated source resolution order in notstrap:

1. `--key-file` CLI arg
2. `YubikeySource` (PIV slot 9c)
3. `BitwardenSource`
4. `PromptSource`

### `notstrap` (updated)

`BootstrapOptions` gains:

```rust
pub tailscale: Option<TailscaleOptions>,
```

`None` = skip notnet entirely (existing machines, tests, CI).

`notstrap.toml` gains a `[tailscale]` section. When present, `gitea_url` is used as the clone
URL instead of `[bootstrap].dotfiles_repo`.

## YubiKey PIV Slot Layout

| Slot | Purpose                          | Managed by  |
|------|----------------------------------|-------------|
| 9a   | SSH authentication               | 1Password   |
| 9c   | Age private key (dotfiles/SOPS)  | notsecrets  |
| 9d   | Tailscale auth key               | notnet      |

Slot 9a is untouched — 1Password agent remains the primary SSH auth for day-to-day use.

## Configuration

`notstrap.toml` example with Tailscale enabled:

```toml
[bootstrap]
dotfiles_dir = "~/dotfiles"
bw_age_item = "age-key-dotfiles"
sops_file = "secrets/bootstrap.sops.env"
# dotfiles_repo is optional when [tailscale] is present

[tailscale]
peer_hostname = "minibox"
gitea_url = "http://minibox:3000/joe/dotfiles.git"
install = true
```

## Full Bootstrap Sequence

| Step              | Crate         | Skip condition                        |
|-------------------|---------------|---------------------------------------|
| 1. Prerequisites  | notstrap      | —                                     |
| 2. Tailscale      | notnet        | Already on tailnet                    |
| 3. Clone dotfiles | notstrap/repo | Already cloned                        |
| 4. Age key        | notsecrets    | —                                     |
| 5. Decrypt SOPS   | notstrap      | No injector configured                |
| 6. Link dotfiles  | notfiles      | —                                     |
| 7. Dot hooks      | nothooks      | —                                     |
| 8. Setup hooks    | nothooks      | Already run (state file)              |

## Error Handling

- If `TS_AUTHKEY` is unset and no YubiKey is connected and user declines prompt → clear error,
  bootstrap aborts at step 2
- If Tailscale installs but peer is unreachable → error naming the peer hostname, bootstrap aborts
- If YubiKey slot 9c is empty → `YubikeySource` returns `Err`, resolver falls through to
  `BitwardenSource`
- All steps follow the existing `StepStatus::Failed` / early-return pattern in notstrap

## Dependencies (new)

| Crate       | Used by             | Purpose                        |
|-------------|---------------------|-------------------------------|
| `yubikey`   | notnet, notsecrets  | PIV slot access (pure Rust)   |
| `tailscale` | notnet              | tailscale-rs SDK               |

## Out of Scope

- Multi-YubiKey support (first connected device wins)
- Tailscale exit node or subnet router configuration
- Rotating auth keys automatically
- Windows support
