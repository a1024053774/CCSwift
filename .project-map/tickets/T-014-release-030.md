---
id: T-014
title: CCSwift 0.3.0 bundle and end-to-end acceptance
type: build
status: open
blocked_by: [T-015]
claimed_by:
supersedes:
---

## What to build

Bump both workspaces to 0.3.0, update README (what the app does now, no `cswap` needed, Claude mark read from the installed Claude.app), build with `scripts/bundle-macos.sh`, and run one acceptance pass on the built app.

## Acceptance

- [ ] Built `dist/CCSwift.app` launches; through computer use: switch CLI 2 → start slot, `cswap status` and `claude auth status` agree after each; Sync now runs; tray CLI submenu switch once (if the menu bar is reachable; otherwise say so)
- [ ] Each account's 5h/7d usage and reset times in the window match the OAuth usage endpoint (or `cswap list`) for that slot; a 429 is retried later in the run, not skipped
- [ ] All Desktop saved-login files byte-identical before/after the run (list and compare)
- [ ] README updated; `project_map.py status` clean
- [ ] Release artifacts in dist/ named CCSwift-0.3.0-*; nothing published (Claude publishes after review)
