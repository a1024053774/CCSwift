---
id: T-009
title: End-to-end acceptance by computer use
type: build
status: open
blocked_by: [T-008]
claimed_by:
supersedes:
---

## What to build

Release build of the app, accepted through its real UI with computer use, against the user's real accounts.

Important: Claude Desktop is hosting the session that runs you. Do NOT click a Desktop account switch and do NOT quit or restart Claude Desktop. Desktop switching is unchanged upstream code; check only that the Desktop page and its account list render as before.

## Acceptance

- [ ] Release build (`scripts/bundle-macos.sh` or the release cargo profile) launches; time from launch to first window measured, compared with the installed CC Same
- [ ] Desktop page shows the saved Desktop accounts as before (screenshot); no switch clicked
- [ ] CLI page: switch 2 -> 3 -> back to the starting slot through the window; `cswap status` + `claude auth status` agree each time (outputs saved)
- [ ] Tray menu: CLI submenu lists accounts and a switch from it works (slot 2, then back), checked with `cswap status`
- [ ] Usage page shows every account once with numbers matching `cswap list --json` for CLI
- [ ] Light and dark appearance screenshots of all three pages
- [ ] Ends with CLI on the starting account and the original CC Same app running again

## Resolution

