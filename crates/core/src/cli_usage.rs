//! Read-only CLI usage. Call on a worker; never refresh or write credentials.
use crate::{cli_accounts::Account, fsx, usage::Usage};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    Unavailable,
    RenewsOnSwitch,
    Expired,
}

impl Problem {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::RenewsOnSwitch => "Renews on switch",
            Self::Expired => "Expired",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub resets_at: [Option<String>; 2],
    pub usage: Option<Usage>,
    pub problem: Option<Problem>,
    attempted: Option<Instant>,
}

impl Reading {
    pub fn status(&self) -> &'static str {
        match self.problem {
            Some(Problem::RenewsOnSwitch) => "Renews on switch",
            Some(Problem::Expired) => "Expired",
            Some(Problem::Unavailable) => "Unknown",
            None if self.usage.is_some() => "Valid",
            None => "Unknown",
        }
    }

    pub fn reset_label(&self, window: usize) -> String {
        self.resets_at[window]
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|at| {
                at.with_timezone(&chrono::Local)
                    .format(if window == 0 { "%H:%M 重置" } else { "%m月%d日 %H:%M 重置" })
                    .to_string()
            })
            .unwrap_or_else(|| "—".into())
    }
}

/// Lives with the app store, so reopening the window does not reset the throttle.
#[derive(Clone, Default)]
pub struct Cache {
    pub readings: BTreeMap<u64, Reading>,
}

impl Cache {
    pub fn refresh(&mut self, accounts: &[Account]) {
        self.refresh_with(accounts, fetch);
    }

    fn refresh_with(&mut self, accounts: &[Account], mut fetch: impl FnMut(&Account) -> Result<Reading, Problem>) {
        for account in accounts {
            let entry = self.readings.entry(account.slot).or_default();
            if entry.attempted.is_some_and(|at| at.elapsed() < Duration::from_secs(60)) {
                continue;
            }
            entry.attempted = Some(Instant::now());
            match fetch(account) {
                Ok(reading) => {
                    entry.usage = reading.usage;
                    entry.resets_at = reading.resets_at;
                    entry.problem = None;
                }
                Err(problem) => entry.problem = Some(problem),
            }
        }
    }
}

/// The Keychain item that holds `account`'s current login. The active account's saved copy goes
/// stale once Claude Code renews the live login (claude-swap only saves on switch), so the
/// active account reads the live item, as `claude` itself does.
fn credential_item(account: &Account, user: &str) -> (&'static str, String) {
    if account.active {
        ("Claude Code-credentials", user.to_string())
    } else {
        ("claude-swap", format!("account-{}-{}", account.slot, account.email))
    }
}

fn os_user() -> Result<String, Problem> {
    let output = Command::new("/usr/bin/id").arg("-un").output().map_err(|_| Problem::Unavailable)?;
    if !output.status.success() {
        return Err(Problem::Unavailable);
    }
    String::from_utf8(output.stdout).map(|s| s.trim().to_string()).map_err(|_| Problem::Unavailable)
}

fn fetch(account: &Account) -> Result<Reading, Problem> {
    let (service, item) = credential_item(account, &os_user()?);
    let output = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", service, "-a", &item, "-w"])
        .output()
        .map_err(|_| Problem::Unavailable)?;
    if !output.status.success() {
        return Err(Problem::Unavailable);
    }
    let data: Value = serde_json::from_slice(&output.stdout).map_err(|_| Problem::Unavailable)?;
    let token = access_token(&data, fsx::now_secs())?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| Problem::Unavailable)?;
    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "claude-swap/1.0")
        .send()
        .map_err(|_| Problem::Unavailable)?;
    response_status(response.status().as_u16())?;
    let bytes = response.bytes().map_err(|_| Problem::Unavailable)?;
    parse(&bytes, fsx::now_secs())
}

fn access_token(data: &Value, now: f64) -> Result<&str, Problem> {
    let oauth = &data["claudeAiOauth"];
    if oauth["expiresAt"].as_f64().is_some_and(|at| at <= now * 1000.0) {
        return Err(if oauth["refreshToken"].as_str().is_some_and(|s| !s.is_empty()) {
            Problem::RenewsOnSwitch
        } else {
            Problem::Unavailable
        });
    }
    oauth["accessToken"].as_str().filter(|s| !s.is_empty()).ok_or(Problem::Unavailable)
}

fn response_status(status: u16) -> Result<(), Problem> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(Problem::Expired),
        _ => Err(Problem::Unavailable),
    }
}

