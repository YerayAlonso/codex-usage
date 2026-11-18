use crate::{
    app::AppState,
    i18n,
    model::{self, Snapshot},
};
use std::sync::{Arc, Mutex};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};

// Supersampled RGBA; no image codec or renderer needed in the resident process.
pub fn icon(snapshot: &Snapshot, size: u32) -> Image<'static> {
    let timestamp = model::now();
    let stale = snapshot.error.is_some()
        || snapshot
            .usage
            .as_ref()
            .is_none_or(|u| model::is_stale(u, timestamp));
    let remaining = |minutes| {
        snapshot
            .usage
            .as_ref()
            .and_then(|u| u.windows.iter().find(|w| w.minutes == minutes))
            .map(|w| 1.0 - w.used / 100.0)
    };
    let rings = [(0.405, remaining(300)), (0.245, remaining(10080))];
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let mut channels = [0.0_f64; 4];
            for sy in 0..3 {
                for sx in 0..3 {
                    let dx = (x as f64 + (sx as f64 + 0.5) / 3.0) / size as f64 - 0.5;
                    let dy = (y as f64 + (sy as f64 + 0.5) / 3.0) / size as f64 - 0.5;
                    let distance = dx.hypot(dy);
                    let angle = (dx.atan2(-dy) + std::f64::consts::TAU) % std::f64::consts::TAU;
                    for (radius, value) in rings {
                        if (distance - radius).abs() <= 0.047 {
                            let color = match value {
                                _ if stale => [155.0, 163.0, 175.0, 180.0],
                                None => [155.0, 163.0, 175.0, 110.0],
                                Some(left) if angle / std::f64::consts::TAU >= left => {
                                    [135.0, 148.0, 165.0, 85.0]
                                }
                                Some(left) if left <= 0.1 => [251.0, 113.0, 133.0, 255.0],
                                Some(left) if left <= 0.25 => [251.0, 191.0, 100.0, 255.0],
                                Some(_) => [101.0, 220.0, 180.0, 255.0],
                            };
                            for i in 0..4 {
                                channels[i] += color[i] / 9.0;
                            }
                        }
                    }
                    // Amber center indicates stale/error/unknown instead of fake empty quota.
                    if stale && distance < 0.055 {
                        for (i, channel) in [251.0, 191.0, 100.0, 255.0].iter().enumerate() {
                            channels[i] += channel / 9.0;
                        }
                    }
                }
            }
            let offset = ((y * size + x) * 4) as usize;
            for i in 0..4 {
                rgba[offset + i] = channels[i].clamp(0.0, 255.0) as u8;
            }
        }
    }
    Image::new_owned(rgba, size, size)
}

pub fn tooltip(snapshot: &Snapshot) -> String {
    let locale = snapshot.locale.as_str();
    let mut parts = vec![format!("Codex Usage · {}", i18n::text(locale, "usedQuota"))];
    if let Some(usage) = &snapshot.usage {
        for (minutes, label) in [(300, "5 h"), (10080, "7 d")] {
            if let Some(window) = usage.windows.iter().find(|w| w.minutes == minutes) {
                let left = window.resets_at.saturating_sub(model::now()) / 60;
                let reset = if window.resets_at == 0 {
                    i18n::text(locale, "resetUnknown").into()
                } else if window.resets_at <= model::now() {
                    i18n::text(locale, "resetDue").into()
                } else {
                    format!("{}h {}m", left / 60, left % 60)
                };
                parts.push(format!("{label}: {:.0}% · {reset}", window.used));
            }
        }
        if model::is_stale(usage, model::now()) {
            parts.push(i18n::text(locale, "stale").into());
        }
    } else {
        parts.push(i18n::text(locale, "tray.unknown").into());
    }
    if snapshot.error.is_some() {
        parts.push(i18n::text(locale, "tray.error").into());
    }
    if let Some(operation) = &snapshot.busy {
        parts.push(operation.clone());
    }
    parts.join("\n")
}

pub fn update(app: &AppHandle, snapshot: &Snapshot) {
    static MENU_LOCALE: Mutex<String> = Mutex::new(String::new());
    if let Some(tray) = app.tray_by_id("codex") {
        let _ = tray.set_icon(Some(icon(snapshot, 64)));
        let _ = tray.set_tooltip(Some(tooltip(snapshot)));
        let changed = {
            let mut last = MENU_LOCALE.lock().unwrap();
            let changed = *last != snapshot.locale;
            if changed {
                last.clone_from(&snapshot.locale);
            }
            changed
        };
        // Menu APIs dispatch to the UI thread; never hold a mutex while waiting for it.
        if changed {
            if let Ok(menu) = menu(app, &snapshot.locale) {
                let _ = tray.set_menu(Some(menu));
            }
        }
    }
}

