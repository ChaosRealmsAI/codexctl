pub(crate) const ROOT_AFTER_HELP: &str = r#"Quick start:
  1. Check that Codex app-server can start:
       codexctl doctor

  2. Install or update codexctl from the public repository:
       curl -fsSL https://raw.githubusercontent.com/ChaosRealmsAI/codexctl/main/install.sh | sh
       cargo install --git https://github.com/ChaosRealmsAI/codexctl --tag v0.1.0 --force

  3. Discover app-server protocol names:
       codexctl methods
       codexctl modes
       codexctl features

  4. Read an existing app-server thread from the default Codex home:
       codexctl read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --compact

  5. Run one-shot Plan mode with a Goal, unlimited goal budget, unlimited runtime wait, and highest local permission:
       codexctl plan --objective "Validate Goal, Plan mode, and structured questions" --token-budget unlimited --timeout unlimited --prompt-file input.md --dangerously-full-access --question-mode auto-recommended

     For app integrations that need multiple turns, structured answers, plan confirmation, or execution, prefer:
       codexctl session start --prompt-file input.md --dangerously-full-access
       codexctl session answer --run-id <run_id> --pick recommended
       codexctl session send --run-id <run_id> --prompt "I confirm this plan."
       codexctl session execute --run-id <run_id>

  6. Select a different Codex install or account directory only when needed:
       codexctl --codex-bin /path/to/codex doctor
       codexctl --codex-home ~/.codex-work doctor
       codexctl --codex-bin /path/to/codex --codex-home ~/.codex-work doctor
       codexctl --codex-home ~/.codex-personal read --thread-id <thread_id> --compact

  7. Inspect command-specific help before wiring an app:
       codexctl account --help
       codexctl quota --help
       codexctl models --help
       codexctl status --help
       codexctl plan --help
       codexctl session --help
       codexctl session start --help
       codexctl session answer --help
       codexctl session execute --help
       codexctl view --help
       codexctl goal --help
       codexctl goal set --help
       codexctl read --help
       codexctl raw --help

Command index:
  Health/account         doctor, account, quota, models, status
  Protocol discovery     methods, modes, features, raw
  Threads/goals          read, goal set, goal get, goal clear
  One-shot planning      plan, answer
  Long sessions          session start, answer, send, execute, list, resume, interrupt, stop (recommended for app integrations)
  Viewing                view
  Daemon debugging       daemon status, daemon start, daemon stop

Help forms:
  Top-level command      codexctl <command> --help
  Top-level via help     codexctl help <command>
  Session subcommand     codexctl session <subcommand> --help
  Session via help       codexctl session help <subcommand>

Install and project:
  Repository             https://github.com/ChaosRealmsAI/codexctl
  One-command install    curl -fsSL https://raw.githubusercontent.com/ChaosRealmsAI/codexctl/main/install.sh | sh
  Pinned release         cargo install --git https://github.com/ChaosRealmsAI/codexctl --tag v0.1.0 --force

Workflow chooser:
  doctor                 First command on a machine; proves Codex app-server starts.
  models                 Check available model ids and supported reasoning efforts before hardcoding automation.
  quota                  Check account limits before starting long or parallel runs.
  goal set/get/clear     Store a durable objective on a thread. Use before long work so later turns know what "done" means.
  plan                   One-shot Plan-mode turn. Best for smoke tests, quick design, or a single structured question cycle.
  session                Multi-turn CLI flow. Best for app integrations, plan confirmation, request_user_input, execute, resume, and inspect.
  raw                    Escape hatch for app-server methods not wrapped by codexctl.

Global options:
  --codex-bin <path>     Codex executable or wrapper. Use for a different installed binary. Default: codex.
  --codex-home <dir>     Optional Codex account/config/session home. Use for another account/session directory.
                          Default: cleared for the spawned Codex process, so Codex uses its normal default home.
  --account-home <dir>   Alias for --codex-home.
  --log-dir <dir>        Fixed JSONL log directory. Example: target/codexctl-logs.
  --log-mode <mode>      off disables logs, summary writes compact protocol summaries, full writes full JSON messages.
  --session-socket <p>   Unix socket for CLI-only long sessions. Default: /tmp/codexctl-<user>.sock.

