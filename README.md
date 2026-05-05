# codex-app-cli

Rust CLI wrapper around `codex app-server --listen stdio://`.

The binary is `codex-app`. It speaks Codex app-server JSONL, adds stable
high-level commands for Goal, Plan, and structured follow-up questions, and
keeps raw access to all app-server methods through `raw`.

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
codex-app goal get --thread-id <thread-id>
codex-app goal clear --thread-id <thread-id>
```

Plan mode:

```bash
codex-app plan --prompt "Plan only. Ask one question first."
codex-app plan --prompt-file input.md --question-mode auto-recommended
```

Highest local authority:

```bash
codex-app plan --prompt-file input.md --full-auto
```

`--full-auto` maps to:

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
  --answer "A 首版只做 doctor/modes (Recommended)"
```

External mode answer format:

```json
{
  "answers": {
    "first_version_scope": {
      "answers": ["A 首版只做 doctor/modes (Recommended)"]
    }
  }
}
```

## Design Boundary

`codex-app` intentionally exposes only stable typed commands for the workflows
most useful to automation:

- app-server health
- collaboration modes
- thread Goal
- Plan turns
- structured question answering
- high-permission execution switch
- logs and summaries

All other app-server APIs are still available through `raw <method> --params`.

## Viewing Sessions

For app-server-created sessions, prefer:

```bash
codex-app read --thread-id <thread-id> --compact
```

`codex resume` is a TUI command and is not the primary verification path for
this wrapper. When it works, use the `resume_command` printed by `goal` or
`plan`, because app-server may store sessions under a non-default `CODEX_HOME`.
