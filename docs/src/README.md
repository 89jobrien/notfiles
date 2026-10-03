# notfiles

A modern dotfiles manager written in Rust — a pure Rust alternative to
GNU Stow. It symlinks (or copies) files from organized "package"
directories into a target location (typically `~`).

`notfiles` is a Cargo workspace of small, single-purpose crates rather
than one monolithic binary:

- **[notfiles](./crates/notfiles.md)** — the linker itself, and the CLI
  users actually run.
- **[notcore](./crates/notcore.md)** — shared types used across every
  other crate.
- **[notsecrets](./crates/notsecrets.md)** — secret resolution and
  native age/SOPS-compatible encryption.
- **[nothooks](./crates/nothooks.md)** — Nushell hook execution around
  linking and bootstrap.
- **[notnet](./crates/notnet.md)** — Tailscale and Yubikey integration.
- **[notstrap](./crates/notstrap.md)** — new-machine bootstrap,
  orchestrating all of the above.
- **[notgraph](./crates/notgraph.md)** — dependency/import graph
  generation for the workspace itself.
- **[notforge](./crates/notforge.md)** — Gitea API integration.

See the [Architecture Overview](./architecture/overview.md) for how
these fit together, and [Architecture Decision Records](./adr/README.md)
for the reasoning behind the major structural choices.
