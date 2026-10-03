# notcore

Shared foundation crate. Every other crate in the workspace depends on
`notcore` for the types and helpers that would otherwise be duplicated
or drift between crates.

## What lives here

- **`Config`** — the parsed `notfiles.toml` shape: global defaults
  (`include`/`exclude`, `method`, `platforms`) and per-package
  overrides.
- **`NotfilesError`** — the workspace's shared error type.
- **`Reporter`** — the output port implemented by
  `TerminalReporter`/`JsonReporter` in the `notfiles` crate.
- **`LinkEvent`** / **`Report`** — structured records of what a link,
  unlink, or status run did, used both for terminal output and
  `--json` NDJSON output.
- **`expand_tilde`** — tilde/`~` path expansion.
- **`suggest_package`** — typo suggestions for package names (used by
  `notfiles` when a requested package isn't found).
- **`HookPhase`** / **`HookSpec`** — shared vocabulary for `nothooks`'s
  dot/setup phase distinction.

## Why a separate crate

Keeping these types in `notcore` rather than in `notfiles` means
`nothooks`, `notsecrets`, `notstrap`, and other crates can depend on
the shared vocabulary (config shape, error type, reporting/event
types) without depending on the linker binary itself.
