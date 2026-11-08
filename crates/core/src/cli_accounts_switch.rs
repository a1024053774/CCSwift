//! macOS switching for the existing claude-swap OAuth store.
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

const ACTIVE: &str = "Claude Code-credentials";

/// Switch the macOS CLI login, saving the outgoing credential first. Run off the UI thread.
pub fn switch(home: &Path, slot: u64) -> Result<()> {
    ensure!(cfg!(target_os = "macos"), "CLI switching is only available on macOS");
    let user = std::env::var("USER")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("/usr/bin/id")
                .arg("-un")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_owned())
        })
        .context("Cannot determine the Keychain account")?;
    #[cfg(unix)]
    let _locks = Locks::acquire(home)?;
    switch_with(home, slot, &user, security)
}

// The I/O boundary also lets fixture tests refuse a write without touching real credentials.
fn switch_with(
    home: &Path,
    slot: u64,
    user: &str,
    mut keychain: impl FnMut(&str, &str, Option<&str>) -> Result<String>,
) -> Result<()> {
    let root = home.join(".claude-swap-backup");
    let sequence_path = root.join("sequence.json");
    let mut sequence: Value = serde_json::from_slice(&fs::read(&sequence_path)?)?;
    let accounts = super::list(home)?;
    let target = accounts.iter().find(|a| a.slot == slot).context("CLI account no longer exists")?;
    if target.active {
        return Ok(());
    }
    let current = accounts
        .iter()
        .find(|a| a.active)
        .context("Current CLI account is not in claude-swap; run cswap before switching")?;
    let config_path = home.join(".claude.json");
    let original_config = fs::read(&config_path)?;
    let mut config: Value = serde_json::from_slice(&original_config)?;
    let backup_path = |number, email: &str| -> Result<_> {
        ensure!(!email.contains(['/', '\\', '\n', '\r']), "Invalid CLI account email");
        Ok(root.join("configs").join(format!(".claude-config-{number}-{email}.json")))
    };
    let target_config: Value = serde_json::from_slice(&fs::read(backup_path(slot, &target.email)?)?)?;
    let oauth = &target_config["oauthAccount"];
    ensure!(oauth.is_object() && oauth["emailAddress"].as_str() == Some(&target.email), "Invalid saved CLI identity");
    ensure!(
        oauth["organizationUuid"].as_str().unwrap_or("")
            == sequence["accounts"][slot.to_string()]["organizationUuid"].as_str().unwrap_or(""),
        "Saved CLI organization does not match slot"
    );
    let original = keychain(ACTIVE, user, None)?;
    let live = credential(&original)?;
    let target_key = format!("account-{slot}-{}", target.email);
    let mut next = credential(&keychain("claude-swap", &target_key, None)?)?;
    for field in ["mcpOAuth", "mcpOAuthClientConfig", "mcpXaaIdp", "mcpXaaIdpConfig", "pluginSecrets"] {
        next.as_object_mut().unwrap().remove(field);
        if let Some(value) = live.get(field) {
            next[field] = value.clone();
        }
    }
    let next = serde_json::to_string(&next)?;
    config["oauthAccount"] = oauth.clone();
    sequence["activeAccountNumber"] = slot.into();
    sequence["lastUpdated"] = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true).into();
    let config_bytes = serde_json::to_vec_pretty(&config)?;
    let sequence_bytes = serde_json::to_vec_pretty(&sequence)?;
    // Save refreshes before touching the active login. A failed backup aborts here.
    keychain("claude-swap", &format!("account-{}-{}", current.slot, current.email), Some(&original))?;
    crate::fsx::atomic_write(&backup_path(current.slot, &current.email)?, &original_config, None, false)?;
    let mut config_written = false;
    let result = (|| -> Result<()> {
        keychain(ACTIVE, user, Some(&next))?;
        crate::fsx::atomic_write(&config_path, &config_bytes, None, false)?;
        config_written = true;
        crate::fsx::atomic_write(&sequence_path, &sequence_bytes, None, false)?;
        Ok(())
    })();
    if let Err(error) = result {
        // A refused command can have partially succeeded: always restore the live item.
        let credential_restore = keychain(ACTIVE, user, Some(&original));
        let config_restore = if config_written {
            crate::fsx::atomic_write(&config_path, &original_config, None, false).map(|_| ())
        } else {
            Ok(())
        };
        if credential_restore.is_err() || config_restore.is_err() {
            bail!("CLI switch failed; restoring the previous login also failed. Use cswap to recover. {error:#}");
        }
        return Err(error).context("CLI switch failed; previous login restored");
    }
    Ok(())
}

