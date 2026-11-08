# Glossary

- **Desktop account**: a Claude Desktop sign-in, saved by CC Same under `<state>/logins/`. Switching it restarts Claude Desktop.
- **CLI account**: a Claude Code (terminal `claude`) sign-in, kept in claude-swap's store (`~/.claude-swap-backup/sequence.json` plus its Keychain items). Switching it changes the active `Claude Code-credentials` Keychain item and `oauthAccount` in `~/.claude.json`; Desktop is untouched.
- **Active**: the account currently in use by Desktop or by the CLI, decided separately for each. The CLI list matches `.claude.json`'s email and organization to a stored slot, as `cswap list` does; `activeAccountNumber` can be stale.
- **Usage**: 5-hour and weekly plan percentages for an account, with when they were read.
- **Usage view**: one list of every account (Desktop and CLI, joined by email) with its usage.