Highest permission mode:
  --dangerously-full-access is an alias for --full-auto.
  It starts the Codex thread with sandbox=danger-full-access and approvalPolicy=never.

Permission chooser:
  read-only              Default. Use for account checks, thread reads, planning, and audits that must not edit files.
  workspace-write        Use when Codex should edit files inside the workspace but not have unrestricted local access.
  danger-full-access     Use only for trusted local automation that may need broad filesystem/process access.
  --full-auto            Shortcut for sandbox=danger-full-access and approvalPolicy=never.
  --dangerously-full-access
                          Alias for --full-auto.

Model and reasoning chooser:
  List current choices first:
       codexctl models

  Default model          Let Codex choose the account default when you do not pass --model.
  --model <id>           Pin a model for reproducible automation, for example gpt-5.5 or gpt-5.3-codex-spark.
  --effort medium        Default balance for everyday planning and coding.
  --effort low           Faster, cheaper turns for simple checks and short replies.
  --effort high          More reasoning for nontrivial implementation or debugging.
  --effort xhigh         Use for hard architecture, large refactors, or long-running autonomous work.
  --reasoning-effort     Alias for --effort on commands that support it.

Multiple Codex accounts or installs:
  A shell alias like `CODEX_HOME=$HOME/.codex-work codex` becomes:
       codexctl --codex-home ~/.codex-work doctor

  A separate binary or wrapper becomes:
       codexctl --codex-bin /path/to/codex doctor

  Use both only when the binary and account home both differ:
       codexctl --codex-bin /path/to/codex --codex-home ~/.codex-alt doctor
"#;

pub(crate) const DOCTOR_AFTER_HELP: &str = r#"Examples:
  codexctl doctor
  codexctl --codex-home ~/.codex-work doctor
  codexctl --codex-bin /opt/homebrew/bin/codex --log-dir target/codexctl-logs doctor
  codexctl --codex-bin /path/to/codex --codex-home ~/.codex-work doctor

What it checks:
  --codex-bin            Executable or wrapper used for `codex --version` and `codex app-server`.
  --codex-home           Optional account/config/session home passed as CODEX_HOME only for the spawned Codex process.
                         Use this to translate aliases such as `CODEX_HOME=$HOME/.codex-work codex`.
  --log-dir              Where run-*.jsonl and latest.jsonl are written.
  --log-mode             How much protocol traffic is logged.

Output:
  ok                     true when `codex --version` and collaborationMode/list both succeed.
  codex                  version command status/stdout/stderr.
  app_server             initialization status, codex_home, and collaboration modes.
  log_path               Per-run JSONL log path when logging is enabled.
  latest_log_path        Stable latest.jsonl path when logging is enabled.
"#;

pub(crate) const METHODS_AFTER_HELP: &str = r#"Examples:
  codexctl methods
  codexctl methods | jq -r '.methods[]' | rg 'thread/'

Output:
  methods                Known app-server method names accepted by `codexctl raw`.

Use this before `raw` when you need the exact protocol method name.
"#;

pub(crate) const MODES_AFTER_HELP: &str = r#"Examples:
  codexctl modes
  codexctl --log-mode full --log-dir target/codexctl-logs modes

Output:
  Raw collaborationMode/list app-server response.

Use this to confirm available collaboration modes before starting turns.
"#;

pub(crate) const FEATURES_AFTER_HELP: &str = r#"Examples:
  codexctl features

Output:
  Raw experimentalFeature/list app-server response.

Use this to discover app-server feature flags exposed by the installed Codex build.
"#;

pub(crate) const ACCOUNT_AFTER_HELP: &str = r#"Examples:
  codexctl account
  codexctl --codex-home ~/.codex-work account

Output:
  ok, account, codex_home, log_path, and raw account/read response.
"#;

pub(crate) const QUOTA_AFTER_HELP: &str = r#"Examples:
  codexctl quota
  codexctl --codex-home ~/.codex-work quota

Output:
  ok, rate_limits, codex_home, log_path, and raw account/rateLimits/read response.
"#;

pub(crate) const MODELS_AFTER_HELP: &str = r#"Examples:
  codexctl models
  codexctl models | jq -r '.models[] | [.id, .is_default, .default_reasoning_effort] | @tsv'

