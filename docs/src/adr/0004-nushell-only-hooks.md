# 4. Nushell-only hook scripts

Date: 2026-07-08

## Status

Accepted

## Context

`nothooks` runs per-package setup/dot scripts during linking and
bootstrap. Supporting both POSIX shell and Nushell hooks would mean
detecting shebangs/extensions, handling two quoting and error-handling
models, and testing both on every platform.

## Decision

All hook scripts are Nushell (`.nu`), executed via `nu <script>`. No
`.sh` scripts are supported anywhere in the workspace, including CI.
`HookRunner` recognizes two phases — `HookPhase::Dot` (always runs) and
`HookPhase::Setup` (runs once, tracked in `.nothooks-state.toml`,
re-run with `--force`).

## Consequences

One scripting language to test, document, and reason about across
macOS and Linux targets. Users who only know POSIX shell have to learn
Nushell syntax for custom hooks. `nu` becomes a required runtime
dependency for anyone using hooks (already true for `notgraph` output
and CI).
