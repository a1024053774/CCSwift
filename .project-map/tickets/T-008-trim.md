---
id: T-008
title: Trim and polish after first slices
type: build
status: closed
blocked_by: []
claimed_by: Codex
supersedes:
---

## What to build

Review findings on T-005..T-007 (Claude, 2026-11-08). Make the app simpler, not bigger:

1. Usage page: plan usage belongs to the account, not to Desktop or CLI. One compact row per account: email, then 5h and 7d as two short bars side by side with percent, and one small caption "<Desktop|CLI> · <age>" for whichever reading is newer. Drop the per-source blocks. Whole list for 3 accounts fits without scrolling in the default window.
2. CLI usage for the *active* CLI slot reads the live `Claude Code-credentials` item (as cswap does), not the possibly stale backup.
3. CLI page: give the slot number a fixed-width column so emails align; hide the organization line when it is just "<email>'s Organization".
4. Turn off the upstream self-updater (T-002): no update check, no download, no "Updates" section in Settings. Delete the code path rather than adding a flag, if that stays small; otherwise the smallest switch-off.
5. Remove the `eprintln!` throttle/attempt logging in CLI usage (tests already prove the throttle).
6. Tab labels "Desktop / CLI / Usage" and "Refresh" go through i18n like other strings (zh-CN at least).

## Acceptance

- [x] Screenshot of Usage with the real accounts: one row per account, no scrolling needed
- [x] With the update feed unreachable or not, the app makes no request to GitHub releases (log or proxy evidence)
- [x] CLI page screenshot shows aligned emails and no redundant org line
- [x] cargo test and clippy pass for both workspaces; net diff of this ticket is smaller in added lines than removed lines where possible

## Resolution

PASS — implemented 2026-11-08. Commit: `T-008: compact usage and remove upstream updater` (the commit containing this resolution).

- Usage now shows one compact row per email, two side-by-side bars, and the source/age of the newest successful reading. CLI slot numbers have a fixed-width column; default email-based organization names are hidden. Tabs and Refresh use i18n, including zh-CN.
- Active CLI usage reads `Claude Code-credentials`; inactive slots still read their `claude-swap` backup. Removed throttle/attempt logging. Keychain/HTTP remain on the existing background worker.
- Deleted the upstream updater, its startup/quit hooks, menu items, Settings group, and obsolete screenshot scenes. Removed its unused direct dependencies. Kept the local agent-installation path check and its existing test in `login.rs`; persisted configuration fields remain untouched.

Evidence (all artifacts under `work/T-008/`):

- Computer use against the real accounts: `baseline-usage-visible.png` shows the original list extending below the default window; `usage-final.png` shows all three compact rows without scrolling. `cli-final.png` shows aligned emails and no redundant organization lines. `settings-footer.png` and `settings-ax.txt` show the Settings footer without Updates. Early stale native captures are retained; the final images were taken after launching the bundle through the native app tool.
- `python3 work/T-008/proxy-check.py`: `proxy.log` records the old binary requesting `api.github.com:443` with the former development updater override enabled; the new binary makes no request during the same 22-second startup observation. The proxy rejects connections (502), so no update can download. Source inspection also finds no remaining release request/check/install path.
- `python3 work/T-008/credential-check.py`: the same fetch body is exercised with only the security executable and HTTP endpoint replaced by isolated fixtures. `credential-before.log` fails on revision `17feeaf` because active usage reads the backup (22 instead of the independently specified live value 11); `credential-after.log` passes for both active and inactive cases. No real credential is read by this probe.
- `cargo test --workspace --all-features`: PASS, 80 tests; one existing isolated Keychain-write test remains ignored (`cargo-test.log`).
- `cargo clippy --workspace --all-targets --all-features`: PASS (`cargo-clippy.log`).
- `cargo test --manifest-path crates/app/Cargo.toml`: PASS, 17 tests (`app-test-final2.log`).
- `cargo clippy --manifest-path crates/app/Cargo.toml --all-targets`: PASS (`app-clippy-final2.log`). Existing dependency `block 0.1.6` produces a future-incompatibility notice. Initial example compile failures and intermediate edit errors are preserved in earlier logs.
- `cargo build --manifest-path crates/app/Cargo.toml`: PASS (`build-retry.log`). Test bundle: `work/T-008/CC Same Fork.app`; built executable copied into the existing bundle layout and ad-hoc signed with `codesign --force --deep --sign -`.
- `git diff --check`: PASS. Net change removes substantially more lines than it adds (see `diff-stat.txt`). README and CONTEXT describe the new behavior.
- `cswap-before.txt` and `cswap-after.txt`: CLI remains on Account-1. No real switch was invoked and duplicate credentials were not repaired. The fork was quit and `open -a "CC Same"` restored `/Applications/CC Same.app`, verified by computer use.
- `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py status --root .`: see `project-map-before-commit.txt` and `project-map-status.txt`.

All acceptance boxes have evidence. Real OAuth requests can fail independently of the layout; the final screenshot correctly retains available Desktop readings when newer CLI readings are unavailable. No macOS installation replacement was performed. Undo: revert this ticket's commit; account stores require no rollback.

