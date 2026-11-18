use crate::{
    codex, i18n,
    model::{self, DiskState, LogEntry, PendingReset, Settings, Snapshot, Usage},
    storage::Storage,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tokio::sync::{Mutex as AsyncMutex, Notify, OwnedMutexGuard};

pub struct Inner {
    pub disk: DiskState,
    pub usage: Option<Usage>,
    pub error: Option<String>,
    pub busy: Option<String>,
    pub autostart: bool,
    pub logs: Vec<LogEntry>,
}

pub struct AppState {
    pub inner: Mutex<Inner>,
    pub storage: Storage,
    gate: Arc<AsyncMutex<()>>,
    wake: Notify,
}

impl AppState {
    pub fn new(storage: Storage, disk: DiskState, autostart: bool) -> Self {
        let logs = storage.recent_logs();
        Self {
            inner: Mutex::new(Inner {
                disk,
                usage: None,
                error: None,
                busy: None,
                autostart,
                logs,
            }),
            storage,
            gate: Arc::new(AsyncMutex::new(())),
            wake: Notify::new(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock().unwrap();
        let locale = i18n::language(&inner.disk.settings.language);
        Snapshot {
            locale: locale.into(),
            settings: inner.disk.settings.clone(),
            usage: inner.usage.clone(),
            error: inner
                .error
                .as_ref()
                .map(|source| i18n::render(locale, source)),
            busy: inner
                .busy
                .as_ref()
                .map(|source| i18n::render(locale, source)),
            last_prompt_at: inner.disk.last_prompt_at,
            next_prompt_at: inner.disk.next_prompt_at,
            pending_reset: inner.disk.pending_reset.is_some(),
            autostart: inner.autostart,
            logs: inner
                .logs
                .iter()
                .cloned()
                .map(|mut entry| {
                    entry.message =
                        i18n::render(locale, entry.source.as_deref().unwrap_or(&entry.message));
                    entry
                })
                .collect(),
        }
    }

    pub fn log(&self, level: &str, message: &str) {
        let mut inner = self.inner.lock().unwrap();
        match self.storage.append_log(level, message) {
            Ok(entry) => inner.logs.push(entry),
            Err(error) => {
                let warning = i18n::detail("error.log", error);
                inner.error = Some(warning.clone());
                inner.logs.push(LogEntry {
                    at: model::now(),
                    level: "error".into(),
                    message: warning,
                    source: None,
                });
            }
        }
        if inner.logs.len() > 80 {
            let excess = inner.logs.len() - 80;
            inner.logs.drain(..excess);
        }
    }

    pub fn publish(&self, app: &AppHandle) {
        let snapshot = self.snapshot();
        crate::tray::update(app, &snapshot);
        let _ = app.emit("state-changed", snapshot);
    }

    fn begin(&self, app: &AppHandle, operation: &str) -> Result<OwnedMutexGuard<()>, String> {
        let guard = self
            .gate
            .clone()
            .try_lock_owned()
            .map_err(|_| "@error.busy".to_string())?;
        self.inner.lock().unwrap().busy = Some(operation.into());
        self.publish(app);
        Ok(guard)
    }

    fn finish(&self, app: &AppHandle, result: &Result<String, String>) {
        self.inner.lock().unwrap().busy = None;
        match result {
            Ok(message) => self.log("info", message),
            Err(error) => {
                self.inner.lock().unwrap().error = Some(error.clone());
                self.log("error", error);
            }
        }
        self.publish(app);
    }

    async fn fetch_usage(&self) -> Result<Usage, String> {
        let settings = self.inner.lock().unwrap().disk.settings.clone();
        let result = codex::rpc(
            &settings,
            &self.storage.directory.join("workspace"),
            "account/rateLimits/read",
            None,
        )
        .await?;
        model::parse_usage(&result, model::now())
    }

    pub async fn refresh(&self, app: &AppHandle) -> Result<(), String> {
        let _guard = self.begin(app, "@operation.refresh")?;
        let result = self.fetch_usage().await.map(|usage| {
            let mut inner = self.inner.lock().unwrap();
            inner.usage = Some(usage);
            inner.error = None;
            "@log.refreshed".to_string()
        });
        self.finish(app, &result);
        result.map(|_| ())
    }

    pub async fn run_prompt(&self, app: &AppHandle) -> Result<(), String> {
        let _guard = self.begin(app, "@operation.prompt")?;
        let result = self.prompt_inner(app).await;
        self.finish(app, &result);
        result.map(|_| ())
    }

    async fn prompt_inner(&self, app: &AppHandle) -> Result<String, String> {
        let settings = {
            let mut inner = self.inner.lock().unwrap();
            let mut disk = inner.disk.clone();
            disk.settings.validate()?;
            let timestamp = model::now();
            disk.last_prompt_at = Some(timestamp);
            disk.next_prompt_at = disk
                .settings
                .automation
                .then_some(timestamp + model::FIVE_HOURS);
            // Reserve the slot durably before the network call, including failed attempts.
            self.storage.save(&disk)?;
            inner.disk = disk;
            inner.disk.settings.clone()
        };
        self.log(
            "info",
            &i18n::msg("log.promptStart", json!({"model": settings.model})),
        );
        self.publish(app);
        let result = codex::prompt(&settings, &self.storage.directory.join("workspace")).await;
        match self.fetch_usage().await {
            Ok(usage) => {
                let mut inner = self.inner.lock().unwrap();
                inner.usage = Some(usage);
                inner.error = None;
            }
            Err(error) => {
                self.inner.lock().unwrap().error = Some(error.clone());
                self.log("error", &i18n::detail("error.followup", error));
            }
        }
        result
    }

    pub async fn reset(&self, app: &AppHandle, confirmed: bool) -> Result<(), String> {
        if !confirmed {
            return Err("@error.confirm".into());
        }
        let _guard = self.begin(app, "@operation.reset")?;
        let expected_account = self
            .inner
            .lock()
            .unwrap()
            .usage
            .as_ref()
            .and_then(|u| u.account_id.clone());
        let result = self.reset_inner(app, expected_account).await;
        self.finish(app, &result);
        result.map(|_| ())
    }

    async fn reset_inner(
        &self,
        app: &AppHandle,
        expected_account: Option<String>,
    ) -> Result<String, String> {
        let fresh = self.fetch_usage().await?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.usage = Some(fresh.clone());
            inner.error = None;
        }
        let account = fresh
            .account_id
            .as_ref()
            .filter(|id| !id.is_empty())
            .ok_or("@error.noAccount")?;
        if Some(account) != expected_account.as_ref() {
            return Err("@error.accountChanged".into());
        }
        let (settings, key) = {
            let mut inner = self.inner.lock().unwrap();
            let mut disk = inner.disk.clone();
            if let Some(pending) = &disk.pending_reset {
                if pending.account_id != *account {
                    return Err("@error.pendingAccount".into());
                }
            } else {
                if !fresh.credits.is_some_and(|n| n > 0) {
                    return Err("@error.noCredits".into());
                }
                disk.pending_reset = Some(PendingReset {
                    account_id: account.clone(),
                    key: uuid::Uuid::new_v4().to_string(),
                });
                // Never submit unless the idempotency key is safely persisted.
                self.storage.save(&disk)?;
                inner.disk = disk.clone();
            }
            (disk.settings, disk.pending_reset.unwrap().key)
        };
        self.log("info", "@log.resetStart");
        self.publish(app);
        let result = codex::rpc(
            &settings,
            &self.storage.directory.join("workspace"),
            "account/rateLimitResetCredit/consume",
            Some(json!({"idempotencyKey":key})),
        )
        .await;
        let outcome = match result {
            Ok(value) => value
                .get("outcome")
                .and_then(|o| o.as_str())
                .unwrap_or("")
                .to_owned(),
            Err(error) => {
                // Refresh for visibility but retain the key on all ambiguous errors.
                if let Ok(usage) = self.fetch_usage().await {
                    self.inner.lock().unwrap().usage = Some(usage);
                }
                return Err(i18n::detail("error.uncertain", error));
            }
        };
        let message = match outcome.as_str() {
            "reset" => "@log.resetDone",
            "alreadyRedeemed" => "@log.alreadyRedeemed",
            "nothingToReset" => "@log.nothingToReset",
            "noCredit" => "@log.noCredit",
            _ => return Err("@error.unknownReset".into()),
        };
        {
            let mut inner = self.inner.lock().unwrap();
            let mut disk = inner.disk.clone();
            disk.pending_reset = None;
            self.storage.save(&disk)?;
            inner.disk = disk;
        }
        match self.fetch_usage().await {
            Ok(usage) => self.inner.lock().unwrap().usage = Some(usage),
            Err(error) => {
                self.inner.lock().unwrap().error = Some(error.clone());
                self.log("error", &i18n::detail("error.followup", error));
            }
        }
        Ok(message.into())
    }
}

#[tauri::command]
pub fn get_state(state: tauri::State<'_, Arc<AppState>>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub async fn refresh_usage(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.refresh(&app).await
}

#[tauri::command]
pub async fn execute_prompt(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.run_prompt(&app).await
}

#[tauri::command]
pub async fn reset_quota(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    confirmed: bool,
) -> Result<(), String> {
    state.reset(&app, confirmed).await
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<(), String> {
    settings.validate()?;
    let _guard = state
        .gate
        .clone()
        .try_lock_owned()
        .map_err(|_| "@error.busy")?;
    {
        let mut inner = state.inner.lock().unwrap();
        let mut disk = inner.disk.clone();
        let was_enabled = disk.settings.automation;
        disk.settings = settings;
        if !disk.settings.automation {
            disk.next_prompt_at = None;
        } else if !was_enabled || disk.next_prompt_at.is_none() {
            disk.next_prompt_at = Some(
                disk.last_prompt_at
                    .map(|last| last + model::FIVE_HOURS)
                    .unwrap_or(model::now()),
            );
        }
        state.storage.save(&disk)?;
        inner.disk = disk;
    }
    state.log("info", "@log.settings");
    state.publish(&app);
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn set_autostart(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("@error.autostartDev".into());
    }
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|e| e.to_string())?;
    state.inner.lock().unwrap().autostart = manager.is_enabled().map_err(|e| e.to_string())?;
    state.log(
        "info",
        if enabled {
            "@log.autostartOn"
        } else {
            "@log.autostartOff"
        },
    );
    state.publish(&app);
    Ok(())
}

#[tauri::command]
pub fn open_logs(app: AppHandle, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(state.storage.directory.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_panel(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("panel") {
        window.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub async fn scheduler(app: AppHandle, state: Arc<AppState>) {
    let mut next_poll = 0;
    loop {
        let timestamp = model::now();
        if timestamp >= next_poll {
            let _ = state.refresh(&app).await;
            next_poll = model::now() + model::POLL_INTERVAL;
        }
        let run = model::due(&state.inner.lock().unwrap().disk, model::now());
        if run {
            let _ = state.run_prompt(&app).await;
        }
        let timestamp = model::now();
        let next_prompt = {
            let inner = state.inner.lock().unwrap();
            if inner.disk.settings.automation {
                inner.disk.next_prompt_at.unwrap_or(timestamp)
            } else {
                u64::MAX
            }
        };
        let delay = next_poll
            .min(next_prompt)
            .saturating_sub(timestamp)
            .clamp(15, model::POLL_INTERVAL);
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {},
            _ = state.wake.notified() => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_localizes_errors_operations_and_persisted_activity() {
        let directory = std::env::temp_dir()
            .join("opencode")
            .join(format!("usage-snapshot-{}", uuid::Uuid::new_v4()));
        let storage = Storage::new(directory.clone()).unwrap();
        let mut disk = DiskState::default();
        disk.settings.language = "it".into();
        let state = AppState::new(storage, disk, false);
        state.log("info", "@log.refreshed");
        {
            let mut inner = state.inner.lock().unwrap();
            inner.error = Some("@error.model".into());
            inner.busy = Some("@operation.refresh".into());
        }
        let snapshot = state.snapshot();
        assert_eq!(snapshot.locale, "it");
        assert_eq!(
            snapshot.error.as_deref(),
            Some("Nome del modello non valido.")
        );
        assert_eq!(snapshot.busy.as_deref(), Some("Lettura della quota…"));
        assert_eq!(snapshot.logs[0].message, "Quota aggiornata.");
        assert_eq!(state.storage.recent_logs()[0].message, "Usage updated.");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
