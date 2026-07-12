# notsecrets: Age-Native Redesign

**Date:** 2026-04-01
**Status:** Approved

## Overview

Redesign `notsecrets` from a thin wrapper around external tools (sops, Bitwarden CLI) into a self-contained age encryption/decryption library implemented in pure Rust. Modeled architecturally after `rage` but with no dependency on it. Supports multiple recipients, passphrases, passphrase-protected identity files, and SSH keys.

Replaces: `sops --decrypt` shell-out, `resolve_age_key`, `install_age_key`.
Keeps: existing `sources/` resolvers (Bitwarden, file, prompt) — repurposed as identity resolvers.

---

## Domain Layer (Ports)

Two core traits define the age conceptual split:

```rust
pub trait Identity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>>;
}

pub trait Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError>;
}
```

Core domain types:

```rust
pub struct FileKey([u8; 16]);
pub struct Stanza { pub tag: String, pub args: Vec<String>, pub body: Vec<u8> }
pub struct Header { pub recipients: Vec<Stanza>, pub mac: Vec<u8> }
```

`Stanza` is the age wire-format recipient block. `FileKey` is the symmetric payload key. No infrastructure details appear in domain types.

---

## Identity Implementations (`src/identities/`)

| File | Type | Key material | Crypto |
|------|------|-------------|--------|
| `x25519.rs` | `X25519Identity` | `AGE-SECRET-KEY-1...` bech32 | X25519 + HKDF + ChaCha20-Poly1305 |
| `scrypt.rs` | `ScryptIdentity` | passphrase string | scrypt + ChaCha20-Poly1305 |
| `ssh_ed25519.rs` | `SshEd25519Identity` | OpenSSH ed25519 private key | Ed25519→X25519 twist + HKDF + ChaCha20-Poly1305 |
| `ssh_rsa.rs` | `SshRsaIdentity` | OpenSSH RSA private key (incl. passphrase-protected PEM) | OAEP-SHA256 |
| `encrypted.rs` | `EncryptedIdentity` | age-encrypted identity file | Wraps inner `Identity`; decrypts file first via `ScryptIdentity` |

Each implements `Identity`, maps its errors to `AgeError`, and has no knowledge of sources (Bitwarden, file, prompt).

---

## Recipient Implementations (`src/recipients/`)

| File | Type | Key material | Crypto |
|------|------|-------------|--------|
| `x25519.rs` | `X25519Recipient` | `age1...` bech32 public key | ephemeral X25519 + HKDF + ChaCha20-Poly1305 |
| `scrypt.rs` | `ScryptRecipient` | passphrase + work factor | scrypt + ChaCha20-Poly1305 |
| `ssh_ed25519.rs` | `SshEd25519Recipient` | OpenSSH ed25519 public key | Ed25519→X25519 + HKDF + ChaCha20-Poly1305 |
| `ssh_rsa.rs` | `SshRsaRecipient` | OpenSSH RSA public key | OAEP-SHA256 |

Multiple recipients: `Encryptor` calls `wrap_file_key` on each, writing all stanzas to the header. The single-scrypt-recipient invariant is enforced at `Encryptor` construction, not in the `ScryptRecipient` itself.

---

## Encryptor and Decryptor

### `Encryptor`

```rust
pub struct Encryptor { recipients: Vec<Box<dyn Recipient>> }

impl Encryptor {
    pub fn with_recipients(recipients: Vec<Box<dyn Recipient>>) -> Result<Self, AgeError>;
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, AgeError>;
}
```

Encrypt steps:
1. Generate random `FileKey`
2. `wrap_file_key` on each recipient → collect stanzas
3. Serialize `Header` (base64 stanza bodies + HMAC-SHA256 MAC)
4. Derive payload key from `FileKey` via HKDF
5. Encrypt payload with ChaCha20-Poly1305 (random nonce)
6. Write age binary format: version line + header stanzas + `---` separator + nonce + ciphertext

### `Decryptor`

```rust
pub struct Decryptor { identities: Vec<Box<dyn Identity>> }

impl Decryptor {
    pub fn with_identities(identities: Vec<Box<dyn Identity>>) -> Self;
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, AgeError>;
}
```

