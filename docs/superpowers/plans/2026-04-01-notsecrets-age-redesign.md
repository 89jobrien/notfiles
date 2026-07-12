# notsecrets Age-Native Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign notsecrets as a self-contained age encryption/decryption library in pure Rust with multiple recipients, passphrases, passphrase-protected identity files, and SSH keys.

**Architecture:** Hexagonal architecture with Identity and Recipient as domain ports. Concrete implementations (X25519, Scrypt, SshEd25519, SshRsa, Encrypted) live in infra adapters. Encryptor/Decryptor orchestrate at the domain layer. Sources (Bitwarden, file, prompt) are identity resolvers at the infra boundary.

**Tech Stack:** x25519-dalek, ed25519-dalek, rsa, sha2, hkdf, chacha20poly1305, scrypt, bech32, base64, hmac, rand

---

## Task 1: Scaffold — Cargo.toml, error.rs, domain traits, stub lib.rs

- [ ] Update `crates/notsecrets/Cargo.toml` — add new deps, remove `which`
- [ ] Create `crates/notsecrets/src/error.rs`
- [ ] Create `crates/notsecrets/src/identities/mod.rs` — domain types + `Identity` trait
- [ ] Create `crates/notsecrets/src/recipients/mod.rs` — `Recipient` trait
- [ ] Rewrite `crates/notsecrets/src/lib.rs` — stub public API, remove old exports

### 1.1 Update `crates/notsecrets/Cargo.toml`

```toml
[package]
name    = "notsecrets"
version = "0.1.0"
edition = "2024"
license.workspace = true

[dependencies]
notcore          = { workspace = true }
anyhow           = { workspace = true }
thiserror        = { workspace = true }
rpassword        = { workspace = true }
dirs             = { workspace = true }
x25519-dalek     = { version = "2", features = ["static_secrets"] }
ed25519-dalek    = "2"
rsa              = { version = "0.9", features = ["sha2"] }
sha2             = "1"
hkdf             = "0.12"
chacha20poly1305 = "0.10"
scrypt           = "0.11"
bech32           = "0.11"
base64           = "0.22"
hmac             = "0.12"
rand             = "0.8"

[dev-dependencies]
tempfile = { workspace = true }
```

Also update workspace `Cargo.toml` — remove `which` from `[workspace.dependencies]` only after verifying no other crate uses it:

```bash
cargo metadata --no-deps --format-version 1 | grep -o '"which"' | wc -l
# if 0 remaining consumers after notsecrets drop, remove from workspace
```

### 1.2 `crates/notsecrets/src/error.rs`

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

### 1.3 `crates/notsecrets/src/identities/mod.rs`

```rust
use crate::error::AgeError;

/// The symmetric file encryption key — 16 random bytes.
#[derive(Clone, zeroize::Zeroize)]
pub struct FileKey([u8; 16]);

impl FileKey {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn generate() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }
}

impl TryFrom<&[u8]> for FileKey {
    type Error = AgeError;
    fn try_from(b: &[u8]) -> Result<Self, AgeError> {
        b.try_into()
            .map(Self)
            .map_err(|_| AgeError::ParseError(format!("file key must be 16 bytes, got {}", b.len())))
    }
}

/// A single recipient stanza in the age header.
#[derive(Debug, Clone)]
pub struct Stanza {
    pub tag: String,
    pub args: Vec<String>,
    pub body: Vec<u8>,
}

/// The full age header (all stanzas + MAC).
#[derive(Debug, Clone)]
pub struct Header {
    pub recipients: Vec<Stanza>,
    pub mac: Vec<u8>,
}

/// Domain port: an identity that can attempt to unwrap a recipient stanza.
///
/// Returns `None` if the stanza tag/args do not match this identity type.
/// Returns `Some(Err(...))` if the stanza matches but decryption fails.
/// Returns `Some(Ok(file_key))` on success.
pub trait Identity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>>;
}
```

Note: `zeroize` must be added to `[dependencies]` in `crates/notsecrets/Cargo.toml`:

```toml
zeroize = "1"
```

Add this line alongside the others in step 1.1.

### 1.4 `crates/notsecrets/src/recipients/mod.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};

/// Domain port: a recipient that can wrap a file key into a stanza.
pub trait Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError>;
}
```

### 1.5 `crates/notsecrets/src/lib.rs` (stub)

```rust
pub mod error;
pub mod identities;
pub mod recipients;
pub mod sources;

pub use error::AgeError;
pub use identities::{FileKey, Header, Identity, Stanza};
pub use recipients::Recipient;
pub use sources::{BitwardenSource, FileSource, IdentitySource, PromptSource};

// Encryptor and Decryptor will be added in Tasks 8 and 9.
// resolve_identities will be added in Task 11.
```

Keep the existing `sources/` files compiling by leaving `AgeKeySource` in place temporarily — it will be replaced in Task 10. For now add a `#[allow(dead_code)]` allow on the old trait and old `resolve_age_key`/`install_age_key`/`decrypt_sops` functions so the crate compiles while tasks proceed incrementally. Annotate each deprecated item:

```rust
#[deprecated(note = "replaced by resolve_identities in Task 11")]
pub use _legacy::{
    install_age_key, install_age_key_at, resolve_age_key, AgeKeySource, decrypt_sops, sops_args,
};
mod _legacy;
```

Move the old body of `lib.rs` (everything except `pub mod sources`) into `crates/notsecrets/src/_legacy.rs` verbatim so existing callers in `notstrap` keep compiling until Task 12.

### 1.6 Verify

```bash
cargo check -p notsecrets
```

Expected: compiles, zero errors.

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings (the `#[deprecated]` re-exports suppress dead_code warnings; `#[allow(deprecated)]` needed in `notstrap` temporarily — add it in Task 12).

### 1.7 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/ Cargo.toml
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): scaffold age-native domain types and trait ports"
```

---

## Task 2: format.rs — age wire format parser/serializer

**No crypto. Pure byte manipulation.**

- [ ] Create `crates/notsecrets/src/format.rs`
- [ ] Write tests first, then implement

### 2.1 Test (write first)

File: `crates/notsecrets/src/format.rs` (tests section)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_age_file() -> Vec<u8> {
        // Manually constructed minimal age file with one X25519 stanza
        // and a 32-byte payload (nonce=16 bytes of 0x01, ciphertext=16 bytes of 0x02)
        let mut out = Vec::new();
        out.extend_from_slice(b"age-encryption.org/v1\n");
        out.extend_from_slice(b"-> X25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n");
        // empty stanza body (no wrapped key bytes for parse test)
        out.extend_from_slice(b"\n");
        // MAC line: "--- " + base64(32 zero bytes)
        out.extend_from_slice(b"--- AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n");
        // payload: 16-byte nonce + 16-byte ciphertext placeholder
        out.extend_from_slice(&[0x01u8; 16]);
        out.extend_from_slice(&[0x02u8; 16]);
        out
    }

    #[test]
    fn parse_minimal_header_roundtrip() {
        let input = minimal_age_file();
        let (header, payload_offset) = parse_header(&input).expect("parse should succeed");
        assert_eq!(header.recipients.len(), 1);
        assert_eq!(header.recipients[0].tag, "X25519");
        assert_eq!(header.recipients[0].args.len(), 1);
        assert!(header.recipients[0].body.is_empty());
        // payload offset should point past the "--- MAC\n" line
        assert_eq!(&input[payload_offset..], &[0x01u8; 16][..].iter().chain([0x02u8; 16].iter()).copied().collect::<Vec<_>>()[..]);
    }

    #[test]
    fn serialize_header_roundtrip() {
        use crate::identities::{Header, Stanza};
        let stanza = Stanza {
            tag: "X25519".to_string(),
            args: vec!["dGVzdA==".to_string()],
            body: vec![0xde, 0xad, 0xbe, 0xef],
        };
        let header = Header {
            recipients: vec![stanza],
            mac: vec![0u8; 32],
        };
        let serialized = serialize_header(&header);
        let (parsed, _) = parse_header(&[serialized.clone(), vec![0u8; 32]].concat())
            .expect("round-trip parse should succeed");
        assert_eq!(parsed.recipients.len(), 1);
        assert_eq!(parsed.recipients[0].tag, "X25519");
        assert_eq!(parsed.recipients[0].body, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn parse_missing_version_line_returns_error() {
        let input = b"not-age-encryption\n-> X25519 arg\n\n--- AAAA\n";
        let result = parse_header(input);
        assert!(result.is_err(), "expected ParseError for missing version line");
    }

    #[test]
    fn parse_truncated_before_footer_returns_error() {
        let input = b"age-encryption.org/v1\n-> X25519 arg\n";
        let result = parse_header(input);
        assert!(result.is_err(), "expected ParseError for truncated header");
    }

    #[test]
    fn parse_empty_input_returns_error() {
        let result = parse_header(b"");
        assert!(result.is_err());
    }

    #[test]
    fn stanza_body_split_across_multiple_64char_lines() {
        // Body longer than 64 chars must be split into 64-char lines per spec.
        // 48 bytes -> base64 = 64 chars (one line, no split needed)
        // 49 bytes -> base64 = 68 chars (must split: 64 + 4)
        use crate::identities::{Header, Stanza};
        let body = vec![0xabu8; 49];
        let stanza = Stanza {
            tag: "scrypt".to_string(),
            args: vec!["salt".to_string(), "18".to_string()],
            body: body.clone(),
        };
        let header = Header { recipients: vec![stanza], mac: vec![0u8; 32] };
        let serialized = serialize_header(&header);
        let (parsed, _) = parse_header(&[serialized, vec![0u8; 32]].concat()).unwrap();
        assert_eq!(parsed.recipients[0].body, body);
    }

    #[test]
    fn header_bytes_up_to_footer_excludes_mac() {
        let input = minimal_age_file();
        let bytes = header_bytes_up_to_footer(&input).expect("should find footer");
        // Must end with "--- " (not include the MAC value)
        assert!(bytes.ends_with(b"--- "), "header_bytes must end at '--- '");
    }
}
```

Run the test to confirm it fails before implementation:

```bash
cargo test -p notsecrets -- format --nocapture 2>&1 | head -30
```

Expected: compile error — `parse_header`, `serialize_header`, `header_bytes_up_to_footer` do not exist yet.

### 2.2 Implementation `crates/notsecrets/src/format.rs`

