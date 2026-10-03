# notsecrets

Multi-provider secret resolution and native age/SOPS-compatible
encryption. Used by `notstrap` during bootstrap, and available to any
crate that needs to resolve a secret without caring which backend
holds it.

## Secret resolution

`SecretResolver` is the entry point. Concrete backends implement a
common source trait:

- `EnvSource` — plain environment variables
- `OpSource` — 1Password
- `BitwardenSource` — Bitwarden
- `FileSource` — plain files
- `DotenvxSource` — encrypted `.env` via dotenvx
- `SopsSource` — SOPS-managed files

See [ADR 0005](../adr/0005-multi-provider-secret-resolution.md) for
why this replaced a single hardcoded `EnvInjector`.

## Encryption

Native age encryption/decryption — x25519, SSH ed25519/RSA, and
scrypt identity support — plus SOPS-compatible file handling. No
external `sops` or `age` binaries are shelled out to; see
[ADR 0003](../adr/0003-native-age-encryption.md) for the reasoning
and the tradeoffs that come with owning the crypto implementation.

## Identity management

`notsecrets` manages age identities (key generation, storage,
lookup) used both to encrypt secrets at rest and to decrypt them
during `notstrap`'s bootstrap flow.
