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

- [x] Built `dist/CCSwift.app` launches; through computer use: switch CLI 2 → start slot, `cswap status` and `claude auth status` agree after each; Sync now runs; tray CLI submenu switch once (if the menu bar is reachable; otherwise say so)
- [ ] Each account's 5h/7d usage and reset times in the window match the OAuth usage endpoint (or `cswap list`) for that slot; a 429 is retried later in the run, not skipped
- [x] All Desktop saved-login files byte-identical before/after the run (list and compare)
- [x] README updated; `project_map.py status` clean
- [x] Release artifacts in dist/ named CCSwift-0.3.0-*; nothing published (Claude publishes after review)

## Resolution

INCOMPLETE — release preparation is implemented in the commit containing this resolution
(`Prepare CCSwift 0.3.0 and record T-014 acceptance`); the ticket stays open because usage
could not be verified for every slot. No app behavior or dependencies changed, and nothing
was published.

### Changes

- Both workspace versions and their lockfile package entries are 0.3.0.
- README clarifies standalone CLI switching without `cswap`, the runtime-only Claude mark,
  and the CCSwift bundle name.
- The CLI archive now follows the required `CCSwift-0.3.0-*` naming convention.

### Commands and results

All evidence is local, gitignored, under `work/T-014/` unless noted otherwise.

- `scripts/bundle-macos.sh > work/T-014/bundle.log 2>&1` — exit 0; ad-hoc signature
  verification passed. `bundle-version.txt` and `release-artifacts.json` verify version 0.3.0
  in the app and ZIP and the CLI member in the tar archive. Outputs:
  `dist/CCSwift.app`, `dist/CCSwift-0.3.0-macos-arm64.dmg`,
  `dist/CCSwift-0.3.0-macos-arm64.zip`, `dist/CCSwift-0.3.0-cli-macos-arm64.tar.gz`.
- `cargo test --workspace` — exit 0, 84 passed; two native Keychain tests remain ignored
  by their existing declarations (`root-test.log`). No acceptance tests were edited.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0 (`root-clippy.log`).
- `cargo test --manifest-path crates/app/Cargo.toml` — exit 0, 17 passed (`app-test.log`).
- `cargo clippy --manifest-path crates/app/Cargo.toml --all-targets -- -D warnings` —
  exit 0 (`app-clippy.log`); Cargo reports the existing future-incompatibility notice
  for dependency `block 0.1.6`.
- `git diff --check` — passed.

### Real-app acceptance

1. Copied the CLI filesystem store to `cli-store-before/` before switching; token scan
   passed before copying. Keychain credentials were not exported. Original active slot: 1.
   `osascript -e 'quit app "CCSwift"'` stopped the installed app before
   `open /Users/luckye/Documents/Code/cc-same/dist/CCSwift.app` launched the release bundle.
2. Computer use skipped the first-run tutorial and clicked Desktop **Sync now**;
   the toast reported four sessions copied (`sync-now.png`). No Desktop account switch
   was clicked, and Claude Desktop was not quit or restarted.
3. Computer use selected CLI slots 2, then 3, then restored starting slot 1.
   After each click, `~/.local/bin/cswap status` and `claude auth status` agreed on email
   and organization/slot identity. Evidence: `start-*`, `slot2-*`, `slot3-*`, `restored-*`
   and `cli-before.png`, `cli-slot2.png`, `cli-slot3.png`, `cli-restored.png`.
   Slot 3's `cswap status` also reports a dead refresh token; identity agreement does not
   establish usable authentication. The existing duplicate credential was not repaired.
4. Tray switch was not performed: the app AX surface exposed only its application menu;
   `cua.getApp('com.apple.systemuiserver')` returned `timeoutReached` (-10005).
   This is the ticket's explicit unreachable-menu-bar exception.
5. Independent oracle: `python3 work/T-014/read_usage.py work/T-014/<output>.json` reads
   each stored credential in memory and calls the OAuth usage endpoint, saving only usage
   and HTTP status. Outputs: `usage-before.json`, `usage-retry-1.json`,
   `usage-after-switches.json`, `usage-final-retry.json`.
   Slot 1's initial 429 was retried successfully: 0% / 73%, weekly reset November 11 22:59
   local. Slot 2 initially returned 401, then returned 200 after the switch/status round:
   0% / 19%, weekly reset November 13 17:59 local (the endpoint varied by a fraction of
   a second around 18:00). These match `cli-usage-final.png`; unknown 5h reset times are
   shown as dashes. `desktop-final.png` shows those successful readings joined by email.
   Slot 3 continued returning 401, then 429 with Retry-After 3600 on the final attempt;
   its window usage remained unavailable. Later slot-1 429s were also retried, but the
   last retry still returned 429. **The all-slot usage checkbox remains unchecked.**
6. `python3 work/T-014/desktop_guard.py` held all saved-login bytes only in memory from
   before launch; `touch work/T-014/compare-desktop` triggered exact comparison after
   the run. `desktop-before.json` lists all 70 files/sizes; `desktop-comparison.json`
   confirms identical file lists and all bytes unchanged. No hashes or secret copies.
7. Quit the built app and ran `open -a CCSwift`; process inspection confirmed
   `/Applications/CCSwift.app/Contents/MacOS/CCSwift` running again. System language and
   appearance were not changed. Skipping the tutorial saved its normal completion flag.
8. `python3 work/T-014/check_artifacts.py` scanned artifacts, the existing app log, and
   the diff against credential values retained only in memory and token patterns:
   no token values found (`credential-scan.json`).
9. `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py status --root .`
   — no structural problems or stale living docs (`project-map-status.txt`).

A fresh read-only behavioral acceptance review found no reproduced defects and returned
INCOMPLETE for the missing slot-3 oracle (`acceptance-review.txt`). Its missing project-map
artifact limitation is resolved by the subsequently saved `project-map-status.txt`.

### Remaining acceptance / undo

Repeat the per-slot usage comparison when slot 3 has a usable credential and the endpoint's
retry window has elapsed; this task does not repair or replace that credential. Preserve the
unchecked box until real usage and reset evidence exists for all slots. Local release files
are ready for review, not publication. Reverting this commit undoes release metadata, README,
and archive naming changes; it does not roll back account stores or the session sync.