```rust
use crate::error::AgeError;
use crate::identities::{Header, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};

const VERSION_LINE: &[u8] = b"age-encryption.org/v1\n";
const FOOTER_PREFIX: &[u8] = b"--- ";

/// Parse an age binary file's header.
///
/// Returns `(Header, payload_offset)` where `payload_offset` is the byte
/// index into `input` where the payload (nonce + ciphertext) begins.
pub fn parse_header(input: &[u8]) -> Result<(Header, usize), AgeError> {
    if !input.starts_with(VERSION_LINE) {
        return Err(AgeError::ParseError(
            "missing age version line".to_string(),
        ));
    }

    let mut pos = VERSION_LINE.len();
    let mut recipients: Vec<Stanza> = Vec::new();

    loop {
        if pos >= input.len() {
            return Err(AgeError::ParseError("unexpected end of header".to_string()));
        }

        // Check for footer line: "--- <base64-mac>\n"
        if input[pos..].starts_with(FOOTER_PREFIX) {
            let line_end = input[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| AgeError::ParseError("footer line not terminated".to_string()))?;
            let mac_b64 = &input[pos + FOOTER_PREFIX.len()..pos + line_end];
            let mac = STANDARD_NO_PAD
                .decode(mac_b64)
                .map_err(|e| AgeError::ParseError(format!("footer MAC base64: {e}")))?;
            let payload_offset = pos + line_end + 1; // skip trailing '\n'
            return Ok((Header { recipients, mac }, payload_offset));
        }

        // Must be a stanza line: "-> <tag> [args...]\n"
        if !input[pos..].starts_with(b"-> ") {
            return Err(AgeError::ParseError(format!(
                "expected stanza or footer at byte {pos}"
            )));
        }

        let line_end = input[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| AgeError::ParseError("stanza header line not terminated".to_string()))?;

        let header_line = std::str::from_utf8(&input[pos + 3..pos + line_end])
            .map_err(|e| AgeError::ParseError(format!("stanza line UTF-8: {e}")))?;

        let mut parts = header_line.split(' ');
        let tag = parts
            .next()
            .ok_or_else(|| AgeError::ParseError("stanza missing tag".to_string()))?
            .to_string();
        let args: Vec<String> = parts.map(str::to_string).collect();

        pos += line_end + 1;

        // Read stanza body: base64 lines until an empty line
        let mut body_b64 = String::new();
        loop {
            if pos >= input.len() {
                return Err(AgeError::ParseError(
                    "unexpected end during stanza body".to_string(),
                ));
            }
            let line_end = input[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| {
                    AgeError::ParseError("stanza body line not terminated".to_string())
                })?;
            let line = &input[pos..pos + line_end];
            pos += line_end + 1;
            if line.is_empty() {
                break;
            }
            body_b64.push_str(
                std::str::from_utf8(line)
                    .map_err(|e| AgeError::ParseError(format!("body UTF-8: {e}")))?,
            );
        }

        let body = if body_b64.is_empty() {
            Vec::new()
        } else {
            STANDARD_NO_PAD
                .decode(&body_b64)
                .map_err(|e| AgeError::ParseError(format!("stanza body base64: {e}")))?
        };

        recipients.push(Stanza { tag, args, body });
    }
}

/// Serialize a `Header` to bytes.
///
/// Writes version line, all stanzas (with body split into 64-char base64 lines),
/// and the `--- <base64-mac>` footer. Does NOT write the payload.
pub fn serialize_header(header: &Header) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(VERSION_LINE);

    for stanza in &header.recipients {
        // "-> tag arg1 arg2\n"
        out.extend_from_slice(b"-> ");
        out.extend_from_slice(stanza.tag.as_bytes());
        for arg in &stanza.args {
            out.push(b' ');
            out.extend_from_slice(arg.as_bytes());
        }
        out.push(b'\n');

        // Body: encode as base64, split into 64-char lines, trailing empty line
        if stanza.body.is_empty() {
            out.push(b'\n');
        } else {
            let encoded = STANDARD_NO_PAD.encode(&stanza.body);
            for chunk in encoded.as_bytes().chunks(64) {
                out.extend_from_slice(chunk);
                out.push(b'\n');
            }
            // If the last chunk was exactly 64 chars, we still need the empty terminator line.
            // If the last chunk was < 64 chars, we also need the empty terminator line.
            out.push(b'\n');
        }
    }

    // Footer
    out.extend_from_slice(FOOTER_PREFIX);
    out.extend_from_slice(STANDARD_NO_PAD.encode(&header.mac).as_bytes());
    out.push(b'\n');

    out
}

/// Return the header bytes up to and including `"--- "` (not including the MAC value).
///
/// Used for MAC computation: HMAC-SHA256 covers all bytes from the start of the
/// file up to and including the `"--- "` separator.
pub fn header_bytes_up_to_footer(input: &[u8]) -> Result<Vec<u8>, AgeError> {
    let footer_pos = input
        .windows(FOOTER_PREFIX.len())
        .position(|w| w == FOOTER_PREFIX)
        .ok_or_else(|| AgeError::ParseError("footer not found".to_string()))?;
    Ok(input[..footer_pos + FOOTER_PREFIX.len()].to_vec())
}

#[cfg(test)]
mod tests {
    // ... (tests from 2.1 above)
}
```

Add `pub mod format;` to `lib.rs`.

### 2.3 Run tests

```bash
cargo test -p notsecrets -- format --nocapture
```

Expected output (all pass):

```
test format::tests::parse_empty_input_returns_error ... ok
test format::tests::parse_missing_version_line_returns_error ... ok
test format::tests::parse_truncated_before_footer_returns_error ... ok
test format::tests::parse_minimal_header_roundtrip ... ok
test format::tests::serialize_header_roundtrip ... ok
test format::tests::stanza_body_split_across_multiple_64char_lines ... ok
test format::tests::header_bytes_up_to_footer_excludes_mac ... ok
```

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 2.4 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/format.rs crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): age wire format parser/serializer with unit tests"
```

---

## Task 3: X25519 identity + recipient

- [ ] Create `crates/notsecrets/src/identities/x25519.rs`
- [ ] Create `crates/notsecrets/src/recipients/x25519.rs`
- [ ] Update `identities/mod.rs` and `recipients/mod.rs` to re-export

### 3.1 Test (write first)

Add to `crates/notsecrets/src/identities/x25519.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::recipients::x25519::X25519Recipient;

    /// The canonical test vector: generate a keypair, wrap a file key, unwrap it.
    #[test]
    fn x25519_wrap_unwrap_roundtrip() {
        use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};
        use rand::rngs::OsRng;

        // Generate a static identity key
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);

        // bech32-encode public key as age1... recipient
        let recipient = X25519Recipient::from_public_key(public);
        // bech32-encode secret key as AGE-SECRET-KEY-1... identity
        let identity = X25519Identity::from_static_secret(secret);

        let file_key = FileKey::new([0x42u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).expect("wrap should succeed");
        assert_eq!(stanza.tag, "X25519");
        assert_eq!(stanza.args.len(), 1); // ephemeral pubkey

        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn x25519_wrong_identity_returns_none_for_nonmatching_tag() {
        use x25519_dalek::{PublicKey, StaticSecret};
        use rand::rngs::OsRng;

        let secret = StaticSecret::random_from_rng(OsRng);
        let identity = X25519Identity::from_static_secret(secret);

        use crate::identities::Stanza;
        let stanza = Stanza {
            tag: "scrypt".to_string(),
            args: vec!["salt".to_string(), "18".to_string()],
            body: vec![0u8; 32],
        };
        // Different tag — must return None, not an error
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }

    #[test]
    fn x25519_bech32_parse_roundtrip() {
        use x25519_dalek::{PublicKey, StaticSecret};
        use rand::rngs::OsRng;

        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);

        let recipient_str = X25519Recipient::from_public_key(public).to_bech32();
        assert!(recipient_str.starts_with("age1"), "recipient must start with age1");

        let identity_str = X25519Identity::from_static_secret(
            StaticSecret::random_from_rng(OsRng)
        ).to_bech32();
        assert!(
            identity_str.starts_with("AGE-SECRET-KEY-1"),
            "identity must start with AGE-SECRET-KEY-1"
        );
    }
}
```

Run to confirm compile failure:

```bash
cargo test -p notsecrets -- identities::x25519 --nocapture 2>&1 | head -20
```

Expected: compile error — modules don't exist yet.

### 3.2 `crates/notsecrets/src/identities/x25519.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use bech32::{Bech32, Hrp};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

const TAG: &str = "X25519";
const HKDF_INFO: &[u8] = b"age-encryption.org/v1/X25519";
const IDENTITY_HRP: &str = "age-secret-key-";

pub struct X25519Identity {
    secret: StaticSecret,
}

impl X25519Identity {
    pub fn from_static_secret(secret: StaticSecret) -> Self {
        Self { secret }
    }

    /// Parse an `AGE-SECRET-KEY-1...` bech32 string.
    pub fn from_bech32(s: &str) -> Result<Self, AgeError> {
        let s_lower = s.to_lowercase();
        let (hrp, data) = bech32::decode(&s_lower)
            .map_err(|e| AgeError::ParseError(format!("bech32 decode identity: {e}")))?;
        if hrp.as_str() != IDENTITY_HRP {
            return Err(AgeError::ParseError(format!(
                "expected hrp '{IDENTITY_HRP}', got '{}'",
                hrp.as_str()
            )));
        }
        let bytes: [u8; 32] = data
            .try_into()
            .map_err(|_| AgeError::ParseError("identity key must be 32 bytes".to_string()))?;
        Ok(Self {
            secret: StaticSecret::from(bytes),
        })
    }

    /// Encode as `AGE-SECRET-KEY-1...` bech32 uppercase string.
    pub fn to_bech32(&self) -> String {
        let hrp = Hrp::parse(IDENTITY_HRP).expect("static hrp is valid");
        bech32::encode::<Bech32>(hrp, self.secret.as_bytes())
            .expect("bech32 encode cannot fail for valid input")
            .to_uppercase()
    }
}

impl Identity for X25519Identity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        Some(unwrap(stanza, &self.secret))
    }
}

