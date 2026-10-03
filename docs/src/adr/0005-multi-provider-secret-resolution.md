# 5. Multi-provider secret resolution via SecretResolver

Date: 2026-07-08

## Status

Accepted

## Context

Bootstrap previously used a single `EnvInjector` tied to plain
environment variables. Different machines and users need secrets from
different backends — 1Password, Bitwarden, `.env`/dotenvx files, SOPS,
plain files — and hardcoding one provider per deployment forces
per-user forks of the bootstrap flow.

## Decision

Replace `EnvInjector` with a `SecretResolver` abstraction in
`notsecrets`. Each backend (`EnvSource`, `OpSource`, `BitwardenSource`,
`FileSource`, `DotenvxSource`, `SopsSource`, …) implements a common
source trait; `notstrap` resolves secrets through `SecretResolver`
without knowing which concrete provider is in use.

## Consequences

New secret backends are additive (new `*Source` impl), not a rewrite
of `notstrap`. Config must now specify which provider(s) to use per
secret, adding a small amount of setup complexity compared to a single
hardcoded source. Provider-specific bugs (e.g. the Bitwarden password
exposure and ChaCha nonce issues fixed 2026-04-11) are isolated to
their own source module.
