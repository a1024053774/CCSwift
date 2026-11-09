//! Capture the terminal login into the existing cswap store. All callers run on workers.
#[cfg(unix)]
use super::switching::Locks;
use super::switching::{credential, keychain_user, security, ACTIVE};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::{fs, io::ErrorKind, path::Path, process::Command};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentLogin {
    pub email: String,
    pub organization_uuid: String,
    pub organization: String,
    pub credential_exists: bool,
    pub existing_slot: Option<u64>,
}

fn config(home: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(home.join(".claude.json")) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("Cannot read terminal login"),
    }
}

fn identity(config: &[u8]) -> Result<Option<CurrentLogin>> {
    let data: Value = serde_json::from_slice(config).context("Invalid terminal configuration")?;
    let oauth = &data["oauthAccount"];
    let Some(email) = oauth["emailAddress"].as_str().filter(|s| !s.is_empty()) else { return Ok(None) };
    ensure!(!email.contains(['/', '\\', '\n', '\r']), "Invalid CLI account email");
    Ok(Some(CurrentLogin {
        email: email.into(),
        organization_uuid: oauth["organizationUuid"].as_str().unwrap_or("").into(),
        organization: oauth["organizationName"].as_str().unwrap_or("").into(),
        credential_exists: false,
        existing_slot: None,
    }))
}

fn sequence(home: &Path) -> Result<Value> {
    let bytes = match fs::read(home.join(".claude-swap-backup/sequence.json")) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Ok(json!({"sequence":[],"accounts":{},"activeAccountNumber":null}))
        }
        Err(e) => return Err(e).context("Cannot read CLI store"),
    };
    let data: Value = serde_json::from_slice(&bytes).context("Invalid CLI store")?;
    ensure!(data["accounts"].is_object() && data["sequence"].is_array(), "Invalid CLI store layout");
    Ok(data)
}

fn matches(account: &Value, login: &CurrentLogin) -> bool {
    account["email"].as_str() == Some(login.email.as_str())
        && account["organizationUuid"].as_str().unwrap_or("") == login.organization_uuid
}

fn existing(data: &Value, login: &CurrentLogin) -> Result<Option<u64>> {
    data["accounts"]
        .as_object()
        .context("Missing accounts")?
        .iter()
        .find(|(_, account)| matches(account, login))
        .map(|(slot, _)| slot.parse().context("Invalid CLI slot"))
        .transpose()
}

/// Metadata only. The Keychain probe does not request the item's secret.
pub fn current_login(home: &Path) -> Result<Option<CurrentLogin>> {
    let Some(bytes) = config(home)? else { return Ok(None) };
    let Some(mut login) = identity(&bytes)? else { return Ok(None) };
    login.existing_slot = existing(&sequence(home)?, &login)?;
    login.credential_exists = keychain_exists(ACTIVE, &keychain_user()?)?;
    Ok(Some(login))
}

fn keychain_exists(service: &str, account: &str) -> Result<bool> {
    let output =
        Command::new("/usr/bin/security").args(["find-generic-password", "-s", service, "-a", account]).output()?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(44) => Ok(false),
        _ => bail!("Cannot inspect CLI Keychain item"),
    }
}

enum KeychainOp<'a> {
    Read,
    Write(&'a str),
    Delete,
}