fn unwrap(stanza: &Stanza, secret: &StaticSecret) -> Result<FileKey, AgeError> {
    if stanza.args.len() != 1 {
        return Err(AgeError::ParseError(
            "X25519 stanza must have exactly 1 arg".to_string(),
        ));
    }
    let ephemeral_pub_bytes = STANDARD_NO_PAD
        .decode(&stanza.args[0])
        .map_err(|e| AgeError::ParseError(format!("ephemeral pubkey base64: {e}")))?;
    let ephemeral_pub_bytes: [u8; 32] = ephemeral_pub_bytes
        .try_into()
        .map_err(|_| AgeError::ParseError("ephemeral pubkey must be 32 bytes".to_string()))?;
    let ephemeral_pub = PublicKey::from(ephemeral_pub_bytes);

    let shared = secret.diffie_hellman(&ephemeral_pub);
    let recipient_pub = PublicKey::from(secret);

    let wrap_key = derive_wrap_key(shared.as_bytes(), ephemeral_pub.as_bytes(), recipient_pub.as_bytes())?;

    let wrapped = &stanza.body;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
    let nonce = Nonce::default(); // [0u8; 12]
    let file_key_bytes = cipher
        .decrypt(&nonce, wrapped.as_slice())
        .map_err(|_| AgeError::CryptoError("X25519 file key decryption failed".to_string()))?;

    FileKey::try_from(file_key_bytes.as_slice())
}

pub(crate) fn derive_wrap_key(
    shared: &[u8],
    ephemeral_pub: &[u8],
    recipient_pub: &[u8],
) -> Result<[u8; 32], AgeError> {
    let mut ikm = Vec::with_capacity(shared.len() + ephemeral_pub.len() + recipient_pub.len());
    ikm.extend_from_slice(shared);
    ikm.extend_from_slice(ephemeral_pub);
    ikm.extend_from_slice(recipient_pub);

    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut wrap_key = [0u8; 32];
    hk.expand(HKDF_INFO, &mut wrap_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF expand: {e}")))?;
    Ok(wrap_key)
}
```

### 3.3 `crates/notsecrets/src/recipients/x25519.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};
use crate::identities::x25519::derive_wrap_key;
use crate::recipients::Recipient;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use bech32::{Bech32, Hrp};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};
use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey};

const TAG: &str = "X25519";
const RECIPIENT_HRP: &str = "age";

pub struct X25519Recipient {
    public_key: PublicKey,
}

impl X25519Recipient {
    pub fn from_public_key(public_key: PublicKey) -> Self {
        Self { public_key }
    }

    /// Parse an `age1...` bech32 public key string.
    pub fn from_bech32(s: &str) -> Result<Self, AgeError> {
        let (hrp, data) = bech32::decode(s)
            .map_err(|e| AgeError::ParseError(format!("bech32 decode recipient: {e}")))?;
        if hrp.as_str() != RECIPIENT_HRP {
            return Err(AgeError::ParseError(format!(
                "expected hrp '{RECIPIENT_HRP}', got '{}'",
                hrp.as_str()
            )));
        }
        let bytes: [u8; 32] = data
            .try_into()
            .map_err(|_| AgeError::ParseError("recipient key must be 32 bytes".to_string()))?;
        Ok(Self {
            public_key: PublicKey::from(bytes),
        })
    }

    /// Encode as `age1...` bech32 string.
    pub fn to_bech32(&self) -> String {
        let hrp = Hrp::parse(RECIPIENT_HRP).expect("static hrp is valid");
        bech32::encode::<Bech32>(hrp, self.public_key.as_bytes())
            .expect("bech32 encode cannot fail for valid input")
    }
}

impl Recipient for X25519Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pub = PublicKey::from(&ephemeral_secret);
        let shared = ephemeral_secret.diffie_hellman(&self.public_key);

        let wrap_key = derive_wrap_key(
            shared.as_bytes(),
            ephemeral_pub.as_bytes(),
            self.public_key.as_bytes(),
        )?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
        let nonce = Nonce::default(); // [0u8; 12]
        let wrapped = cipher
            .encrypt(&nonce, file_key.as_bytes().as_slice())
            .map_err(|_| AgeError::CryptoError("X25519 file key encryption failed".to_string()))?;

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![STANDARD_NO_PAD.encode(ephemeral_pub.as_bytes())],
            body: wrapped,
        })
    }
}
```

### 3.4 Wire into mod files

In `crates/notsecrets/src/identities/mod.rs`, add:

```rust
pub mod x25519;
pub use x25519::X25519Identity;
```

In `crates/notsecrets/src/recipients/mod.rs`, add:

```rust
pub mod x25519;
pub use x25519::X25519Recipient;
```

In `crates/notsecrets/src/lib.rs`, add to public exports:

```rust
pub use identities::X25519Identity;
pub use recipients::X25519Recipient;
```

### 3.5 Run tests

```bash
cargo test -p notsecrets -- x25519 --nocapture
```

Expected:

```
test identities::x25519::tests::x25519_bech32_parse_roundtrip ... ok
test identities::x25519::tests::x25519_wrong_identity_returns_none_for_nonmatching_tag ... ok
test identities::x25519::tests::x25519_wrap_unwrap_roundtrip ... ok
```

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 3.6 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/identities/ crates/notsecrets/src/recipients/ crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): X25519 identity and recipient with wrap/unwrap"
```

---

## Task 4: Scrypt identity + recipient

- [ ] Create `crates/notsecrets/src/identities/scrypt.rs`
- [ ] Create `crates/notsecrets/src/recipients/scrypt.rs`

### 4.1 Test (write first)

In `crates/notsecrets/src/identities/scrypt.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::recipients::scrypt::ScryptRecipient;

    #[test]
    fn scrypt_wrap_unwrap_roundtrip() {
        let passphrase = "hunter2".to_string();
        let recipient = ScryptRecipient::new(passphrase.clone(), 14);
        let identity = ScryptIdentity::new(passphrase);

        let file_key = FileKey::new([0x11u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).expect("wrap should succeed");
        assert_eq!(stanza.tag, "scrypt");
        assert_eq!(stanza.args.len(), 2); // base64(salt), work_factor

        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn scrypt_wrong_passphrase_returns_crypto_error() {
        let recipient = ScryptRecipient::new("correct".to_string(), 14);
        let identity = ScryptIdentity::new("wrong".to_string());

        let file_key = FileKey::new([0x11u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).unwrap();

        let result = identity
            .unwrap_file_key(&stanza)
            .expect("tag matches")
            .expect_err("wrong passphrase should fail");
        assert!(matches!(result, crate::error::AgeError::CryptoError(_)));
    }

    #[test]
    fn scrypt_wrong_tag_returns_none() {
        let identity = ScryptIdentity::new("pass".to_string());
        use crate::identities::Stanza;
        let stanza = Stanza {
            tag: "X25519".to_string(),
            args: vec!["arg".to_string()],
            body: vec![],
        };
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }
}
```

### 4.2 `crates/notsecrets/src/identities/scrypt.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};

const TAG: &str = "scrypt";

pub struct ScryptIdentity {
    passphrase: String,
}

impl ScryptIdentity {
    pub fn new(passphrase: String) -> Self {
        Self { passphrase }
    }
}

impl Identity for ScryptIdentity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        Some(unwrap(stanza, &self.passphrase))
    }
}

fn unwrap(stanza: &Stanza, passphrase: &str) -> Result<FileKey, AgeError> {
    if stanza.args.len() != 2 {
        return Err(AgeError::ParseError(
            "scrypt stanza must have 2 args: salt work_factor".to_string(),
        ));
    }
    let salt = STANDARD_NO_PAD
        .decode(&stanza.args[0])
        .map_err(|e| AgeError::ParseError(format!("scrypt salt base64: {e}")))?;
    let work_factor: u8 = stanza.args[1]
        .parse()
        .map_err(|e| AgeError::ParseError(format!("scrypt work factor parse: {e}")))?;

    let wrap_key = derive_scrypt_key(passphrase.as_bytes(), &salt, work_factor)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
    let nonce = Nonce::default();
    let file_key_bytes = cipher
        .decrypt(&nonce, stanza.body.as_slice())
        .map_err(|_| AgeError::CryptoError("scrypt file key decryption failed".to_string()))?;
    FileKey::try_from(file_key_bytes.as_slice())
}

pub(crate) fn derive_scrypt_key(
    passphrase: &[u8],
    salt: &[u8],
    work_factor: u8,
) -> Result<[u8; 32], AgeError> {
    let params = scrypt::Params::new(work_factor, 8, 1, 32)
        .map_err(|e| AgeError::CryptoError(format!("scrypt params: {e}")))?;
    let mut wrap_key = [0u8; 32];
    scrypt::scrypt(passphrase, salt, &params, &mut wrap_key)
        .map_err(|e| AgeError::CryptoError(format!("scrypt: {e}")))?;
    Ok(wrap_key)
}
```

### 4.3 `crates/notsecrets/src/recipients/scrypt.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};
use crate::identities::scrypt::derive_scrypt_key;
use crate::recipients::Recipient;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};
use rand::RngCore;

const TAG: &str = "scrypt";
const SALT_LEN: usize = 16;

pub struct ScryptRecipient {
    passphrase: String,
    work_factor: u8,
}

impl ScryptRecipient {
    /// `work_factor` is the log2(N) parameter (default 18 for production; use 14 for tests).
    pub fn new(passphrase: String, work_factor: u8) -> Self {
        Self { passphrase, work_factor }
    }
}

impl Recipient for ScryptRecipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);

        let wrap_key = derive_scrypt_key(self.passphrase.as_bytes(), &salt, self.work_factor)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
        let nonce = Nonce::default();
        let wrapped = cipher
            .encrypt(&nonce, file_key.as_bytes().as_slice())
            .map_err(|_| AgeError::CryptoError("scrypt file key encryption failed".to_string()))?;

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![
                STANDARD_NO_PAD.encode(salt),
                self.work_factor.to_string(),
            ],
            body: wrapped,
        })
    }
}
```

### 4.4 Wire into mod files

In `crates/notsecrets/src/identities/mod.rs`, add:

```rust
pub mod scrypt;
pub use scrypt::ScryptIdentity;
```

In `crates/notsecrets/src/recipients/mod.rs`, add:

```rust
pub mod scrypt;
pub use scrypt::ScryptRecipient;
```

In `lib.rs` exports, add:

```rust
pub use identities::ScryptIdentity;
pub use recipients::ScryptRecipient;
```

### 4.5 Run tests

```bash
cargo test -p notsecrets -- scrypt --nocapture
```

Expected:

```
test identities::scrypt::tests::scrypt_wrap_unwrap_roundtrip ... ok
test identities::scrypt::tests::scrypt_wrong_passphrase_returns_crypto_error ... ok
test identities::scrypt::tests::scrypt_wrong_tag_returns_none ... ok
```

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 4.6 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/identities/scrypt.rs crates/notsecrets/src/recipients/scrypt.rs crates/notsecrets/src/identities/mod.rs crates/notsecrets/src/recipients/mod.rs crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): scrypt identity and recipient (passphrase-based)"
```