Output:
  ok, models, codex_home, log_path, and raw model/list response.

Use when:
  Before hardcoding --model in scripts or apps.
  Before choosing --effort / --reasoning-effort for a run.
  When a different Codex account home may expose different defaults or limits.

How to apply a model:
  codexctl plan --model gpt-5.5 --effort high --prompt-file input.md
  codexctl session start --model gpt-5.3-codex-spark --effort medium --prompt-file input.md
  codexctl session execute --run-id <run_id> --model gpt-5.5 --reasoning-effort xhigh

Reasoning guidance:
  low                    Fast and light. Use for small checks and short answers.
  medium                 Default. Use for normal planning and coding.
  high                   Deeper reasoning. Use for debugging, implementation, and review.
  xhigh                  Maximum local reasoning. Use for hard architecture, risky refactors, and long runs.
"#;

pub(crate) const STATUS_AFTER_HELP: &str = r#"Examples:
  codexctl status

Output:
  daemon status without auto-starting the daemon, plus account, rate_limits, models, codex_home, and log_path.
"#;

pub(crate) const READ_AFTER_HELP: &str = r#"Examples:
  codexctl read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --compact
  codexctl --codex-home ~/.codex-work read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --compact
  codexctl read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --metadata-only
  codexctl read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618

Parameters:
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process.
  --thread-id <id>       Required app-server thread id.
  --metadata-only        Sends includeTurns=false to thread/read.
  --compact              Converts the raw thread/read response into a small user/agent message summary.

Output:
  --compact              ok, thread_id, cwd, source, status, path, turn_count, messages.
  default                Raw thread/read response from app-server.

Tip:
  If a thread belongs to a non-default account home, pass --codex-home <dir>. The wrapper does not assume any private Codex home by default.
"#;

pub(crate) const RAW_AFTER_HELP: &str = r#"Examples:
  codexctl raw model/list --params '{}'
  codexctl --codex-home ~/.codex-work raw model/list --params '{}'
  codexctl raw thread/read --params '{"threadId":"019df7b8-3282-7003-984e-6f95c54d9618","includeTurns":true}'
  codexctl raw thread/read --params-file params.json --include-events

Parameters:
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process.
  <method>               Required app-server method name, for example model/list or thread/read.
  --params <json>        JSON object string. Defaults to {}.
  --params-file <file>   File containing a JSON object. Mutually exclusive with --params.
  --include-events       Returns notifications observed before the response. Useful for debugging turn/start.

Output:
  default                The matching JSON-RPC response.
  --include-events       {"response": <response>, "events": [...], "log_path": <path>}.
"#;

pub(crate) const GOAL_AFTER_HELP: &str = r#"Subcommands:
  codexctl goal set --help
  codexctl goal get --help
  codexctl goal clear --help

Goal behavior:
  `goal set` creates a thread if --thread-id is omitted.
  `--token-budget unlimited` leaves the goal uncapped and does not send tokenBudget to app-server.
  `--dangerously-full-access` is available when a new thread must be created.
  `--codex-home <dir>` selects a Codex account/config/session directory. When omitted, the wrapper clears CODEX_HOME so Codex uses its own default home.

Use Goal when:
  You want a durable objective attached to a thread across multiple turns.
  A caller will later resume, inspect, execute, or ask whether the work is complete.
  You want a token budget cap or an explicit unlimited budget recorded at the thread level.

Do not use Goal for:
  A one-off account check, model list, quota check, or raw protocol probe.
  A tiny prompt where the objective is already fully contained in that prompt.

Typical sequence:
  codexctl goal set --objective "Ship the CLI help update" --token-budget unlimited --dangerously-full-access
  codexctl plan --objective "Design the CLI help update" --prompt-file input.md
  codexctl session start --objective "Implement and verify the approved plan" --prompt-file input.md --dangerously-full-access
"#;

pub(crate) const GOAL_SET_AFTER_HELP: &str = r#"Examples:
  codexctl goal set --objective "Validate Goal, Plan mode, and structured questions" --token-budget unlimited --dangerously-full-access
  codexctl --codex-home ~/.codex-work goal set --objective "Validate the work account" --token-budget unlimited
  codexctl goal set --objective "Ship the CLI wrapper" --token-budget 6000 --full-auto
  codexctl goal set --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --objective "Improve CLI help" --status active
  codexctl goal set --cwd /path/to/project --sandbox read-only --approval-policy never --objective "Read-only planning"