fn keychain(service: &str, account: &str, op: KeychainOp<'_>) -> Result<Option<String>> {
    match op {
        KeychainOp::Read => {
            if !keychain_exists(service, account)? {
                return Ok(None);
            }
            security(service, account, None).map(Some)
        }
        KeychainOp::Write(value) => security(service, account, Some(value)).map(|_| None),
        KeychainOp::Delete => {
            let output = Command::new("/usr/bin/security")
                .args(["delete-generic-password", "-s", service, "-a", account])
                .output()?;
            ensure!(
                output.status.success() || output.status.code() == Some(44),
                "Cannot remove new CLI Keychain item during rollback"
            );
            Ok(None)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddResult {
    Added(u64),
    Existing(u64),
}

pub fn add_current(home: &Path) -> Result<AddResult> {
    capture(home, None)
}

pub fn update_slot(home: &Path, slot: u64) -> Result<()> {
    capture(home, Some(slot)).map(|_| ())
}

fn capture(home: &Path, slot: Option<u64>) -> Result<AddResult> {
    ensure!(cfg!(target_os = "macos"), "CLI capture is only available on macOS");
    // Refuse signed-out captures before creating a store.
    ensure!(
        config(home)?.as_deref().map(identity).transpose()?.flatten().is_some(),
        "Sign in with claude auth login first"
    );
    fs::create_dir_all(home.join(".claude-swap-backup"))?;
    fs::create_dir_all(home.join(".claude"))?;
    #[cfg(unix)]
    let _locks = Locks::acquire(home)?;
    capture_with(home, slot, &keychain_user()?, keychain)
}

fn capture_with(
    home: &Path,
    target: Option<u64>,
    user: &str,
    mut keys: impl FnMut(&str, &str, KeychainOp<'_>) -> Result<Option<String>>,
) -> Result<AddResult> {
    let bytes = config(home)?.context("Sign in with claude auth login first")?;
    let login = identity(&bytes)?.context("Sign in with claude auth login first")?;
    let mut data = sequence(home)?;
    let slot = if let Some(slot) = target {
        let account = data["accounts"].get(slot.to_string()).context("CLI slot no longer exists")?;
        ensure!(
            matches(account, &login),
            "This is not {} in the saved organization; sign in to that account first",
            account["email"].as_str().unwrap_or("the selected account")
        );
        slot
    } else {
        if let Some(slot) = existing(&data, &login)? {
            return Ok(AddResult::Existing(slot));
        }
        data["accounts"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.parse::<u64>().context("Invalid CLI slot"))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .context("CLI slot overflow")?
    };
    let live = keys(ACTIVE, user, KeychainOp::Read)?.context("No terminal credential; sign in first")?;
    credential(&live)?;
    let root = home.join(".claude-swap-backup");
    let path = root.join("configs").join(format!(".claude-config-{slot}-{}.json", login.email));
    let key = format!("account-{slot}-{}", login.email);
    let old_key = keys("claude-swap", &key, KeychainOp::Read)?;
    let old_config = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    if target.is_none() {
        let cfg: Value = serde_json::from_slice(&bytes)?;
        data["accounts"][slot.to_string()] = json!({"email":login.email,"uuid":cfg["oauthAccount"]["accountUuid"].as_str().unwrap_or(""),
            "organizationUuid":login.organization_uuid,"organizationName":login.organization,"added":now});
        let sequence = data["sequence"].as_array_mut().unwrap();
        sequence.push(slot.into());
        sequence.sort_by_key(|v| v.as_u64());
    }
    data["activeAccountNumber"] = slot.into();
    data["lastUpdated"] = now.into();
    fs::create_dir_all(root.join("configs"))?;
    let result = (|| -> Result<()> {
        keys("claude-swap", &key, KeychainOp::Write(&live))?;
        crate::fsx::atomic_write(&path, &bytes, None, false)?;
        crate::fsx::atomic_write(&root.join("sequence.json"), &serde_json::to_vec_pretty(&data)?, None, false)?;
        Ok(())
    })();
    if let Err(error) = result {
        let restored_key = keys(
            "claude-swap",
            &key,
            match old_key.as_deref() {
                Some(value) => KeychainOp::Write(value),
                None => KeychainOp::Delete,
            },
        );
        let restored_config = match old_config {
            Some(bytes) => crate::fsx::atomic_write(&path, &bytes, None, false).map(|_| ()),
            None => match fs::remove_file(&path) {
                Err(e) if e.kind() != ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        };
        ensure!(
            restored_key.is_ok() && restored_config.is_ok(),
            "CLI save failed and rollback failed; restore the store backup"
        );
        return Err(error).context("CLI save failed; saved login restored");
    }
    Ok(AddResult::Added(slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    const CONFIG: &str = r#"{"oauthAccount":{"emailAddress":"capture@example.invalid","organizationUuid":"org-a","organizationName":"Personal","accountUuid":"uuid-a"},"custom":17}"#;
    const LIVE: &str = r#"{"claudeAiOauth":{"accessToken":"synthetic-access","refreshToken":"synthetic-refresh","expiresAt":9999999999999}}"#;
    fn fixture() -> (tempfile::TempDir, BTreeMap<String, String>) {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude-swap-backup/configs")).unwrap();
        fs::write(home.path().join(".claude.json"), CONFIG).unwrap();
        (home, BTreeMap::from([(ACTIVE.into(), LIVE.into())]))
    }
    fn io(
        keys: &mut BTreeMap<String, String>,
        service: &str,
        account: &str,
        op: KeychainOp<'_>,
    ) -> Result<Option<String>> {
        let key = if service == ACTIVE { ACTIVE.to_string() } else { account.to_string() };
        Ok(match op {
            KeychainOp::Read => keys.get(&key).cloned(),
            KeychainOp::Write(value) => {
                keys.insert(key, value.into());
                None
            }
            KeychainOp::Delete => {
                keys.remove(&key);
                None
            }
        })
    }
    #[test]
    fn add_writes_cswap_layout_and_duplicate_writes_nothing() {
        let (home, mut keys) = fixture();
        assert_eq!(
            capture_with(home.path(), None, "tester", |s, a, o| io(&mut keys, s, a, o)).unwrap(),
            AddResult::Added(1)
        );
        let sequence_path = home.path().join(".claude-swap-backup/sequence.json");
        let before = fs::read(&sequence_path).unwrap();
        let data: Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(data["sequence"], json!([1]));
        assert_eq!(data["activeAccountNumber"], 1);
        assert_eq!(data["accounts"]["1"]["email"], "capture@example.invalid");
        assert_eq!(data["accounts"]["1"]["uuid"], "uuid-a");
        assert_eq!(data["accounts"]["1"]["organizationUuid"], "org-a");
        assert!(data["accounts"]["1"]["added"].is_string());
        assert_eq!(keys["account-1-capture@example.invalid"], LIVE);
        assert_eq!(
            fs::read_to_string(
                home.path().join(".claude-swap-backup/configs/.claude-config-1-capture@example.invalid.json")
            )
            .unwrap(),
            CONFIG
        );
        assert_eq!(
            capture_with(home.path(), None, "tester", |_, _, _| panic!("duplicate must not touch Keychain")).unwrap(),
            AddResult::Existing(1)
        );
        assert_eq!(fs::read(sequence_path).unwrap(), before);
    }
    #[test]
    fn update_refuses_different_email_or_organization_and_preserves_slot() {
        let (home, mut keys) = fixture();
        capture_with(home.path(), None, "tester", |s, a, o| io(&mut keys, s, a, o)).unwrap();
        for config in [CONFIG.replace("capture@", "other@"), CONFIG.replace("org-a", "org-b")] {
            fs::write(home.path().join(".claude.json"), config).unwrap();
            assert!(capture_with(home.path(), Some(1), "tester", |_, _, _| panic!("mismatch must not touch Keychain"))
                .unwrap_err()
                .to_string()
                .contains("This is not"));
        }
        fs::write(home.path().join(".claude.json"), CONFIG).unwrap();
        keys.insert(ACTIVE.into(), LIVE.replace("synthetic-access", "renewed-access"));
        capture_with(home.path(), Some(1), "tester", |s, a, o| io(&mut keys, s, a, o)).unwrap();
        assert_eq!(keys["account-1-capture@example.invalid"], keys[ACTIVE]);
        assert_eq!(sequence(home.path()).unwrap()["sequence"], json!([1]));
    }
    #[test]
    fn failed_sequence_write_restores_saved_credential_and_config() {
        for update in [false, true] {
            let (home, mut keys) = fixture();
            if update {
                capture_with(home.path(), None, "tester", |s, a, o| io(&mut keys, s, a, o)).unwrap();
            }
            let original = keys.clone();
            keys.insert(ACTIVE.into(), LIVE.replace("synthetic-access", "renewed-access"));
            let mut injected = false;
            let result = capture_with(home.path(), update.then_some(1), "tester", |s, a, o| {
                let writing = matches!(o, KeychainOp::Write(_));
                let result = io(&mut keys, s, a, o);
                if writing && !injected {
                    injected = true;
                    let path = home.path().join(".claude-swap-backup/sequence.json");
                    if path.exists() {
                        fs::rename(&path, path.with_extension("original")).unwrap();
                    }
                    fs::create_dir(path).unwrap();
                }
                result
            });
            assert!(result.unwrap_err().to_string().contains("saved login restored"));
            assert_eq!(
                keys.get("account-1-capture@example.invalid"),
                original.get("account-1-capture@example.invalid")
            );
            let path = home.path().join(".claude-swap-backup/configs/.claude-config-1-capture@example.invalid.json");
            if update {
                assert_eq!(fs::read_to_string(path).unwrap(), CONFIG);
            } else {
                assert!(!path.exists());
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn sequence_refusal_preserves_bytes_active_marker_and_saved_login() {
        let (home, mut keys) = fixture();
        capture_with(home.path(), None, "tester", |s, a, o| io(&mut keys, s, a, o)).unwrap();
        let path = home.path().join(".claude-swap-backup/sequence.json");
        let mut data = sequence(home.path()).unwrap();
        // A stale recorded active number is valid cswap input; it must survive a failed update.
        data["activeAccountNumber"] = 2.into();
        let before = serde_json::to_vec_pretty(&data).unwrap();
        let backing = path.with_extension("original");
        fs::write(&backing, &before).unwrap();
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&backing, &path).unwrap();
        let original_keys = keys.clone();
        keys.insert(ACTIVE.into(), LIVE.replace("synthetic-access", "renewed-access"));
        fs::write(home.path().join(".claude.json"), CONFIG.replace("17", "99")).unwrap();
        let error = capture_with(home.path(), Some(1), "tester", |s, a, o| io(&mut keys, s, a, o)).unwrap_err();
        assert!(format!("{error:#}").contains("refusing to write through a symlink"));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(sequence(home.path()).unwrap()["activeAccountNumber"], 2);
        assert_eq!(keys["account-1-capture@example.invalid"], original_keys["account-1-capture@example.invalid"]);
        assert_eq!(
            fs::read_to_string(
                home.path().join(".claude-swap-backup/configs/.claude-config-1-capture@example.invalid.json")
            )
            .unwrap(),
            CONFIG
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn capture_obeys_cswap_and_each_claude_lock_without_stealing_them() {
        let (home, mut keys) = fixture();
        // An existing identity guarantees even a mutant that skips locks cannot read/write
        // real Keychain credentials: capture_with returns Existing before the secret boundary.
        capture_with(home.path(), None, "tester", |s, a, o| io(&mut keys, s, a, o)).unwrap();
        fs::create_dir(home.path().join(".claude")).unwrap();
        let before = fs::read(home.path().join(".claude-swap-backup/sequence.json")).unwrap();
        {
            let _held = Locks::acquire(home.path()).unwrap();
            assert!(add_current(home.path()).unwrap_err().to_string().contains("claude-swap is busy"));
        }
        for relative in [".claude/.oauth_refresh.lock", ".claude.lock", ".claude.json.lock"] {
            let path = home.path().join(relative);
            fs::create_dir(&path).unwrap();
            fs::write(path.join("owner"), "another process").unwrap();
            assert!(add_current(home.path()).unwrap_err().to_string().contains("Claude Code is busy"));
            assert_eq!(fs::read_to_string(path.join("owner")).unwrap(), "another process");
            fs::remove_file(path.join("owner")).unwrap();
            fs::remove_dir(path).unwrap();
        }
        assert_eq!(fs::read(home.path().join(".claude-swap-backup/sequence.json")).unwrap(), before);
    }

    /// Uses only synthetic credentials and removes every created native item, even on panic.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "writes and cleans up isolated native Keychain items; requires CSWAP_TEST_BIN"]
    fn sandbox_add_is_readable_by_cswap() {
        let (home, _) = fixture();
        let email = format!("t012-{}@example.invalid", std::process::id());
        fs::write(home.path().join(".claude.json"), CONFIG.replace("capture@example.invalid", &email)).unwrap();
        let active_service = format!("CCSwift-T012-{}", std::process::id());
        let account = format!("account-1-{email}");
        struct Cleanup(Vec<(String, String)>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for (s, a) in &self.0 {
                    let _ = keychain(s, a, KeychainOp::Delete);
                }
            }
        }
        assert!(!keychain_exists(&active_service, "tester").unwrap());
        assert!(!keychain_exists("claude-swap", &account).unwrap());
        println!("security before: isolated active and slot items absent (44)");
        let cleanup = Cleanup(vec![(active_service.clone(), "tester".into()), ("claude-swap".into(), account.clone())]);
        security(&active_service, "tester", Some(LIVE)).unwrap();
        assert_eq!(
            capture_with(home.path(), None, "tester", |s, a, o| keychain(
                if s == ACTIVE { &active_service } else { s },
                a,
                o
            ))
            .unwrap(),
            AddResult::Added(1)
        );
        assert_eq!(security("claude-swap", &account, None).unwrap(), LIVE);
        let output = Command::new(std::env::var("CSWAP_TEST_BIN").expect("absolute cswap path"))
            .args(["list", "--json"])
            .env("HOME", home.path())
            .env("USER", &active_service)
            .env_remove("CLAUDE_CONFIG_DIR")
            .output()
            .unwrap();
        assert!(output.status.success(), "cswap list failed");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(&email), "cswap list must read the added identity");
        println!("cswap list --json: success; added identity present");
        drop(cleanup);
        assert!(!keychain_exists(&active_service, "tester").unwrap());
        assert!(!keychain_exists("claude-swap", &account).unwrap());
        println!("security after: isolated active and slot items absent (44)");
    }
}
