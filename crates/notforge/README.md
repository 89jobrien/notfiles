# notforge

Forge lifecycle and repository provisioning for notfiles.

A Gitea-compatible forge API client plus the provisioning lifecycle used to
provision repositories and manage local git remotes — a backend for clone
flows (e.g. notstrap's Tailscale clone) that needs repo metadata, repo
creation, and remote management.

## API

- `GiteaHttpApi` — implements `ForgeApi`: `version()`, `repo(owner, name)`
  (404 → `Ok(None)`), `create_repo(spec, auth)`. Real transport:
  `UreqGiteaTransport` (blocking, via `ureq`).
- Ports in `ports`: `ForgeApi`, `ForgeLifecycle` (`ensure_available`),
  `GitRemoteManager` (`remote_url`, `set_remote`, `push`),
  `SecretResolverPort` (`resolve`). Types: `ForgeAuth` (token/basic),
  `RemoteRepository`, `ForgeVersion`, `LocalRepository`,
  `LifecycleStatus`, `RemoteStatus`, `PushStatus`.
- `config` — `ForgeConfig` (`gitea`, `repositories`), `GiteaConfig`
  (`base_url`, `owner`, `ssh_host`, `ssh_port`, `mode`, `auth`), `GiteaMode`
  (`local-process`, `local-container`, `vm-systemd`, `existing`),
  `RepoSpec`, `ForgeAuthConfig`, `ForgeSecretRef` (`Env { key }` /
  `Op { uri }`).
- `NotforgeError` — config / secret / http / command / git / repository.

## Testing

`cargo test -p notforge`
