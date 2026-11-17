//! Desktop preferences that should follow the person, not the account, across a switch.
//!
//! Claude keeps most preferences machine-wide in `claude_desktop_config.json`, so a switch never
//! touches them. A few are keyed by account UUID; an allowlist of those is copied from the
//! account left to the account switched to. Permission modes and acknowledgements, bypass gates,
//! capability caches and rate-limit runtime state stay with their own account.
//!
//! The side browser keeps its site sign-ins according to Claude's `launchPreviewStorage`:
//! `none` (Claude's default) holds them in memory and clears them at startup; `shared` keeps one
//! jar on disk under `Partitions/`, which is not keyed by account and which a switch never moves.
//! With `keepBrowserSignIns` on, each switch sets it to `shared`. Open tabs are not kept anywhere.

use crate::fsx;
use crate::model::is_uuid;
use crate::paths::Paths;
use anyhow::{bail, Context as _, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

pub const FILE: &str = "claude_desktop_config.json";
/// `epitaxyPrefs` entries stored as `<name>.<account>`.
const CARRIED_SUFFIXED: &[&str] = &[
    "code-projects-order",
    "autoResumeRateLimitOptIn",
    "epitaxy-auto-default-notice-seen",
    "epitaxy-auto-entry-warning-seen",
    "epitaxy-auto-setup-band-seen",
    "epitaxy-keep-awake-band-used",
];
/// `preferences` entries stored as `{ <account>: value }`.
const CARRIED_BY_ACCOUNT: &[&str] = &["coworkModelAutoFallbackByAccount"];
const BROWSER_KEY: &str = "launchPreviewStorage";

fn path(paths: &Paths) -> PathBuf {
    paths.user_data.join(FILE)
}

fn backups(paths: &Paths) -> PathBuf {
    paths.state_dir.join("logins").join("desktop-prefs-before")
}

/// Change `config` in place; what changed. Accounts, when given, must be UUIDs; entries are
/// copied only when both are given and differ.
pub fn plan(
    config: &mut Map<String, Value>,
    from: Option<&str>,
    to: Option<&str>,
    shared_browser: bool,
) -> Result<Vec<String>> {
    if from.into_iter().chain(to).any(|a| !is_uuid(a)) {
        bail!("not an account id");
    }
    let prefs = match config.entry("preferences").or_insert_with(|| Value::Object(Map::new())) {
        Value::Object(prefs) => prefs,
        _ => bail!("Claude's preferences are not an object"),
    };
    let mut changed = Vec::new();
    if let (Some(from), Some(to)) = (from, to) {
        if from != to {
            if let Some(epitaxy) = prefs.get_mut("epitaxyPrefs") {
                let Value::Object(epitaxy) = epitaxy else { bail!("Claude's epitaxyPrefs are not an object") };
                for name in CARRIED_SUFFIXED {
                    let (key_from, key_to) = (format!("{name}.{from}"), format!("{name}.{to}"));
                    // A missing source entry leaves the target's as it is.
                    if let Some(value) = epitaxy.get(&key_from).cloned() {
                        if epitaxy.get(&key_to) != Some(&value) {
                            changed.push(format!("epitaxyPrefs.{key_to}"));
                            epitaxy.insert(key_to, value);
                        }
                    }
                }
            }
            for name in CARRIED_BY_ACCOUNT {
                if let Some(Value::Object(table)) = prefs.get_mut(*name) {
                    if let Some(value) = table.get(from).cloned() {
                        if table.get(to) != Some(&value) {
                            changed.push(format!("{name}.{to}"));
                            table.insert(to.to_string(), value);
                        }
                    }
                }
            }
        }
    }
    if shared_browser && prefs.get(BROWSER_KEY).and_then(Value::as_str) != Some("shared") {
        prefs.insert(BROWSER_KEY.into(), "shared".into());
        changed.push(BROWSER_KEY.into());
    }
    Ok(changed)
}

/// Apply [`plan`] to Claude's `claude_desktop_config.json`. Only while Claude is not running: it
/// rewrites the file as it runs. The file's exact bytes are kept before it is written.
pub fn carry(paths: &Paths, from: Option<&str>, to: Option<&str>, shared_browser: bool) -> Result<Vec<String>> {
    let path = path(paths);
    let before = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let mut config: Map<String, Value> =
        serde_json::from_slice(&before).with_context(|| format!("reading {}", path.display()))?;
    let changed = plan(&mut config, from, to, shared_browser)?;
    if changed.is_empty() {
        return Ok(changed);
    }
    let dir = backups(paths);
    fsx::create_private_dir_all(&dir)?;
    let copy = fsx::unique_path(dir.join(format!("{}.json", fsx::now_secs() as u64)));
    fsx::atomic_write(&copy, &before, None, true).with_context(|| format!("keeping {}", copy.display()))?;
    fsx::atomic_write(&path, &serde_json::to_vec_pretty(&config)?, None, false)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "11111111-1111-4111-8111-111111111111";
    const B: &str = "22222222-2222-4222-8222-222222222222";
    const C: &str = "33333333-3333-4333-8333-333333333333";

    fn sample() -> Map<String, Value> {
        let mut epitaxy = Map::new();
        for (key, value) in [
            (format!("code-projects-order.{A}"), json!(["/work/one", "/work/two"])),
            (format!("code-projects-order.{B}"), json!(["/old"])),
            (format!("epitaxy-auto-entry-warning-seen.{A}"), json!(true)),
            (format!("epitaxy-keep-awake-band-used.{B}"), json!(true)),
            (format!("autoResumeRateLimitOptIn.{A}"), json!(true)),
            (format!("autoResumeRateLimit.{A}"), json!({ "local_x": { "attempt": 1 } })),
            (format!("epitaxy-folder-permission-mode-v2.{A}"), json!({ "/work/one": "bypassPermissions" })),
            (format!("epitaxy-perm-mode-acks-v2.{A}"), json!(["/work/one:bypassPermissions"])),
            (format!("ultrathinkAvailable.claude-opus-5-5.{A}"), json!(true)),
            (format!("code-projects-order.{C}"), json!(["/third"])),
        ] {
            epitaxy.insert(key, value);
        }
        let value = json!({ "deploymentMode": "1p", "preferences": {
            "keepAwakeEnabled": true,
            "coworkModelAutoFallbackByAccount": { A: false, B: true, C: true },
            "bypassPermissionsOptInByAccount": { A: true },
            "epitaxyPrefs": epitaxy,
        }});
        value.as_object().unwrap().clone()
    }

    #[test]
    fn copies_only_allowlisted_entries_to_the_target() {
        let before = sample();
        let mut config = before.clone();
        plan(&mut config, Some(A), Some(B), false).unwrap();
        let prefs = &config["preferences"];
        let epitaxy = &prefs["epitaxyPrefs"];
        // Carried: project order, notices already seen, opt-ins.
        assert_eq!(epitaxy[format!("code-projects-order.{B}")], json!(["/work/one", "/work/two"]));
        assert_eq!(epitaxy[format!("epitaxy-auto-entry-warning-seen.{B}")], json!(true));
        assert_eq!(epitaxy[format!("autoResumeRateLimitOptIn.{B}")], json!(true));
        assert_eq!(prefs["coworkModelAutoFallbackByAccount"][B], json!(false));
        // Not carried: permissions, bypass gates, capability caches, runtime state.
        for name in [
            "autoResumeRateLimit",
            "epitaxy-folder-permission-mode-v2",
            "epitaxy-perm-mode-acks-v2",
            "ultrathinkAvailable.claude-opus-5-5",
        ] {
            assert!(epitaxy.get(format!("{name}.{B}")).is_none(), "{name}");
        }
        assert!(prefs["bypassPermissionsOptInByAccount"].get(B).is_none());
        // Target-only entries stay; other accounts and machine-wide preferences are untouched.
        assert_eq!(epitaxy[format!("epitaxy-keep-awake-band-used.{B}")], json!(true));
        assert_eq!(prefs["coworkModelAutoFallbackByAccount"][C], json!(true));
        assert!(prefs.get(BROWSER_KEY).is_none());
        for (key, value) in before["preferences"]["epitaxyPrefs"].as_object().unwrap() {
            if !key.ends_with(B) {
                assert_eq!(&epitaxy[key], value, "{key}");
            }
        }
        assert_eq!(config["deploymentMode"], json!("1p"));
        assert_eq!(prefs["keepAwakeEnabled"], json!(true));
    }

    #[test]
    fn shared_browser_storage_only_when_asked() {
        let mut config = sample();
        assert!(plan(&mut config, Some(B), Some(B), false).unwrap().is_empty());
        assert_eq!(plan(&mut config, None, None, true).unwrap(), vec![BROWSER_KEY.to_string()]);
        assert_eq!(config["preferences"][BROWSER_KEY], json!("shared"));
        assert!(plan(&mut config, None, None, true).unwrap().is_empty());
    }

    #[test]
    fn a_bad_account_id_changes_nothing() {
        let mut config = sample();
        assert!(plan(&mut config, Some("../x"), Some(B), true).is_err());
        assert_eq!(config, sample());
    }

    #[test]
    fn carry_keeps_the_exact_old_bytes_first() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().join("Claude"), tmp.path().join("state"));
        fs::create_dir_all(&paths.user_data).unwrap();
        let raw = serde_json::to_vec(&sample()).unwrap(); // compact: differs from what is written
        fs::write(path(&paths), &raw).unwrap();
        let changed = carry(&paths, Some(A), Some(B), true).unwrap();
        assert!(changed.contains(&BROWSER_KEY.to_string()));
        let kept: Vec<Vec<u8>> =
            fs::read_dir(backups(&paths)).unwrap().map(|e| fs::read(e.unwrap().path()).unwrap()).collect();
        assert_eq!(kept, vec![raw]);
        let after = fsx::read_json(&path(&paths), 1 << 20).unwrap();
        assert_eq!(after["preferences"][BROWSER_KEY], json!("shared"));
        // Nothing left to change: no write, no new copy.
        assert!(carry(&paths, Some(A), Some(B), true).unwrap().is_empty());
        assert_eq!(fs::read_dir(backups(&paths)).unwrap().count(), 1);
    }
}
