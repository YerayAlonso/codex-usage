use crate::{
    i18n,
    model::{now, DiskState, LogEntry},
};
use regex::Regex;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::LazyLock,
};

const LOG_LIMIT: u64 = 1024 * 1024;

pub struct Storage {
    pub directory: PathBuf,
}

impl Storage {
    // Atomic rename preserves pending reset keys and schedules. Never merge or overwrite two states.
    pub fn migrate_legacy(directory: &Path) -> Result<bool, String> {
        let legacy = directory.with_file_name("dev.ciclo.codex");
        let mut migrated = false;
        if !directory.exists() && legacy.is_dir() {
            fs::rename(&legacy, directory).map_err(|e| i18n::detail("error.storage", e))?;
            migrated = true;
        }
        for suffix in [".log", ".1.log", ".2.log"] {
            let old = directory.join(format!("ciclo-codex{suffix}"));
            let new = directory.join(format!("codex-usage{suffix}"));
            if old.is_file() && !new.exists() {
                fs::rename(old, new).map_err(|e| i18n::detail("error.storage", e))?;
            }
        }
        Ok(migrated)
    }

    pub fn new(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory).map_err(|e| i18n::detail("error.storage", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(directory.join("workspace")).map_err(|e| e.to_string())?;
        Ok(Self { directory })
    }

    pub fn load(&self) -> Result<DiskState, String> {
        let file = self.directory.join("state.json");
        match fs::read_to_string(file) {
            Ok(text) => {
                serde_json::from_str(&text).map_err(|e| i18n::detail("error.corruptState", e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DiskState::default()),
            Err(e) => Err(i18n::detail("error.storage", e)),
        }
    }

    pub fn save(&self, state: &DiskState) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
        let temporary = self.directory.join("state.tmp");
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(temporary, self.directory.join("state.json"))
            .map_err(|e| i18n::detail("error.save", e))
    }

    pub fn append_log(&self, level: &str, message: &str) -> Result<LogEntry, String> {
        let log = self.directory.join("codex-usage.log");
        if fs::metadata(&log).is_ok_and(|m| m.len() >= LOG_LIMIT) {
            let oldest = self.directory.join("codex-usage.2.log");
            if oldest.exists() {
                fs::remove_file(&oldest).map_err(|e| e.to_string())?;
            }
            let previous = self.directory.join("codex-usage.1.log");
            if previous.exists() {
                fs::rename(&previous, oldest).map_err(|e| e.to_string())?;
            }
            fs::rename(&log, previous).map_err(|e| e.to_string())?;
        }
        let entry = LogEntry {
            at: now(),
            level: level.into(),
            message: redact(&i18n::render("en", message)),
            source: message.starts_with('@').then(|| redact_source(message)),
        };
        let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        writeln!(options.open(log).map_err(|e| e.to_string())?, "{line}")
            .map_err(|e| e.to_string())?;
        Ok(entry)
    }

    pub fn recent_logs(&self) -> Vec<LogEntry> {
        fs::read_to_string(self.directory.join("codex-usage.log"))
            .ok()
            .map(|text| {
                let mut entries: Vec<_> = text
                    .lines()
                    .rev()
                    .take(80)
                    .filter_map(|line| serde_json::from_str::<LogEntry>(line).ok())
                    .collect();
                entries.reverse();
                entries
            })
            .unwrap_or_default()
    }
}

pub fn redact(text: &str) -> String {
    static SECRETS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)(?:Bearer\s+[A-Za-z0-9._~+/\-=]+|sk-[A-Za-z0-9_\-]+|eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+)"#).unwrap()
    });
    SECRETS
        .replace_all(text, "[redacted]")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(4000)
        .collect()
}

// Keep structured messages valid even when diagnostics are truncated/redacted.
fn redact_source(source: &str) -> String {
    fn scrub(source: &str, depth: u8) -> String {
        if depth > 5 {
            return redact(source);
        }
        let Some((key, raw)) = source.split_once(' ') else {
            return redact(source);
        };
        let Ok(serde_json::Value::Object(mut params)) = serde_json::from_str(raw) else {
            return redact(source);
        };
        for value in params.values_mut() {
            if let Some(text) = value.as_str() {
                *value = serde_json::Value::String(if text.starts_with('@') {
                    scrub(text, depth + 1)
                } else {
                    redact(text)
                });
            }
        }
        format!("{key} {}", serde_json::Value::Object(params))
    }
    scrub(source, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_common_credentials() {
        assert_eq!(
            redact("error Bearer abc.def-123 sk-secret_123"),
            "error [redacted] [redacted]"
        );
    }

    #[test]
    fn long_structured_logs_remain_translatable_and_redacted() {
        let source = i18n::detail("error.log", format!("sk-secret {}", "x".repeat(6000)));
        let cleaned = redact_source(&source);
        let translated = i18n::render("fr", &cleaned);
        assert!(translated.starts_with("Impossible d’écrire"));
        assert!(translated.contains("[redacted]"));
        assert!(!translated.contains("sk-secret"));
        assert!(translated.len() < 4500);
    }

    #[test]
    fn state_round_trip_and_bounded_log_rotation() {
        let directory = std::env::temp_dir()
            .join("opencode")
            .join(format!("codex-usage-test-{}", uuid::Uuid::new_v4()));
        let storage = Storage::new(directory.clone()).unwrap();
        let mut state = DiskState::default();
        state.settings.automation = true;
        storage.save(&state).unwrap();
        storage.save(&state).unwrap();
        assert!(storage.load().unwrap().settings.automation);
        fs::write(
            directory.join("codex-usage.log"),
            vec![b' '; LOG_LIMIT as usize],
        )
        .unwrap();
        storage.append_log("info", "rotation").unwrap();
        assert!(directory.join("codex-usage.1.log").exists());
        assert_eq!(storage.recent_logs()[0].message, "rotation");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn migration_preserves_pending_keys_and_does_not_overwrite_existing_data() {
        let root = std::env::temp_dir()
            .join("opencode")
            .join(format!("usage-migrate-{}", uuid::Uuid::new_v4()));
        let legacy = root.join("dev.ciclo.codex");
        let destination = root.join("dev.codex.usage");
        let old = Storage::new(legacy.clone()).unwrap();
        let state = DiskState {
            pending_reset: Some(crate::model::PendingReset {
                account_id: "account".into(),
                key: "persisted-key".into(),
            }),
            ..Default::default()
        };
        old.save(&state).unwrap();
        fs::write(legacy.join("ciclo-codex.log"), "legacy log").unwrap();
        assert!(Storage::migrate_legacy(&destination).unwrap());
        assert!(!legacy.exists());
        assert!(destination.join("codex-usage.log").exists());
        let loaded = Storage::new(destination.clone()).unwrap().load().unwrap();
        assert_eq!(loaded.pending_reset.unwrap().key, "persisted-key");
        let other = Storage::new(legacy.clone()).unwrap();
        other.save(&DiskState::default()).unwrap();
        assert!(!Storage::migrate_legacy(&destination).unwrap());
        assert!(legacy.join("state.json").exists());
        assert!(Storage::new(destination)
            .unwrap()
            .load()
            .unwrap()
            .pending_reset
            .is_some());
        fs::remove_dir_all(root).unwrap();
    }
}