Decrypt steps:
1. Parse header (format module)
2. Verify header MAC
3. Try each identity against each stanza until `FileKey` recovered
4. Derive payload key, decrypt with ChaCha20-Poly1305

---

## Format Module (`src/format.rs`)

Pure serialization/deserialization of the age binary wire format:
- Version line: `age-encryption.org/v1`
- Recipient stanzas: `-> <tag> <args...>\n<base64-body>\n`
- Footer: `--- <base64-mac>\n`
- Payload: nonce + ciphertext

No crypto here. Gets thorough unit tests covering round-trip, malformed input, truncated files.

---

## Sources (Kept, Repurposed)

Existing `sources/` (Bitwarden, file, prompt) are kept but now produce `Box<dyn Identity>` instead of a raw key string. `resolve_age_key` is replaced by:

```rust
pub fn resolve_identities(sources: Vec<Box<dyn IdentitySource>>) -> Result<Vec<Box<dyn Identity>>, AgeError>;
```

Where `IdentitySource` is a new thin trait:

```rust
pub trait IdentitySource {
    fn name(&self) -> &str;
    fn load(&self) -> Result<Box<dyn Identity>, AgeError>;
}
```

`FileSource` loads a key file and returns `X25519Identity` if the content starts with `AGE-SECRET-KEY-1`, or `EncryptedIdentity` if the content starts with the age version line (`age-encryption.org/v1`). `PromptSource` returns `ScryptIdentity`. `BitwardenSource` retrieves the raw key and returns `X25519Identity`.

---

## Module Structure

```
crates/notsecrets/src/
  lib.rs                  # public API
  format.rs               # age wire format parser/serializer
  error.rs                # AgeError
  identities/
    mod.rs                # Identity trait, FileKey, Stanza, Header
    x25519.rs
    scrypt.rs
    ssh_ed25519.rs
    ssh_rsa.rs
    encrypted.rs
  recipients/
    mod.rs                # Recipient trait
    x25519.rs
    scrypt.rs
    ssh_ed25519.rs
    ssh_rsa.rs
  sources/
    mod.rs                # IdentitySource trait
    bitwarden.rs
    file.rs
    prompt.rs
```

---

## Dependencies Added to `Cargo.toml`

```toml
x25519-dalek     = { version = "2", features = ["static_secrets"] }
ed25519-dalek    = "2"
rsa              = "0.9"
sha2             = "1"
hkdf             = "0.12"
chacha20poly1305 = "0.10"
scrypt           = "0.11"
bech32           = "0.11"
base64           = "0.22"
hmac             = "0.12"
```

**Removed:** `which` (was only used for sops check).
**Kept:** `rpassword` (used by `PromptSource`), `dirs`, `anyhow`, `thiserror`.

---

## Removed from Public API

- `resolve_age_key()`
- `install_age_key()` / `install_age_key_at()`
- `decrypt_sops()` / `sops_args()`
- `AgeKeySource` trait

---

## notstrap Impact

Two call sites to update:
- `resolve_age_key(sources)` → `resolve_identities(sources)`
- `decrypt_sops(path)` → `Decryptor::with_identities(identities).decrypt(&bytes)`

---

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum AgeError {
    #[error("no identity could decrypt any recipient stanza")]
    NoMatch,
    #[error("header MAC verification failed")]
    MacMismatch,
    #[error("malformed age file: {0}")]
    ParseError(String),
    #[error("crypto error: {0}")]
    CryptoError(String),
    #[error("unsupported key type: {0}")]
    UnsupportedKeyType(String),
    #[error("identity source failed ({name}): {source}")]
    SourceError { name: String, source: anyhow::Error },
}
```

---

## Testing Strategy

- **Unit tests** on `format.rs`: round-trip parse/serialize, malformed input, truncated files
- **Unit tests** on each identity/recipient pair: wrap then unwrap with matching identity
- **Integration tests**: encrypt with multiple recipients, verify each identity can independently decrypt
- **Negative tests**: wrong identity returns `NoMatch`, corrupted MAC returns `MacMismatch`
- No external process spawning in tests