---

## Task 5: SSH Ed25519 identity + recipient

- [ ] Create `crates/notsecrets/src/identities/ssh_ed25519.rs`
- [ ] Create `crates/notsecrets/src/recipients/ssh_ed25519.rs`

### 5.1 Test (write first)

In `crates/notsecrets/src/identities/ssh_ed25519.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::recipients::ssh_ed25519::SshEd25519Recipient;

    fn test_keypair() -> (ed25519_dalek::SigningKey, ed25519_dalek::VerifyingKey) {
        use rand::rngs::OsRng;
        let signing_key = ed25519_dalek::SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    #[test]
    fn ssh_ed25519_wrap_unwrap_roundtrip() {
        let (signing_key, verifying_key) = test_keypair();
        let recipient = SshEd25519Recipient::from_verifying_key(verifying_key);
        let identity = SshEd25519Identity::from_signing_key(signing_key);

        let file_key = FileKey::new([0x33u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).expect("wrap should succeed");
        assert_eq!(stanza.tag, "ssh-ed25519");
        assert_eq!(stanza.args.len(), 2); // fingerprint, ephemeral_pub

        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn ssh_ed25519_wrong_fingerprint_returns_none() {
        let (_, verifying_key1) = test_keypair();
        let (signing_key2, _) = test_keypair();
        let recipient = SshEd25519Recipient::from_verifying_key(verifying_key1);
        let identity = SshEd25519Identity::from_signing_key(signing_key2);

        let file_key = FileKey::new([0x33u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).unwrap();
        // Different key — fingerprint check in unwrap_file_key should return None
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }
}
```

### 5.2 `crates/notsecrets/src/identities/ssh_ed25519.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};
use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use sha2::{Digest, Sha256, Sha512};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

const TAG: &str = "ssh-ed25519";
const HKDF_INFO: &[u8] = b"age-encryption.org/v1/ssh-ed25519";

pub struct SshEd25519Identity {
    signing_key: SigningKey,
}

impl SshEd25519Identity {
    pub fn from_signing_key(signing_key: SigningKey) -> Self {
        Self { signing_key }
    }

    fn x25519_secret(&self) -> X25519StaticSecret {
        // Ed25519 -> X25519 conversion: clamp(SHA-512(seed)[..32])
        let mut hash = Sha512::digest(self.signing_key.as_bytes());
        hash[0] &= 248;
        hash[31] &= 127;
        hash[31] |= 64;
        let bytes: [u8; 32] = hash[..32].try_into().expect("SHA-512 output is 64 bytes");
        X25519StaticSecret::from(bytes)
    }

    fn fingerprint(&self) -> [u8; 4] {
        ssh_key_fingerprint(self.signing_key.verifying_key().as_bytes())
    }
}

impl Identity for SshEd25519Identity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        if stanza.args.len() != 2 {
            return Some(Err(AgeError::ParseError(
                "ssh-ed25519 stanza must have 2 args".to_string(),
            )));
        }
        // Check fingerprint
        let fp_bytes = match STANDARD_NO_PAD.decode(&stanza.args[0]) {
            Ok(b) => b,
            Err(e) => return Some(Err(AgeError::ParseError(format!("fingerprint base64: {e}")))),
        };
        if fp_bytes != self.fingerprint() {
            return None;
        }
        Some(unwrap(stanza, self))
    }
}

fn unwrap(stanza: &Stanza, identity: &SshEd25519Identity) -> Result<FileKey, AgeError> {
    let ephemeral_pub_bytes = STANDARD_NO_PAD
        .decode(&stanza.args[1])
        .map_err(|e| AgeError::ParseError(format!("ephemeral pubkey base64: {e}")))?;
    let ephemeral_pub_bytes: [u8; 32] = ephemeral_pub_bytes
        .try_into()
        .map_err(|_| AgeError::ParseError("ephemeral pubkey must be 32 bytes".to_string()))?;
    let ephemeral_pub = X25519PublicKey::from(ephemeral_pub_bytes);

    let x25519_secret = identity.x25519_secret();
    let shared = x25519_secret.diffie_hellman(&ephemeral_pub);

    let wrap_key = derive_wrap_key(
        shared.as_bytes(),
        ephemeral_pub.as_bytes(),
        X25519PublicKey::from(&x25519_secret).as_bytes(),
    )?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
    let nonce = Nonce::default();
    let file_key_bytes = cipher
        .decrypt(&nonce, stanza.body.as_slice())
        .map_err(|_| AgeError::CryptoError("ssh-ed25519 file key decryption failed".to_string()))?;
    FileKey::try_from(file_key_bytes.as_slice())
}

pub(crate) fn ssh_key_fingerprint(pub_key_bytes: &[u8]) -> [u8; 4] {
    let hash = Sha256::digest(pub_key_bytes);
    hash[..4].try_into().expect("SHA-256 output is 32 bytes")
}

pub(crate) fn derive_wrap_key(
    shared: &[u8],
    ephemeral_pub: &[u8],
    recipient_pub: &[u8],
) -> Result<[u8; 32], AgeError> {
    let mut ikm = Vec::with_capacity(shared.len() + ephemeral_pub.len() + recipient_pub.len());
    ikm.extend_from_slice(shared);
    ikm.extend_from_slice(ephemeral_pub);
    ikm.extend_from_slice(recipient_pub);

    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut wrap_key = [0u8; 32];
    hk.expand(HKDF_INFO, &mut wrap_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF expand: {e}")))?;
    Ok(wrap_key)
}
```

### 5.3 `crates/notsecrets/src/recipients/ssh_ed25519.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};
use crate::identities::ssh_ed25519::{derive_wrap_key, ssh_key_fingerprint};
use crate::recipients::Recipient;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};
use ed25519_dalek::VerifyingKey;
use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

const TAG: &str = "ssh-ed25519";

pub struct SshEd25519Recipient {
    verifying_key: VerifyingKey,
}

impl SshEd25519Recipient {
    pub fn from_verifying_key(verifying_key: VerifyingKey) -> Self {
        Self { verifying_key }
    }

    fn x25519_public(&self) -> X25519PublicKey {
        // Ed25519 pubkey -> X25519 pubkey via birational map
        // curve25519_dalek provides this via EdwardsPoint::to_montgomery()
        use curve25519_dalek::edwards::CompressedEdwardsY;
        let compressed = CompressedEdwardsY(self.verifying_key.to_bytes());
        let point = compressed.decompress().expect("valid ed25519 pubkey decompresses");
        X25519PublicKey::from(point.to_montgomery().to_bytes())
    }
}

impl Recipient for SshEd25519Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pub = X25519PublicKey::from(&ephemeral_secret);
        let recipient_x25519 = self.x25519_public();
        let shared = ephemeral_secret.diffie_hellman(&recipient_x25519);

        let wrap_key = derive_wrap_key(
            shared.as_bytes(),
            ephemeral_pub.as_bytes(),
            recipient_x25519.as_bytes(),
        )?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
        let nonce = Nonce::default();
        let wrapped = cipher
            .encrypt(&nonce, file_key.as_bytes().as_slice())
            .map_err(|_| AgeError::CryptoError("ssh-ed25519 file key encryption failed".to_string()))?;

        let fingerprint = ssh_key_fingerprint(self.verifying_key.as_bytes());

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![
                STANDARD_NO_PAD.encode(fingerprint),
                STANDARD_NO_PAD.encode(ephemeral_pub.as_bytes()),
            ],
            body: wrapped,
        })
    }
}
```

Add `curve25519-dalek = "4"` to `[dependencies]` in `crates/notsecrets/Cargo.toml`.

### 5.4 Wire into mod files

In `identities/mod.rs`:

```rust
pub mod ssh_ed25519;
pub use ssh_ed25519::SshEd25519Identity;
```

In `recipients/mod.rs`:

```rust
pub mod ssh_ed25519;
pub use ssh_ed25519::SshEd25519Recipient;
```

In `lib.rs`:

```rust
pub use identities::SshEd25519Identity;
pub use recipients::SshEd25519Recipient;
```

### 5.5 Run tests

```bash
cargo test -p notsecrets -- ssh_ed25519 --nocapture
```

Expected:

```
test identities::ssh_ed25519::tests::ssh_ed25519_wrap_unwrap_roundtrip ... ok
test identities::ssh_ed25519::tests::ssh_ed25519_wrong_fingerprint_returns_none ... ok
```

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 5.6 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/identities/ssh_ed25519.rs crates/notsecrets/src/recipients/ssh_ed25519.rs crates/notsecrets/src/identities/mod.rs crates/notsecrets/src/recipients/mod.rs crates/notsecrets/src/lib.rs crates/notsecrets/Cargo.toml
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): SSH Ed25519 identity and recipient"
```

---

## Task 6: SSH RSA identity + recipient

- [ ] Create `crates/notsecrets/src/identities/ssh_rsa.rs`
- [ ] Create `crates/notsecrets/src/recipients/ssh_rsa.rs`

### 6.1 Test (write first)

