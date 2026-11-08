---
id: T-005
title: CLI page lists accounts
type: build
status: open
blocked_by: []
claimed_by:
supersedes:
---

## What to build

A second page in the app window, "CLI", reached by a two-segment switch (Desktop | CLI) at the top. It lists the CLI accounts from claude-swap's store in slot order with email, organization and which one is active. Desktop page unchanged. Missing store shows one line telling the user to run `cswap add`.

## Acceptance

- [ ] With the user's real store, the page shows the same accounts and active marker as `cswap list` (screenshot + `cswap list` output saved)
- [ ] With an empty temp HOME, the page shows the empty-state line, no crash
- [ ] `cargo test` and `cargo clippy` pass for the workspace
- [ ] Window opens to first paint as fast as before (no blocking Keychain or network call on open)

## Resolution

