---
id: T-015
title: First-launch tutorial, replayable
type: build
status: open
blocked_by: [T-013]
claimed_by:
supersedes:
---

## What to build

User request, 2026-11-09: guide the user on first open, never pop up again on later opens, and let them replay it.

- A short step-by-step overlay on the real window (coach marks: dim the window, highlight one element, a card with text and Back / Next / Skip, step dots), in the T-013 look. Five steps:
  1. Desktop | CLI tabs: the two switch independently; Desktop switching restarts Claude, CLI switching does not.
  2. The orange current-account card and "Next →".
  3. The account table: 5h / 7d usage, reset times, status, Switch / Sign in again.
  4. "+ Add" on the CLI tab: log in in a terminal first, then the app saves it.
  5. Session sync card (switches to the Desktop tab): Sync now, the Background sync switch is off until you turn it on, Backups.
- Shown automatically only when the app has never finished or skipped it. Store that as one boolean in the app's existing config/state (no new file); finishing or skipping sets it.
- Replay: "Show tutorial" in Settings and in the app menu.
- zh-CN and en strings; other languages fall back to English. Keyboard: → / ← / Esc.
- If a step's element is not on screen (for example no CLI accounts yet), the card still shows, centered, without a highlight.

## Acceptance

- [ ] Through the real app (computer use): fresh state (flag cleared in a copy of the state, not by deleting the user's state) shows the tutorial; walk all 5 steps with screenshots; quit and reopen → no tutorial; Settings → Show tutorial replays it; Esc skips and does not come back
- [ ] Unit test for the flag logic (never-seen → show; finished/skipped → hide; replay does not reset it), failing on a mutant
- [ ] cargo test + clippy pass for both workspaces
