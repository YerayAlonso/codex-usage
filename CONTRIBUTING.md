# Contributing

Thanks for helping improve Codex Usage. Small, focused changes and reproducible
bug reports are welcome. Contributions are licensed under the project's MIT license.

## Before starting

- Search existing issues and pull requests.
- For substantial behavior changes, open an issue first.
- Keep this a lightweight tray app: no resident browser, Node process, animations,
  or unnecessary polling.
- Preserve the separation between reading usage, sending a generation prompt, and
  consuming a reset credit.

## Local setup

Use Node.js 22.12+, npm, stable Rust with `rustfmt` and `clippy`, and the
[native prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```sh
npm ci
npm run dev
```

The app starts in the tray. Reopen its panel after a frontend rebuild.
Do not run two differently named/older builds with automation enabled at once.

## Checks

```sh
npm run verify
```

Individual commands:

```sh
npm run check:project
npm run build:frontend
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

Tests are offline by default. Do not add tests that send real prompts, consume
credits, modify real account settings, or require a personal Codex login in CI.

### Browser layout regression

Start `npm run dev:frontend`, open `http://127.0.0.1:1420`, and run in the browser
console:

```js
await (await import("/layout.browser.mjs")).checkLayout()
```

The test uses isolated, mocked IPC frames and should report **30 passed, no
failures**. It checks every card's inner padding boundary, the prompt note,
scrollbars, and expanded settings at five viewport sizes for six languages.
The helper is never included in production assets.

Also check empty data, errors/stale data, exhausted quota, unknown credits, and a
pending reset. Manual desktop checks should cover tray menus, Escape/native close,
multiple monitors and DPI, sleep/resume, autostart, and closing the last webview.

### Optional read-only Codex integration

Only run this yourself with an installed, signed-in CLI:

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml live_read_only_usage -- --ignored --nocapture
```

This reads account limits and prints only the number of returned windows and
whether credit availability is known. Do not print raw protocol responses,
credentials, or account IDs. Never change it into an automatic reset test.

## Project map

| Path | Purpose |
| --- | --- |
| `src-tauri/src/app.rs` | App state, commands, scheduling, reset safety |
| `src-tauri/src/codex.rs` | Bounded, short-lived Codex subprocesses and protocol |
| `src-tauri/src/storage.rs` | Persistent state, migration, log rotation/redaction |
| `src-tauri/src/tray.rs` | Remaining-quota tray rings, tooltip, native window/menu |
| `src-tauri/src/i18n.rs`, `locales/` | Shared translations |
| `src/` | Framework-free panel; used-quota bars and percentages |
| `test/` | Offline frontend tests and browser layout regression |
| `scripts/` | Frontend build and metadata checks |

Keep both lockfiles. Do not commit `node_modules`, `dist`, native `target`/`gen`
directories, installers, logs, local state, auth data, or signing keys.

## Versions and packages

Update `package.json`, the npm lockfile, `src-tauri/Cargo.toml`, the Cargo lockfile,
and `src-tauri/tauri.conf.json` together. The panel reads its version from
`package.json`; `npm run check:project` detects mismatches.

The **Build packages** workflow is manual. It uploads unsigned installers as
Actions artifacts and does not publish a release or require signing secrets.
Before publishing a GitHub release, test those packages on the target systems,
review the release notes and hashes, and clearly label unsigned or unverified
platform builds. Binaries belong in Releases, never in source commits.

See the [publishing checklist](docs/PUBLISHING.md) for the first push, privacy
review, and manual binary-release process.

## Reporting problems

Use the issue templates for non-sensitive bugs and feature requests. Include the
OS, app and Codex CLI versions, reproduction steps, and redacted diagnostics.
For security issues, follow [SECURITY.md](SECURITY.md) instead of posting details
publicly.
