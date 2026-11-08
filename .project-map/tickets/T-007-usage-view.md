---
id: T-007
title: Unified usage view
type: build
status: open
blocked_by: [T-005]
claimed_by:
supersedes:
---

## What to build

One usage list for every account (Desktop and CLI, joined by email): 5-hour and weekly percent as small bars, with how old each reading is. Desktop numbers from Desktop's file, CLI numbers from the OAuth usage endpoint on open and on a refresh button (T-004).

## Acceptance

- [ ] With the real accounts, every account appears once; numbers for CLI accounts match `cswap list` within a few percent
- [ ] Offline: CLI rows show their last reading or "unavailable", the window stays responsive
- [ ] No request more than once a minute per account (log or test proves it)

## Resolution