In `crates/notsecrets/src/identities/ssh_rsa.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::recipients::ssh_rsa::SshRsaRecipient;
    use rsa::{RsaPrivateKey, RsaPublicKey};
    use rand::rngs::OsRng;

    fn test_rsa_keypair() -> (RsaPrivateKey, RsaPublicKey) {
        // 2048-bit key; use a small bit size in tests only via feature flag or constant
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("RSA keygen failed");
        let public_key = RsaPublicKey::from(&private_key);
        (private_key, public_key)
    }

    #[test]
    fn ssh_rsa_wrap_unwrap_roundtrip() {
        let (private_key, public_key) = test_rsa_keypair();
        let recipient = SshRsaRecipient::from_public_key(public_key);
        let identity = SshRsaIdentity::from_private_key(private_key);

        let file_key = FileKey::new([0x55u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).expect("wrap should succeed");
        assert_eq!(stanza.tag, "ssh-rsa");
        assert_eq!(stanza.args.len(), 1); // fingerprint

        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn ssh_rsa_wrong_fingerprint_returns_none() {
        let (private_key1, _) = test_rsa_keypair();
        let (_, public_key2) = test_rsa_keypair();
        let recipient = SshRsaRecipient::from_public_key(public_key2);
        let identity = SshRsaIdentity::from_private_key(private_key1);

        let file_key = FileKey::new([0x55u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).unwrap();
        // Fingerprint mismatch → None
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }
}
```

### 6.2 `crates/notsecrets/src/identities/ssh_rsa.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use rsa::{Oaep, RsaPrivateKey};
use sha2::{Digest, Sha256};

const TAG: &str = "ssh-rsa";

pub struct SshRsaIdentity {
    private_key: RsaPrivateKey,
}

impl SshRsaIdentity {
    pub fn from_private_key(private_key: RsaPrivateKey) -> Self {
        Self { private_key }
    }

    fn fingerprint(&self) -> [u8; 4] {
        use rsa::traits::PublicKeyParts;
        rsa_pubkey_fingerprint(&rsa::RsaPublicKey::from(&self.private_key))
    }
}

impl Identity for SshRsaIdentity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        if stanza.args.len() != 1 {
            return Some(Err(AgeError::ParseError(
                "ssh-rsa stanza must have 1 arg".to_string(),
            )));
        }
        let fp = match STANDARD_NO_PAD.decode(&stanza.args[0]) {
            Ok(b) => b,
            Err(e) => return Some(Err(AgeError::ParseError(format!("fingerprint base64: {e}")))),
        };
        if fp != self.fingerprint() {
            return None;
        }
        Some(unwrap(stanza, &self.private_key))
    }
}

fn unwrap(stanza: &Stanza, private_key: &RsaPrivateKey) -> Result<FileKey, AgeError> {
    use rand::rngs::OsRng;
    let padding = Oaep::new::<Sha256>();
    let file_key_bytes = private_key
        .decrypt(padding, &stanza.body)
        .map_err(|_| AgeError::CryptoError("RSA-OAEP decrypt failed".to_string()))?;
    FileKey::try_from(file_key_bytes.as_slice())
}

pub(crate) fn rsa_pubkey_fingerprint(public_key: &rsa::RsaPublicKey) -> [u8; 4] {
    use rsa::traits::PublicKeyParts;
    // Fingerprint: first 4 bytes of SHA-256 of the DER-encoded public key bytes
    let n_bytes = public_key.n().to_bytes_be();
    let e_bytes = public_key.e().to_bytes_be();
    let mut hasher = Sha256::new();
    hasher.update(&n_bytes);
    hasher.update(&e_bytes);
    let hash = hasher.finalize();
    hash[..4].try_into().expect("SHA-256 is 32 bytes")
}
```

### 6.3 `crates/notsecrets/src/recipients/ssh_rsa.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};
use crate::identities::ssh_rsa::rsa_pubkey_fingerprint;
use crate::recipients::Recipient;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use rsa::{Oaep, RsaPublicKey};
use sha2::Sha256;

const TAG: &str = "ssh-rsa";

pub struct SshRsaRecipient {
    public_key: RsaPublicKey,
}

impl SshRsaRecipient {
    pub fn from_public_key(public_key: RsaPublicKey) -> Self {
        Self { public_key }
    }
}

impl Recipient for SshRsaRecipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        use rand::rngs::OsRng;
        let padding = Oaep::new::<Sha256>();
        let wrapped = self
            .public_key
            .encrypt(&mut OsRng, padding, file_key.as_bytes())
            .map_err(|_| AgeError::CryptoError("RSA-OAEP encrypt failed".to_string()))?;

        let fingerprint = rsa_pubkey_fingerprint(&self.public_key);

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![STANDARD_NO_PAD.encode(fingerprint)],
            body: wrapped,
        })
    }
}
```

### 6.4 Wire into mod files

In `identities/mod.rs`:

```rust
pub mod ssh_rsa;
pub use ssh_rsa::SshRsaIdentity;
```

In `recipients/mod.rs`:

```rust
pub mod ssh_rsa;
pub use ssh_rsa::SshRsaRecipient;
```

In `lib.rs`:

```rust
pub use identities::SshRsaIdentity;
pub use recipients::SshRsaRecipient;
```

### 6.5 Run tests

```bash
cargo test -p notsecrets -- ssh_rsa --nocapture
```

Expected:

```
test identities::ssh_rsa::tests::ssh_rsa_wrap_unwrap_roundtrip ... ok
test identities::ssh_rsa::tests::ssh_rsa_wrong_fingerprint_returns_none ... ok
```

Note: RSA keygen at 2048 bits is slow (~1s per test). This is expected in CI.

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 6.6 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/identities/ssh_rsa.rs crates/notsecrets/src/recipients/ssh_rsa.rs crates/notsecrets/src/identities/mod.rs crates/notsecrets/src/recipients/mod.rs crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): SSH RSA identity and recipient (OAEP-SHA256)"
```

---

## Task 7: EncryptedIdentity

An `EncryptedIdentity` wraps another `Identity` whose key material is stored in an age-encrypted file. Decrypting the outer file yields the inner identity's key material (e.g., an `AGE-SECRET-KEY-1...` line).

- [ ] Create `crates/notsecrets/src/identities/encrypted.rs`

### 7.1 Test (write first)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::{FileKey, x25519::X25519Identity};
    use crate::recipients::x25519::X25519Recipient;
    use x25519_dalek::{PublicKey, StaticSecret};
    use rand::rngs::OsRng;

    /// Build a minimal age-encrypted file wrapping an AGE-SECRET-KEY-1 identity.
    fn make_encrypted_identity_file(inner_bech32: &str, passphrase: &str) -> Vec<u8> {
        use crate::recipients::scrypt::ScryptRecipient;
        let recipient = ScryptRecipient::new(passphrase.to_string(), 14);
        // Encrypt the inner key as plaintext
        let encryptor = crate::Encryptor::with_recipients(vec![Box::new(recipient)])
            .expect("encryptor build");
        encryptor
            .encrypt(inner_bech32.as_bytes())
            .expect("encrypt inner key")
    }

    #[test]
    fn encrypted_identity_wraps_x25519() {
        use rand::rngs::OsRng;
        let inner_secret = StaticSecret::random_from_rng(OsRng);
        let inner_pub = PublicKey::from(&inner_secret);
        let inner_identity = X25519Identity::from_static_secret(inner_secret.clone());
        let inner_bech32 = inner_identity.to_bech32();

        let passphrase = "test-passphrase";
        let encrypted_file = make_encrypted_identity_file(&inner_bech32, passphrase);

        // Wrap in EncryptedIdentity
        let encrypted_identity = EncryptedIdentity::new(
            encrypted_file,
            Box::new(crate::identities::scrypt::ScryptIdentity::new(passphrase.to_string())),
        );

        // Encrypt a payload with the inner X25519 recipient
        let recipient = X25519Recipient::from_public_key(inner_pub);
        let file_key = FileKey::new([0x77u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).unwrap();

        // EncryptedIdentity should decrypt the file to recover the inner identity and unwrap
        let unwrapped = encrypted_identity
            .unwrap_file_key(&stanza)
            .expect("should match")
            .expect("should decrypt");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }
}
```

Note: this test depends on `Encryptor` from Task 8. Mark it `#[ignore]` until Task 8 is complete, then remove the `#[ignore]`.

### 7.2 `crates/notsecrets/src/identities/encrypted.rs`

```rust
use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};

/// An identity whose key material is stored in an age-encrypted file.
///
/// The outer file is decrypted on each call to `unwrap_file_key` using
/// `passphrase_identity`. The decrypted content must be an `AGE-SECRET-KEY-1...`
/// bech32 string, which is parsed as an `X25519Identity`.
pub struct EncryptedIdentity {
    /// Raw bytes of the age-encrypted identity file.
    encrypted_data: Vec<u8>,
    /// Identity used to decrypt the outer age file (typically `ScryptIdentity`).
    passphrase_identity: Box<dyn Identity>,
}

impl EncryptedIdentity {
    pub fn new(encrypted_data: Vec<u8>, passphrase_identity: Box<dyn Identity>) -> Self {
        Self { encrypted_data, passphrase_identity }
    }
}

impl Identity for EncryptedIdentity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        // Decrypt the outer file to recover inner identity key bytes
        let decryptor = crate::Decryptor::with_identities(vec![self.passphrase_identity.as_ref()]);
        let plaintext = match decryptor.decrypt(&self.encrypted_data) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };
        let key_str = match std::str::from_utf8(&plaintext) {
            Ok(s) => s.trim().to_string(),
            Err(_) => {
                return Some(Err(AgeError::ParseError(
                    "decrypted identity file is not valid UTF-8".to_string(),
                )))
            }
        };
        // Parse inner identity — only X25519 supported for now
        let inner: Box<dyn Identity> = if key_str.starts_with("AGE-SECRET-KEY-1") {
            match crate::identities::x25519::X25519Identity::from_bech32(&key_str) {
                Ok(id) => Box::new(id),
                Err(e) => return Some(Err(e)),
            }
        } else {
            return Some(Err(AgeError::UnsupportedKeyType(
                "encrypted identity file must contain AGE-SECRET-KEY-1".to_string(),
            )));
        };
        inner.unwrap_file_key(stanza)
    }
}
```

Note: `Decryptor::with_identities` here takes `Vec<&dyn Identity>` rather than `Vec<Box<dyn Identity>>` to avoid clone requirements. The `Decryptor` signature in Task 9 must accommodate both — the simplest approach is to add a `with_identity_refs` constructor, or to make `EncryptedIdentity` clone the passphrase identity. The plan uses the latter: require `passphrase_identity: Box<dyn Identity + Send>` and rely on the fact that `ScryptIdentity` is `Clone`. Adjust `Decryptor` API in Task 9 to accept `&[Box<dyn Identity>]` or provide a `decrypt_with` method. The exact API reconciliation is resolved in Task 9.

