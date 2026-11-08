//! Account metadata and switching using claude-swap’s existing store.

use anyhow::{Context, Result};
use serde_json::Value;
use std::{fs, io::ErrorKind, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub slot: u64,
    pub email: String,
    pub organization: String,
    pub active: bool,
}

/// Read the sequence and current CLI identity, without opening the Keychain or writing files.
/// Like `cswap list`, the active marker follows `.claude.json`, not activeAccountNumber.
pub fn list(home: &Path) -> Result<Vec<Account>> {
    let path = home.join(".claude-swap-backup/sequence.json");
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("Cannot read claude-swap account list"),
    };
    let data: Value = serde_json::from_slice(&bytes).context("Invalid claude-swap account list")?;
    let sequence = data["sequence"].as_array().context("Missing claude-swap sequence")?;
    let accounts = data["accounts"].as_object().context("Missing claude-swap accounts")?;
    let config = match fs::read(home.join(".claude.json")) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes).context("Invalid CLI account identity")?,
        Err(e) if e.kind() == ErrorKind::NotFound => Value::Null,
        Err(e) => return Err(e).context("Cannot read CLI account identity"),
    };
    let identity = &config["oauthAccount"];
    let email = identity["emailAddress"].as_str().filter(|email| !email.is_empty());
    let org = identity["organizationUuid"].as_str().unwrap_or("");
    // Preserve the first matching slot in object order, as cswap's _find_account_slot does.
    let active = accounts.iter().find_map(|(slot, account)| {
        (email.is_some()
            && account["email"].as_str() == email
            && account.get("organizationUuid").map_or(org.is_empty(), |v| v.as_str() == Some(org)))
        .then_some(slot)
    });
    sequence
        .iter()
        .map(|number| {
            let slot = number.as_u64().context("Invalid claude-swap slot number")?;
            let key = slot.to_string();
            let account = accounts.get(&key).context("Missing claude-swap slot")?;
            Ok(Account {
                slot,
                email: account["email"].as_str().unwrap_or("unknown").to_owned(),
                organization: account["organizationName"].as_str().unwrap_or("").to_owned(),
                active: active == Some(&key),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent format fixtures: sequence order, identity versus recorded active slot,
    // same email across organizations, absent store/identity, and malformed input.
    const STORE: &str = r#"{
        "sequence": [3, 1, 2], "activeAccountNumber": 2,
        "accounts": {
            "1": {"email": "same@example.com", "organizationUuid": "personal", "organizationName": "Personal"},
            "2": {"email": "same@example.com", "organizationUuid": "work", "organizationName": "Work"},
            "3": {"email": "third@example.com", "organizationName": null}
        }
    }"#;

    fn fixture() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join(".claude-swap-backup")).unwrap();
        fs::write(home.path().join(".claude-swap-backup/sequence.json"), STORE).unwrap();
        home
    }

    #[test]
    fn sequence_and_live_identity_match_cswap_without_writes() {
        let home = fixture();
        let identity = r#"{"oauthAccount":{"emailAddress":"same@example.com","organizationUuid":"personal"}}"#;
        fs::write(home.path().join(".claude.json"), identity).unwrap();
        assert_eq!(
            list(home.path()).unwrap(),
            vec![
                Account { slot: 3, email: "third@example.com".into(), organization: "".into(), active: false },
                Account { slot: 1, email: "same@example.com".into(), organization: "Personal".into(), active: true },
                Account { slot: 2, email: "same@example.com".into(), organization: "Work".into(), active: false },
            ]
        );
        assert_eq!(fs::read_to_string(home.path().join(".claude-swap-backup/sequence.json")).unwrap(), STORE);
        assert_eq!(fs::read_to_string(home.path().join(".claude.json")).unwrap(), identity);
    }

    #[test]
    fn empty_home_and_missing_identity() {
        let empty = tempfile::tempdir().unwrap();
        assert!(list(empty.path()).unwrap().is_empty());
        assert_eq!(fs::read_dir(empty.path()).unwrap().count(), 0);
        let home = fixture();
        let accounts = list(home.path()).unwrap();
        assert_eq!(accounts.len(), 3);
        assert!(accounts.iter().all(|a| !a.active));
    }

    #[test]
    fn corrupt_store_is_an_error_not_an_empty_account_list() {
        let home = fixture();
        fs::write(home.path().join(".claude-swap-backup/sequence.json"), "{").unwrap();
        assert!(list(home.path()).is_err());
    }
}

#[path = "cli_accounts_switch.rs"]
mod switching;
pub use switching::switch;
