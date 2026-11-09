---
id: T-011
title: CCSwift redesign and in-app CLI add
type: decide
status: closed
blocked_by: []
claimed_by:
supersedes: T-001
---

## Question

What should the CCSwift window look like, and how do CLI accounts get added without `cswap`?

## Resolution

Decided by the user on 2026-11-09 over the design canvas (copies of the final artboards in `work/design/`: `Main.dc.html` is the reference, `Many.dc.html` the dark/English/8-account state, `AddFlow.dc.html` and `Backups.dc.html` the dialogs).

- One window, 720 px wide, with **Desktop | CLI tabs** (segmented control). The separate Usage page goes away: every account row carries its 5h and 7d usage and reset times.
- Under the tabs: one translucent Claude-orange ("jelly") card for the selected tab's current account (email initial avatar, email, plan, "Next →").
- Desktop tab only: a **Session sync** card with "Sync now", a **Background sync** switch that is off unless the user turns it on (never forced on), and a **Backups** row that opens a backup dialog (list, back up now, restore, reveal in Finder, how many to keep).
- Account table: email (initial avatar, no work/life names), plan (CLI: slot number), 5h, 7d, status, action (In use / Switch / Sign in again). Search box appears past four accounts; sort by 5h usage.
- CLI status: Valid, Renews on switch (access token expired, refresh token present), Expired (server rejected the stored login). Desktop status: Sign-in saved, Sign-in needed.
- **Add CLI account in the app; `cswap` no longer needs to be installed.** Add prompts the user to log in in a terminal (`claude auth login`, copy and open-terminal buttons), detects the new login, and saves it into the same claude-swap store. "Sign in again" on an expired row replaces that slot's stored login after a re-login with the same email, and refuses a different email.
- Look: Claude orange `#D97757` as the only accent (jelly gradient on the current card and primary buttons); warm greys; light = white, dark = black with off-white `#E6E1D9` text. Latin text in Times New Roman, Chinese in Songti SC (Claude's own CJK serif); regular weights, nothing heavier than medium. Claude's mark beside the name, read at runtime from the installed Claude.app, not bundled.
- UI in Chinese and English (other languages fall back to English).

T-001's "one usage view" and "separate pages" are replaced by the tabbed window above; its independent Desktop/CLI switching stands. T-003 still holds: one store, the claude-swap format, and `cswap` keeps working if installed.
