# 3. Native age/SOPS-compatible encryption, no external binaries

Date: 2026-07-08

## Status

Accepted

## Context

Secrets need to be encrypted at rest and decrypted during bootstrap
(`notstrap`). Shelling out to `age` and `sops` binaries would add
install-time dependencies to every target machine and make argument
handling (and injection risk) a recurring concern.

## Decision

Implement age encryption/decryption natively in `notsecrets`: x25519,
SSH ed25519/RSA, and scrypt identity support, plus SOPS-compatible file
handling, without invoking external `age` or `sops` processes.

## Consequences

New machines need no `age`/`sops` install step, and there is no
`Command`-injection surface for secret handling. This shifts crypto
correctness and maintenance onto `notsecrets` itself — a bug or missed
upstream format change has to be caught and fixed here rather than
inherited from an upstream binary release. A 2026-04-11 audit
confirmed no argument-injection risk from remaining `Command` call
sites elsewhere in the workspace.
