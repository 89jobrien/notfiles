# HANDOFF.md

Legacy note as of 2026-04-15.

This repo no longer uses `HANDOFF.md` as a running history log.

Active workflow:

1. Sync GitHub issues into `doob`
2. Read short-lived handoff context from `.ctx/HANDOFF.*.*.yaml`
3. Update issue state in `doob`
4. Sync `doob` back to GitHub issues

Rules:

- Handoff context should be brief and ephemeral
- Only open work should appear in handoff files
- Closed issues should be removed from handoff files after sync
- Historical status belongs in GitHub issues and git history, not here

Current state:

- No open handoff items are tracked locally
- Use GitHub issues plus `doob` as the task source of truth
