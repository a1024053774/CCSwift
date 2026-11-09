# Project map

## Destination

CCSwift, one macOS app (this fork of CC Same), switches **Desktop** accounts (existing cc-same behavior) and **CLI** accounts (list, switch, add, re-login; no `cswap` needed) on two tabs, with every account's usage and status in one table. Current window and add flow: [T-011](tickets/T-011-redesign-decisions.md). Store and switching spec: [specs/desktop-and-cli.md](specs/desktop-and-cli.md).

## Notes

- Upstream: songkeys/cc-same (Rust, gpui-kit). Work branch `switch-cli`. CLI behavior reference: realiti4/claude-swap (Python), read-only.
- Keep it small: no auto-switch, no TUI features, no new settings beyond what a slice needs. UI simple and fast to open.
- The user's real accounts are live in both tools; never require re-login, never delete their stores.
- Build tickets are done by Codex (model medium) and accepted by Codex computer use; Claude reviews and trims over-design.

## Decisions so far

- [T-001 Scope](tickets/T-001-scope.md): superseded by T-011 (Desktop/CLI still switch independently)
- [T-011 CCSwift redesign](tickets/T-011-redesign-decisions.md): one 720 px window with Desktop | CLI tabs, usage in every row, orange current-account card, opt-in background sync and backups, CLI add/re-login in the app without `cswap`, Times + Songti, zh/en
- [T-002 Fork and state](tickets/T-002-fork-and-state.md): fork in place of CC Same, same state dir so saved Desktop logins carry over; upstream auto-update off
- [T-003 CLI account store](tickets/T-003-cli-store.md): read and write claude-swap's own store; no second store, `cswap` keeps working
- [T-004 Usage source](tickets/T-004-usage-source.md): Desktop from Desktop's usage file; CLI from the OAuth usage endpoint per stored token; joined by email

## Not yet specified


## Out of scope

- CLI auto-switch, thresholds, priorities, aliases, dir mappings, `run`, export/import: user asked only for list + switch.
- Windows/Linux CLI switching: user is on macOS.
- [T-009](tickets/T-009-acceptance.md), [T-010](tickets/T-010-mono-and-resets.md): the 0.2.0 window they cover is replaced by T-013; open checks moved to T-014.

## Living docs

| Doc | Covers | Verified |
| --- | --- | --- |
| [README.md](../README.md) | `crates/app/src/**`, `crates/core/src/cli_accounts*.rs`, `crates/core/src/cli_usage.rs` | 96d6cee |
| [CONTEXT.md](../CONTEXT.md) | `crates/core/src/**` | |
