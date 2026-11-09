---
id: T-015
title: First-launch tutorial, replayable
type: build
status: closed
blocked_by: [T-013]
claimed_by:
supersedes:
---

## What to build

User request, 2026-11-09: guide the user on first open, never pop up again on later opens, and let them replay it.

- A short step-by-step overlay on the real window (coach marks: dim the window, highlight one element, a card with text and Back / Next / Skip, step dots), in the T-013 look. Five steps:
  1. Desktop | CLI tabs: the two switch independently; Desktop switching restarts Claude, CLI switching does not.
  2. The orange current-account card and "Next →".
  3. The account table: 5h / 7d usage, reset times, status, Switch / Sign in again.
  4. "+ Add" on the CLI tab: log in in a terminal first, then the app saves it.
  5. Session sync card (switches to the Desktop tab): Sync now, the Background sync switch is off until you turn it on, Backups.
- Shown automatically only when the app has never finished or skipped it. Store that as one boolean in the app's existing config/state (no new file); finishing or skipping sets it.
- Replay: "Show tutorial" in Settings and in the app menu.
- zh-CN and en strings; other languages fall back to English. Keyboard: → / ← / Esc.
- If a step's element is not on screen (for example no CLI accounts yet), the card still shows, centered, without a highlight.

## Acceptance

- [x] Through the real app (computer use): fresh state (flag cleared in a copy of the state, not by deleting the user's state) shows the tutorial; walk all 5 steps with screenshots; quit and reopen → no tutorial; Settings → Show tutorial replays it; Esc skips and does not come back
- [x] Unit test for the flag logic (never-seen → show; finished/skipped → hide; replay does not reset it), failing on a mutant
- [x] cargo test + clippy pass for both workspaces


## Resolution

PASS — implemented in `b82787d` on `switch-cli` (2026-11-09).

- Five coach marks shade the window around actual element bounds. Step 4 selects CLI;
  step 5 selects Desktop. Missing accounts leave a centered card without a highlight.
  Back/Next, arrow keys, Skip/Esc and Finish work; replay resets the content scroll position.
- `tutorialDone` is one boolean in the existing config, defaulting to false for old configs.
  Finish/Skip save true through the existing settings save path. Replay does not clear it.
  Settings and the app menu both expose Show tutorial. English and zh-CN use existing
  fonts/theme tokens; unsupported languages use English. No dependencies added.

### Acceptance evidence

All artifacts are local under `work/T-015/` (gitignored).

1. Real app, computer use: built and ad-hoc signed `work/T-015/CCSwift.app`.
   Copied only the user's config into `work/T-015/state-copy/config.json`, cleared
   `tutorialDone` in that copy, and selected English there. Saved logins were not copied.
   Launched with:
   ```sh
   cargo build --manifest-path crates/app/Cargo.toml
   CC_SAME_STATE_DIR="$PWD/work/T-015/state-copy" work/T-015/CCSwift.app/Contents/MacOS/CCSwift
   ```
   Walked all five steps via clicks and arrow keys: `step-1.png` through `step-5.png`.
   Finish → quit → reopen: `finished.png`, `reopen-finished.png` (no tutorial).
   Settings → Show tutorial: `settings-replay.png`; Esc → quit → reopen:
   `reopen-skipped.png` (no tutorial). App-menu replay in Chinese/dark:
   `menu-replay-zh-dark.png`, `step-5-zh-dark.png`. Language/appearance changes were
   confined to the test config; system preferences were unchanged.
2. Empty-account run used `CC_SAME_STATE_DIR="$PWD/work/T-015/empty-state"` and
   `CC_SAME_USER_DATA="$PWD/work/T-015/empty-desktop"`, with a copy of config set to
   `tutorialDone: false`, `language: de-AT`, `appearance: light`. Actual GUI showed
   English and a centered card without a highlight: `missing-target-en-fallback.png`,
   `empty-table-centered.png`. Esc from this never-completed tour → quit → reopen:
   `fresh-skip-reopen.png` (no tutorial). Both test configs persisted `tutorialDone: true`.
3. `cargo test --manifest-path crates/app/Cargo.toml tutorial_flag_lifecycle`:
   initial contract failed because the config field did not exist (`contract-red.log`);
   implementation passed (`flag-green.log`). Replacing `(!done).then_some(0)` with
   `done.then_some(0)` failed the first-launch assertion, exit 101 (`flag-mutant.log`).
   Mutation restored. Final test covers missing config, separate fresh Skip/Finish cases,
   config serialization, hidden on reopen, and replay preserving completion.
4. Required checks, all exit 0:
   - `cargo test`: 84 passed; two existing native-Keychain tests remain explicitly ignored
     (`root-test.log`). They exercise CLI writes, outside this tutorial change.
   - `cargo clippy`: passed (`root-clippy.log`).
   - `cargo test --manifest-path crates/app/Cargo.toml`: 17 passed (`app-test-complete.log`).
   - `cargo clippy --manifest-path crates/app/Cargo.toml`: passed (`app-clippy-complete.log`).
     Cargo reports the existing dependency `block 0.1.6` future-incompatibility warning.
   - `git diff --check`: passed.
   - `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py status --root .`:
     zero problems and zero stale living docs (`project-map-status.log`); T-014 is the next frontier.
5. Preserved intermediate evidence: `initial-overlay.png` shows the first attempt's bright
   gaps; the continuous spotlight replaces that rendering. `build-final.log` preserves a
   missing-trait-import build failure; `build-final-2.log` records the corrected build.
   Logs and text artifacts scanned for credential values: `credential-scan.json`, zero
   matches. Screenshots show account labels, not tokens.

No CLI-store writes or account switches were performed. Claude Desktop was not quit or
restarted. The installed CCSwift was quit before test launches and reopened at the end;
computer use verified its original window. README and CONTEXT describe the tutorial.
Nothing remains unchecked.
