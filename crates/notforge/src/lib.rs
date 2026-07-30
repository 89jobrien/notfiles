//! Forge lifecycle and repository provisioning for notfiles.

pub mod config;
pub mod error;
pub mod gitea;
pub mod lifecycle;
pub mod ports;

/// Current `notforge` crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
