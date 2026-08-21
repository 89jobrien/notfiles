# notforge

Forge (Gitea) lifecycle and repository provisioning.

## Status: scaffolded, not wired up

Read this before planning work here. As it stands:

- `src/main.rs` is a **stub** — the `notforge` binary prints the version and
  exits. There is no CLI.
- `ports.rs` defines the traits (`ForgeApi`, `ForgeLifecycle`,
  `GitRemoteManager`, `SecretResolverPort`) but the **only implementations in
  the tree are test fakes** in `tests/api.rs`. Don't go looking for production
  impls; they haven't been written.
- `gitea.rs` is the exception — `GiteaHttpApi` is real, over a `GiteaTransport`
  seam with `UreqGiteaTransport` as the live adapter.
- Nothing else in the workspace depends on `notforge`, and no root-level docs
  describe it.

So: the config and port shapes are settled, the behavior mostly isn't. New work
here is filling in implementations, not refactoring around existing ones.

## Config (`config.rs`)

`ForgeConfig` → `GiteaConfig` → `GiteaMode`, plus `ForgeAuthConfig`,
`ForgeSecretRef`, and `RepoSpec`.

`GiteaMode` has a **hand-written `Deserialize`** because it accepts two TOML
spellings:

```toml
mode = "existing"                    # bare string — only valid for `existing`

[gitea.mode]                         # tagged form
type   = "local-process"             # or local-container | vm-systemd | existing
config = { ... }                     # required for every mode except existing
```

Any new mode needs a match arm added in that impl — deriving won't pick it up.
Unknown mode names are a deserialization error, not a silent default.

## Ports

| Trait                | Role                                          |
| -------------------- | --------------------------------------------- |
| `ForgeApi`           | Talk to a running forge (repos, versions)     |
| `ForgeLifecycle`     | Start/stop/verify the forge itself            |
| `GitRemoteManager`   | Manage local git remotes and pushes           |
| `SecretResolverPort` | Resolve a `ForgeSecretRef` — keeps `notsecrets` out of the dep graph |

That last one matters: `notforge` depends on `notcore` only. Resolving secrets
through a port instead of importing `notsecrets` is deliberate — keep it that
way and let the composition root inject an implementation.

Status enums (`LifecycleStatus`, `RemoteStatus`, `PushStatus`) distinguish
"already in the desired state" from "changed it", the same convention `notnet`
and `notstrap` use for idempotent steps.

## Testing

```bash
cargo test -p notforge
```

`tests/gitea.rs` drives `GiteaHttpApi` through a fake `GiteaTransport` — no
network. `tests/api.rs` holds the fake port impls. Any real implementation you
add should be testable the same way, behind its trait.
