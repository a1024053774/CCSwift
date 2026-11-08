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
    NeedsRefresh,
}

impl Problem {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::NeedsRefresh => "needs cswap refresh",
        }
    }
}

#[derive(Clone, Default)]
pub struct Reading {
    pub usage: Option<Usage>,
    pub problem: Option<Problem>,
    attempted: Option<Instant>,
}

/// Lives with the app store, so reopening the window does not reset the throttle.
#[derive(Clone, Default)]
pub struct Cache {
    pub readings: BTreeMap<String, Reading>,
}

impl Cache {
    pub fn refresh(&mut self, accounts: &[Account]) {
        self.refresh_with(accounts, fetch);
    }

    fn refresh_with(&mut self, accounts: &[Account], mut fetch: impl FnMut(&Account) -> Result<Usage, Problem>) {
        for account in accounts {
            let entry = self.readings.entry(account.email.clone()).or_default();
            if entry.attempted.is_some_and(|at| at.elapsed() < Duration::from_secs(60)) {
                continue;
            }
            entry.attempted = Some(Instant::now());
            match fetch(account) {
                Ok(usage) => {
                    entry.usage = Some(usage);
                    entry.problem = None;
                }
                Err(problem) => entry.problem = Some(problem),
            }
        }
    }
}

fn fetch(account: &Account) -> Result<Usage, Problem> {
    let mut command = Command::new("/usr/bin/security");
    command.arg("find-generic-password");
    if account.active {
        command.args(["-s", "Claude Code-credentials"]);
    } else {
        command.args(["-s", "claude-swap", "-a", &format!("account-{}-{}", account.slot, account.email)]);
    }
    let output = command.arg("-w").output().map_err(|_| Problem::Unavailable)?;
    if !output.status.success() {
        return Err(Problem::Unavailable);
    }
    let data: Value = serde_json::from_slice(&output.stdout).map_err(|_| Problem::Unavailable)?;
    let oauth = &data["claudeAiOauth"];
    if oauth["expiresAt"].as_f64().is_some_and(|at| at <= fsx::now_secs() * 1000.0) {
        return Err(Problem::NeedsRefresh);
    }
    let token = oauth["accessToken"].as_str().filter(|s| !s.is_empty()).ok_or(Problem::NeedsRefresh)?;
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
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Problem::NeedsRefresh);
    }
    if !response.status().is_success() {
        return Err(Problem::Unavailable);
    }
    let bytes = response.bytes().map_err(|_| Problem::Unavailable)?;
    parse(&bytes, fsx::now_secs())
}

fn parse(bytes: &[u8], at: f64) -> Result<Usage, Problem> {
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
    Ok(Usage { at, five_hour: window("five_hour")?, weekly: window("seven_day")? })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Contract failures: repeat requests within 60s (including duplicate emails and failures),
    // lost last reading, error bodies treated as zero, nullable windows treated as zero.
    #[test]
    fn throttle_and_offline_preserve_last_reading() {
        let accounts = vec![
            Account { slot: 2, email: "ada@example.com".into(), organization: "".into(), active: true },
            Account { slot: 3, email: "ada@example.com".into(), organization: "".into(), active: false },
        ];
        let mut cache = Cache::default();
        let mut calls = 0;
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Ok(Usage { at: 100.0, five_hour: Some(12.0), weekly: Some(34.0) })
        });
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Err(Problem::Unavailable)
        });
        assert_eq!(calls, 1, "open + refresh + duplicate slot must make one request");
        let entry = cache.readings.get_mut("ada@example.com").unwrap();
        entry.attempted = Some(Instant::now() - Duration::from_secs(61));
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Err(Problem::Unavailable)
        });
        cache.refresh_with(&accounts, |_| {
            calls += 1;
            Err(Problem::Unavailable)
        });
        assert_eq!(calls, 2, "failed requests must also be throttled");
        let entry = &cache.readings["ada@example.com"];
        assert_eq!(entry.usage, Some(Usage { at: 100.0, five_hour: Some(12.0), weekly: Some(34.0) }));
        assert_eq!(entry.problem, Some(Problem::Unavailable));
    }

    #[test]
    fn response_percentages_and_nullable_windows() {
        let result = parse(br#"{"five_hour":{"utilization":12.5},"seven_day":{"utilization":64}}"#, 123.).unwrap();
        assert_eq!(result, Usage { at: 123., five_hour: Some(12.5), weekly: Some(64.) });
        assert_eq!(parse(br#"{"five_hour":{"utilization":null},"seven_day":null}"#, 1.).unwrap().five_hour, None);
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
