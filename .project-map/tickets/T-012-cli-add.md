---
id: T-012
title: Add and re-login CLI accounts in the app, with per-account status
type: build
status: open
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

- [ ] Unit tests for: add writes the cswap layout; add of an existing email+org returns the slot and writes nothing; update refuses a different email; status classification (expired access token → RenewsOnSwitch without network; 401 → Expired; 429 → Unknown). Each test fails on a mutant (record which)
- [ ] Add path end to end in a sandbox: temporary HOME and a throwaway Keychain items prefix or cleanup, real `cswap list` reading the result; every Keychain item the test created is deleted afterwards (show `security` before/after)
- [ ] Real store, update path: "Update login" on the currently active slot through the window; afterwards `cswap status` and `claude auth status` agree and no login prompt appears; sequence.json slot count unchanged
- [ ] The app runs with `cswap` hidden from PATH (rename not allowed; use a PATH without it) and lists/switches/adds normally
- [ ] cargo test + clippy pass for both workspaces
