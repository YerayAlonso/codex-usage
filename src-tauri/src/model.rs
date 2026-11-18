use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub const FIVE_HOURS: u64 = 5 * 60 * 60;
pub const POLL_INTERVAL: u64 = 5 * 60;

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub automation: bool,
    pub model: String,
    pub codex_path: String,
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            automation: false,
            model: "gpt-6-luna".into(),
            codex_path: String::new(),
            language: "auto".into(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.language != "auto" && !crate::i18n::LANGUAGES.contains(&self.language.as_str()) {
            return Err("@error.language".into());
        }
        if self.model.trim().is_empty()
            || self.model.len() > 120
            || self.model.chars().any(char::is_control)
        {
            return Err("@error.model".into());
        }
        if self.codex_path.len() > 4096 || self.codex_path.chars().any(char::is_control) {
            return Err("@error.path".into());
        }
        if !self.codex_path.is_empty() {
            let path = std::path::Path::new(&self.codex_path);
            if !path.is_absolute() || !path.is_file() {
                return Err("@error.absolutePath".into());
            }
            #[cfg(windows)]
            if !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            {
                return Err("@error.exe".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub used: f64,
    pub minutes: u64,
    pub resets_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub windows: Vec<QuotaWindow>,
    pub credits: Option<u64>,
    pub expires_at: Option<u64>,
    pub account_id: Option<String>,
    pub updated_at: u64,
}

pub fn parse_usage(value: &Value, timestamp: u64) -> Result<Usage, String> {
    let snapshot = value
        .pointer("/rateLimitsByLimitId/codex")
        .filter(|v| !v.is_null())
        .or_else(|| value.get("rateLimits"))
        .ok_or("@error.noQuota")?;
    let windows: Vec<_> = ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let window = snapshot.get(key)?;
            let used = window.get("usedPercent")?.as_f64()?;
            used.is_finite().then(|| QuotaWindow {
                used: used.clamp(0.0, 100.0),
                minutes: window
                    .get("windowDurationMins")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                resets_at: window.get("resetsAt").and_then(Value::as_u64).unwrap_or(0),
            })
        })
        .collect();
    if windows.is_empty() {
        return Err("@error.noWindows".into());
    }
    let summary = value.get("rateLimitResetCredits");
    let credits = summary
        .and_then(|s| s.get("availableCount"))
        .and_then(Value::as_u64);
    let expires_at = summary
        .and_then(|s| s.get("credits"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|credit| credit.get("status").and_then(Value::as_str) == Some("available"))
        .filter_map(|credit| credit.get("expiresAt").and_then(Value::as_u64))
        .filter(|expiry| *expiry > timestamp)
        .min()
        .filter(|_| credits.is_some_and(|n| n > 0));
    Ok(Usage {
        windows,
        credits,
        expires_at,
        account_id: value
            .get("accountId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        updated_at: timestamp,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingReset {
    pub account_id: String,
    pub key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiskState {
    pub settings: Settings,
    pub last_prompt_at: Option<u64>,
    pub next_prompt_at: Option<u64>,
    pub pending_reset: Option<PendingReset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub at: u64,
    pub level: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub locale: String,
    pub settings: Settings,
    pub usage: Option<Usage>,
    pub error: Option<String>,
    pub busy: Option<String>,
    pub last_prompt_at: Option<u64>,
    pub next_prompt_at: Option<u64>,
    pub pending_reset: bool,
    pub autostart: bool,
    pub logs: Vec<LogEntry>,
}

pub fn is_stale(usage: &Usage, timestamp: u64) -> bool {
    timestamp.saturating_sub(usage.updated_at) > POLL_INTERVAL * 2
        || usage
            .windows
            .iter()
            .any(|w| w.resets_at > 0 && timestamp >= w.resets_at)
}

// At most one catch-up after sleep. The timestamp is persisted before starting a prompt.
pub fn due(disk: &DiskState, timestamp: u64) -> bool {
    disk.settings.automation && disk.next_prompt_at.is_none_or(|next| timestamp >= next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_actual_windows_and_clamps_percentages() {
        let value = json!({"rateLimitsByLimitId":{"codex":{
            "primary":{"usedPercent":-1,"windowDurationMins":300,"resetsAt":1000},
            "secondary":{"usedPercent":130,"windowDurationMins":10080}
        }},"accountId":"a","rateLimitResetCredits":{"availableCount":2,"credits":[
            {"status":"available","expiresAt":900},{"status":"available","expiresAt":800},
            {"status":"used","expiresAt":700},{"status":"available","expiresAt":1}
        ]}});
        let usage = parse_usage(&value, 100).unwrap();
        assert_eq!(usage.windows[0].used, 0.0);
        assert_eq!(usage.windows[1].used, 100.0);
        assert_eq!(usage.expires_at, Some(800));
        assert_eq!(usage.credits, Some(2));
    }

    #[test]
    fn unknown_is_not_zero_and_only_returned_windows_exist() {
        let usage =
            parse_usage(&json!({"rateLimits":{"primary":{"usedPercent":42}}}), 100).unwrap();
        assert_eq!(usage.credits, None);
        assert_eq!(usage.windows.len(), 1);
        assert!(parse_usage(&json!({"rateLimits":{}}), 0).is_err());
    }

    #[test]
    fn default_never_runs_prompts() {
        assert!(!due(&DiskState::default(), u64::MAX));
        let mut disk = DiskState::default();
        disk.settings.automation = true;
        disk.next_prompt_at = Some(100);
        assert!(!due(&disk, 99));
        assert!(due(&disk, 100));
        disk.next_prompt_at = Some(100 + FIVE_HOURS);
        assert!(!due(&disk, 101));
    }

    #[test]
    fn uncertain_reset_survives_restart() {
        let disk = DiskState {
            pending_reset: Some(PendingReset {
                account_id: "a".into(),
                key: "stable-key".into(),
            }),
            ..Default::default()
        };
        let loaded: DiskState =
            serde_json::from_str(&serde_json::to_string(&disk).unwrap()).unwrap();
        assert_eq!(loaded.pending_reset.unwrap().key, "stable-key");
    }

    #[test]
    fn expired_or_old_data_is_stale() {
        let usage = parse_usage(
            &json!({"rateLimits":{"primary":{"usedPercent":0,"resetsAt":200}}}),
            100,
        )
        .unwrap();
        assert!(!is_stale(&usage, 150));
        assert!(is_stale(&usage, 200));
        assert!(is_stale(&usage, 1000));
    }
}
