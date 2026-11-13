# Glossary

- **Desktop account**: a Claude Desktop sign-in, saved by CC Same under `<state>/logins/`. Switching it restarts Claude Desktop.
- **CLI account**: a Claude Code (terminal `claude`) sign-in, kept in claude-swap's store (`~/.claude-swap-backup/sequence.json` plus its Keychain items). Switching first saves the outgoing credential into its slot, then changes the active `Claude Code-credentials` Keychain item and `oauthAccount` in `~/.claude.json`; Desktop is untouched.
- **Active**: the account currently in use by Desktop or by the CLI, decided separately for each. The CLI list matches `.claude.json`'s email and organization to a stored slot, as `cswap list` does; `activeAccountNumber` can be stale.
- **Usage**: 5-hour and weekly plan percentages for an account, with when they were read.
- **Inline usage**: 5-hour and weekly readings in each Desktop or CLI account row. Desktop rows prefer a successful CLI reading for the same email, otherwise Desktop’s local history; reset times come from that CLI reading or the last ones saved for the email (`cli-resets.json`), shown only while still ahead.

- **CLI capture**: save the terminal’s current login into the claude-swap store. Add returns an existing slot without rewriting it; Update login replaces only a slot with the same email and organization. Both use the switch locks and roll back failed writes.
- **CLI status**: Valid after a successful usage fetch; RenewsOnSwitch when stored access has expired and a refresh token exists; Expired after a non-expired credential receives HTTP 401/403; Unknown before reading or after a network/429 failure. CCSwift does not refresh tokens.

- **Tutorial completion**: `tutorialDone` in the existing app config is false until the first tutorial is finished or skipped. Replay starts the five steps without clearing that flag.