fn credential(text: &str) -> Result<Value> {
    // Do not include the JSON/parser input in an error; it contains credentials.
    let data: Value = serde_json::from_str(text).map_err(|_| anyhow::anyhow!("Invalid CLI credential"))?;
    ensure!(
        data["claudeAiOauth"]["accessToken"].as_str().is_some_and(|s| !s.is_empty()),
        "Missing CLI OAuth credential; run cswap to refresh"
    );
    Ok(data)
}

fn security(service: &str, account: &str, value: Option<&str>) -> Result<String> {
    if let Some(value) = value {
        ensure!(!account.contains(['\n', '\r']), "Invalid Keychain account");
        let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        let hex: String = value.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        let command = format!("add-generic-password -U -a {} -s {} -X {hex}\n", quote(account), quote(service));
        ensure!(command.len() <= 4032, "CLI credential is too large for secure Keychain input");
        let mut child = Command::new("/usr/bin/security")
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        child.stdin.take().context("Cannot open Keychain input")?.write_all(command.as_bytes())?;
        ensure!(child.wait()?.success(), "Keychain write refused");
        // security -i can exit successfully even when its command failed.
        ensure!(security(service, account, None)? == value, "Keychain write verification failed");
        return Ok(String::new());
    }
    let output = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-a", account, "-s", service, "-w"])
        .output()?;
    ensure!(output.status.success(), "Cannot read CLI Keychain item (status {})", output.status);
    let value = String::from_utf8(output.stdout).context("Invalid Keychain text")?;
    Ok(value.strip_suffix('\n').unwrap_or(&value).to_owned())
}

#[cfg(unix)]
struct Locks {
    _store: fs::File,
    directories: Vec<std::path::PathBuf>,
    stop: std::sync::mpsc::Sender<()>,
    toucher: Option<std::thread::JoinHandle<()>>,
}