fn parse(bytes: &[u8], at: f64) -> Result<Reading, Problem> {
    let data: Value = serde_json::from_slice(bytes).map_err(|_| Problem::Unavailable)?;
    if !data.is_object() || (data.get("five_hour").is_none() && data.get("seven_day").is_none()) {
        return Err(Problem::Unavailable);
    }
    let window = |key: &str| -> Result<Option<f64>, Problem> {
        let value = &data[key];
        if value.is_null() {
            return Ok(None);
        }
        let percent = value.get("utilization").ok_or(Problem::Unavailable)?;
        if percent.is_null() {
            return Ok(None);
        }
        percent.as_f64().filter(|n| n.is_finite() && *n >= 0.0).map(Some).ok_or(Problem::Unavailable)
    };
    Ok(Reading {
        usage: Some(Usage { at, five_hour: window("five_hour")?, weekly: window("seven_day")? }),
        resets_at: ["five_hour", "seven_day"].map(|key| data[key]["resets_at"].as_str().map(str::to_owned)),
        ..Reading::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_classification_gates_network_on_expired_access() {
        let data =
            serde_json::json!({"claudeAiOauth":{"accessToken":"synthetic","expiresAt":1000,"refreshToken":"refresh"}});
        // fetch uses this gate before constructing its HTTP client.
        assert_eq!(access_token(&data, 2.), Err(Problem::RenewsOnSwitch));
        assert_eq!(access_token(&data, 0.), Ok("synthetic"));
        assert_eq!(response_status(401), Err(Problem::Expired));
        assert_eq!(response_status(403), Err(Problem::Expired));
        assert_eq!(response_status(429), Err(Problem::Unavailable));
        assert_eq!(Reading::default().status(), "Unknown");
    }

    // Fails if the active account reads its saved copy: Claude Code renews only the live login,
    // so the saved copy expires hours before the account does.
    #[test]
    fn active_account_reads_the_live_login() {
        let mut account = Account {
            slot: 2,
            email: "ada@example.com".into(),
            organization: "".into(),
            organization_uuid: "".into(),
            active: true,
        };
        assert_eq!(credential_item(&account, "ada"), ("Claude Code-credentials", "ada".to_string()));
        account.active = false;
        assert_eq!(credential_item(&account, "ada"), ("claude-swap", "account-2-ada@example.com".to_string()));
    }

    // Contract failures: repeat requests within 60s (including duplicate emails and failures),
    // lost last reading, error bodies treated as zero, nullable windows treated as zero.
    #[test]
    fn throttle_and_offline_preserve_last_reading() {
        let accounts = vec![
            Account {
                slot: 2,
                email: "ada@example.com".into(),
                organization: "".into(),
                organization_uuid: "".into(),
                active: true,
            },
            Account {
                slot: 3,
                email: "ada@example.com".into(),
                organization: "".into(),
                organization_uuid: "".into(),
                active: false,
            },
        ];
        let mut cache = Cache::default();
        let mut calls = 0;
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Ok(Reading {
                usage: Some(Usage { at: 100.0, five_hour: Some(12.0), weekly: Some(34.0) }),
                ..Reading::default()
            })
        });
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Err(Problem::Unavailable)
        });
        assert_eq!(calls, 2, "each slot has independent credentials; reopening is throttled");
        let entry = cache.readings.get_mut(&2).unwrap();
        entry.attempted = Some(Instant::now() - Duration::from_secs(61));
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Err(Problem::Unavailable)
        });
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Err(Problem::Unavailable)
        });
        assert_eq!(calls, 3, "failed requests must also be throttled");
        let entry = &cache.readings[&2];
        assert_eq!(entry.usage, Some(Usage { at: 100.0, five_hour: Some(12.0), weekly: Some(34.0) }));
        assert_eq!(entry.problem, Some(Problem::Unavailable));
    }

    // Reset fields can be dropped, swapped, null, or malformed. Unknown resets must
    // remain unknown without losing an otherwise valid percentage reading.
    #[test]
    fn retains_window_resets() {
        let reading = parse(br#"{"five_hour":{"utilization":12,"resets_at":"2026-11-08T23:00:00Z"},"seven_day":{"utilization":73,"resets_at":"2026-11-12T04:00:00Z"}}"#, 123.).unwrap();
        assert_eq!(reading.resets_at, [Some("2026-11-08T23:00:00Z".into()), Some("2026-11-12T04:00:00Z".into())]);
        let unknown = parse(
            br#"{"five_hour":{"utilization":0,"resets_at":null},"seven_day":{"utilization":73,"resets_at":"invalid"}}"#,
            123.,
        )
        .unwrap();
        assert_eq!(unknown.reset_label(0), "—");
        assert_eq!(unknown.reset_label(1), "—");
        assert_eq!(unknown.usage.unwrap().weekly, Some(73.));
    }

    #[test]
    fn response_percentages_and_nullable_windows() {
        let result = parse(br#"{"five_hour":{"utilization":12.5},"seven_day":{"utilization":64}}"#, 123.).unwrap();
        assert_eq!(result.status(), "Valid");
        assert_eq!(result.usage.unwrap(), Usage { at: 123., five_hour: Some(12.5), weekly: Some(64.) });
        assert_eq!(
            parse(br#"{"five_hour":{"utilization":null},"seven_day":null}"#, 1.).unwrap().usage.unwrap().five_hour,
            None
        );
        for body in [
            r#"{"error":"unauthorized"}"#,
            r#"{"five_hour":{}}"#,
            r#"{"five_hour":{"utilization":-1}}"#,
            r#"{"seven_day":{"utilization":"50"}}"#,
        ] {
            assert_eq!(parse(body.as_bytes(), 1.), Err(Problem::Unavailable));
        }
    }
}