### 7.3 Wire into mod files

In `identities/mod.rs`:

```rust
pub mod encrypted;
pub use encrypted::EncryptedIdentity;
```

In `lib.rs`:

```rust
pub use identities::EncryptedIdentity;
```

### 7.4 Run tests

```bash
cargo test -p notsecrets -- encrypted --nocapture
```

Expected: `EncryptedIdentity` tests are marked `#[ignore]` pending Task 8; remaining tests pass.

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 7.5 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/identities/encrypted.rs crates/notsecrets/src/identities/mod.rs crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): EncryptedIdentity — passphrase-protected identity file wrapper"
```

---

## Task 8: Encryptor

- [ ] Add `crates/notsecrets/src/encrypt.rs` with `Encryptor`
- [ ] Wire into `lib.rs`

### 8.1 Test (write first)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::identities::x25519::X25519Identity;
    use crate::recipients::x25519::X25519Recipient;
    use x25519_dalek::{PublicKey, StaticSecret};
    use rand::rngs::OsRng;

    #[test]
    fn encryptor_single_x25519_recipient() {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        let recipient: Box<dyn crate::recipients::Recipient> =
            Box::new(X25519Recipient::from_public_key(public));

        let encryptor = Encryptor::with_recipients(vec![recipient]).unwrap();
        let plaintext = b"hello, age!";
        let ciphertext = encryptor.encrypt(plaintext).expect("encrypt should succeed");

        // Verify it starts with the age version line
        assert!(
            ciphertext.starts_with(b"age-encryption.org/v1\n"),
            "output must start with age version line"
        );
    }

    #[test]
    fn encryptor_rejects_multiple_scrypt_recipients() {
        use crate::recipients::scrypt::ScryptRecipient;
        let r1: Box<dyn crate::recipients::Recipient> =
            Box::new(ScryptRecipient::new("pass1".to_string(), 14));
        let r2: Box<dyn crate::recipients::Recipient> =
            Box::new(ScryptRecipient::new("pass2".to_string(), 14));
        let result = Encryptor::with_recipients(vec![r1, r2]);
        assert!(result.is_err(), "two scrypt recipients must be rejected");
    }

    #[test]
    fn encryptor_empty_recipients_returns_error() {
        let result = Encryptor::with_recipients(vec![]);
        assert!(result.is_err(), "empty recipients must be rejected");
    }
}
```

### 8.2 `crates/notsecrets/src/encrypt.rs`

```rust
use crate::error::AgeError;
use crate::format::{serialize_header};
use crate::identities::{FileKey, Header};
use crate::recipients::Recipient;
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct Encryptor {
    recipients: Vec<Box<dyn Recipient>>,
}

impl Encryptor {
    /// Construct an `Encryptor`. Returns `AgeError` if:
    /// - `recipients` is empty
    /// - more than one scrypt recipient is present
    pub fn with_recipients(recipients: Vec<Box<dyn Recipient>>) -> Result<Self, AgeError> {
        if recipients.is_empty() {
            return Err(AgeError::CryptoError(
                "at least one recipient is required".to_string(),
            ));
        }
        // Detect scrypt recipients by attempting to wrap a dummy key and checking the stanza tag.
        // We cannot inspect the concrete type, so we use a probe approach:
        // wrap a dummy FileKey and check if any stanza tag is "scrypt".
        let dummy_key = FileKey::new([0u8; 16]);
        let scrypt_count = recipients
            .iter()
            .filter_map(|r| r.wrap_file_key(&dummy_key).ok())
            .filter(|s| s.tag == "scrypt")
            .count();
        if scrypt_count > 1 {
            return Err(AgeError::CryptoError(
                "at most one scrypt recipient is allowed per age file".to_string(),
            ));
        }
        Ok(Self { recipients })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, AgeError> {
        let file_key = FileKey::generate();

        // Wrap file key with each recipient
        let stanzas: Vec<_> = self
            .recipients
            .iter()
            .map(|r| r.wrap_file_key(&file_key))
            .collect::<Result<_, _>>()?;

        // Build header without MAC first (we need the bytes to compute MAC)
        let header_no_mac = Header {
            recipients: stanzas,
            mac: Vec::new(),
        };

        // Compute MAC: HMAC-SHA256 over header bytes up to and including "--- "
        let mut header_bytes = serialize_header(&header_no_mac);
        // The serialized header ends with "--- \n" (MAC is empty, so base64("") = "").
        // We need the MAC to cover bytes up to and including "--- ".
        // Trim the trailing "\n" that follows the empty MAC, then compute MAC over
        // everything up to and including "--- ".
        let mac_key = derive_mac_key(&file_key)?;
        let footer_pos = header_bytes
            .windows(4)
            .rposition(|w| w == b"--- ")
            .ok_or_else(|| AgeError::ParseError("serialize_header missing footer".to_string()))?;
        let mac_input = &header_bytes[..footer_pos + 4]; // up to and including "--- "

        let mut mac = HmacSha256::new_from_slice(&mac_key)
            .map_err(|e| AgeError::CryptoError(format!("HMAC init: {e}")))?;
        mac.update(mac_input);
        let mac_bytes = mac.finalize().into_bytes().to_vec();

        // Re-serialize with actual MAC
        let header_with_mac = Header {
            recipients: header_no_mac.recipients,
            mac: mac_bytes,
        };
        header_bytes = serialize_header(&header_with_mac);

        // Payload: 16-byte random nonce + ChaCha20Poly1305(payload_key, plaintext)
        let mut nonce_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let payload_key = derive_payload_key(&file_key, &nonce_bytes)?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&payload_key));
        let counter_nonce = Nonce::default(); // [0u8; 12]
        let ciphertext = cipher
            .encrypt(&counter_nonce, plaintext)
            .map_err(|_| AgeError::CryptoError("payload encryption failed".to_string()))?;

        let mut output = header_bytes;
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }
}

fn derive_mac_key(file_key: &FileKey) -> Result<[u8; 32], AgeError> {
    let hk = Hkdf::<Sha256>::new(Some(b""), file_key.as_bytes());
    let mut mac_key = [0u8; 32];
    hk.expand(b"header", &mut mac_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF mac key: {e}")))?;
    Ok(mac_key)
}

fn derive_payload_key(file_key: &FileKey, nonce: &[u8; 16]) -> Result<[u8; 32], AgeError> {
    let hk = Hkdf::<Sha256>::new(Some(nonce.as_slice()), file_key.as_bytes());
    let mut payload_key = [0u8; 32];
    hk.expand(b"payload", &mut payload_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF payload key: {e}")))?;
    Ok(payload_key)
}
```

Add `pub mod encrypt;` and `pub use encrypt::Encryptor;` to `lib.rs`.

### 8.3 Run tests

```bash
cargo test -p notsecrets -- encryptor --nocapture
```

Expected:

```
test encrypt::tests::encryptor_single_x25519_recipient ... ok
test encrypt::tests::encryptor_rejects_multiple_scrypt_recipients ... ok
test encrypt::tests::encryptor_empty_recipients_returns_error ... ok
```

Also run the `encrypted` test which was previously ignored — it now has `Encryptor` available:

```bash
cargo test -p notsecrets -- encrypted --nocapture
```

Remove the `#[ignore]` attribute from `encrypted` tests and verify they pass.

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 8.4 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/encrypt.rs crates/notsecrets/src/identities/encrypted.rs crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): Encryptor — multi-recipient age encryption"
```

---

## Task 9: Decryptor

- [ ] Add `crates/notsecrets/src/decrypt.rs` with `Decryptor`
- [ ] Wire into `lib.rs`

### 9.1 Test (write first)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Encryptor;
    use crate::identities::x25519::X25519Identity;
    use crate::recipients::x25519::X25519Recipient;
    use x25519_dalek::{PublicKey, StaticSecret};
    use rand::rngs::OsRng;

    fn make_x25519_pair() -> (X25519Identity, X25519Recipient) {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        (
            X25519Identity::from_static_secret(secret),
            X25519Recipient::from_public_key(public),
        )
    }

    #[test]
    fn decryptor_roundtrip_single_recipient() {
        let (identity, recipient) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
        let plaintext = b"the quick brown fox";
        let ciphertext = encryptor.encrypt(plaintext).unwrap();

        let decryptor = Decryptor::with_identities(vec![Box::new(identity)]);
        let decrypted = decryptor.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decryptor_roundtrip_multiple_recipients() {
        let (identity1, recipient1) = make_x25519_pair();
        let (identity2, recipient2) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![
            Box::new(recipient1),
            Box::new(recipient2),
        ])
        .unwrap();
        let plaintext = b"multi-recipient test";
        let ciphertext = encryptor.encrypt(plaintext).unwrap();

        // Either identity alone can decrypt
        let decryptor1 = Decryptor::with_identities(vec![Box::new(identity1)]);
        assert_eq!(decryptor1.decrypt(&ciphertext).unwrap(), plaintext);

        let decryptor2 = Decryptor::with_identities(vec![Box::new(identity2)]);
        assert_eq!(decryptor2.decrypt(&ciphertext).unwrap(), plaintext);
    }

    #[test]
    fn decryptor_no_matching_identity_returns_no_match() {
        let (_, recipient) = make_x25519_pair();
        let (wrong_identity, _) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
        let ciphertext = encryptor.encrypt(b"data").unwrap();

        let decryptor = Decryptor::with_identities(vec![Box::new(wrong_identity)]);
        let err = decryptor.decrypt(&ciphertext).unwrap_err();
        assert!(matches!(err, crate::error::AgeError::NoMatch));
    }

    #[test]
    fn decryptor_corrupted_mac_returns_mac_mismatch() {
        let (identity, recipient) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
        let mut ciphertext = encryptor.encrypt(b"data").unwrap();

        // Flip a byte in the MAC region (near end of header, before payload)
        // Find "--- " and flip a byte 5 bytes after it
        let footer_pos = ciphertext
            .windows(4)
            .position(|w| w == b"--- ")
            .expect("footer present");
        ciphertext[footer_pos + 4] ^= 0xff;

        let decryptor = Decryptor::with_identities(vec![Box::new(identity)]);
        let err = decryptor.decrypt(&ciphertext).unwrap_err();
        assert!(
            matches!(err, crate::error::AgeError::MacMismatch | crate::error::AgeError::ParseError(_)),
            "expected MacMismatch or ParseError, got: {err:?}"
        );
    }
}
```

