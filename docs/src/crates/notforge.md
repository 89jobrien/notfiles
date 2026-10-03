# notforge

Forge lifecycle and repository provisioning for `notfiles` — a Gitea
API foundation for creating/verifying dotfiles repositories on a
self-hosted forge as part of onboarding a new machine or repo.

## Modules

- **config** — `ForgeConfig`, `ForgeSecretRef`, `RepoSpec`: how a forge
  and its target repository are described in config.
- **ports** — the trait boundary for forge operations, plus the value
  types returned across it:
  - `ForgeAuth` — `Token` or `Basic` authentication.
  - `RemoteRepository` — owner, name, HTTP/SSH clone URLs.
  - `ForgeVersion` — server version metadata.
  - `LocalRepository` — local filesystem path.
- **gitea** — the concrete Gitea API adapter implementing the `ports`
  trait boundary.
- **error** — `NotforgeError`.

## Status

Foundational — the Gitea API client and port definitions exist and
are tested (`tests/gitea.rs`, `tests/api.rs`), but `notforge` is not
yet wired into the `notstrap` bootstrap flow.
