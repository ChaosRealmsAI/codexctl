# Changelog

## v0.1.0 - 2026-05-05

Initial open-source release candidate for `codexctl`.

### Added

- Codex app-server health checks through `codexctl doctor`.
- Raw app-server method access through `codexctl raw`.
- Account, quota, model, collaboration mode, and feature discovery commands.
- Thread Goal helpers: `goal set`, `goal get`, and `goal clear`.
- One-shot Plan-mode turns with optional Goal setup and structured question handling.
- CLI-only long sessions through a local daemon: `session start`, `answer`, `send`, `execute`, `resume`, `interrupt`, `list`, and `stop`.
- Local Codex rollout JSONL viewer through `codexctl view`.
- Multi-install and multi-account selection with `--codex-bin` and `--codex-home`.
- High-permission shortcut through `--dangerously-full-access`.
- Lightweight install paths through `install.sh` and `cargo install --git`.

### Notes

- `codexctl` does not vendor Codex auth, config, session state, or provider secrets.
- Local `.jsonl` rollout/sample files are ignored by default.
- Released under the MIT License.