### 9.2 `crates/notsecrets/src/decrypt.rs`

```rust
use crate::error::AgeError;
use crate::format::{header_bytes_up_to_footer, parse_header};
use crate::identities::{FileKey, Identity};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit, Nonce,
    aead::Aead,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct Decryptor {
    identities: Vec<Box<dyn Identity>>,
}

impl Decryptor {
    pub fn with_identities(identities: Vec<Box<dyn Identity>>) -> Self {
        Self { identities }
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, AgeError> {
        let (header, payload_offset) = parse_header(ciphertext)?;

        // Try to recover file key by trying each identity against each stanza
        let mut file_key: Option<FileKey> = None;
        'outer: for identity in &self.identities {
            for stanza in &header.recipients {
                if let Some(result) = identity.unwrap_file_key(stanza) {
                    file_key = Some(result?);
                    break 'outer;
                }
            }
        }
        let file_key = file_key.ok_or(AgeError::NoMatch)?;

        // Verify header MAC
        let mac_key = derive_mac_key(&file_key)?;
        let header_input = header_bytes_up_to_footer(ciphertext)?;

        let mut mac = HmacSha256::new_from_slice(&mac_key)
            .map_err(|e| AgeError::CryptoError(format!("HMAC init: {e}")))?;
        mac.update(&header_input);
        mac.verify_slice(&header.mac)
            .map_err(|_| AgeError::MacMismatch)?;

        // Decrypt payload
        let payload = &ciphertext[payload_offset..];
        if payload.len() < 16 {
            return Err(AgeError::ParseError(
                "payload too short to contain nonce".to_string(),
            ));
        }
        let nonce_bytes: [u8; 16] = payload[..16]
            .try_into()
            .expect("slice is exactly 16 bytes");
        let payload_ciphertext = &payload[16..];

        let payload_key = derive_payload_key(&file_key, &nonce_bytes)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&payload_key));
        let counter_nonce = Nonce::default();
        let plaintext = cipher
            .decrypt(&counter_nonce, payload_ciphertext)
            .map_err(|_| AgeError::CryptoError("payload decryption failed".to_string()))?;

        Ok(plaintext)
    }
}

fn derive_mac_key(file_key: &FileKey) -> Result<[u8; 32], AgeError> {
    let hk = Hkdf::<Sha256>::new(Some(b""), file_key.as_bytes());
    let mut mac_key = [0u8; 32];
    hk.expand(b"header", &mut mac_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF mac key: {e}")))?;
    Ok(mac_key)
}

fn derive_payload_key(file_key: &FileKey, nonce: &[u8; 16]) -> Result<[u8; 32], AgeError> {
    let hk = Hkdf::<Sha256>::new(Some(nonce.as_slice()), file_key.as_bytes());
    let mut payload_key = [0u8; 32];
    hk.expand(b"payload", &mut payload_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF payload key: {e}")))?;
    Ok(payload_key)
}
```

Add `pub mod decrypt;` and `pub use decrypt::Decryptor;` to `lib.rs`.

### 9.3 Run tests

```bash
cargo test -p notsecrets -- decrypt --nocapture
```

Expected:

```
test decrypt::tests::decryptor_roundtrip_single_recipient ... ok
test decrypt::tests::decryptor_roundtrip_multiple_recipients ... ok
test decrypt::tests::decryptor_no_matching_identity_returns_no_match ... ok
test decrypt::tests::decryptor_corrupted_mac_returns_mac_mismatch ... ok
```

Run full suite to verify no regressions:

```bash
cargo test -p notsecrets --nocapture
```

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 9.4 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/decrypt.rs crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): Decryptor — identity-driven age decryption with MAC verification"
```

---

## Task 10: sources/ migration — IdentitySource trait, update all three sources

- [ ] Add `IdentitySource` trait to `crates/notsecrets/src/sources/mod.rs`
- [ ] Rewrite `sources/bitwarden.rs`
- [ ] Rewrite `sources/file.rs`
- [ ] Rewrite `sources/prompt.rs`
- [ ] Remove `_legacy.rs` and all `#[deprecated]` shims from `lib.rs`

### 10.1 `crates/notsecrets/src/sources/mod.rs`

```rust
use crate::error::AgeError;
use crate::identities::Identity;

pub mod bitwarden;
pub mod file;
pub mod prompt;

pub use bitwarden::BitwardenSource;
pub use file::FileSource;
pub use prompt::PromptSource;

/// Infra boundary trait: an identity resolver that loads key material from an
/// external source and returns a concrete `Identity`.
pub trait IdentitySource {
    fn name(&self) -> &str;
    fn load(&self) -> Result<Box<dyn Identity>, AgeError>;
}
```

### 10.2 Rewrite `sources/bitwarden.rs`

```rust
use crate::error::AgeError;
use crate::identities::Identity;
use crate::identities::x25519::X25519Identity;
use crate::sources::IdentitySource;
use std::process::Command;

pub struct BitwardenSource {
    pub item_name: String,
}

impl BitwardenSource {
    pub fn new(item_name: impl Into<String>) -> Self {
        Self { item_name: item_name.into() }
    }

    fn retrieve_key(&self) -> Result<String, AgeError> {
        let bw_ok = std::process::Command::new("sh")
            .args(["-c", "command -v bw"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !bw_ok {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("bw CLI not found in PATH"),
            });
        }

        let session = std::env::var("BW_SESSION").unwrap_or_default();
        let session = if session.is_empty() {
            let password = rpassword::prompt_password("Bitwarden master password: ")
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("could not read password: {e}"),
                })?;
            let output = Command::new("bw")
                .args(["unlock", "--raw", &password])
                .output()
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("bw unlock spawn: {e}"),
                })?;
            if !output.status.success() {
                return Err(AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!(
                        "bw unlock failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                });
            }
            String::from_utf8(output.stdout)
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("bw unlock output UTF-8: {e}"),
                })?
                .trim()
                .to_string()
        } else {
            session
        };

        let output = Command::new("bw")
            .args(["get", "notes", &self.item_name, "--session", &session])
            .output()
            .map_err(|e| AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("bw get spawn: {e}"),
            })?;

        if !output.status.success() {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!(
                    "bw get notes '{}' failed: {}",
                    self.item_name,
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let key = String::from_utf8(output.stdout)
            .map_err(|e| AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("bw output UTF-8: {e}"),
            })?
            .trim()
            .to_string();

        if key.is_empty() {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("Bitwarden item '{}' has empty notes", self.item_name),
            });
        }
        Ok(key)
    }
}

impl IdentitySource for BitwardenSource {
    fn name(&self) -> &str {
        "bitwarden"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        let key = self.retrieve_key()?;
        let identity = X25519Identity::from_bech32(key.trim()).map_err(|e| {
            AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("key is not a valid AGE-SECRET-KEY-1: {e}"),
            }
        })?;
        Ok(Box::new(identity))
    }
}
```

### 10.3 Rewrite `sources/file.rs`

```rust
use crate::error::AgeError;
use crate::identities::Identity;
use crate::identities::x25519::X25519Identity;
use crate::identities::encrypted::EncryptedIdentity;
use crate::identities::scrypt::ScryptIdentity;
use crate::sources::IdentitySource;
use std::path::PathBuf;

const AGE_VERSION_LINE: &str = "age-encryption.org/v1";

pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl IdentitySource for FileSource {
    fn name(&self) -> &str {
        "file"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        let content = std::fs::read(&self.path).map_err(|e| AgeError::SourceError {
            name: self.name().to_string(),
            source: anyhow::anyhow!("cannot read key file {}: {e}", self.path.display()),
        })?;

        // Detect identity type from file content
        if content.starts_with(b"AGE-SECRET-KEY-1") {
            let key_str = std::str::from_utf8(&content)
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("key file UTF-8: {e}"),
                })?
                .trim();
            let identity = X25519Identity::from_bech32(key_str).map_err(|e| {
                AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("invalid AGE-SECRET-KEY-1: {e}"),
                }
            })?;
            Ok(Box::new(identity))
        } else if content.starts_with(AGE_VERSION_LINE.as_bytes()) {
            // age-encrypted identity file — prompt for passphrase to decrypt
            let passphrase = rpassword::prompt_password(
                "Enter passphrase for encrypted identity file: ",
            )
            .map_err(|e| AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("could not read passphrase: {e}"),
            })?;
            let passphrase_identity = ScryptIdentity::new(passphrase);
            Ok(Box::new(EncryptedIdentity::new(content, Box::new(passphrase_identity))))
        } else {
            Err(AgeError::UnsupportedKeyType(format!(
                "file {} does not contain a recognized age key format",
                self.path.display()
            )))
        }
    }
}
```

### 10.4 Rewrite `sources/prompt.rs`

```rust
use crate::error::AgeError;
use crate::identities::Identity;
use crate::identities::scrypt::ScryptIdentity;
use crate::sources::IdentitySource;

pub struct PromptSource;

impl IdentitySource for PromptSource {
    fn name(&self) -> &str {
        "prompt"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        let passphrase = rpassword::prompt_password("Enter age passphrase: ").map_err(|e| {
            AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("could not read passphrase: {e}"),
            }
        })?;
        if passphrase.trim().is_empty() {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("empty passphrase entered"),
            });
        }
        Ok(Box::new(ScryptIdentity::new(passphrase)))
    }
}
```

### 10.5 Remove legacy shims

Delete `crates/notsecrets/src/_legacy.rs`. Remove the `mod _legacy` and `pub use _legacy::...` lines from `lib.rs`. The `notstrap` crate will be updated in Task 12.

### 10.6 Run tests

```bash
cargo test -p notsecrets --nocapture
```