Parameters:
  --codex-bin <path>     Optional global Codex executable path. Default: codex.
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process.
  --thread-id <id>       Existing app-server thread id. Omit it to create a new thread first.
  --objective <text>     Required goal objective stored on the thread.
  --token-budget <n>     Positive integer token budget, for example 6000.
  --token-budget unlimited
                          Unlimited goal budget. Also accepts infinite, infinity, none, off, or 0.
  --status <status>      Optional goal status: active, paused, budget-limited, complete.
  --cwd <dir>            Working directory used only when creating a new thread.
  --sandbox <mode>       Sandbox used only when creating a new thread.
  --approval-policy <p>  Approval policy used only when creating a new thread.
  --full-auto            Highest permission shortcut: sandbox=danger-full-access and approvalPolicy=never.
  --dangerously-full-access
                          Alias for --full-auto.

When to use:
  Use `goal set` before a long session or implementation run when the run needs a stable definition of done.
  Use --thread-id to attach a goal to an existing thread.
  Omit --thread-id when you want codexctl to create a new thread and return its resume_command.

Output:
  thread_id, thread_path, codex_home, resume_command, set/get payloads, and log_path.

Next:
  codexctl --codex-home <codex_home> read --thread-id <thread_id> --compact
"#;

pub(crate) const GOAL_GET_AFTER_HELP: &str = r#"Examples:
  codexctl goal get --thread-id 019df7b8-3282-7003-984e-6f95c54d9618
  codexctl --codex-home ~/.codex-work goal get --thread-id 019df7b8-3282-7003-984e-6f95c54d9618

Parameters:
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process.
  --thread-id <id>       Required app-server thread id.

Output:
  Raw thread/goal/get app-server response.
"#;

pub(crate) const GOAL_CLEAR_AFTER_HELP: &str = r#"Examples:
  codexctl goal clear --thread-id 019df7b8-3282-7003-984e-6f95c54d9618
  codexctl --codex-home ~/.codex-work goal clear --thread-id 019df7b8-3282-7003-984e-6f95c54d9618

Parameters:
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process.
  --thread-id <id>       Required app-server thread id.

Effect:
  Clears the active goal for that thread through thread/goal/clear.
"#;

pub(crate) const PLAN_AFTER_HELP: &str = r#"What it does:
  Starts a Codex app-server thread, optionally sets a Goal, then sends one turn/start using collaborationMode plan.

Use Plan mode when:
  You need a plan, architecture sketch, review, or structured request_user_input question before editing.
  You want a one-shot smoke test for Goal + Plan + question handling.
  A human or app should approve the plan before a default-mode execution turn.

Do not use one-shot plan when:
  The caller needs many turns, answer/confirm/execute cycles, or durable inspection by run id.
  In those cases use `codexctl session start`, then `session answer`, `session send`, and `session execute`.

Examples:
  codexctl plan --prompt "Enter Plan mode. Ask one structured question first." --question-mode fail

  codexctl plan \
    --objective "Validate Goal, Plan mode, and structured questions" \
    --token-budget unlimited \
    --timeout unlimited \
    --prompt-file input.md \
    --dangerously-full-access \
    --question-mode auto-recommended \
    --log-dir target/codexctl-logs \
    --log-mode summary

  codexctl --codex-home ~/.codex-work plan \
    --objective "Validate the work account" \
    --token-budget unlimited \
    --timeout unlimited \
    --prompt-file input.md \
    --dangerously-full-access

  printf '{"answers":{"scope":{"answers":["A Validate the chain first (Recommended)"]}}}\n' \
    | codexctl plan --prompt-file input.md --question-mode external --timeout unlimited

