---
id: T-008
title: Trim and polish after first slices
type: build
status: open
blocked_by: []
claimed_by:
supersedes:
---

## What to build

Review findings on T-005..T-007 (Claude, 2026-11-08). Make the app simpler, not bigger:

1. Usage page: plan usage belongs to the account, not to Desktop or CLI. One compact row per account: email, then 5h and 7d as two short bars side by side with percent, and one small caption "<Desktop|CLI> · <age>" for whichever reading is newer. Drop the per-source blocks. Whole list for 3 accounts fits without scrolling in the default window.
2. CLI usage for the *active* CLI slot reads the live `Claude Code-credentials` item (as cswap does), not the possibly stale backup.
3. CLI page: give the slot number a fixed-width column so emails align; hide the organization line when it is just "<email>'s Organization".
4. Turn off the upstream self-updater (T-002): no update check, no download, no "Updates" section in Settings. Delete the code path rather than adding a flag, if that stays small; otherwise the smallest switch-off.
5. Remove the `eprintln!` throttle/attempt logging in CLI usage (tests already prove the throttle).
6. Tab labels "Desktop / CLI / Usage" and "Refresh" go through i18n like other strings (zh-CN at least).

## Acceptance

- [ ] Screenshot of Usage with the real accounts: one row per account, no scrolling needed
- [ ] With the update feed unreachable or not, the app makes no request to GitHub releases (log or proxy evidence)
- [ ] CLI page screenshot shows aligned emails and no redundant org line
- [ ] cargo test and clippy pass for both workspaces; net diff of this ticket is smaller in added lines than removed lines where possible

## Resolution

