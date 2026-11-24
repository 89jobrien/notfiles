# Handoff — notfiles (2026-11-21)

**Branch:** test/link-conflict-coverage | **Build:** unknown | **Tests:** unknown

## Items

| ID | P | Status | Title |
|---|---|---|---|
| push-pr-nottools-doctor | P1 | open | Open the PR for feat/nottools-doctor |
| confirm-global-hook-source | P2 | open | Confirm where the global pre-commit hook is sourced from |
| env-provider-blocked-keys | P2 | open | Decide Env secrets provider policy for blocked keys |
| wire-secrets-config-cli | P2 | open | Wire secrets_config into the notstrap CLI |
| filestore-metadata-contract | P3 | open | FileStore metadata returns an unfakeable type |

## Log

- 20260606.135918: done=13 running=0 pending=0 blocked=0
- 2026-11-21: SOLID ports (nothooks, notnet, notstrap git, notfiles detect), conformance and property tests, fuzz crate, expect() cleanup, Python removed, notstrap secrets without unsafe set_var [d5d0ba3, 6cec7f8, b60378f, 6489ece, 5b0ad8a, 35aa9df, 650e22e, 577bfb3, 3696ae9, 261a973, 3b9b39d, ed6e6bc]
- 2026-07-08: hexagonal architecture refactor with adapters and integration tests, notsecrets cleanup, config payload moved to notfiles-config repo [6427d18, 25624c9]
- 2026-04-17: Added notnet crate with YubikeySource and threaded Tailscale into notstrap; design spec complete
- 2026-04-15: Hardened bootstrap and hook-state failure handling, made notfiles FileStore usage real, and fixed copy-drift safety coverage