Parameters:
  --codex-bin <path>     Optional global Codex executable path. Default: codex.
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process, so Codex uses its normal home.
  --prompt <text>        Inline user prompt. Mutually exclusive with --prompt-file.
  --prompt-file <file>   File containing the user prompt. Mutually exclusive with --prompt.
  --objective <text>     Optional goal objective. When present, plan sets thread/goal/set before turn/start.
  --token-budget <n>     Positive integer goal budget used only with --objective.
  --token-budget unlimited
                          Unlimited goal budget. Also accepts infinite, infinity, none, off, or 0.
  --question-mode <mode> Structured question handling mode. See below.
  --effort <effort>      Reasoning effort: none, minimal, low, medium, high, xhigh.
  --model <model>        Optional model override sent to collaborationMode settings.
  --jsonl                Print each app-server event as a JSONL line while running.
  --timeout <seconds>    Positive integer seconds to wait for one app-server event.
  --timeout unlimited    Wait forever. Also accepts infinite, infinity, none, off, or 0. Default: unlimited.
  --timeout-secs <value> Compatibility alias for --timeout.
  --cwd <dir>            Working directory for thread/start. Default: current shell directory.
  --sandbox <mode>       Sandbox for thread/start: read-only, workspace-write, danger-full-access.
  --approval-policy <p>  Approval policy for thread/start: untrusted, on-failure, on-request, never.
  --full-auto            Highest permission shortcut: sandbox=danger-full-access and approvalPolicy=never.
  --dangerously-full-access
                          Alias for --full-auto.

Model and effort:
  --model <id>           Pin a model from `codexctl models`, for example gpt-5.5.
  --effort medium        Default. Good for normal plans.
  --effort high          Use for architecture, complex debugging, or implementation plans.
  --effort xhigh         Use for hard, ambiguous, or high-risk planning.
  --reasoning-effort     Alias for --effort.

Permissions:
  read-only              Default and recommended for pure planning.
  workspace-write        Use if the planning turn may need to inspect generated local artifacts or prepare files.
  --dangerously-full-access
                          Use only when you trust the run and want no local permission prompts.

Question modes:
  auto-recommended       Selects the option label containing "(Recommended)", then falls back to the first option.
  auto-first             Selects the first option.
  interactive            Prompts in this terminal.
  external               Prints a needs_input JSON line, then reads one JSON answer line from stdin.
  fail                   Stops and returns needs_input so another process can answer later.

Output:
  ok, status, thread_id, turn_id, codex_home, thread_path, resume_command, goal, plans, questions, answers, usage, warnings, errors, items, and log_path.

After a run:
  codexctl --codex-home <codex_home> read --thread-id <thread_id> --compact
"#;

pub(crate) const ANSWER_AFTER_HELP: &str = r#"Examples:
  codexctl answer --question first_version_scope --answer "A Validate the chain first (Recommended)"
  codexctl answer --question scope --answer "custom free-form answer"
  codexctl answer --question scope --answer "A" --answer "B"

Parameters:
  --question <id>        Required question id from request_user_input.
  --answer <text>        Selected option label or free-form answer. Repeat for multiple answers.

Output shape:
  {
    "answers": {
      "<question_id>": {
        "answers": ["<selected option label>"]
      }
    }
  }

Use with plan --question-mode external when another process wants to build the response payload.
"#;

pub(crate) const VIEW_AFTER_HELP: &str = r#"Examples:
  codexctl view ~/.codex/sessions/2026/05/05/rollout-019df7b8.jsonl
  codexctl view sample-session.jsonl --no-open --out target/view.html
  codexctl view --run-id <run_id>

What it opens:
  FILE                  A local Codex rollout JSONL file.
  --run-id <run_id>     The local thread_path backing a currently in-memory daemon run.
                        This is not durable across daemon restarts; use FILE when a run_id is no longer listed.

Display:
  The top of the generated page shows a copyable resume command:
    cd <cwd> && CODEX_HOME=<home> <codex-bin> resume --include-non-interactive <thread_id>
  CODEX_HOME is included when it is known from --codex-home or the rollout path.

Parameters:
  --out <file>          Write the generated standalone HTML viewer to this path.
  --no-open             Generate the HTML but do not open a browser.

Boundary:
  The viewer only loads local JSONL. It does not read codexctl state dumps or synthetic daemon events.
  If --run-id fails with "unknown in-memory run id", run `codexctl session list` or open the rollout file path directly.
"#;

