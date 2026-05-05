# Changelog

## v0.1.0 - 2026-05-05

Initial open-source release candidate for `codexctl`.

### Added

- Codex app-server health checks through `codexctl doctor`.
- Raw app-server method access through `codexctl raw`.
- Account, quota, model, collaboration mode, and feature discovery commands.
- Official Codex workflow guide through `codexctl guide`.
- Thread Goal helpers: `goal set`, `goal get`, and `goal clear`.
- One-shot Plan-mode turns with optional Goal setup and structured question handling.
- CLI-only long sessions through a local daemon: `session start`, `answer`, `send`, `execute`, `resume`, `interrupt`, `list`, and `stop`.
- Nonblocking run snapshots and JSONL progress polling through `session read` and `session watch`.
- Semantic session exit codes for `needs_input`, `running`, `failed`, and `stopped`.
- Compatibility aliases for older `session start --question-mode` and `--version-dir` scripts.
- Local Codex rollout JSONL viewer through `codexctl view`.
- External viewer templates with `codexctl view --viewer-html`.
- Multi-install and multi-account selection with `--codex-bin` and `--codex-home`.
- High-permission shortcut through `--dangerously-full-access`.
- Lightweight install paths through `install.sh` and `cargo install --git`.
- Cross-platform session daemon support: Unix sockets on Unix/macOS, localhost TCP endpoint files on Windows.
- PowerShell installer for Windows.
- GitHub Actions CI for Linux, macOS, and Windows.

### Notes

- `codexctl` does not vendor Codex auth, config, session state, or provider secrets.
- Local `.jsonl` rollout/sample files are ignored by default.
- Released under the MIT License.
