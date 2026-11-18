use crate::{i18n, model::Settings, storage::redact};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    time::timeout,
};

fn executable(settings: &Settings) -> PathBuf {
    if !settings.codex_path.is_empty() {
        return PathBuf::from(&settings.codex_path);
    }
    // Desktop launches may not inherit the PATH used in a terminal.
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let path = PathBuf::from(local).join("Programs/OpenAI/Codex/bin/codex.exe");
            if path.is_file() {
                return path;
            }
        }
    }
    #[cfg(target_os = "macos")]
    for prefix in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let path = PathBuf::from(prefix).join("codex");
        if path.is_file() {
            return path;
        }
    }
    PathBuf::from("codex")
}

fn command(settings: &Settings, workspace: &Path) -> Command {
    let mut cmd = Command::new(executable(settings));
    cmd.current_dir(workspace)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    cmd
}

async fn stop(child: &mut Child) {
    // Always reap the short-lived app-server / exec, including error and timeout paths.
    let _ = child.kill().await;
    let _ = timeout(Duration::from_secs(3), child.wait()).await;
}

pub async fn rpc(
    settings: &Settings,
    workspace: &Path,
    method: &str,
    params: Option<Value>,
) -> Result<Value, String> {
    let mut child = command(settings, workspace)
        .args([
            "app-server",
            "--disable",
            "plugins",
            "--disable",
            "apps",
            "--disable",
            "hooks",
            "-c",
            "mcp_servers={}",
            "-c",
            "analytics.enabled=false",
        ])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| i18n::detail("error.start", e))?;
    let stderr = child.stderr.take().ok_or("@error.incomplete")?;
    let diagnostics = tokio::spawn(bounded_read(stderr, 32 * 1024));
    let result = timeout(Duration::from_secs(25), async {
        let mut stdin = child.stdin.take().ok_or("@error.incomplete")?;
        let stdout = child.stdout.take().ok_or("@error.incomplete")?;
        let mut lines = BufReader::new(stdout.take(1024 * 1024)).lines();
        let init = json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"codex-usage","version":env!("CARGO_PKG_VERSION")}}});
        stdin.write_all(format!("{init}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
            let id = message.get("id").and_then(Value::as_u64);
            if id != Some(1) && id != Some(2) { continue; }
            if let Some(error) = message.get("error") {
                let detail = error.get("message").and_then(Value::as_str).unwrap_or("Protocol error");
                return Err(i18n::msg("error.protocol", json!({"method":method,"detail":redact(detail)})));
            }
            if id == Some(1) {
                let request = json!({"id":2,"method":method,"params":params});
                stdin.write_all(format!("{{\"method\":\"initialized\"}}\n{request}\n").as_bytes())
                    .await.map_err(|e| e.to_string())?;
            } else {
                return message.get("result").cloned().ok_or("@error.incomplete".into());
            }
        }
        Err("@error.closed".into())
    }).await.unwrap_or_else(|_| Err("@error.timeout".into()));
    stop(&mut child).await;
    match (result, timeout(Duration::from_secs(3), diagnostics).await) {
        (Err(error), Ok(Ok(Ok(bytes)))) if !bytes.is_empty() => Err(i18n::detail(
            "error.communication",
            format!(
                "{}\n{}",
                i18n::render("en", &error),
                redact(&String::from_utf8_lossy(&bytes))
            ),
        )),
        (result, _) => result,
    }
}

async fn bounded_read(reader: impl AsyncRead + Unpin, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("@error.outputLimit".into());
    }
    Ok(bytes)
}

pub async fn prompt(settings: &Settings, workspace: &Path) -> Result<String, String> {
    let mut cmd = command(settings, workspace);
    cmd.args([
        "exec",
        "--ignore-user-config",
        "--ignore-rules",
        "--ephemeral",
        "--disable",
        "plugins",
        "--disable",
        "apps",
        "--disable",
        "hooks",
        "--disable",
        "multi_agent",
        "--disable",
        "shell_tool",
        "--disable",
        "unified_exec",
        "--disable",
        "view_image",
        "-c",
        "mcp_servers={}",
        "-c",
        "web_search=disabled",
        "--sandbox",
        "read-only",
        "--model",
        &settings.model,
        "-c",
        "model_reasoning_effort=none",
        "--skip-git-repo-check",
        "--color",
        "never",
        "--json",
        "Responde solo: OK",
    ])
    .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| i18n::detail("error.start", e))?;
    let stdout = child.stdout.take().ok_or("@error.incomplete")?;
    let stderr = child.stderr.take().ok_or("@error.incomplete")?;
    let result = timeout(Duration::from_secs(120), async {
        let (out, err, status) = tokio::try_join!(
            bounded_read(stdout, 256 * 1024),
            bounded_read(stderr, 32 * 1024),
            async { child.wait().await.map_err(|e| e.to_string()) }
        )?;
        let output = String::from_utf8_lossy(&out);
        let diagnostics = String::from_utf8_lossy(&err);
        if !status.success() {
            return Err(i18n::msg("error.exitCode", json!({
                "code": status.code().map(|code| code.to_string()).unwrap_or_else(|| "signal".into()),
                "detail": format!("{}\n{}", redact(&diagnostics), extract_error(&output))
            })));
        }
        let message = output
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|event| {
                event.pointer("/item/type").and_then(Value::as_str) == Some("agent_message")
            })
            .filter_map(|event| {
                event
                    .pointer("/item/text")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .next_back()
            .ok_or("@error.noResponse")?;
        Ok(i18n::detail("log.promptDone", redact(&message)))
    })
    .await
    .unwrap_or_else(|_| Err("@error.promptTimeout".into()));
    stop(&mut child).await;
    result
}

fn extract_error(output: &str) -> String {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| {
            matches!(
                event.get("type").and_then(Value::as_str),
                Some("error" | "turn.failed")
            )
        })
        .map(|event| {
            redact(
                event
                    .pointer("/error/message")
                    .or_else(|| event.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("Codex error."),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Read-only integration test against the installed, signed-in Codex CLI"]
    async fn live_read_only_usage() {
        let result = rpc(
            &Settings::default(),
            &std::env::current_dir().unwrap(),
            "account/rateLimits/read",
            None,
        )
        .await
        .expect("Codex usage read failed");
        let usage = crate::model::parse_usage(&result, crate::model::now()).unwrap();
        assert!(!usage.windows.is_empty());
        // Never print account IDs, raw protocol responses, or credentials.
        println!(
            "Read-only usage check: {} windows; credits known: {}",
            usage.windows.len(),
            usage.credits.is_some()
        );
    }

    #[tokio::test]
    async fn rejects_oversized_output() {
        assert!(bounded_read(&b"12345"[..], 4).await.is_err());
        assert_eq!(bounded_read(&b"1234"[..], 4).await.unwrap(), b"1234");
    }

    #[test]
    fn json_errors_are_extracted_without_raw_events() {
        let output = "{\"type\":\"thread.started\",\"thread_id\":\"private\"}\n{\"type\":\"turn.failed\",\"error\":{\"message\":\"No quota\"}}";
        assert_eq!(extract_error(output), "No quota");
    }
}