pub(crate) const SESSION_AFTER_HELP: &str = r#"CLI-only long session flow:
Use Session mode when:
  A caller can only invoke CLI commands but needs a long-lived multi-turn Codex run.
  You need structured question delivery, answer by run_id, plan confirmation, execution, interrupt, resume, or JSONL viewing.
  You want app integration behavior without entering the Codex TUI.

  1. Start a run. The daemon keeps app-server alive and returns when a question or completion appears:
       codexctl session start --prompt-file input.md --dangerously-full-access

     For in-progress inspection, detach after submit and open the run's local Codex JSONL:
       codexctl session start --prompt-file input.md --dangerously-full-access --detach
       codexctl view --run-id <run_id>

     `view --run-id` only works while the local daemon still has that run in memory. The durable fallback is the `thread_path` printed by session commands:
       codexctl view <thread_path>

  2. Answer a structured question by run id.
       codexctl session answer --run-id <run_id> --answer scope="A Small plan (Recommended)"
       codexctl session answer --run-id <run_id> --pick recommended

  3. Send a normal follow-up on the same run, for example plan confirmation.
       codexctl session send --run-id <run_id> --prompt "I confirm this plan. Continue."

  4. Execute the approved plan in default collaboration mode.
       codexctl session execute --run-id <run_id> --detach

  5. Inspect, resume, interrupt, or stop runs.
       codexctl session list --threads
       codexctl view --run-id <run_id>
       codexctl session resume --thread-id <thread_id>
       codexctl session interrupt --run-id <run_id>
       codexctl session stop --run-id <run_id>

Return types:
  needs_input            The model asked structured request_user_input questions. Show questions to the caller, then call session answer.
  completed              The turn completed. Show agent_messages/plans to the caller. Use session send for the next user turn.
  failed                 The app-server returned an error.
  warning                Warnings are included in the response but do not always stop the run.

Run response fields:
  status                 running, needs_input, completed, failed, or stopped.
  current_phase          Current coarse phase such as starting, reasoning, agent_message, needs_input, or completed.
  elapsed_ms             Milliseconds since run creation.
  questions              Pending structured questions when status=needs_input.
  agent_deltas           Text deltas collected so far while status=running.
  thread_path            Local Codex rollout JSONL path used by `codexctl view --run-id`.

Use `session` rather than one-shot `plan` for app integrations that need question answering, plan confirmation, execution, or later inspection.
The caller only uses CLI commands. The local daemon is an implementation detail and is auto-started by session commands.

Model, effort, and permissions:
  session start          Accepts --model, --effort/--reasoning-effort, --sandbox, --approval-policy, and --dangerously-full-access.
  session send           Accepts --model and --effort for follow-up turns.
  session execute        Accepts --model and --effort for implementation turns.
  session resume         Accepts --model, --effort, and runtime permission flags for future turns.
  Planning only          Prefer read-only or workspace-write.
  Implementation         Use workspace-write or --dangerously-full-access when trusted automation must edit files.
"#;

pub(crate) const SESSION_START_AFTER_HELP: &str = r#"Examples:
  codexctl session start --prompt-file input.md --dangerously-full-access
  codexctl session start --prompt-file input.md --dangerously-full-access --detach
  codexctl session start --objective "Plan the feature" --token-budget unlimited --timeout unlimited --prompt "Ask one question first."
  codexctl session start --model gpt-5.5 --effort high --prompt-file input.md
  codexctl session start --sandbox workspace-write --approval-policy never --prompt-file input.md

Parameters:
  --detach               Return immediately after submitting turn/start. Use `codexctl view --run-id <run_id>` while the daemon is alive, or `codexctl view <thread_path>` as the durable fallback.
  --model <id>           Optional model id from `codexctl models`.
  --effort <effort>      Reasoning effort: low, medium, high, or xhigh for most current Codex models.
  --reasoning-effort     Alias for --effort.
  --sandbox <mode>       read-only, workspace-write, or danger-full-access.
  --approval-policy <p>  untrusted, on-failure, on-request, or never.
  --dangerously-full-access
                          Highest trusted local automation mode.

Output:
  run_id                 Stable id for future CLI calls.
  thread_id              Codex app-server thread id.
  status                 running when detached, otherwise needs_input, completed, or failed.
  current_phase          Coarse current phase for status display.
  questions              Present when status=needs_input.
  agent_messages/plans   Present when the model produced visible output.
"#;

