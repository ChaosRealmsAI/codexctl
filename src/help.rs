pub(crate) const ROOT_AFTER_HELP: &str = r#"Quick start:
  1. Check that Codex app-server can start:
       codex-app doctor

  2. Discover app-server protocol names:
       codex-app methods
       codex-app modes
       codex-app features

  3. Read an existing app-server thread from the default Codex home:
       codex-app read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --compact

  4. Run Plan mode with a Goal, unlimited goal budget, unlimited runtime wait, and highest local permission:
       codex-app plan --objective "Validate Goal, Plan mode, and structured questions" --token-budget unlimited --timeout unlimited --prompt-file input.md --dangerously-full-access --question-mode auto-recommended

  5. Select a different Codex install or account directory only when needed:
       codex-app --codex-bin /path/to/codex --codex-home ~/.codex-work doctor
       codex-app --codex-home ~/.codex-personal read --thread-id <thread_id> --compact

  6. Inspect command-specific help before wiring an app:
       codex-app plan --help
       codex-app session --help
       codex-app session start --help
       codex-app goal --help
       codex-app goal set --help
       codex-app read --help
       codex-app raw --help

Global options:
  --codex-bin <path>     Codex executable. Default: codex.
  --codex-home <dir>     Optional Codex account/config/session home. Default: cleared for the spawned Codex process, so Codex uses its normal default home.
  --account-home <dir>   Alias for --codex-home.
  --log-dir <dir>        Fixed JSONL log directory. Example: target/codex-app-logs.
  --log-mode <mode>      off disables logs, summary writes compact protocol summaries, full writes full JSON messages.
  --session-socket <p>   Unix socket for CLI-only long sessions. Default: /tmp/codex-app-<user>.sock.

Highest permission mode:
  --dangerously-full-access is an alias for --full-auto.
  It starts the Codex thread with sandbox=danger-full-access and approvalPolicy=never.
"#;

pub(crate) const DOCTOR_AFTER_HELP: &str = r#"Examples:
  codex-app doctor
  codex-app --codex-bin /opt/homebrew/bin/codex --log-dir target/codex-app-logs doctor
  codex-app --codex-bin /path/to/codex --codex-home ~/.codex-work doctor

What it checks:
  --codex-bin            Executable used for `codex --version` and `codex app-server`.
  --codex-home           Optional account/config/session home passed as CODEX_HOME only for the spawned Codex process.
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
  codex-app methods
  codex-app methods | jq -r '.methods[]' | rg 'thread/'

Output:
  methods                Known app-server method names accepted by `codex-app raw`.

Use this before `raw` when you need the exact protocol method name.
"#;

pub(crate) const MODES_AFTER_HELP: &str = r#"Examples:
  codex-app modes
  codex-app --log-mode full --log-dir target/codex-app-logs modes

Output:
  Raw collaborationMode/list app-server response.

Use this to confirm available collaboration modes before starting turns.
"#;

pub(crate) const FEATURES_AFTER_HELP: &str = r#"Examples:
  codex-app features

Output:
  Raw experimentalFeature/list app-server response.

Use this to discover app-server feature flags exposed by the installed Codex build.
"#;

pub(crate) const READ_AFTER_HELP: &str = r#"Examples:
  codex-app read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --compact
  codex-app --codex-home ~/.codex-work read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --compact
  codex-app read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --metadata-only
  codex-app read --thread-id 019df7b8-3282-7003-984e-6f95c54d9618

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
  codex-app raw model/list --params '{}'
  codex-app --codex-home ~/.codex-work raw model/list --params '{}'
  codex-app raw thread/read --params '{"threadId":"019df7b8-3282-7003-984e-6f95c54d9618","includeTurns":true}'
  codex-app raw thread/read --params-file params.json --include-events

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
  codex-app goal set --help
  codex-app goal get --help
  codex-app goal clear --help

Goal behavior:
  `goal set` creates a thread if --thread-id is omitted.
  `--token-budget unlimited` leaves the goal uncapped and does not send tokenBudget to app-server.
  `--dangerously-full-access` is available when a new thread must be created.
  `--codex-home <dir>` selects a Codex account/config/session directory. When omitted, the wrapper clears CODEX_HOME so Codex uses its own default home.
"#;

pub(crate) const GOAL_SET_AFTER_HELP: &str = r#"Examples:
  codex-app goal set --objective "Validate Goal, Plan mode, and structured questions" --token-budget unlimited --dangerously-full-access
  codex-app --codex-home ~/.codex-work goal set --objective "Validate the work account" --token-budget unlimited
  codex-app goal set --objective "Ship the CLI wrapper" --token-budget 6000 --full-auto
  codex-app goal set --thread-id 019df7b8-3282-7003-984e-6f95c54d9618 --objective "Improve CLI help" --status active
  codex-app goal set --cwd /path/to/project --sandbox read-only --approval-policy never --objective "Read-only planning"

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

Output:
  thread_id, thread_path, codex_home, resume_command, set/get payloads, and log_path.

Next:
  codex-app --codex-home <codex_home> read --thread-id <thread_id> --compact
"#;

pub(crate) const GOAL_GET_AFTER_HELP: &str = r#"Examples:
  codex-app goal get --thread-id 019df7b8-3282-7003-984e-6f95c54d9618
  codex-app --codex-home ~/.codex-work goal get --thread-id 019df7b8-3282-7003-984e-6f95c54d9618

