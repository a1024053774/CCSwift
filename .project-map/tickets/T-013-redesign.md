---
id: T-013
title: Rebuild the window to the CCSwift design
type: build
status: open
blocked_by: [T-012]
claimed_by:
supersedes:
---

## What to build

Decision: [T-011](T-011-redesign-decisions.md). Design reference: `work/design/Main.dc.html` (its `renderVals()` holds every zh/en string and both theme token sets; copy those values), `Many.dc.html` (dark, English, 8 accounts), `AddFlow.dc.html`, `Backups.dc.html`.

1. Window 720×660 default, min 640×480. Title bar: name + Claude mark (load `TrayIconTemplate@2x.png`/`@3x` from `/Applications/Claude.app/Contents/Resources/` at runtime, tint `#D97757`; missing → no mark; never copy it into the repo or bundle). Right side: "usage updated …" + refresh + settings.
2. Desktop | CLI segmented tabs; right side: search (only past 4 accounts), sort by 5h, "+ Add" (CLI: T-012 dialog; Desktop: the existing sign-in flow).
3. Current-account card: translucent Claude-orange gradient (gpui `linear_gradient`, inset highlight, soft orange shadow), white text, initial avatar on white glass, email, plan line, "Next →" (existing next-account actions).
4. Desktop tab only: Session sync card: Sync now (existing `sync_now`), Background sync switch (existing `set_background`; off unless the user turns it on; nothing turns it on automatically), Backups row with latest snapshot time and count, opening the Backups dialog: list snapshots (time, reason, account count, size computed off the UI thread), Back up now (new manual snapshot), Restore (existing confirm/restore path; Claude must be quit), Reveal in Finder, keep 10/20/50 (`keep_snapshots`). Say truthfully when automatic snapshots happen (read `snapshot_every_seconds`; do not claim "before every sync" unless it is).
5. Account table for the selected tab: initial avatar (active = orange), email, plan/slot, 5h and 7d block bars (10 blocks, ≥80% orange) with percent and reset time, status (T-012 for CLI; saved / sign-in needed for Desktop), action (In use pill in soft orange / Switch / Sign in again in orange). Remove the Usage page and the rings picture.
6. Theme: exactly the two token sets in `Main.dc.html`. Fonts: Latin Times New Roman, Chinese Songti SC (if gpui cannot cascade, use Songti SC for zh and Times New Roman for en); weights regular, medium at most; no bold white text in dark mode.
7. Strings: zh-CN and en for every new string in `locales/app.yml`; remove strings no longer used. Other languages fall back to English.
8. Keep startup fast: no Keychain or network on the UI thread; first frame before any fetch.

Do not add settings, themes, animations or features beyond this list.

## Acceptance

- [ ] Screenshots through the real app (computer use), real accounts: Desktop tab light zh, CLI tab dark en, Backups dialog, Add dialog; each matched side by side against the design artboard, with differences listed
- [ ] Background sync switch: starts in its real current state; toggle it twice so it ends where it started, and `launchctl` shows the same agent state as before (show before/after)
- [ ] Back up now creates a snapshot that appears in the list; restore is not run on real data (show only the confirm dialog)
- [ ] Removing `/Applications/Claude.app` is not allowed; instead show the no-mark path by pointing the loader at a missing path in a unit test
- [ ] Time from launch to first frame measured before and after this change (same machine, 3 runs each)
- [ ] cargo test + clippy pass for both workspaces
