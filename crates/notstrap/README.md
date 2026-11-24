# notstrap

New-machine bootstrap orchestrator — ties the notfiles crates together.

Runs the full bootstrap sequence: prerequisites check → load
`notstrap.toml` → (optional interactive wizard) → join Tailscale (notnet) →
clone the dotfiles repo (git) → resolve age identities (notsecrets:
YubiKey, Bitwarden, prompt) → resolve secrets from `notsecrets.toml` into
hook environment variables → link dotfiles (notfiles) → run hooks (dot +
setup phases, via nothooks) → reconcile tools declared by the dotfiles
manifest (`[doctor]` in `notfiles.toml`, via nottools).

## CLI

```
notstrap run [--config notstrap.toml] [--force] [--key-file <age key>]
             [--dotfiles <dir>] [--yes|-y]
```

`--yes` accepts every default without prompting and tells hooks not to
prompt (`NOTSTRAP_YES=1`).

## Flow

```
notstrap run
1.  prerequisites  — `[prereqs]` commands must exist (default: nu, sops, age)
2.  wizard         — optional interactive confirm/adjustment of the plan
3.  tailscale      — `[tailscale]` section → notnet::ensure_connected
4.  clone dotfiles — git clone (skipped if the directory exists)
5.  age key        — FileSource / YubiKey / Bitwarden / prompt
6.  secrets        — resolve + validate (key, value) env pairs for hooks
7.  link dotfiles  — notfiles::link
8.  hooks          — dot phase, then setup phase
                     (setup tracked in .nothooks-state.toml)
9.  tools          — nottools reconcile against `[doctor]` in notfiles.toml
```

## Modules

- `prereqs` — `DEFAULT_REQUIRED = ["nu", "sops", "age"]`,
  `check_prerequisites`, install hints
- `repo` — `GitClone` trait, `SystemGit`, `FakeGit`, `clone_if_missing`
- `tools` — `reconcile` / `reconcile_system`, `ToolsSummary` (`installed`,
  `unresolved`, `undeclared`)
- `wizard` — `run` / `apply`, `Choices`
- `prompt` — `Prompter` trait, `AssumeYes`, `LinePrompter`
- `lib` — `run(BootstrapOptions) -> Result<Report>`, `NotstrapConfig`

## Testing

`cargo test -p notstrap`
