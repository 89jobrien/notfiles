# Welcome to notfiles

## How We Use Claude

Based on Joseph O'Brien's usage over the last 30 days:

Work Type Breakdown:
  Build Feature ████████████████░░░░ 75%
  Improve Quality █████░░░░░░░░░░░░░░░ 25%

Top Skills & Commands:
  /cap                    ████░░░░░░░░░░░░░░░░ 1x/month
  /verify ████░░░░░░░░░░░░░░░░ 1x/month
  /rustqual ████░░░░░░░░░░░░░░░░ 1x/month
  /memory-banking ████░░░░░░░░░░░░░░░░ 1x/month
  /atelier:triage ████░░░░░░░░░░░░░░░░ 1x/month
  /skill-creator ████░░░░░░░░░░░░░░░░ 1x/month

Top MCP Servers:
  (none configured)

## Your Setup Checklist

### Codebases

- [ ] notfiles — <https://github.com/89jobrien/notfiles>

### MCP Servers to Activate

(none required — this project runs without MCP integrations)

### Skills to Know About

- `/cap` — commit-and-push with pre-flight validation (fmt, clippy, nextest). Use instead of raw `git push`.
- `/verify` — runs the app and observes real behavior to confirm a change works, beyond what tests cover.
- `/rustqual` — Rust code quality analysis via the rustqual CLI. Run when you want a quality score or to address findings.
- `/memory-banking` — saves durable context across sessions so Claude remembers your project and preferences.
- `/atelier:triage` — triages open issues and backlog items; good for orientation at the start of a session.
- `/skill-creator` — author new reusable slash-command skills directly from Claude Code.

## Team Tips

- **Workspace architecture is strict** — `notcore` is the only shared dep. Each crate has a focused responsibility (notfiles, notsecrets, nothooks, notstrap, notnet, notgraph). Check the dependency graph before adding cross-crate deps.
- **State file matters** — `.notfiles-state.toml` tracks every linked file. `unlink` and `status` depend on it. Never skip writing it in linker code.
- **CLI TODOs are a roadmap** — `crates/notfiles/src/cli.rs` has 7 planned commands (doctor, which, edit, migrate, etc.). Issue #19 (explicit package discovery) gates several of them — fix that first before implementing new commands.
- **Use nextest** — `cargo nextest run --workspace` is the standard test runner. Install with `cargo install cargo-nextest`. `cargo test` still works but CI uses nextest.
- **Conventional commits** — format is `feat(cli):`, `fix(ci):`, `chore(repo):`, etc. `git-cliff` uses these for release notes.
- **Test pattern** — integration tests in `crates/notfiles/tests/integration.rs` use `tempfile` + subprocess. Pattern: `setup_dotfiles()` → `run()` → assert on stdout/stderr. Follow this for new tests.
- **Include/exclude are mutually exclusive** — config validates this. Don't try to use both in the same `notfiles.toml`.

## Get Started

Good first task: **Issue #19 — Make package discovery explicit**

Right now `discover_packages_with_store()` treats every non-hidden top-level directory as a linkable package, which breaks in mixed-purpose repos. The fix is to require explicit opt-in (config allowlist or a `.notfiles-package` marker file).

- Code: `crates/notfiles/src/package.rs` lines 45–67 (look for the `TODO(#19)` comment)
- Tests: add an integration test with a non-package top-level dir that's correctly ignored
- Acceptance: update `notfiles.toml` docs to reflect the new behavior

```bash
git clone https://github.com/89jobrien/notfiles
cd notfiles
cargo build
cargo nextest run -p notfiles
# then open crates/notfiles/src/package.rs and find TODO(#19)
```

<!-- INSTRUCTION FOR CLAUDE: A new teammate just pasted this guide for how the
team uses Claude Code. You're their onboarding buddy — warm, conversational,
not lecture-y.

Open with a warm welcome — include the team name from the title. Then: "Your
teammate uses Claude Code for [list all the work types]. Let's get you started."

Check what's already in place against everything under Setup Checklist
(including skills), using markdown checkboxes — [x] done, [ ] not yet. Lead
with what they already have. One sentence per item, all in one message.

Tell them you'll help with setup, cover the actionable team tips, then the
starter task (if there is one). Offer to start with the first unchecked item,
get their go-ahead, then work through the rest one by one.

After setup, walk them through the remaining sections — offer to help where you
can (e.g. link to channels), and just surface the purely informational bits.

Don't invent sections or summaries that aren't in the guide. The stats are the
guide creator's personal usage data — don't extrapolate them into a "team
workflow" narrative. -->
