---
id: T-005
title: CLI page lists accounts
type: build
status: closed
blocked_by: []
claimed_by:
supersedes:
---

## What to build

A second page in the app window, "CLI", reached by a two-segment switch (Desktop | CLI) at the top. It lists the CLI accounts from claude-swap's store in slot order with email, organization and which one is active. Desktop page unchanged. Missing store shows one line telling the user to run `cswap add`.

## Acceptance

- [x] With the user's real store, the page shows the same accounts and active marker as `cswap list` (screenshot + `cswap list` output saved)
- [x] With an empty temp HOME, the page shows the empty-state line, no crash
- [x] `cargo test` and `cargo clippy` pass for the workspace
- [x] Window opens to first paint as fast as before (no blocking Keychain or network call on open)

## Resolution

PASS, 2026-11-08. Implementation commit: `554897aad295ff82b7b17208d1bd9cd6d55c045f`.

Added the Desktop | CLI selector and a read-only core metadata reader. CLI reads
run in the background when the page is selected or refreshed. No dependencies,
Keychain calls, network calls, store writes, switching, or settings were added.

Evidence is under `work/T-005/`; detailed reproduction steps are in
`work/T-005/acceptance.md`.

- **Real app:** baseline `baseline.png` has no selector (expected red).
  `cswap list > work/T-005/cswap-list-before.txt 2>&1` supplied the oracle.
  Computer use verified all three slots, emails, organizations, and the slot-1
  active marker in `cli-real-refreshed.png`, plus refresh and return to Desktop
  (`desktop-after.png`). The live identity marks slot 1 although sequence.json
  records slot 3; matching `.claude.json` follows cswap. No credential repair.
- **Empty HOME:** launched the same built app with a new empty temporary HOME
  and isolated Desktop paths. `empty-home-launch.txt` records paths;
  `cli-empty-home.png` shows the `cswap add` instruction without a crash.
- **Root workspace:** `cargo test --workspace --target-dir work/T-005/core-target`
  passed 49 core and 26 integration tests (`cargo-test.log`).
  `cargo clippy --workspace --all-targets --target-dir work/T-005/core-target`
  passed (`cargo-clippy.log`).
- **App workspace:** `cargo test --manifest-path crates/app/Cargo.toml --workspace`
  passed 27 tests; one pre-existing GitHub network test remains explicitly ignored
  (`app-test.log`). `cargo clippy --manifest-path crates/app/Cargo.toml --workspace
  --all-targets` passed (`app-clippy.log`). The existing dependency `block 0.1.6`
  emits a future-Rust compatibility notice, not a test or clippy failure.
- **Test sensitivity:** the active-marker fixture rejected a mutation that trusted
  activeAccountNumber (`mutant-active-marker.log`, exit 101). After restoring the
  implementation, `cargo test --workspace --target-dir work/T-005/core-target
  cli_accounts::tests` passed all three fixtures (`fixture-green.log`). Fixtures
  also cover sequence order, organization matching, missing and malformed input,
  and unchanged input bytes after reading.
- **Startup:** `python3 work/T-005/measure-first-frame.py` used identical temporary
  first-frame callbacks in baseline `53ff612` and candidate. Three warm launches
  had medians 397.8 ms and 277.8 ms (`first-frame.json`); no regression observed.
  This is a local callback proxy, not an OS compositor measurement. Probe removed;
  normal build passed (`app-build.log`). CLI I/O is deferred until tab selection.
- **Review:** independent design-integrity review PASS, no findings
  (`design-review.txt`). `git diff --check` passed.
- **Map:** `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py
  status --root .` reported zero problems and zero stale living docs
  (`project-map-status.txt`). T-006 and T-007 are the next frontier.
- **Restoration:** original `/Applications/CC Same.app` reopened and process path
  verified (`original-app-restored.txt`). `cswap status` still reports slot 1
  (`cswap-status-before.txt`, `cswap-status-after.txt`). No switch was performed.

Built artifact: `work/T-005/CC Same Fork.app` (debug, ad-hoc signed and verified).
Not installed over the original app. All acceptance boxes have evidence. Undo by
reverting the implementation commit; no account-store migration is required.