pub fn open_panel(app: &AppHandle, anchor: Option<PhysicalPosition<f64>>) {
    if let Some(window) = app.get_webview_window("panel") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let builder = WebviewWindowBuilder::new(app, "panel", WebviewUrl::App("index.html".into()))
        .title("Codex Usage")
        .inner_size(460.0, 600.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .on_navigation(|url| {
            (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                || (matches!(url.scheme(), "http" | "https")
                    && url.host_str() == Some("tauri.localhost"))
                || (cfg!(debug_assertions)
                    && url.scheme() == "http"
                    && url.port() == Some(1420)
                    && url
                        .host_str()
                        .is_some_and(|h| matches!(h, "localhost" | "127.0.0.1")))
        });
    match builder.build() {
        Ok(window) => {
            let monitor = anchor
                .and_then(|point| {
                    window
                        .available_monitors()
                        .ok()?
                        .into_iter()
                        .find(|monitor| {
                            let origin = monitor.position();
                            let size = monitor.size();
                            point.x >= origin.x as f64
                                && point.y >= origin.y as f64
                                && point.x < origin.x as f64 + size.width as f64
                                && point.y < origin.y as f64 + size.height as f64
                        })
                })
                .or_else(|| window.current_monitor().ok().flatten());
            if let Some(monitor) = monitor {
                let area = monitor.work_area();
                let scale = monitor.scale_factor();
                // Move onto the target monitor before measuring decorations and sizing:
                // Windows may change DPI when crossing monitors.
                let _ = window.set_position(area.position);
                let inner = window.inner_size().unwrap_or_default();
                let outer = window.outer_size().unwrap_or(inner);
                let border_width = outer.width.saturating_sub(inner.width);
                let border_height = outer.height.saturating_sub(inner.height);
                let inner_width = (460.0 * scale)
                    .min(area.size.width.saturating_sub(border_width + 24) as f64)
                    .max(1.0) as u32;
                let inner_height = (600.0 * scale)
                    .min(area.size.height.saturating_sub(border_height + 24) as f64)
                    .max(1.0) as u32;
                let _ = window.set_size(tauri::PhysicalSize::new(inner_width, inner_height));
                let width = (inner_width + border_width) as i32;
                let height = (inner_height + border_height) as i32;
                let right = area.position.x + area.size.width as i32;
                let bottom = area.position.y + area.size.height as i32;
                let x = anchor
                    .map(|p| p.x as i32 - width / 2)
                    .unwrap_or(right - width - 12)
                    .clamp(area.position.x, (right - width).max(area.position.x));
                let y = anchor
                    .map(|p| p.y as i32 - height - 8)
                    .unwrap_or(bottom - height - 12)
                    .clamp(area.position.y, (bottom - height).max(area.position.y));
                let _ = window.set_position(PhysicalPosition::new(x, y));
            }
            let _ = window.show();
            let _ = window.set_focus();
        }
        Err(error) => {
            let state = app.state::<Arc<AppState>>();
            state.log("error", &i18n::detail("error.panel", error));
        }
    }
}

fn menu(app: &AppHandle, locale: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(
        app,
        "open",
        i18n::text(locale, "tray.open"),
        true,
        None::<&str>,
    )?;
    let refresh = MenuItem::with_id(
        app,
        "refresh",
        i18n::text(locale, "tray.refresh"),
        true,
        None::<&str>,
    )?;
    let logs = MenuItem::with_id(
        app,
        "logs",
        i18n::text(locale, "tray.logs"),
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        i18n::text(locale, "tray.quit"),
        true,
        None::<&str>,
    )?;
    Menu::with_items(app, &[&open, &refresh, &logs, &separator, &quit])
}

pub fn setup(app: &AppHandle, snapshot: &Snapshot) -> tauri::Result<()> {
    let menu = menu(app, &snapshot.locale)?;
    TrayIconBuilder::with_id("codex")
        .icon(icon(snapshot, 64))
        .tooltip(tooltip(snapshot))
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "linux"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_panel(app, None),
            "refresh" => {
                let app = app.clone();
                let state = app.state::<Arc<AppState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = state.refresh(&app).await;
                });
            }
            "logs" => {
                use tauri_plugin_opener::OpenerExt;
                let state = app.state::<Arc<AppState>>();
                if let Err(error) = app
                    .opener()
                    .open_path(state.storage.directory.to_string_lossy(), None::<&str>)
                {
                    state.log("error", &error.to_string());
                }
            }
            "quit" => {
                let state = app.state::<Arc<AppState>>();
                state.log("info", "@log.stopped");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                open_panel(tray.app_handle(), Some(position));
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{QuotaWindow, Settings, Snapshot, Usage};

    #[test]
    fn unknown_tray_is_not_a_zero_percent_claim() {
        let snapshot = Snapshot {
            locale: "ca".into(),
            settings: Settings::default(),
            usage: None,
            error: None,
            busy: None,
            last_prompt_at: None,
            next_prompt_at: None,
            pending_reset: false,
            autostart: false,
            logs: vec![],
        };
        assert!(tooltip(&snapshot).contains("desconegut"));
        assert_eq!(icon(&snapshot, 32).rgba().len(), 32 * 32 * 4);
    }

    #[test]
    fn tooltip_reports_used_quota_not_remaining() {
        let timestamp = model::now();
        let snapshot = Snapshot {
            locale: "ca".into(),
            settings: Settings::default(),
            usage: Some(Usage {
                windows: vec![
                    QuotaWindow {
                        used: 29.0,
                        minutes: 300,
                        resets_at: timestamp + 3600,
                    },
                    QuotaWindow {
                        used: 34.0,
                        minutes: 10080,
                        resets_at: timestamp + 86400,
                    },
                ],
                credits: Some(2),
                expires_at: None,
                account_id: None,
                updated_at: timestamp,
            }),
            error: None,
            busy: None,
            last_prompt_at: None,
            next_prompt_at: None,
            pending_reset: false,
            autostart: false,
            logs: vec![],
        };
        let text = tooltip(&snapshot);
        assert!(text.starts_with("Codex Usage · Quota utilitzada"));
        assert!(text.contains("5 h: 29%"));
        assert!(text.contains("7 d: 34%"));
        assert!(!text.contains("71%"));
        assert!(!text.contains("66%"));
    }
}
