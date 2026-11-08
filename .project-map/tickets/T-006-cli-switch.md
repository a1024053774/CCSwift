---
id: T-006
title: CLI page switches account
type: build
status: open
blocked_by: [T-005]
claimed_by:
supersedes:
---

## What to build

Clicking a non-active CLI account switches the CLI to it, the same way `cswap switch <n>` does, without touching Claude Desktop or its saved logins. The current active credential is saved back into its slot first, so token refreshes are not lost. Also in the tray menu as a "CLI" submenu.

## Acceptance

- [ ] Real switch A→B: `cswap status` and `claude auth status` (or `/status`) show B; switch back B→A shows A; no login prompt either way
- [ ] Desktop's signed-in account and `<state>/logins` unchanged across the CLI switch
- [ ] A failure mid-switch (e.g. Keychain write refused) leaves the previous account active
- [ ] Tests cover the store read/write against a fixture copied from claude-swap's format

## Resolution