#[cfg(unix)]
impl Locks {
    fn acquire(home: &Path) -> Result<Self> {
        use std::os::fd::AsRawFd;
        let store = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(home.join(".claude-swap-backup/.lock"))?;
        // Same flock as cswap. Contention is reported, never stolen or retried.
        ensure!(
            unsafe { libc::flock(store.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "claude-swap is busy; try again when it finishes"
        );
        let (stop, received) = std::sync::mpsc::channel();
        let mut locks = Self { _store: store, directories: Vec::new(), stop, toucher: None };
        for path in
            [home.join(".claude/.oauth_refresh.lock"), home.join(".claude.lock"), home.join(".claude.json.lock")]
        {
            fs::create_dir(&path).context("Claude Code is busy; try again when it finishes")?;
            locks.directories.push(path);
        }
        let paths = locks.directories.clone();
        locks.toucher = Some(std::thread::spawn(move || {
            while received.recv_timeout(std::time::Duration::from_secs(3)).is_err() {
                for path in &paths {
                    if let Ok(file) = fs::File::open(path) {
                        let _ = file.set_modified(std::time::SystemTime::now());
                    }
                }
            }
        }));
        Ok(locks)
    }
}

#[cfg(unix)]
impl Drop for Locks {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.toucher.take() {
            let _ = thread.join();
        }
        for path in self.directories.iter().rev() {
            let _ = fs::remove_dir(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    // Format copied from claude-swap's sequence/config writers, with synthetic identities.
    const SEQUENCE: &str = r#"{"sequence":[2,3],"activeAccountNumber":3,"custom":"keep","accounts":{"2":{"email":"a@example.com","organizationUuid":"org-a"},"3":{"email":"b@example.com","organizationUuid":"org-b"}}}"#;
    const CONFIG: &str = r#"{"oauthAccount":{"emailAddress":"a@example.com","organizationUuid":"org-a"},"projects":{"/work":{"trusted":true}},"custom":17}"#;
    const LIVE: &str = r#"{"claudeAiOauth":{"accessToken":"a-refreshed","refreshToken":"a-refresh"},"mcpOAuth":{"server":"live"},"trustedDeviceToken":"a-device"}"#;
    const TARGET: &str = r#"{"claudeAiOauth":{"accessToken":"b-token","refreshToken":"b-refresh"},"pluginSecrets":{"deleted":"stale"},"trustedDeviceToken":"b-device"}"#;

    fn fixture() -> (tempfile::TempDir, BTreeMap<(String, String), String>) {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join(".claude-swap-backup");
        fs::create_dir_all(root.join("configs")).unwrap();
        fs::write(root.join("sequence.json"), SEQUENCE).unwrap();
        fs::write(home.path().join(".claude.json"), CONFIG).unwrap();
        fs::write(root.join("configs/.claude-config-3-b@example.com.json"), r#"{"oauthAccount":{"emailAddress":"b@example.com","organizationUuid":"org-b"},"projects":{"old":"discard"}}"#).unwrap();
        let keys = BTreeMap::from([
            ((ACTIVE.into(), "tester".into()), LIVE.into()),
            (
                ("claude-swap".into(), "account-2-a@example.com".into()),
                r#"{"claudeAiOauth":{"accessToken":"a-old"}}"#.into(),
            ),
            (("claude-swap".into(), "account-3-b@example.com".into()), TARGET.into()),
        ]);
        (home, keys)
    }

    fn io(
        keys: &mut BTreeMap<(String, String), String>,
        service: &str,
        account: &str,
        value: Option<&str>,
    ) -> Result<String> {
        let key = (service.to_string(), account.to_string());
        match value {
            Some(value) => {
                keys.insert(key, value.into());
                Ok(String::new())
            }
            None => keys.get(&key).cloned().context("Missing fixture Keychain item"),
        }
    }

    #[test]
    fn switch_round_trip_preserves_refreshes_and_unrelated_fields() {
        let (home, mut keys) = fixture();
        switch_with(home.path(), 3, "tester", |s, a, v| io(&mut keys, s, a, v)).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(home.path().join(".claude.json")).unwrap()).unwrap();
        assert_eq!(config["oauthAccount"]["emailAddress"], "b@example.com");
        assert_eq!(config["projects"]["/work"]["trusted"], true);
        assert_eq!(config["custom"], 17);
        assert_eq!(keys[&("claude-swap".into(), "account-2-a@example.com".into())], LIVE);
        let active: Value = serde_json::from_str(&keys[&(ACTIVE.into(), "tester".into())]).unwrap();
        assert_eq!(active["claudeAiOauth"]["accessToken"], "b-token");
        assert_eq!(active["trustedDeviceToken"], "b-device");
        assert_eq!(active["mcpOAuth"]["server"], "live");
        assert!(active.get("pluginSecrets").is_none());
        let sequence: Value =
            serde_json::from_slice(&fs::read(home.path().join(".claude-swap-backup/sequence.json")).unwrap()).unwrap();
        assert_eq!(sequence["activeAccountNumber"], 3);
        assert_eq!(sequence["custom"], "keep");
        switch_with(home.path(), 2, "tester", |s, a, v| io(&mut keys, s, a, v)).unwrap();
        let active: Value = serde_json::from_str(&keys[&(ACTIVE.into(), "tester".into())]).unwrap();
        assert_eq!(active["claudeAiOauth"]["accessToken"], "a-refreshed");
        assert!(super::super::list(home.path()).unwrap().iter().any(|a| a.slot == 2 && a.active));
    }

    #[test]
    fn refused_writes_and_partial_active_write_keep_previous_login() {
        for stage in ["backup", "active", "partial-active", "config", "sequence"] {
            let (home, mut keys) = fixture();
            let mut failed = false;
            let result = switch_with(home.path(), 3, "tester", |s, a, v| {
                if v.is_some() && !failed {
                    if (stage == "backup" && s == "claude-swap") || (stage == "active" && s == ACTIVE) {
                        failed = true;
                        bail!("write refused");
                    }
                    if s == ACTIVE {
                        failed = true;
                        match stage {
                            "partial-active" => {
                                io(&mut keys, s, a, v)?;
                                bail!("write changed item but returned error");
                            }
                            "config" => {
                                fs::rename(home.path().join(".claude.json"), home.path().join("original-config"))
                                    .unwrap();
                                fs::create_dir(home.path().join(".claude.json")).unwrap();
                            }
                            "sequence" => {
                                let p = home.path().join(".claude-swap-backup/sequence.json");
                                fs::rename(&p, p.with_extension("saved")).unwrap();
                                fs::create_dir(p).unwrap();
                            }
                            _ => {}
                        }
                    }
                }
                io(&mut keys, s, a, v)
            });
            assert!(result.is_err(), "{stage}");
            assert_eq!(keys[&(ACTIVE.into(), "tester".into())], LIVE, "{stage}");
            if stage != "config" {
                assert_eq!(fs::read_to_string(home.path().join(".claude.json")).unwrap(), CONFIG, "{stage}");
            }
            if stage != "sequence" {
                assert_eq!(
                    fs::read_to_string(home.path().join(".claude-swap-backup/sequence.json")).unwrap(),
                    SEQUENCE,
                    "{stage}"
                );
            }
        }
    }

    #[test]
    fn missing_target_or_invalid_credential_never_writes() {
        for invalid in [false, true] {
            let (home, mut keys) = fixture();
            if invalid {
                keys.insert(("claude-swap".into(), "account-3-b@example.com".into()), "broken credential".into());
            }
            let before = keys.clone();
            assert!(switch_with(home.path(), if invalid { 3 } else { 9 }, "tester", |s, a, v| io(&mut keys, s, a, v))
                .is_err());
            assert_eq!(keys, before);
            assert_eq!(fs::read_to_string(home.path().join(".claude.json")).unwrap(), CONFIG);
        }
    }

    /// Uses synthetic credentials in isolated Keychain services, never the user's login.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "writes isolated test items to the macOS Keychain"]
    fn native_keychain_rolls_back_after_sequence_write_failure() {
        let (home, keys) = fixture();
        let prefix = format!("cc-same-T-006-{}", std::process::id());
        let services = [(ACTIVE, format!("{prefix}-active")), ("claude-swap", format!("{prefix}-backup"))];
        let service = |name: &str| services.iter().find(|(s, _)| *s == name).unwrap().1.as_str();
        struct Cleanup(Vec<(String, String)>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for (service, account) in &self.0 {
                    let _ = Command::new("/usr/bin/security")
                        .args(["delete-generic-password", "-s", service, "-a", account])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
            }
        }
        let mut cleanup = Cleanup(Vec::new());
        for ((s, a), v) in &keys {
            cleanup.0.push((service(s).to_owned(), a.clone()));
            security(service(s), a, Some(v)).unwrap();
        }
        let mut injected = false;
        let result = switch_with(home.path(), 3, "tester", |s, a, v| {
            let result = security(service(s), a, v)?;
            if s == ACTIVE && v.is_some() && !injected {
                injected = true;
                let sequence = home.path().join(".claude-swap-backup/sequence.json");
                fs::rename(&sequence, sequence.with_extension("saved")).unwrap();
                fs::create_dir(sequence).unwrap();
            }
            Ok(result)
        });
        assert!(injected);
        assert!(result.unwrap_err().to_string().contains("previous login restored"));
        assert_eq!(security(service(ACTIVE), "tester", None).unwrap(), LIVE);
        assert_eq!(fs::read_to_string(home.path().join(".claude.json")).unwrap(), CONFIG);
        assert_eq!(security(service("claude-swap"), "account-2-a@example.com", None).unwrap(), LIVE);
    }
}
