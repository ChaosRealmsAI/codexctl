# codexctl

[![CI](https://github.com/ChaosRealmsAI/codexctl/actions/workflows/ci.yml/badge.svg)](https://github.com/ChaosRealmsAI/codexctl/actions/workflows/ci.yml)

Rust CLI wrapper around `codex app-server --listen stdio://`.

The binary is `codexctl`. It speaks Codex app-server JSONL, adds stable
high-level commands for Goal, Plan, and structured follow-up questions, and
keeps raw access to all app-server methods through `raw`.

## Requirements

- Rust toolchain for building from source.
- Codex CLI with `app-server --listen stdio://` support.
- macOS, Linux, or Windows.

## Install

One-command install from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/ChaosRealmsAI/codexctl/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/ChaosRealmsAI/codexctl/main/install.ps1 | iex
```

Pinned release install:

```bash
curl -fsSL https://raw.githubusercontent.com/ChaosRealmsAI/codexctl/main/install.sh | CODEXCTL_TAG=v0.1.0 sh
```

Pinned Windows PowerShell install:

```powershell
$env:CODEXCTL_TAG="v0.1.0"; irm https://raw.githubusercontent.com/ChaosRealmsAI/codexctl/main/install.ps1 | iex
```

Direct Cargo install:

```bash
cargo install --git https://github.com/ChaosRealmsAI/codexctl --tag v0.1.0 --force
```

From a local checkout:

```bash
cargo install --path . --force
codexctl --help
codexctl doctor
```

By default, the wrapper runs `codex` and clears `CODEX_HOME` for the spawned
Codex process. That means it uses the installed Codex CLI and Codex's normal
default account/config/session directory. Use explicit flags only when you need
a different install or account:

```bash
codexctl --codex-bin /path/to/codex doctor
codexctl --codex-home ~/.codex-work doctor
codexctl --codex-bin /path/to/codex --codex-home ~/.codex-work doctor
```

If your shell alias is only selecting another Codex home, translate it to
`--codex-home`:

```bash
# alias example: CODEX_HOME=$HOME/.codex-work codex
codexctl --codex-home ~/.codex-work doctor

# alias example: CODEX_HOME=$HOME/.codex-e-codex codex
codexctl --codex-home ~/.codex-e-codex doctor
```

Use `--codex-bin` only for another installed binary or wrapper. Use both flags
only when the executable and account/session home both differ.

Local rollout logs and sample `.jsonl` files are intentionally ignored by git.
They may include large session transcripts and should stay local unless a
specific fixture is reviewed and explicitly tracked.

## Build

```bash
cargo build
```

Cross-platform CI runs on Linux, macOS, and Windows.

## Fixed Logs

Default logs are written to:

```text
~/.codexctl/logs/
```

Each run writes:

```text
run-<timestamp>.jsonl
latest.jsonl
```

Log modes:

```bash
codexctl --log-mode summary doctor
codexctl --log-mode full doctor
codexctl --log-mode off doctor
```

`summary` keeps method names, ids, question counts, and response keys. `full`
keeps full JSON messages.

## Core Commands

```bash
codexctl doctor
codexctl guide
codexctl methods
codexctl modes
codexctl features
codexctl account
codexctl quota
codexctl models
codexctl status
codexctl read --thread-id <thread-id> --compact
codexctl view ~/.codex/sessions/2026/05/05/rollout-<thread-id>.jsonl
codexctl --codex-home ~/.codex-work read --thread-id <thread-id> --compact
```

Official workflow guide:

```bash
codexctl guide
codexctl guide --help
```

`guide` maps official Codex best practices, Plan mode, Goal, model/effort,
permissions, and app-server multi-turn APIs to the `codexctl` session workflow.
For app integrations, the recommended path is Plan-first `session`:

```bash
codexctl session start --prompt-file input.md --sandbox workspace-write --approval-policy never
codexctl session answer --run-id <run-id> --pick recommended
codexctl session send --run-id <run-id> --prompt "I confirm this plan."
codexctl session execute --run-id <run-id>
```

Generic method access:

```bash
codexctl raw collaborationMode/list --params '{}'
codexctl raw model/list --params '{}'
codexctl raw thread/read --params '{"threadId":"...","includeTurns":true}'
```

Goal:

```bash
codexctl goal set --objective "Ship a small CLI" --token-budget 5000
codexctl goal set --objective "Ship a small CLI" --token-budget unlimited
codexctl --codex-home ~/.codex-work goal set --objective "Ship a small CLI"
codexctl goal get --thread-id <thread-id>
codexctl goal clear --thread-id <thread-id>
```

Plan mode:

```bash
codexctl plan --prompt "Plan only. Ask one question first."
codexctl plan --prompt-file input.md --question-mode auto-recommended
codexctl plan --prompt-file input.md --token-budget unlimited --timeout unlimited
codexctl --codex-home ~/.codex-work plan --prompt-file input.md --dangerously-full-access
```

Use `plan` for one-shot smoke tests. Use `session` for app integrations that
need multiple turns, structured answers, plan confirmation, execution, or later
inspection.

CLI-only long sessions:

```bash
codexctl session start \
  --prompt-file input.md \
  --token-budget unlimited \
  --timeout unlimited \
  --dangerously-full-access

codexctl session start \
  --prompt-file input.md \
  --dangerously-full-access \
  --detach

codexctl session list --threads
codexctl session read --run-id <run-id>
codexctl session watch --run-id <run-id>
codexctl view --run-id <run-id>

codexctl session answer \
  --run-id <run-id> \
  --pick recommended

codexctl session send \
  --run-id <run-id> \
  --prompt "I confirm this plan. Continue."

codexctl session execute \
  --run-id <run-id> \
  --detach

codexctl session resume --thread-id <thread-id>
codexctl session interrupt --run-id <run-id>
codexctl session stop --run-id <run-id>
```

`session` commands auto-start a local daemon. Unix/macOS use a Unix socket;
Windows uses localhost TCP and stores the endpoint in the `--session-socket`
path. The caller still only invokes CLI commands. The daemon keeps the app-server
process, thread, pending structured question, and run state alive between CLI
calls.

Use `--detach` on `session start`, `session answer`, or `session send` when the
caller wants to return immediately. Use `codexctl view --run-id <run-id>` to
open the run's local Codex rollout JSONL in the bundled viewer. Use
`--pick recommended`, `--pick first`, or `--pick 1,2,1` to answer pending
structured questions without copying exact option labels.

Use `codexctl session read --run-id <run-id>` for a nonblocking compact snapshot.
Add `--full` for the full run state. Use `codexctl session watch --run-id <run-id>`
to stream JSONL snapshots until the run reaches `needs_input`, `completed`, or
`failed`.

Session commands use semantic exit codes for app wrappers:

```text
0   completed or read/list success
1   failed
20  needs_input
21  still running
22  stopped
```

Local JSONL viewer:

```bash
codexctl view ~/.codex/sessions/2026/05/05/rollout-<thread-id>.jsonl
codexctl view --run-id <run-id>
codexctl view sample-session.jsonl --no-open --out target/view.html
codexctl view rollout.jsonl --viewer-html /path/to/viewer.html
```

The viewer only loads local JSONL. By default it uses the viewer bundled into the
binary; pass `--viewer-html` to use an external editable viewer template.
`--run-id` is a convenience lookup that asks the daemon for the run's
`thread_path`, then loads that file. `run-id` is only valid while the local
daemon still has the run in memory; after a daemon restart, open the durable
`thread_path` directly:

```bash
codexctl view <thread-path>
```

The generated page shows a copyable resume command such as:

```bash
cd <cwd> && CODEX_HOME=<home> codex resume --include-non-interactive <thread-id>
```

When `--codex-bin` or `--codex-home` is supplied, the command uses those values.

Highest local authority:

```bash
codexctl plan --prompt-file input.md --dangerously-full-access
```

`--dangerously-full-access` is an alias for `--full-auto`. Both map to:

```text
sandbox = danger-full-access
approvalPolicy = never
```

## Structured Questions

When Codex emits `item/tool/requestUserInput`, `codexctl plan` can handle it:

```text
auto-recommended  choose option label containing "(Recommended)"
auto-first        choose first option
interactive       ask on stdin/stderr
external          print needs_input JSONL and wait for answer JSON on stdin
fail              return needs_input in the final JSON
```

Build an answer payload:

```bash
codexctl answer \
  --question first_version_scope \
  --answer "A Validate the chain first (Recommended)"
```

External mode answer format:

```json
{
  "answers": {
    "first_version_scope": {
      "answers": ["A Validate the chain first (Recommended)"]
    }
  }
}
```

## Unlimited Runtime and Budget

Use explicit unlimited values when automation should not cap a run:

```bash
codexctl plan \
  --objective "Validate Goal, Plan mode, and structured questions" \
  --token-budget unlimited \
  --timeout unlimited \
  --prompt-file input.md \
  --dangerously-full-access
```

Accepted unlimited spellings:

```text
unlimited
infinite
infinity
none
off
0
```

`--token-budget unlimited` omits `tokenBudget` from the app-server goal payload.
`--timeout unlimited` waits forever for Plan-mode app-server events. The older
`--timeout-secs` flag remains as an alias for `--timeout`.

## Design Boundary

`codexctl` intentionally exposes only stable typed commands for the workflows
most useful to automation:

- app-server health
- account, quota, and model discovery
- collaboration modes
- thread Goal
- Plan turns
- CLI-only multi-round session runs
- local JSONL session viewing
- default-mode execute and turn interrupt
- structured question answering
- high-permission execution switch
- logs and summaries

All other app-server APIs are still available through `raw <method> --params`.

## Viewing Sessions

For app-server-created sessions, prefer:

```bash
codexctl read --thread-id <thread-id> --compact
codexctl --codex-home ~/.codex-work read --thread-id <thread-id> --compact
```

`codex resume` is a TUI command and is not the primary verification path for
this wrapper. When it works, use the `resume_command` printed by `goal` or
`plan`, because app-server may store sessions under a non-default `CODEX_HOME`.