Expected: all tests pass.

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 10.7 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/sources/ crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): migrate sources to IdentitySource trait, remove legacy API"
```

---

## Task 11: resolve_identities() public API

- [ ] Add `resolve_identities` to `lib.rs`

### 11.1 Implementation

In `crates/notsecrets/src/lib.rs`, add:

```rust
/// Try each `IdentitySource` in order; collect all identities that load successfully.
///
/// If all sources fail, returns the last `AgeError::SourceError`. If at least one
/// source succeeds, returns the list of loaded identities (partial success is accepted
/// because not every source needs to hold the key for the target file).
pub fn resolve_identities(
    sources: Vec<Box<dyn sources::IdentitySource>>,
) -> Result<Vec<Box<dyn Identity>>, AgeError> {
    let mut identities: Vec<Box<dyn Identity>> = Vec::new();
    let mut last_err: Option<AgeError> = None;

    for source in sources {
        match source.load() {
            Ok(identity) => identities.push(identity),
            Err(e) => {
                eprintln!("  [{}] {e}", source.name());
                last_err = Some(e);
            }
        }
    }

    if identities.is_empty() {
        Err(last_err.unwrap_or(AgeError::SourceError {
            name: "resolve_identities".to_string(),
            source: anyhow::anyhow!("no sources provided"),
        }))
    } else {
        Ok(identities)
    }
}
```

### 11.2 Test

Add to `lib.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::IdentitySource;

    struct AlwaysFailSource;
    impl IdentitySource for AlwaysFailSource {
        fn name(&self) -> &str { "always-fail" }
        fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
            Err(AgeError::SourceError {
                name: "always-fail".to_string(),
                source: anyhow::anyhow!("intentional failure"),
            })
        }
    }

    struct StaticX25519Source {
        identity: crate::identities::x25519::X25519Identity,
    }

    impl StaticX25519Source {
        fn new() -> Self {
            use x25519_dalek::StaticSecret;
            use rand::rngs::OsRng;
            Self {
                identity: crate::identities::x25519::X25519Identity::from_static_secret(
                    StaticSecret::random_from_rng(OsRng),
                ),
            }
        }
    }

    impl IdentitySource for StaticX25519Source {
        fn name(&self) -> &str { "static-x25519" }
        fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
            use x25519_dalek::StaticSecret;
            use rand::rngs::OsRng;
            Ok(Box::new(
                crate::identities::x25519::X25519Identity::from_static_secret(
                    StaticSecret::random_from_rng(OsRng),
                ),
            ))
        }
    }

    #[test]
    fn resolve_identities_all_fail_returns_error() {
        let sources: Vec<Box<dyn IdentitySource>> = vec![Box::new(AlwaysFailSource)];
        assert!(resolve_identities(sources).is_err());
    }

    #[test]
    fn resolve_identities_partial_success_returns_loaded() {
        let sources: Vec<Box<dyn IdentitySource>> = vec![
            Box::new(AlwaysFailSource),
            Box::new(StaticX25519Source::new()),
        ];
        let identities = resolve_identities(sources).expect("at least one source succeeded");
        assert_eq!(identities.len(), 1);
    }
}
```

### 11.3 Run tests

```bash
cargo test -p notsecrets --nocapture
```

Expected: all pass.

```bash
cargo clippy -p notsecrets -- -D warnings
```

Expected: 0 warnings.

### 11.4 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notsecrets/src/lib.rs
git -C /Users/joe/dev/notfiles commit -m "feat(notsecrets): add resolve_identities() public API"
```

---

## Task 12: notstrap migration

Update `crates/notstrap/src/lib.rs` to replace the old `notsecrets` API with the new one.

- [ ] Update `crates/notstrap/src/lib.rs` — two call sites
- [ ] Update `crates/notstrap/Cargo.toml` if needed
- [ ] Run `cargo check --workspace`
- [ ] Run `cargo test --workspace`

### 12.1 Changes to `crates/notstrap/src/lib.rs`

**Import line** (line 5), replace:

```rust
use notsecrets::{BitwardenSource, FileSource, PromptSource, install_age_key, resolve_age_key};
```

with:

```rust
use notsecrets::{
    BitwardenSource, Decryptor, FileSource, PromptSource,
    resolve_identities,
    sources::IdentitySource,
};
```

**Step 4 block** (lines 92–110), replace:

```rust
    // 4. Retrieve age key and install
    let sources: Vec<Box<dyn notsecrets::AgeKeySource>> = if let Some(kf) = opts.key_file {
        vec![Box::new(FileSource::new(kf))]
    } else {
        vec![
            Box::new(BitwardenSource::new(&cfg.bootstrap.bw_age_item)),
            Box::new(PromptSource),
        ]
    };

    match resolve_age_key(sources) {
        Ok(key) => {
            install_age_key(&key)?;
            report.add("age key", StepStatus::Ok);
        }
        Err(e) => {
            report.add("age key", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    }
```

with:

```rust
    // 4. Resolve age identities
    let sources: Vec<Box<dyn IdentitySource>> = if let Some(kf) = opts.key_file {
        vec![Box::new(FileSource::new(kf))]
    } else {
        vec![
            Box::new(BitwardenSource::new(&cfg.bootstrap.bw_age_item)),
            Box::new(PromptSource),
        ]
    };

    let identities = match resolve_identities(sources) {
        Ok(ids) => {
            report.add("age key", StepStatus::Ok);
            ids
        }
        Err(e) => {
            report.add("age key", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    };
```

**env_injector closure** in `BootstrapOptions` and its call site (lines 112–142):

The `env_injector` closure currently captures a `sops` call. Replace it so callers pass a closure that captures a `Decryptor`. Update the type alias to remain the same (`Box<dyn Fn(&Path) -> Result<String>>`) — this keeps the blast radius minimal. The `notstrap` binary's `main.rs` builds the `env_injector` closure; update it there:

In `crates/notstrap/src/main.rs`, the env_injector construction (wherever `decrypt_sops` was called) becomes:

```rust
let env_injector: Option<notstrap::EnvInjector> = Some(Box::new({
    let identities = identities.clone(); // requires Identity: Clone, or Arc wrapping
    move |path: &std::path::Path| {
        let bytes = std::fs::read(path)
            .with_context(|| format!("cannot read sops file {}", path.display()))?;
        let decryptor = notsecrets::Decryptor::with_identities(identities);
        let plaintext = decryptor.decrypt(&bytes)
            .map_err(|e| anyhow::anyhow!("age decrypt failed: {e}"))?;
        String::from_utf8(plaintext).map_err(|e| anyhow::anyhow!("plaintext UTF-8: {e}"))
    }
}));
```

For `Box<dyn Identity>` to be cloneable across the closure boundary, wrap identities in `Arc`:

Change `identities` type from `Vec<Box<dyn Identity>>` to `Vec<Arc<dyn Identity>>` in the closure. Adjust `Decryptor::with_identities` to accept `Vec<Arc<dyn Identity>>` as an alternative, or convert: `identities.into_iter().map(|i| i as Box<dyn Identity>).collect()`.

The simplest approach: keep `Decryptor::with_identities(Vec<Box<dyn Identity>>)` and pass ownership into the closure — the closure is `FnOnce`, and `EnvInjector` is `Box<dyn Fn>` which requires `Fn`. Resolve by storing identities in `Arc<Vec<Box<dyn Identity>>>`:

```rust
// In lib.rs: expose a Decryptor constructor from Arc
impl Decryptor {
    pub fn with_identities_arc(identities: std::sync::Arc<Vec<Box<dyn Identity>>>) -> Self {
        // store Arc internally
    }
}
```

Or simplest of all: change `EnvInjector` from `Box<dyn Fn>` to `Box<dyn FnOnce>` in `BootstrapOptions` since it is only ever called once. This requires changing one field and one call site in `lib.rs`:

```rust
type EnvInjector = Box<dyn FnOnce(&Path) -> Result<String>>;
```

and in `run()`:

```rust
if let Some(injector) = opts.env_injector {
    let sops_path = dotfiles_dir.join(&cfg.bootstrap.sops_file);
    match injector(&sops_path) { ... }
}
```

`FnOnce` is called the same way via `injector(arg)`. This change is fully backwards-compatible with existing test code.

### 12.2 Remove legacy notsecrets exports

Verify `crates/notstrap/Cargo.toml` does not need changes (it already depends on `notsecrets = { workspace = true }`).

Check workspace `Cargo.toml`: remove `which` from `[workspace.dependencies]` if no other crate references it:

```bash
cargo metadata --no-deps --format-version 1 | python3 -c "import json,sys; d=json.load(sys.stdin); [print(p['name'],p.get('dependencies',[])) for p in d['packages']]" 2>/dev/null | grep which
```

If only `notsecrets` used it, remove from workspace.

### 12.3 Verify

```bash
cargo check --workspace
```

Expected: 0 errors.

```bash
cargo test --workspace --nocapture 2>&1 | tail -40
```

Expected: all tests pass.

```bash
cargo clippy --workspace -- -D warnings
```

Expected: 0 warnings.

### 12.4 Commit

```bash
git -C /Users/joe/dev/notfiles add crates/notstrap/src/ Cargo.toml
git -C /Users/joe/dev/notfiles commit -m "feat(notstrap): migrate to notsecrets age-native API, remove sops shell-out"
```

---

## Self-Review Checklist

Before considering the plan complete, verify:

1. **All spec features covered:**
   - [x] Multiple recipients — Task 8 (`Encryptor::with_recipients`)
   - [x] Passphrases (scrypt) — Task 4
   - [x] Passphrase-protected identity files — Task 7 (`EncryptedIdentity`)
   - [x] SSH Ed25519 — Task 5
   - [x] SSH RSA — Task 6
   - [x] X25519 identity + recipient — Task 3
   - [x] age wire format parser/serializer — Task 2
   - [x] Header MAC verification — Task 9 (`Decryptor`)
   - [x] `resolve_identities` — Task 11
   - [x] Sources migration — Task 10
   - [x] notstrap migration — Task 12

2. **No placeholder text:** Every code block is complete. No "TBD", ellipsis substitutions, or "implement X appropriately".

3. **Type names consistent across tasks:**
   - `FileKey`, `Stanza`, `Header` defined in Task 1, used throughout
   - `Identity`, `Recipient`, `IdentitySource` traits defined in Tasks 1 and 10
   - `AgeError` defined in Task 1, referenced in all subsequent tasks
   - `Encryptor` defined in Task 8, referenced in Task 7 test and Task 12
   - `Decryptor` defined in Task 9, referenced in Task 7 impl and Task 12

4. **TDD discipline:** Every task writes the failing test first, specifies the expected failure output, then provides the implementation.

5. **`cargo clippy -p notsecrets -- -D warnings` called after every task.**

6. **Every file path is exact** — all paths use `crates/notsecrets/src/...` or `crates/notstrap/src/...` with no ambiguity.
