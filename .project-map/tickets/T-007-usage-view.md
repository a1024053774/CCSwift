---
id: T-007
title: Unified usage view
type: build
status: closed
blocked_by: [T-005]
claimed_by: Codex
supersedes:
---

## What to build

One usage list for every account (Desktop and CLI, joined by email): 5-hour and weekly percent as small bars, with how old each reading is. Desktop numbers from Desktop's file, CLI numbers from the OAuth usage endpoint on open and on a refresh button (T-004).

## Acceptance

- [x] With the real accounts, every account appears once; numbers for CLI accounts match `cswap list` within a few percent
- [x] Offline: CLI rows show their last reading or "unavailable", the window stays responsive
- [x] No request more than once a minute per account (log or test proves it)

## Resolution

2026-11-08 — PASS. Implementation commit: `dfa7afc` (Add unified Desktop and CLI usage view (T-007)).

Added the Usage tab, joining Desktop and CLI accounts by email while keeping each
source’s 5-hour/weekly percentages and reading age separate. The core module reads
saved Keychain credentials and calls the OAuth endpoint on a worker. Opening Usage,
Refresh, and Cmd+R share an in-memory per-email throttle; failures retain the last
reading or show unavailable. Expired credentials show “needs cswap refresh”. No
credential refresh, account-store writes, new settings, or background usage polling.
The existing reqwest/rustls stack is enabled through an optional core feature.

Acceptance evidence (all artifacts under `work/T-007/`):

- Baseline expected-red: computer use opened the prior fork; only Desktop and CLI
  tabs existed (`baseline.png`, `baseline-ax.txt`).
- Real accounts: built `CC Same Fork.app`, opened Usage through computer use, and
  observed each of the three emails exactly once with both sources. CLI percentages
  matched `cswap list --json` exactly: slot-2 account 0%/19%, slot-1 account 0%/73%,
  slot-3 account 0%/73% (5-hour/weekly). See `cswap-list-before.json`,
  `usage-loaded.png`, `usage-lower.png`, and `usage-loaded-ax.txt`.
- Offline: launched the same bundle with HTTP/HTTPS/ALL_PROXY pointing to the local
  `stalled-proxy.py` at port 18877, which accepts without forwarding. Navigated
  back to Desktop while Refreshing was displayed. CLI rows then showed unavailable.
  See `offline-navigation-verified.txt`, `offline-unavailable-final.png`,
  `offline.log`, and `proxy.log`. Earlier pending screenshots and an idle-main-thread
  sample are preserved; `reproduce.md` explains the stale native AX observations.
- Throttle: repeated Refresh and Cmd+R produced one attempt and two throttled
  entries per slot in `online.log`. The focused test also covers duplicate emails,
  a new attempt after 61 seconds, failed-attempt throttling, and preserving the
  successful reading and its timestamp. `usage-tests.log` passed. Both deliberate
  faults (no throttle; dropping the cached reading) were rejected, exit 101:
  `mutant-results.txt`, `mutant-no-throttle.log`, `mutant-drop-cached-reading.log`.
  Reproduce with `python3 work/T-007/check-mutants.py`; it restores the source.

Commands and results:

- `cargo build --manifest-path crates/app/Cargo.toml`: exit 0 (`app-build.log`).
  The app bundle was ad-hoc signed and tested through its real UI. Bundle and exact
  reproduction steps: `CC Same Fork.app`, `reproduce.md`.
- `cargo test -p cc-same-core --features cli-usage cli_usage`: 2 passed (`usage-tests.log`).
- `cargo test --workspace --all-features`: 54 unit + 26 integration passed;
  1 existing native Keychain rollback test ignored (`cargo-test.log`).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: exit 0
  (`cargo-clippy.log`).
- `cargo test --manifest-path crates/app/Cargo.toml`: 27 passed;
  1 existing GitHub network test ignored (`app-test.log`).
- `cargo clippy --manifest-path crates/app/Cargo.toml --all-targets -- -D warnings`:
  exit 0 (`app-clippy.log`). Cargo notes the existing dependency `block 0.1.6`
  has a future-incompatibility warning.
- Fresh read-only design-integrity-review: PASS, no findings (`design-review.txt`,
  `design-scan.txt`). Runtime acceptance was performed by computer use above.
- Desktop and CLI pages still render (`desktop-after.png`, `cli-after.png`). No
  real switching was needed. `cswap status` before/after is identical
  (`cswap-status-before.txt`, `cswap-status-after.txt`, empty `identity-diff.txt`).
  The duplicate-credential warning was not repaired. Quit the fork and ran
  `open -a "CC Same"`; computer use confirmed the original app's window.
- `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py status --root .`:
  0 problems and 0 stale living docs (`project-map-status.txt`).

Works: all three acceptance boxes have real UI evidence; throttle and cache
retention additionally reject the corresponding faulty implementations.
No regression: both workspaces’ tests and Clippy pass; existing ignored tests are
listed above. README and glossary updated and checked as UTF-8.
Risk / undo: usage cache and throttle last for the app process, not across restarts;
no live expired-token scenario was forced. Revert `dfa7afc` to remove the feature.
The installed app and the pre-existing dirty `.gitignore` were left unchanged.
