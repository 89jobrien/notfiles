# 2. Hexagonal architecture with FileStore and Reporter ports

Date: 2026-07-08

## Status

Accepted

## Context

`notfiles` needs to run against a real filesystem in production but be
fully testable without touching disk, and needs to emit output in both
human-readable (terminal) and machine-readable (JSON/NDJSON) forms
without duplicating linker/status logic.

## Decision

Adopt a ports-and-adapters (hexagonal) structure. Core logic depends on
two traits — `FileStore` (filesystem I/O) and `Reporter` (output) —
defined in `ports`. Concrete adapters live in `adapters/`:
`FileStoreImpl` (real fs), `InMemoryFileStore` (tests),
`TerminalReporter` (ANSI), `JsonReporter` (NDJSON). Business logic in
`linker`, `package`, `status`, etc. never calls `std::fs` or prints
directly — it only calls through the ports.

## Consequences

Linker/status/adopt logic gets full unit-test coverage against
`InMemoryFileStore` without touching disk. Adding a new output format
or storage backend means writing a new adapter, not editing core logic.
The tradeoff is an extra layer of indirection (trait objects/generics)
for what is otherwise a small CLI tool.