Parameters:
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process.
  --thread-id <id>       Required app-server thread id.

Output:
  Raw thread/goal/get app-server response.
"#;

pub(crate) const GOAL_CLEAR_AFTER_HELP: &str = r#"Examples:
  codex-app goal clear --thread-id 019df7b8-3282-7003-984e-6f95c54d9618
  codex-app --codex-home ~/.codex-work goal clear --thread-id 019df7b8-3282-7003-984e-6f95c54d9618

Parameters:
  --codex-home <dir>     Optional global account/config/session home. Default: cleared for the spawned Codex process.
  --thread-id <id>       Required app-server thread id.

Effect:
  Clears the active goal for that thread through thread/goal/clear.
"#;

pub(crate) const PLAN_AFTER_HELP: &str = r#"What it does:
  Starts a Codex app-server thread, optionally sets a Goal, then sends one turn/start using collaborationMode plan.

Examples:
  codex-app plan --prompt "Enter Plan mode. Ask one structured question first." --question-mode fail

  codex-app plan \
    --objective "Validate Goal, Plan mode, and structured questions" \
    --token-budget unlimited \
    --timeout unlimited \
    --prompt-file input.md \
    --dangerously-full-access \
    --question-mode auto-recommended \
    --log-dir target/codex-app-logs \
    --log-mode summary

  codex-app --codex-home ~/.codex-work plan \
    --objective "Validate the work account" \
    --token-budget unlimited \
    --timeout unlimited \
    --prompt-file input.md \
    --dangerously-full-access

  printf '{"answers":{"scope":{"answers":["A Validate the chain first (Recommended)"]}}}\n' \
    | codex-app plan --prompt-file input.md --question-mode external --timeout unlimited

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

Question modes:
  auto-recommended       Selects the option label containing "(Recommended)", then falls back to the first option.
  auto-first             Selects the first option.
  interactive            Prompts in this terminal.
  external               Prints a needs_input JSON line, then reads one JSON answer line from stdin.
  fail                   Stops and returns needs_input so another process can answer later.

Output:
  ok, status, thread_id, turn_id, codex_home, thread_path, resume_command, goal, plans, questions, answers, usage, warnings, errors, items, and log_path.

After a run:
  codex-app --codex-home <codex_home> read --thread-id <thread_id> --compact
"#;

pub(crate) const ANSWER_AFTER_HELP: &str = r#"Examples:
  codex-app answer --question first_version_scope --answer "A Validate the chain first (Recommended)"
  codex-app answer --question scope --answer "custom free-form answer"
  codex-app answer --question scope --answer "A" --answer "B"

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

pub(crate) const SESSION_AFTER_HELP: &str = r#"CLI-only long session flow:
  1. Start a run. The daemon keeps app-server alive and returns when a question or completion appears.
       codex-app session start --prompt-file input.md --dangerously-full-access

  2. Answer a structured question by run id.
       codex-app session answer --run-id <run_id> --answer scope="A Small plan (Recommended)"

  3. Send a normal follow-up on the same run, for example plan confirmation.
       codex-app session send --run-id <run_id> --prompt "I confirm this plan. Continue."

  4. Read or stop the run.
       codex-app session read --run-id <run_id>
       codex-app session stop --run-id <run_id>

Return types:
  needs_input            The model asked structured request_user_input questions. Show questions to the caller, then call session answer.
  completed              The turn completed. Show agent_messages/plans to the caller. Use session send for the next user turn.
  failed                 The app-server returned an error.
  warning                Warnings are included in the response but do not always stop the run.

The caller only uses CLI commands. The local daemon is an implementation detail and is auto-started by session commands.
"#;

pub(crate) const SESSION_START_AFTER_HELP: &str = r#"Examples:
  codex-app session start --prompt-file input.md --dangerously-full-access
  codex-app session start --objective "Plan the feature" --token-budget unlimited --timeout unlimited --prompt "Ask one question first."

Output:
  run_id                 Stable id for future CLI calls.
  thread_id              Codex app-server thread id.
  status                 needs_input, completed, or failed.
  questions              Present when status=needs_input.
  agent_messages/plans   Present when the model produced visible output.
"#;

pub(crate) const SESSION_ANSWER_AFTER_HELP: &str = r#"Examples:
  codex-app session answer --run-id <run_id> --answer scope="A Small plan (Recommended)"
  codex-app session answer --run-id <run_id> --answers-json '{"scope":{"answers":["A Small plan (Recommended)"]}}'

Output:
  Same shape as session start. It returns after the next question or completion.
"#;

pub(crate) const SESSION_SEND_AFTER_HELP: &str = r#"Examples:
  codex-app session send --run-id <run_id> --prompt "I confirm this plan. Continue."
  codex-app session send --run-id <run_id> --prompt-file follow-up.md --timeout unlimited

Use this for normal multi-turn conversation on the same run after a turn completes.
"#;

pub(crate) const DAEMON_AFTER_HELP: &str = r#"Examples:
  codex-app daemon status
  codex-app daemon start
  codex-app daemon stop

Session commands auto-start the daemon. Manual daemon commands are for debugging.
"#;
