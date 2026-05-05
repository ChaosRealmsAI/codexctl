# codex-app-cli

Rust CLI wrapper around `codex app-server --listen stdio://`.

The binary is `codex-app`. It speaks Codex app-server JSONL, adds stable
high-level commands for Goal, Plan, and structured follow-up questions, and
keeps raw access to all app-server methods through `raw`.

By default, the wrapper runs `codex` and clears `CODEX_HOME` for the spawned
Codex process. That means it uses the installed Codex CLI and Codex's normal
default account/config/session directory. Use explicit flags only when you need
a different install or account:

```bash
codex-app --codex-bin /path/to/codex doctor
codex-app --codex-home ~/.codex-work doctor
codex-app --codex-bin /path/to/codex --codex-home ~/.codex-work doctor
```

## Build

```bash
cargo build
```

## Fixed Logs

Default logs are written to:

```text
~/.codex-app-cli/logs/
```

Each run writes:

```text
run-<timestamp>.jsonl
latest.jsonl
```

Log modes:

```bash
codex-app --log-mode summary doctor
codex-app --log-mode full doctor
codex-app --log-mode off doctor
```

`summary` keeps method names, ids, question counts, and response keys. `full`
keeps full JSON messages.

## Core Commands

```bash
codex-app doctor
codex-app methods
codex-app modes
codex-app features
codex-app read --thread-id <thread-id> --compact
codex-app --codex-home ~/.codex-work read --thread-id <thread-id> --compact
```

Generic method access:

```bash
codex-app raw collaborationMode/list --params '{}'
codex-app raw model/list --params '{}'
codex-app raw thread/read --params '{"threadId":"...","includeTurns":true}'
```

Goal:

```bash
codex-app goal set --objective "Ship a small CLI" --token-budget 5000
codex-app goal set --objective "Ship a small CLI" --token-budget unlimited
codex-app --codex-home ~/.codex-work goal set --objective "Ship a small CLI"
codex-app goal get --thread-id <thread-id>
codex-app goal clear --thread-id <thread-id>
```

Plan mode:

```bash
codex-app plan --prompt "Plan only. Ask one question first."
codex-app plan --prompt-file input.md --question-mode auto-recommended
codex-app plan --prompt-file input.md --token-budget unlimited --timeout unlimited
codex-app --codex-home ~/.codex-work plan --prompt-file input.md --dangerously-full-access
```

CLI-only long sessions:

```bash
codex-app session start \
  --prompt-file input.md \
  --token-budget unlimited \
  --timeout unlimited \
  --dangerously-full-access

codex-app session start \
  --prompt-file input.md \
  --dangerously-full-access \
  --detach

codex-app session read --run-id <run-id>

codex-app session answer \
  --run-id <run-id> \
  --answer 'scope=A Small plan (Recommended)'

codex-app session send \
  --run-id <run-id> \
  --prompt "I confirm this plan. Continue."

codex-app session read --run-id <run-id>
codex-app session stop --run-id <run-id>
```

`session` commands auto-start a local daemon and communicate through a Unix
socket. The caller still only invokes CLI commands. The daemon keeps the
app-server process, thread, pending structured question, and run state alive
between CLI calls.

Use `--detach` on `session start`, `session answer`, or `session send` when the
caller wants to return immediately and poll `session read` for an in-progress
snapshot. Snapshot responses include `status`, `current_phase`, `elapsed_ms`,
`questions`, `agent_deltas`, `agent_messages`, `plans`, `usage`, and errors.

Highest local authority:

```bash
codex-app plan --prompt-file input.md --dangerously-full-access
```

`--dangerously-full-access` is an alias for `--full-auto`. Both map to:

```text
sandbox = danger-full-access
approvalPolicy = never
```

## Structured Questions

When Codex emits `item/tool/requestUserInput`, `codex-app plan` can handle it:

```text
auto-recommended  choose option label containing "(Recommended)"
auto-first        choose first option
interactive       ask on stdin/stderr
external          print needs_input JSONL and wait for answer JSON on stdin
fail              return needs_input in the final JSON
```

Build an answer payload:

```bash
codex-app answer \
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
codex-app plan \
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

`codex-app` intentionally exposes only stable typed commands for the workflows
most useful to automation:

- app-server health
- collaboration modes
- thread Goal
- Plan turns
- CLI-only multi-round session runs
- structured question answering
- high-permission execution switch
- logs and summaries

All other app-server APIs are still available through `raw <method> --params`.

## Viewing Sessions

For app-server-created sessions, prefer:

```bash
codex-app read --thread-id <thread-id> --compact
codex-app --codex-home ~/.codex-work read --thread-id <thread-id> --compact
```

`codex resume` is a TUI command and is not the primary verification path for
this wrapper. When it works, use the `resume_command` printed by `goal` or
`plan`, because app-server may store sessions under a non-default `CODEX_HOME`.
