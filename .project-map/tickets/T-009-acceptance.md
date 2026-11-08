---
id: T-009
title: End-to-end acceptance by computer use
type: build
status: open
blocked_by: [T-008]
claimed_by: Codex
supersedes:
---

## What to build

Release build of the app, accepted through its real UI with computer use, against the user's real accounts.

Important: Claude Desktop is hosting the session that runs you. Do NOT click a Desktop account switch and do NOT quit or restart Claude Desktop. Desktop switching is unchanged upstream code; check only that the Desktop page and its account list render as before.

## Acceptance

- [x] Release build (`scripts/bundle-macos.sh` or the release cargo profile) launches; time from launch to first window measured, compared with the installed CC Same
- [x] Desktop page shows the saved Desktop accounts as before (screenshot); no switch clicked
- [x] CLI page: switch 2 -> 3 -> back to the starting slot through the window; `cswap status` + `claude auth status` agree each time (outputs saved)
- [ ] Tray menu: CLI submenu lists accounts and a switch from it works (slot 2, then back), checked with `cswap status`
- [ ] Usage page shows every account once with numbers matching `cswap list --json` for CLI
- [x] Light and dark appearance screenshots of all three pages
- [x] Ends with CLI on the starting account and the original CC Same app running again

## Resolution

INCOMPLETE — acceptance run on 2026-11-08 against candidate `2bb5857`.
Evidence commit: the commit containing this resolution (`T-009: record release UI acceptance and remaining gaps`). No product code, tests, dependencies, or acceptance requirements changed. The ticket stays open: tray interaction is unverified and active-account usage does not match cswap.

All artifacts are local under `work/T-009/` (ignored by the user's existing `.gitignore` change). No tokens are included in the diagnostic outputs.

### Verified

- `cargo build --release --manifest-path crates/app/Cargo.toml`: PASS, optimized release build in 5m 09s (`release-build.log`). `python3 work/T-009/package-release.py` packages that executable with the existing icon/plist, ad-hoc signs it, and verifies the signature (`package-release.log`). Bundle: `work/T-009/CC Same Fork.app`. It was not installed over the original.
- Launch-to-first-window observation through computer use: installed app **3206 ms**, release fork **2269 ms** (`installed-launch.json`, `fork-launch.json`). Each measurement starts immediately before `cua.getApp(fullAppPath)` launches the stopped app and ends when its first window AX state returns. These are single observations including automation overhead, not precise first-frame timings or a benchmark. The original was quit with `osascript -e 'quit app "CC Same"'` before launching the fork.
- Desktop: `installed-desktop.png`, `desktop-dark.png`, and `desktop-light.png` show the same three account labels and signed-in marker. No Desktop switch, sync, quit, or restart was invoked; Claude Desktop kept running. The fork's added tabs reduce the visible bottom margin, but all three account labels and switch controls are visible.
- Window CLI flow: clicked slot 2, then slot 3, then starting slot 1. After each click, ran `cswap status` and `claude auth status`; both agreed, with `loggedIn: true`. Evidence: `window-slot2-cswap.txt`, `window-slot2-auth.json`, `window-slot2.png`; corresponding `window-slot3-*`; and `window-restored-cswap.txt`, `window-restored-auth.json`. No login prompt appeared.
- Appearance: `desktop-{light,dark}.png`, `cli-{light,dark}.png`, and `usage-{light,dark}.png`. Changed only the app's existing theme override and restored System (`appearance-restored-ax.txt`); macOS appearance was not changed.
- Restoration: `start-cswap.txt` / `final-cswap.txt` show Account-1. `start-auth.json` and `final-auth.json` are semantically identical. `restoration.json` records unchanged paths, sizes, and nanosecond modification times for all 70 saved Desktop-login files; this is a metadata check, not a byte comparison. `open -a "CC Same"` restored the original, verified by `original-restored.png`, `original-restored-ax.txt`, and `/Applications/CC Same.app/Contents/MacOS/CC Same` in `final-process.txt`. `usage-final.json` still reports the slots 1/3 duplicate warning; no repair was attempted.

### Remaining acceptance gaps

- **Tray — BLOCKED by computer-use access.** The app AX tree exposes its window and application menu but no status-bar item (`tray-unavailable-ax.txt`). Ctrl-F8 did not expose one. `cua.getApp('com.apple.systemuiserver')` and `cua.getApp('com.apple.controlcenter')` each timed out with `-10005`; Finder's screenshot exposed its desktop window only. No tray click or tray switch is claimed. Next step: repeat with computer-use access that exposes the native status bar, switch to slot 2 and back to the starting account, and save `cswap status` results. T-006 remains unchanged.
- **Usage — reproduced mismatch.** All three emails appear exactly once. `usage-comparison.json` (`cswap list --json`) reports weekly 19%, 73%, 73% for slot-2 account, slot-1 account, slot-3 account respectively, all at 0% for 5h. The first UI fetch showed CLI 19% for slot 2 but older Desktop readings for the other two (`usage-dark.png`). Reopening Usage after more than a minute updated slot-3 account to CLI 73%; slot-1 account remained Desktop 35% (`usage-retry.png`, `usage-retry-ax.txt`). The whole usage-match checkbox therefore stays unchecked.
- **Root cause for active usage:** `crates/core/src/cli_usage.rs:65` queries `security find-generic-password -s Claude Code-credentials -w` without an account selector. On this machine that returns an item without `claudeAiOauth`. The same query with `-a "$USER"` finds an OAuth access token (`active-keychain-query.json`, presence booleans only; rerun with `python3 work/T-009/reproduce-active-keychain.py`). Switching already selects the current user in `cli_accounts_switch.rs`; claude-swap's Keychain helper also selects an account. `credential-expiry-only.json` confirms inactive slots have unexpired OAuth data. Smallest follow-up: scope the active usage lookup to the same user as switching, prove the ambiguous-service case, then repeat the real usage comparison. Credential stores were written only by the authorized switch path; no lookup fix was made during this acceptance-only ticket.

### Workspace checks

These runs check the frozen candidate's existing behavior; they do not replace the real UI evidence above.

- `cargo test --workspace --all-features`: PASS, 54 core + 26 integration tests; 1 existing native Keychain test ignored (`cargo-test.log`).
- `cargo clippy --workspace --all-targets --all-features`: PASS (`cargo-clippy.log`).
- `cargo test --manifest-path crates/app/Cargo.toml`: PASS, 17 tests (`app-test.log`).
- `cargo clippy --manifest-path crates/app/Cargo.toml --all-targets`: PASS (`app-clippy.log`). Existing dependency `block 0.1.6` emits its future-incompatibility notice.
- `git diff --check` and `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py status --root .`: results saved in `diff-check.txt` and `project-map-status.txt`.

Works: release launch, Desktop rendering, window CLI round trip, six appearance captures, and restoration have direct evidence. No regression: both workspace suites and clippy pass; no product code changed. Risk / undo: acceptance is incomplete for tray and usage; reverting this documentation commit removes the record only. The test bundle and artifacts remain local, the original app is running, and no account-store rollback is needed.
