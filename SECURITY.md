# Security policy

Codex Usage is experimental software. Security fixes target the latest source
version; there is no promise of backports to older builds.

## Report a vulnerability privately

Use GitHub's **Report a vulnerability** feature when private vulnerability
reporting is enabled:

https://github.com/YerayAlonso/codex-usage/security/advisories/new

If that feature is unavailable, open an issue containing only a request for a
private contact channel. Do not post exploit details, credentials, account data,
unredacted logs, or auth files in a public issue or pull request.

## Sensitive information

Do not share:

- Codex auth files, access/refresh tokens, API keys, cookies, or signing keys.
- A full app-data directory or `state.json`; it may include an account identifier
  and a pending reset's idempotency key.
- Raw app-server protocol traffic.

Logs redact several common credential formats and cap diagnostics, but cannot
guarantee removal of every form of sensitive or personal information.
Inspect them before sharing.

## Trust boundaries and limitations

- Codex CLI owns authentication and communicates with its provider. The app does
  not copy or parse its authentication files.
- The selected Codex executable is trusted local code. Only configure an
  executable you trust; a malicious replacement can run with your user rights.
- The panel loads local assets, restricts navigation, and has no generic frontend
  filesystem or shell permissions. Rust exposes specific app operations instead.
- Generation prompts may use quota. Reset requests consume real earned credits and
  require explicit user confirmation; neither a prompt nor an update forces a
  provider-side limit reset.
- Reset availability/account checks, serialization, and durable idempotency keys
  reduce duplicate consumption. They still depend on the external protocol and
  provider's idempotency behavior. Do not delete state to retry an uncertain reset.
- The app uses subprocess timeouts, output limits, a dedicated prompt directory,
  read-only sandboxing, and disabled shell/MCP/plugin/hook features. These are
  defense-in-depth measures, not a guarantee against a compromised CLI.
- Packages are currently unsigned. Verify their source and integrity before
  running them. This repository does not provide an automatic updater.

GitHub workflows run offline tests without a Codex account, use read-only
repository permissions, and must not receive account credentials or reset credits.
Private signing material, if ever introduced, must be handled separately from
pull-request jobs.
