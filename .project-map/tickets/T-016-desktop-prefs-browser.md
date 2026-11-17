---
id: T-016
title: Carry per-account Desktop preferences and keep side-browser sign-ins across switches
type: build
status: open
blocked_by: []
claimed_by: claude-2026-11-16
supersedes:
---

## What to build

User request, 2026-11-16: when switching Desktop accounts, keep the side browser's site sign-ins and carry settings to the next account. Research (same day, Claude 5.11492.5, read from its app.asar):

- The side browser's storage is Claude's own `preferences.launchPreviewStorage` (`none` default: in-memory, cleared at startup; `shared`: one on-disk jar `Partitions/launch-preview-cowork-shared`, not keyed by account). The switch moves only the main session's items, never `Partitions/`, so with `shared` the sign-ins survive restarts and switches. Open tabs are memory-only; not kept.
- Machine-wide preferences in `claude_desktop_config.json` already survive a switch. A few are keyed by account UUID.

Build:

- Setting `keepBrowserSignIns` (default off). When on, each Desktop switch sets `launchPreviewStorage` to `shared` while Claude is stopped. Off leaves Claude's value alone.
- On a switch between two accounts, copy an allowlist of per-account entries from the account left to the target: `epitaxyPrefs` keys `code-projects-order.`, `autoResumeRateLimitOptIn.`, `epitaxy-auto-default-notice-seen.`, `epitaxy-auto-entry-warning-seen.`, `epitaxy-auto-setup-band-seen.`, `epitaxy-keep-awake-band-used.` + `<uuid>`; `coworkModelAutoFallbackByAccount[<uuid>]`. Not copied: permission modes and acks, bypass gates, capability caches, rate-limit runtime state. A missing source entry never deletes the target's.
- Exact bytes of the old file kept under `<state>/logins/desktop-prefs-before/` before writing. Runs before the sign-in swap; an error refuses the switch before anything moved.
- Settings row in Settings › Sync, with text in every app language.

## Acceptance

- [x] Unit tests for the allowlist, no deletion, opt-in browser value, backup bytes, through `logins::switch` with the fake Desktop; each fails on a mutant
- [x] cargo test + clippy pass for both workspaces
- [x] Bundle built and installed to /Applications (previous app kept); the setting row shows in the real app
- [ ] Real switch with the setting on keeps a site sign-in in the side browser (user-run; needs Claude to quit)

## Resolution

Progress 2026-11-16 (not closed: the last box needs a user-run switch with the setting on).

- Core `desktop_prefs` runs in `quit_and_swap` before any sign-in moves; 6 new tests. Mutants
  (permission key copied, browser set without opt-in, missing source overwrites target, backup not
  the original bytes, carry error ignored) each fail a test. Core 63+26 and app 18 tests pass;
  clippy clean (only the third-party `block v0.1.6` future-incompat note).
- CCSwift 0.3.5 built with `scripts/bundle-macos.sh` (log `work/bundle-0.3.5.log`), installed to
  /Applications; 0.3.4 kept at `work/CCSwift-0.3.4-backup.app`. Settings › Sync shows the new row,
  off by default (full-screen screenshot).
- A real user switch zoeyyan49 → firelite182 on 0.3.5 copied `epitaxy-auto-setup-band-seen` to the
  target and kept the old file in `logins/desktop-prefs-before/`; nothing else changed by CCSwift.

