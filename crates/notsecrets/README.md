# notsecrets

Multi-provider secret resolution for the notfiles workspace.

`SecretResolver` builds from a `SecretsConfig` (`notsecrets.toml`): every
bound secret maps to a provider chain, each provider configures one
source, and resolution pulls values as validated `(key, value)` strings.
Providers: env, 1Password, dotenvx, sops, gsm, nuenv, direnv, mise,
bitwarden, vault, dotenvy, prompt, yubikey (age keys, when enabled).

Also native age encryption/decryption with x25519, ed25519 and scrypt
identities, SSH recipients, and `FileKey`. No external `sops` or `age`
binaries required.

## API

- `SecretResolver` — `from_config`, `resolve`, `resolve_all`
- `resolve_identities` — collect all identities from `IdentitySource`s
- `Encryptor` / `Decryptor`
- `Identity` trait: `X25519Identity`, `SshEd25519Identity`,
  `ScryptIdentity`, `EncryptedIdentity`; `Recipient`:
  `X25519Recipient`, `SshEd25519Recipient`, `ScryptRecipient`
- Errors: `SecretsError`, `AgeError`

## Modules

- `config` — `Provider`, `ProviderConfig`, `SecretRef`, `SecretsConfig`,
  `load_config`
- `ports` — `SecretSource`, `EnumerableSecretSource`, `IdentitySource`
- `recipients` — `Recipient` (`X25519`, `SshEd25519`, `Scrypt`)
- `identities` — identity types + `FileKey` (16 random bytes)
- `sources` — one `*Source` per provider
- `resolver` — `SecretResolver`
- `encrypt` / `decrypt` / `format` — age format parsing, file crypto
- `error` — `SecretsError`, `AgeError`

## Testing

`cargo test -p notsecrets`
