//! Shared helpers for the integration tests.

use notsecrets::X25519Identity;
use std::path::Path;
use x25519_dalek::StaticSecret;

/// Writes a throwaway age identity to `path`.
///
/// The key is derived from a fixed seed at run time, so tests are deterministic and no
/// key-shaped literal lives in the repository. It protects nothing.
pub fn write_throwaway_age_key(path: &Path) {
    let identity = X25519Identity::from_static_secret(StaticSecret::from([0x42; 32]));
    std::fs::write(path, format!("{}\n", identity.to_bech32())).expect("write throwaway age key");
}
