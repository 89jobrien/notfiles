# notsecrets

Two things live here that are easy to confuse:

1. **Secret resolution** — look up a named secret across a chain of providers
   (`SecretResolver`, `sources/`).
2. **age encryption** — a from-scratch implementation of the age format
   (`identities/`, `recipients/`, `format.rs`, `encrypt.rs`, `decrypt.rs`).

They meet only at the edges: a resolved secret can be the passphrase or key
material that unlocks an age file.

## Dependency boundary

May depend on `notcore` only. **Not** on `notfiles` or `nothooks` —
`scripts/check-dep-boundaries.py` fails CI on sibling edges.

## No external binaries

There is no shelling out to `age` or `sops` for crypto. The age format is
implemented here directly on top of RustCrypto: `chacha20poly1305`,
`x25519-dalek`, `ed25519-dalek`, `hkdf`, `hmac`, `scrypt`, `sha2`. Adding a
`Command::new("age")` call would be a regression, not a shortcut.

(Some *providers* do shell out — `op`, `bw`, `vault`, `sops` for its own file
decryption. That is provider I/O, not crypto.)

## Secret resolution

`SecretResolver::from_config` builds a provider chain from `SecretsConfig`. It
validates up front that every `[secrets]` binding names a provider present in
the `providers` list, so a typo fails at construction rather than at lookup.

Two ports in `ports.rs`:

- `SecretSource` — `resolve(key) -> Result<Option<String>>`.
- `EnumerableSecretSource: SecretSource` — adds `resolve_all()`. Only sources
  that can list their whole keyspace implement it (`env`, dotenv-style files).

**The contract:** a key that isn't there is `Ok(None)`, not `Err`. `Err` means
the source itself failed — binary missing, vault unreachable, bad auth.
Resolution walks the chain and a source returning `Ok(None)` just moves to the
next one, so returning `Err` for "not found" halts a chain that should have
continued. `tests/conformance_secret_source.rs` asserts this against every
source; a new source must be added to that list.

`SecretSource` providers (the `Provider` enum): `env`, `op` (1Password),
`bitwarden`, `dotenvx`, `dotenvy`, `sops`, `gsm`, `nuenv`, `direnv`, `mise`,
`vault`. Of these, `env`, `dotenvx`, and `sops` are also enumerable.

Note that `sources/` also holds types that are **not** providers:
`FileSource`, `PromptSource`, and `YubikeySource` implement `IdentitySource`
only — they supply age identities, not named secrets, and so have no `Provider`
variant. `BitwardenSource` is the one type implementing both.

## age implementation

- `format.rs` — header parse/serialize. The header is `age-encryption.org/v1`,
  a stanza per recipient, then a `--- <base64 mac>` footer. Base64 throughout is
  **unpadded** (`STANDARD_NO_PAD`); padding will produce files other age
  implementations reject.
- `identities/` — unwrap the file key: `X25519Identity`, `SshEd25519Identity`,
  `ScryptIdentity`, `EncryptedIdentity`.
- `recipients/` — the wrapping side, mirroring the identity set.
- `resolve_identities` succeeds on *partial* success and errors only if every
  source fails, because not every identity is expected to open every file.

`zeroize` is in `Cargo.toml` but currently unused in `src/` — key material is
held in plain `Vec<u8>`/`String`. Treat wiring it up as open work rather than
an invariant you can rely on; don't document it as if it were in place.

## Features

`yubikey` is off by default (`--features yubikey`), so `YubikeySource` is
behind `#[cfg(feature = "yubikey")]`. Check both configurations compile before
touching it:

```bash
cargo test -p notsecrets
cargo check -p notsecrets --features yubikey
```
