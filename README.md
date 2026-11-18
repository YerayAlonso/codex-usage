# Codex Usage

**A lightweight Codex quota monitor that lives in your system tray.**

[Català](docs/README.ca.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [MIT license](LICENSE)

Experimental, unofficial desktop app. Not affiliated with OpenAI or the Codex team.
It uses the account signed in to **Codex CLI**, which may differ from an account
used in another editor or app.

## Features

- **Tray rings:** outer ring = remaining five-hour quota; inner ring = remaining weekly quota.
- **Tooltip and panel:** used percentages, usage bars, and reset countdowns.
- Usage data refreshes every five minutes without keeping a window open.
- Three compact views: Usage, Settings, and paginated Activity.
- Optional minimal prompt every five hours, with manual execution available.
- Earned reset credits, expiration dates, and explicitly confirmed quota resets.
- Persistent logs, bounded output, subprocess timeouts, and single-instance protection.
- Optional launch at sign-in.
- English, Catalan, Spanish, French, German, and Italian; system-language detection
  with English as the fallback.

The resident part is **Rust**. The TypeScript/CSS panel has no UI framework and is
created only when opened, then destroyed when closed. Tauri uses the system webview:
there is no bundled Chromium, Electron, or resident Node.js process.
Codex CLI runs only while an operation is in progress.

**Platform status:** Windows has been tested locally. macOS and Linux are intended
targets with CI checks, but their desktop behavior and packages still need native
validation. Linux tray behavior depends on the desktop environment; the native
menu provides an **Open Codex Usage** fallback.

## Requirements

- Codex CLI installed and signed in with ChatGPT:

  ```sh
  codex --version
  codex login status
  ```

  If necessary, run `codex login` yourself and complete the sign-in flow.
  API-key authentication does not provide ChatGPT subscription quota.
- Windows: Microsoft Edge WebView2.
- macOS/Linux: the system webview and tray dependencies required by
  [Tauri](https://v2.tauri.app/start/prerequisites/).

The protocol was checked against **Codex CLI 0.169.0**. It is experimental and may
change. Only usage windows returned by Codex are displayed; not every account has
both limits or earned reset credits.

## Build and run

Install **Node.js 22.12+**, a current stable **Rust** toolchain, and
[Tauri's native build prerequisites](https://v2.tauri.app/start/prerequisites/).
The repository includes npm and Cargo lockfiles.

```sh
git clone https://github.com/YerayAlonso/codex-usage.git
cd codex-usage
npm ci
npm run dev
```

The app starts in the tray, not in a normal application window. On Windows the icon
may be inside the hidden-icons menu. Click it to open the panel; right-click for
the native menu. **Quit** stops the whole app. Closing the panel only releases its
webview and leaves the Rust scheduler running.

To show the panel immediately in development:

```sh
npm run dev -- -- --show
```

The frontend rebuilds on changes; close and reopen the panel to load them.
Launch at sign-in is intentionally unavailable in development builds.

### Create an installer

Run on the target operating system:

```sh
# Windows
npm run build -- --bundles nsis

# macOS
npm run build -- --bundles app,dmg

# Linux
npm run build -- --bundles deb,appimage
```

Packages are written to `src-tauri/target/release/bundle/`.
The Windows executable is `src-tauri/target/release/codex-usage.exe`.
End users do not need Node.js or Rust. Packages are currently unsigned; Windows
SmartScreen or macOS Gatekeeper may warn about them.

Prebuilt binaries, when published, belong on the repository's
[Releases page](https://github.com/YerayAlonso/codex-usage/releases), not in Git.
The manual **Build packages** workflow also produces downloadable Actions artifacts;
it does not create or publish a release.

## Settings and safety

Choose a language, toggle the five-hour cycle, and optionally enable launch at
sign-in. Advanced settings contain the model and an optional absolute path to Codex.
Leave the executable path empty for auto-detection. On Windows, use `codex.exe`,
not a `.cmd` or `.bat` wrapper.

The default model, `gpt-6-luna`, is inherited from the original prototype and may
not be available on your account. Choose a supported model before enabling the
optional cycle. Both the cycle and launch at sign-in are **off by default**.

These are three different operations:

| Operation | Effect |
| --- | --- |
| Refresh usage | Reads quota through `codex app-server`; does not spend a reset credit. |
| Run a prompt | Sends `Responde solo: OK`; **may use generation quota**, but does not reset limits or spend a reset credit. |
| Reset quota | Consumes a real earned reset credit for eligible limits, after explicit confirmation. |

The app never spends reset credits automatically. Enabling the cycle may send a
prompt immediately if no recent attempt is recorded. Later attempts are scheduled
five hours from the start of the last attempt, including failed attempts. After
sleep or restart, at most one missed attempt is run. The app must be running in a
signed-in desktop session; no Windows scheduled task or background service is needed.

Before a reset, the app refreshes availability and checks the account. An
idempotency key is persisted **before** submitting the request and retained on
uncertain failures, including across restarts. A confirmed retry reuses that key.
There are no automatic retries of uncertain resets. **Do not delete `state.json`
to retry a reset**, because that would remove this protection.

## Local data and privacy

The app does not read or copy authentication files; Codex CLI owns authentication.
There is no app telemetry. Codex CLI still contacts its own services to read quota
and execute requests. External diagnostics and imported historical logs may retain
their original language.

| Platform | Data directory |
| --- | --- |
| Windows | `%APPDATA%\dev.codex.usage` |
| macOS | `~/Library/Application Support/dev.codex.usage` |
| Linux | `$XDG_DATA_HOME/dev.codex.usage`, usually `~/.local/share/dev.codex.usage` |

- `state.json`: settings, schedule, and any pending reset key.
- `codex-usage.log`, `.1.log`, `.2.log`: rotating JSONL activity logs.
- `workspace/`: dedicated working directory for prompt execution.

Use **Open log folder** to locate the actual directory. Logs are limited to roughly
1 MiB per file, with two rotated files; only the latest 80 events are held in memory.
Common credential formats are redacted, but diagnostics may still contain personal
data. Review logs before sharing them. Never upload auth files or a full data folder.

An older `dev.ciclo.codex` directory is renamed on first launch when the new
directory does not already exist. Settings, schedules, pending keys, and logs are
preserved; two existing states are never merged or overwritten. Close older
versions before launching a newer one.

## Development

```sh
npm run verify
```

This checks project metadata, TypeScript, frontend assets, translations, Rust
formatting, Clippy, and offline unit tests. The normal tests require no Codex login
and never spend quota or credits. The installed-CLI integration test is ignored by
default and is read-only; see [CONTRIBUTING.md](CONTRIBUTING.md).

The repository includes a mocked browser layout regression test across all six
languages and five viewport sizes. See the contributing guide for how to run it.

CI is configured for Windows, macOS, and Linux. This does not replace hands-on
testing of each system's tray, focus behavior, autostart, and webview lifecycle.

## License

[MIT](LICENSE) © 2026 YerayAlonso. Third-party components retain their respective
licenses. Codex and OpenAI names and services are not part of this project's license.
