---
id: T-006
title: CLI page switches account
type: build
status: closed
blocked_by: [T-005]
claimed_by:
supersedes:
---

## What to build

Clicking a non-active CLI account switches the CLI to it, the same way `cswap switch <n>` does, without touching Claude Desktop or its saved logins. The current active credential is saved back into its slot first, so token refreshes are not lost. Also in the tray menu as a "CLI" submenu.

## Acceptance

- [x] Real switch A→B: `cswap status` and `claude auth status` (or `/status`) show B; switch back B→A shows A; no login prompt either way
- [x] Desktop's signed-in account and `<state>/logins` unchanged across the CLI switch
- [x] A failure mid-switch (e.g. Keychain write refused) leaves the previous account active
- [x] Tests cover the store read/write against a fixture copied from claude-swap's format

## Resolution

INCOMPLETE, 2026-11-08. Implementation commit: `dfbd139`.
All four written acceptance checks are supported. The ticket remains open because
the native tray submenu has not been exercised through computer use.

The CLI page now switches accounts, and the tray has a CLI submenu using the same
background Store operation. Core uses claude-swap's existing Keychain and JSON
formats, saves the outgoing credential/config before activation, preserves local
config and shared MCP credential fields, and rolls back live state on a failed
switch. No dependencies, settings, auto-switch, or network calls were added.

Evidence: `work/T-006/`.

- **Expected red:** opened the T-005 app, selected CLI, and clicked slot 2. No
  action or identity change; `baseline.png` shows no switch controls.
- **Real GUI round trip:** opened `work/T-006/CC Same Fork.app`, selected CLI,
  switched to slot 2, then 3, then 2. `slot-2.png`, `slot-3.png`, and
  `slot-2-return.png` record the outcomes. After each click ran `cswap status`
  and `claude auth status`; outputs are `cswap-slot-*.txt` and
  `claude-auth-slot-*.json`. Both tools agreed on every target, with
  `loggedIn: true`; no login prompt appeared.
- **Restoration and Desktop:** restored starting slot-1 identity through the GUI
  switch path (`cli-restored.png`, `cswap-status-restored.txt`,
  `claude-auth-restored.json`). `python3 work/T-006/verify-restoration.py` compares
  the private before snapshot with live state; `restoration-final.json` confirms
  semantic credential equality, exact OAuth identity equality, unchanged Desktop
  signed-in identity, and all 70 Desktop saved-login files byte-identical.
  `duplicates-preserved.json` confirms slots 1 and 3 still share OAuth credentials.
  No repair was attempted. The normal restore switch sets activeAccountNumber to
  1; it started at stale value 3. Private snapshots are local, mode-restricted,
  ignored by git, and contain credentials; do not publish them.
- **Failure after real Keychain write:**
  `cargo test --workspace --target-dir work/T-005/core-target
  native_keychain_rolls_back_after_sequence_write_failure -- --ignored`
  passed (`native-rollback.log`). It uses synthetic credentials in isolated
  temporary Keychain services, makes the sequence destination unwritable after
  the target credential write, and verifies actual Keychain and config rollback.
  Test items are removed afterward. `native-rollback-first.log` preserves an
  earlier command with an overly strict filter that ran zero tests; it is not
  acceptance evidence.
- **Fixture sensitivity:** fixture round trip checks refreshed outgoing credentials,
  stale recorded active metadata, unrelated config/sequence fields and shared MCP
  state. Failure fixtures cover backup refusal, active refusal, partial active
  writes, config/sequence write failure, missing target and malformed credentials.
  `mutant-no-backup.log` and `mutant-no-rollback.log` both exit 101 on the expected
  assertions. Implementation restored afterward; final suite passes.
- **Root workspace:** `cargo test --workspace --target-dir work/T-005/core-target`
  passes 52 core and 26 integration tests (`cargo-test-final.log`). The native
  test is ignored in the ordinary suite and was run explicitly above.
  `cargo clippy --workspace --all-targets --target-dir work/T-005/core-target`
  passes (`cargo-clippy-final.log`). The T-005 target directory was reused as a
  compilation cache; all new evidence and the deliverable app are under T-006.
- **App workspace:** `cargo test --manifest-path crates/app/Cargo.toml --workspace`
  passes 27 tests; one pre-existing network test remains ignored
  (`app-test-final.log`). `cargo clippy --manifest-path crates/app/Cargo.toml
  --workspace --all-targets` passes (`app-clippy-final.log`). Existing dependency
  `block 0.1.6` still emits a future-compatibility notice.
- **Built app:** `cargo build --manifest-path crates/app/Cargo.toml` passes
  (`app-build.log`). Copied the binary into `work/T-006/CC Same Fork.app` using
  the existing app resources, then `codesign --force --deep --sign -` and
  `codesign --verify --deep --strict` succeeded. Not installed over the original.
- **Reviews:** independent structural review PASS (`design-review.txt`).
  Independent behavioral review INCOMPLETE solely for native tray verification
  (`acceptance-review.txt`). Computer use exposed the main app menu but not the
  status-bar icon; closing the window and requesting state reopened the window.
  No claim is made that a tray click was tested.
- **Final checks:** `git diff --check` passes. Project-map status command and
  output are in `project-map-status.txt`. Original app reopened with
  `open -a "CC Same"`; `original-app-restored.txt` verifies its `/Applications`
  executable path. Starting CLI state and Desktop files rechecked afterward.

Next step: click the fork's native tray CLI submenu, switch between slots 2 and 3,
verify with `cswap status` and `claude auth status`, restore the starting account,
and save the evidence. Then close T-006. No production defect was reproduced.

Undo: revert `dfbd139`. No migration is required. Keychain restoration itself can
be refused by macOS; that case reports recovery failure instead of claiming
success. Commands exceeding security's safe stdin limit are refused before write.

Closed 2026-11-08 (Claude review): all four boxes have evidence. The tray click itself moved to T-009; the tray item calls the same `Store::switch_cli` path that was exercised for real through the window.
