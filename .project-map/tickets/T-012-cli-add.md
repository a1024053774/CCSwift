---
id: T-012
title: Add and re-login CLI accounts in the app, with per-account status
type: build
status: closed
blocked_by: []
claimed_by:
supersedes:
---

## What to build

Decision: [T-011](T-011-redesign-decisions.md). Design: `work/design/AddFlow.dc.html` (open it as text; colors and layout are reference, the flow is binding).

1. Core (`crates/core/src/cli_accounts*.rs`, no new crate deps):
   - `current_login(home)`: who the terminal is logged in as now (`~/.claude.json` `oauthAccount` email + organization, plus whether `Claude Code-credentials` for `$USER` exists). Never returns or logs tokens.
   - `add_current(home)`: save the current terminal login as the next slot, writing exactly what `cswap add` writes (sequence.json entry, Keychain `claude-swap` item `account-{n}-{email}`, config backup). If that email+organization is already a slot, do not add a duplicate; return which slot it is.
   - `update_slot(home, slot)`: replace a slot's stored credential and config with the current terminal login, only if the email+organization match that slot; otherwise refuse with a clear error.
   - Same locks as switch (cswap's flock and Claude's lock dirs); roll back on failure.
   - Status per slot: Valid (last usage fetch succeeded), RenewsOnSwitch (stored access token past `expiresAt`, refresh token present; do not call the API with it), Expired (the usage endpoint rejected a non-expired token with 401/403), Unknown (network/429/not read yet). No token refresh anywhere.
2. App: "+ Add" on the CLI tab opens the add dialog: step 1 shows `claude auth login` with Copy and Open Terminal; while open, re-read `current_login` every 2 s off the UI thread; step 2 shows the detected account (new → "Save account"; already a slot → say which and offer "Update login"). "Sign in again" on an Expired/RenewsOnSwitch row opens the same dialog targeted at that slot (different email → warning, no write). Minimal styling for now; T-013 restyles it.
3. Nothing requires `cswap` to be installed. Keep `cswap` compatibility: after the app adds or updates, `cswap list` / `cswap status` read it correctly.

## Acceptance

- [x] Unit tests for: add writes the cswap layout; add of an existing email+org returns the slot and writes nothing; update refuses a different email; status classification (expired access token → RenewsOnSwitch without network; 401 → Expired; 429 → Unknown). Each test fails on a mutant (record which)
- [x] Add path end to end in a sandbox: temporary HOME and a throwaway Keychain items prefix or cleanup, real `cswap list` reading the result; every Keychain item the test created is deleted afterwards (show `security` before/after)
- [x] Real store, update path: "Update login" on the currently active slot through the window; afterwards `cswap status` and `claude auth status` agree and no login prompt appears; sequence.json slot count unchanged
- [x] The app runs with `cswap` hidden from PATH (rename not allowed; use a PATH without it) and lists/switches/adds normally
- [x] cargo test + clippy pass for both workspaces

## Resolution

PASS — implemented and verified on macOS. Core capture writes the cswap layout with shared locks and rollback, returns existing slots without writes, and refuses an identity mismatch on update. The CLI sheet polls off the UI thread every two seconds and offers Save/Update plus targeted re-login. Status is per stored slot; no token refresh or cswap executable is used by the app. README and CONTEXT now describe this behavior. T-013 still owns the final styling and localization.

Evidence (all artifacts are local under `work/T-012/`, ignored by git):