pub(crate) const SESSION_ANSWER_AFTER_HELP: &str = r#"Examples:
  codexctl session answer --run-id <run_id> --answer scope="A Small plan (Recommended)"
  codexctl session answer --run-id <run_id> --pick recommended
  codexctl session answer --run-id <run_id> --pick first
  codexctl session answer --run-id <run_id> --pick 1,2,1
  codexctl session answer --run-id <run_id> --answer scope="A Small plan (Recommended)" --detach
  codexctl session answer --run-id <run_id> --answers-json '{"scope":{"answers":["A Small plan (Recommended)"]}}'

Parameters:
  --answer               Exact QUESTION_ID=SELECTED_LABEL answer. Repeat for multiple questions.
  --pick recommended     Select the option label containing "(Recommended)" for every pending question.
  --pick first           Select the first option for every pending question.
  --pick 1,2,1           Select option indexes by pending-question order.

Output:
  Same shape as session start. Without --detach, it returns after the next question or completion. With --detach, it returns after submitting the answer.
"#;

pub(crate) const SESSION_SEND_AFTER_HELP: &str = r#"Examples:
  codexctl session send --run-id <run_id> --prompt "I confirm this plan. Continue."
  codexctl session send --run-id <run_id> --prompt "I confirm this plan. Continue." --detach
  codexctl session send --run-id <run_id> --prompt-file follow-up.md --timeout unlimited
  codexctl session send --run-id <run_id> --model gpt-5.5 --effort high --prompt "Revise the plan."

Use this for normal multi-turn conversation on the same run after a turn completes. Add --detach when the caller wants to return immediately and inspect progress with `codexctl view --run-id`. If the daemon no longer knows the run_id, open the printed thread_path with `codexctl view <thread_path>`.

Model and effort:
  Use --model to change the model for this follow-up turn.
  Use --effort or --reasoning-effort to choose reasoning depth for this follow-up turn.
"#;

pub(crate) const SESSION_EXECUTE_AFTER_HELP: &str = r#"Examples:
  codexctl session execute --run-id <run_id> --detach
  codexctl session execute --run-id <run_id> --prompt "Implement the approved plan."
  codexctl session execute --run-id <run_id> --model gpt-5.5 --effort xhigh --detach

Use this after a plan is approved. It starts a default-mode turn on the same run and may modify files according to Codex permissions. If no prompt is supplied, codexctl sends a small default implementation prompt.

When to use:
  After the human or caller has accepted the Plan-mode output.
  When Codex should switch from planning to default-mode implementation.
  When file edits are expected, make sure the original run was started with workspace-write or --dangerously-full-access.

Model and effort:
  medium                 Default for normal implementation.
  high                   Use for debugging, tests, and multi-file changes.
  xhigh                  Use for hard architecture changes, risky migrations, or long autonomous execution.
"#;

pub(crate) const SESSION_RESUME_AFTER_HELP: &str = r#"Examples:
  codexctl session resume --thread-id 019df7b8-3282-7003-984e-6f95c54d9618
  codexctl session resume --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --model gpt-5.5 --effort high

Attaches a daemon run_id to an existing persisted Codex thread through thread/resume.

Use this when:
  The daemon was restarted but the Codex thread still exists.
  You want to continue a persisted thread through the session command family.
"#;

pub(crate) const SESSION_INTERRUPT_AFTER_HELP: &str = r#"Examples:
  codexctl session interrupt --run-id <run_id>

Interrupts the current active turn through turn/interrupt using the run's thread_id and turn_id. The run remains available for session send or execute.
"#;

pub(crate) const SESSION_LIST_AFTER_HELP: &str = r#"Examples:
  codexctl session list
  codexctl session list --threads --limit 5

Output:
  runs                  Lightweight in-memory daemon run summaries: run_id, status, current_phase, thread_id, thread_path, and timestamps.
  threads               Recent persisted Codex threads when --threads is set.

Note:
  run_id is daemon-local and not durable. thread_id/thread_path are the durable identifiers; use `codexctl view <thread_path>` after a daemon restart.
"#;

pub(crate) const DAEMON_AFTER_HELP: &str = r#"Examples:
  codexctl daemon status
  codexctl daemon start
  codexctl daemon stop

Session commands auto-start the daemon. Manual daemon commands are for debugging.
"#;
