# notstrap

New-machine bootstrap orchestrator. This is the one crate that ties
`notfiles`, `notsecrets`, `nothooks`, and `notnet` together into a
single end-to-end flow.

## Flow

```text
prereqs check
  → load config           (notstrap.toml)
  → clone dotfiles
  → age key                (notsecrets)
  → decrypt SOPS           (notsecrets)
  → link dotfiles          (notfiles)
  → run hooks              (nothooks)
  → final report
```

## Configuration

`notstrap.toml` drives the flow — target repo, dotfiles directory,
which secret providers to resolve through, and hook behavior.

## Modules

- **prereqs** — checks the machine is ready to bootstrap (required
  tools present, etc.) before mutating anything.
- **repo** — clones/updates the dotfiles repository.
- **lib** — wires the pipeline stages above together.

Secrets are resolved through `notsecrets::SecretResolver` rather than
a hardcoded provider — see
[ADR 0005](../adr/0005-multi-provider-secret-resolution.md).