- Contract red/green: `cargo test -p cc-same-core --test cli_capture_contract` initially failed because the new APIs did not exist, then passed. Logs: `contract-red.log`, `contract-green.log`.
- Unit contracts: `cargo test --workspace --all-features` passed (88 tests, two opt-in native Keychain tests ignored by default). `audit-final-root-test.log`. `python3 work/T-012/check_mutants.py` rejected six mutants: wrong UUID layout; duplicate reported as Added; removed identity guard; reversed expiry gate; 401/403 mapped to Unknown; 429 mapped to Expired. `mutants.log`, `mutant-*.log`. `python3 work/T-012/check_rollback_mutant.py` rejected omitted credential rollback (`mutant-rollback.log`). Existing-email/org add makes no Keychain call and leaves sequence bytes unchanged.
- Native sandbox: `CSWAP_TEST_BIN=/Users/luckye/.local/bin/cswap cargo test -p cc-same-core sandbox_add_is_readable_by_cswap -- --ignored --nocapture` passed. Real `security` probes showed both test items absent before and after; real `cswap list --json` read the saved identity (`native-sandbox.log`). The older T-006 opt-in native test was not rerun.
- Computer-use sandbox: launched the built `work/T-012/CCSwift.app/Contents/MacOS/CCSwift` with a temporary HOME, isolated Keychain, and `PATH=/usr/bin:/bin:/usr/sbin:/sbin` (no cswap). Clicked Add/Save for two synthetic identities, confirmed an open dialog detects an identity change, checked an existing slot offers Update, checked a mismatched targeted login disables Update, and switched successfully. Real cswap read both UI-created slots (`sandbox-ui-result.json`). Screenshots: `sandbox-first-saved.png`, `sandbox-existing-slot.png`, `sandbox-poll-changed.png`, `sandbox-mismatch.png`, `sandbox-switched.png`. Every created item and the test Keychain were cleaned up (`ui-keychain-first-cleanup.log`, `ui-keychain-after.log`, `race-keychain-after.log`); the real default Keychain was unchanged.
- Real store, computer use: fresh copies before writes at `cli-store-before-20261109-005649/` and `cli-store-before-real-010948/`. Clicked Update login on the starting active slot 1; no login prompt, still three slots, and `~/.local/bin/cswap status --json` / `~/.local/bin/claude auth status` agreed (`real-after-update.json`). Clicked real switches to 2, then 3, then restored 1 (`real-slot2.json`, `real-slot3.json`, `real-restored-slot1.png`). Repeated Update on the final build (`final-real-update.png`, `real-final.json`). Duplicate credential metadata was not repaired. The app process used the same cswap-free PATH.
- Delayed-network regression: a localhost CONNECT delay with synthetic credentials exposed a late usage result restoring the old UI active marker after a successful switch (`race-after-response.png`, `race-failure-state.json`). Metadata/usage reads now reject obsolete revisions after add/update/switch. The same UI sequence retained slot 2 after requests completed (`race-fixed-complete.png`, `race-fixed-state.json`); the window remained responsive. Reproduction and read/write boundary inventory: `race-procedure.md`. Test proxy stopped.
- Final checks: `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed (`audit-final-root-clippy.log`); `cargo test --manifest-path crates/app/Cargo.toml` passed, 17 tests (`audit-final-app-test.log`); `cargo clippy --manifest-path crates/app/Cargo.toml --all-targets -- -D warnings` passed (`audit-final-app-clippy.log`). `audit-final-checks.json` indexes the final results. An initial app clippy unit-binding failure is preserved in `app-clippy.log`; its correction passed. Windows compilation was not run.
- Acceptance evidence strengthened after the initial incomplete audit: a counting CONNECT proxy first observed two attempts from future-dated synthetic slots, then **zero additional attempts** after restarting the app with the same slots expired; both completed reads showed Renews on switch (`network-positive-control.json`, `proxy-requests.jsonl`, `expired-no-http.json`, `expired-no-http.png`). The proxy never established TLS or logged credentials. A live successful usage response was HTTP 200, saving only utilization/reset fields (`usage-success.json`), and the Valid classification assertion rejects an Unknown-on-success mutant (`mutant-valid.log`). New tests preserve exact sequence bytes/active marker on refusal and reject contention on cswap's flock plus all three Claude lock directories without stealing them; corrupt-sequence and missing-lock mutants both fail (`audit-mutants.log`, `mutant-sequence-corruption.log`, `mutant-missing-lock.log`). Temporary items/proxy were removed (`boundary-keychain-after.log`). HTTP 401/403/429 are unit-classification checks, not claimed live endpoint error tests.
- Independent review: design-integrity review and its scoped portability re-review are recorded in `design-review.txt`. The initial acceptance audit found evidence gaps without a reproduced defect (`acceptance-review-initial.txt`); the targeted final audit passed after the checks above (`acceptance-review-final.txt`).
- Safety/restoration: `python3 work/T-012/scan_credentials.py` compared four authorized Keychain items in memory against artifacts/source and found no token matches (`credential-scan.json`); no token values were logged or saved. Claude Desktop's main PID stayed unchanged, no Desktop switch was clicked, and appearance/language were unchanged (`desktop-preserved.json`). Quit the test build and reopened `/Applications/CCSwift.app` via `open -a CCSwift`; verified the installed executable was running. CLI ends on its starting slot 1.

Project-map check: `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py status --root .` — no problems or stale living docs (`work/T-012/project-map-status.log`). All five acceptance boxes are checked with the evidence above.

Commit: implementation and this resolution are committed together as **Add and update CLI accounts in CCSwift**. Resolve the commit with `git log -1 --format=%H -- .project-map/tickets/T-012-cli-add.md`; its ID is also recorded locally in `work/T-012/commit.txt`. Code undo is `git revert <commit>`; the pre-write store copies remain under `work/T-012/`.
