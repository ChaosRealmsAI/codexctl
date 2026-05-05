use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use std::{fmt, num::NonZeroU64};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::help;

pub const DEFAULT_TIMEOUT_SECS: u64 = 180;

#[derive(Debug, Parser)]
#[command(
    name = "codexctl",
    version,
    about = "Codex control-plane CLI for goals, plan sessions, JSONL viewing, and app-server debugging",
    after_help = help::ROOT_AFTER_HELP
)]
pub struct Cli {
    #[arg(
        long,
        default_value = "codex",
        global = true,
        help = "Codex CLI executable or wrapper, e.g. codex or /opt/homebrew/bin/codex",
        long_help = "Codex CLI executable or wrapper. Use this only when you need a different installed Codex binary or a wrapper script. Account/session switching is usually --codex-home instead."
    )]
    pub codex_bin: String,
    #[arg(
        long,
        global = true,
        visible_alias = "account-home",
        value_name = "DIR",
        help = "Optional Codex home/account directory; defaults to the Codex CLI default",
        long_help = "Optional Codex home/account directory. When set, the wrapper starts Codex with CODEX_HOME=<DIR>. Use this to switch accounts, config, auth, and session storage. A shell alias like `CODEX_HOME=$HOME/.codex-work codex` becomes `codexctl --codex-home ~/.codex-work ...`. When omitted, the wrapper clears CODEX_HOME for the spawned Codex process so Codex uses its normal default home."
    )]
    pub codex_home: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Fixed directory for JSONL app-server logs, e.g. target/codexctl-logs"
    )]
    pub log_dir: Option<PathBuf>,
    #[arg(
        long,
        value_enum,
        default_value_t = LogMode::Summary,
        global = true,
        help = "Log protocol traffic: off, compact summaries, or full JSON messages"
    )]
    pub log_mode: LogMode,
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Daemon endpoint path for CLI-only long sessions; defaults to a temp codexctl endpoint"
    )]
    pub session_socket: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(
        about = "Check Codex CLI and app-server JSONL connectivity",
        after_help = help::DOCTOR_AFTER_HELP
    )]
    Doctor,
    #[command(
        about = "Show official Codex workflow guidance mapped to codexctl",
        after_help = help::GUIDE_AFTER_HELP
    )]
    Guide,
    #[command(
        about = "List known app-server method names exposed through raw",
        after_help = help::METHODS_AFTER_HELP
    )]
    Methods,
    #[command(
        about = "List app-server collaboration modes",
        after_help = help::MODES_AFTER_HELP
    )]
    Modes,
    #[command(
        about = "List app-server experimental features",
        after_help = help::FEATURES_AFTER_HELP
    )]
    Features,
    #[command(about = "Read Codex account information", after_help = help::ACCOUNT_AFTER_HELP)]
    Account,
    #[command(about = "Read Codex quota and rate limit status", after_help = help::QUOTA_AFTER_HELP)]
    Quota,
    #[command(about = "List Codex models and supported reasoning efforts", after_help = help::MODELS_AFTER_HELP)]
    Models,
    #[command(about = "Show daemon, account, quota, and model status", after_help = help::STATUS_AFTER_HELP)]
    Status,
    #[command(
        about = "Read an app-server thread, optionally including turns",
        after_help = help::READ_AFTER_HELP
    )]
    Read(ReadArgs),
    #[command(
        about = "Call any app-server JSON-RPC method",
        after_help = help::RAW_AFTER_HELP
    )]
    Raw(RawArgs),
    #[command(
        subcommand,
        about = "Set, get, or clear a Codex thread goal",
        after_help = help::GOAL_AFTER_HELP
    )]
    Goal(GoalCommand),
    #[command(
        about = "Run a native Plan-mode turn with optional Goal and question handling",
        after_help = help::PLAN_AFTER_HELP
    )]
    Plan(PlanArgs),
    #[command(
        about = "Build a request_user_input answer payload",
        after_help = help::ANSWER_AFTER_HELP
    )]
    Answer(AnswerArgs),
    #[command(
        about = "Open a local Codex JSONL viewer for a file or daemon run",
        after_help = help::VIEW_AFTER_HELP
    )]
    View(ViewArgs),
    #[command(
        subcommand,
        about = "Run CLI-only multi-round sessions through a local daemon",
        after_help = help::SESSION_AFTER_HELP
    )]
    Session(SessionCommand),
    #[command(
        subcommand,
        about = "Manage the local codexctl session daemon",
        after_help = help::DAEMON_AFTER_HELP
    )]
    Daemon(DaemonCommand),
}

#[derive(Debug, Args)]
pub struct RawArgs {
    #[arg(help = "app-server method, e.g. thread/start or model/list")]
    pub method: String,
    #[arg(
        long,
        help = "JSON params object string; defaults to '{}', e.g. '{\"threadId\":\"...\"}'"
    )]
    pub params: Option<String>,
    #[arg(long, help = "Read JSON params object from a file such as params.json")]
    pub params_file: Option<PathBuf>,
    #[arg(long, help = "Also include notifications seen before the response")]
    pub include_events: bool,
}

#[derive(Debug, Args)]
pub struct ReadArgs {
    #[arg(
        long,
        help = "App-server thread id, e.g. 019df7b8-3282-7003-984e-6f95c54d9618"
    )]
    pub thread_id: String,
    #[arg(long, help = "Only return thread metadata; do not include turn items")]
    pub metadata_only: bool,
    #[arg(
        long,
        help = "Return a small summary instead of the raw thread payload"
    )]
    pub compact: bool,
}

#[derive(Debug, Args)]
pub struct ViewArgs {
    #[arg(
        value_name = "FILE",
        conflicts_with = "run_id",
        help = "Local Codex rollout JSONL file to open in the viewer"
    )]
    pub file: Option<PathBuf>,
    #[arg(
        long,
        conflicts_with = "file",
        help = "Open the local Codex rollout JSONL backing an in-memory daemon run"
    )]
    pub run_id: Option<String>,
    #[arg(
        long,
        value_name = "FILE",
        help = "Write the generated viewer HTML to this path"
    )]
    pub out: Option<PathBuf>,
    #[arg(
        long,
        value_name = "FILE",
        help = "Use an external viewer HTML template instead of the bundled viewer"
    )]
    pub viewer_html: Option<PathBuf>,
    #[arg(long, help = "Generate the viewer HTML but do not open the browser")]
    pub no_open: bool,
}

#[derive(Debug, Subcommand)]
pub enum GoalCommand {
    #[command(
        about = "Start a thread if needed, then set a goal",
        after_help = help::GOAL_SET_AFTER_HELP
    )]
    Set(GoalSetArgs),
    #[command(
        about = "Read a goal from an existing thread",
        after_help = help::GOAL_GET_AFTER_HELP
    )]
    Get(ThreadIdArgs),
    #[command(
        about = "Clear a goal from an existing thread",
        after_help = help::GOAL_CLEAR_AFTER_HELP
    )]
    Clear(ThreadIdArgs),
}

#[derive(Debug, Args)]
pub struct GoalSetArgs {
    #[arg(long, help = "Existing thread id; omit to create a new thread first")]
    pub thread_id: Option<String>,
    #[arg(long, help = "Goal objective stored on the thread")]
    pub objective: String,
    #[arg(
        long,
        default_value_t = TokenBudget::Unlimited,
        help = "Goal token budget: a positive integer or unlimited",
        long_help = "Goal token budget. Use a positive integer such as 6000 to cap the goal, or use unlimited/infinite/none/0 to leave the goal uncapped. The default is unlimited."
    )]
    pub token_budget: TokenBudget,
    #[arg(long, value_enum, help = "Optional goal status to write")]
    pub status: Option<GoalStatus>,
    #[command(flatten)]
    pub runtime: RuntimeArgs,
}

#[derive(Debug, Args)]
pub struct ThreadIdArgs {
    #[arg(
        long,
        help = "App-server thread id, e.g. 019df7b8-3282-7003-984e-6f95c54d9618"
    )]
    pub thread_id: String,
}

#[derive(Debug, Args)]
pub struct PlanArgs {
    #[arg(
        long,
        conflicts_with = "prompt_file",
        help = "Inline user prompt sent to the Plan-mode turn"
    )]
    pub prompt: Option<String>,
    #[arg(
        long,
        help = "File containing the user prompt sent to the Plan-mode turn"
    )]
    pub prompt_file: Option<PathBuf>,
    #[arg(long, help = "Set a thread goal before turn/start")]
    pub objective: Option<String>,
    #[arg(
        long,
        default_value_t = TokenBudget::Unlimited,
        help = "Goal token budget used only when --objective is set",
        long_help = "Goal token budget used only when --objective is set. Use a positive integer such as 6000 to cap the goal, or use unlimited/infinite/none/0 to leave the goal uncapped. The default is unlimited."
    )]
    pub token_budget: TokenBudget,
    #[arg(
        long,
        value_enum,
        default_value_t = QuestionMode::AutoRecommended,
        help = "How to handle structured request_user_input questions"
    )]
    pub question_mode: QuestionMode,
    #[arg(
        long,
        visible_alias = "reasoning-effort",
        value_enum,
        default_value_t = Effort::Medium,
        help = "Reasoning effort sent in Plan collaboration settings",
        long_help = "Reasoning effort sent in Plan collaboration settings. Use medium for normal plans, high for architecture/debugging plans, and xhigh for hard or risky planning."
    )]
    pub effort: Effort,
    #[arg(
        long,
        help = "Override model used in collaborationMode settings; run `codexctl models` first"
    )]
    pub model: Option<String>,
    #[arg(long, help = "Write events as JSONL while running")]
    pub jsonl: bool,
    #[arg(
        long = "timeout",
        visible_alias = "timeout-secs",
        default_value_t = RunTimeout::Unlimited,
        help = "Maximum wait time for one Plan-mode app-server event: seconds or unlimited",
        long_help = "Maximum wait time for one Plan-mode app-server event. Use a positive integer such as 600 for seconds, or unlimited/infinite/none/0 to wait forever. The default is unlimited so long Plan-mode runs are not cut off by the wrapper. --timeout-secs is kept as a compatibility alias."
    )]
    pub timeout: RunTimeout,
    #[command(flatten)]
    pub runtime: RuntimeArgs,
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    #[command(
        about = "Start a long session and stop at the first question or completion",
        after_help = help::SESSION_START_AFTER_HELP
    )]
    Start(SessionStartArgs),
    #[command(
        about = "Answer a pending structured question and continue the same run",
        after_help = help::SESSION_ANSWER_AFTER_HELP
    )]
    Answer(SessionAnswerArgs),
    #[command(
        about = "Send a normal follow-up message on the same run",
        after_help = help::SESSION_SEND_AFTER_HELP
    )]
    Send(SessionSendArgs),
    #[command(
        about = "Execute the approved plan in default collaboration mode",
        after_help = help::SESSION_EXECUTE_AFTER_HELP
    )]
    Execute(SessionExecuteArgs),
    #[command(
        about = "Attach daemon state to an existing Codex thread",
        after_help = help::SESSION_RESUME_AFTER_HELP
    )]
    Resume(SessionResumeArgs),
    #[command(
        about = "Read a nonblocking snapshot for one daemon run",
        after_help = help::SESSION_READ_AFTER_HELP
    )]
    Read(SessionReadArgs),
    #[command(
        about = "Watch a daemon run by polling nonblocking snapshots as JSONL",
        after_help = help::SESSION_WATCH_AFTER_HELP
    )]
    Watch(SessionWatchArgs),
    #[command(
        about = "Interrupt the current turn without removing the run",
        after_help = help::SESSION_INTERRUPT_AFTER_HELP
    )]
    Interrupt(SessionRunIdArgs),
    #[command(
        about = "List daemon runs, optionally with recent persisted Codex threads",
        after_help = help::SESSION_LIST_AFTER_HELP
    )]
    List(SessionListArgs),
    #[command(about = "Stop one run and release its app-server process")]
    Stop(SessionRunIdArgs),
}

#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    #[command(about = "Start the local session daemon if it is not running")]
    Start,
    #[command(about = "Check whether the local session daemon is running")]
    Status,
    #[command(about = "List reachable codexctl daemon endpoints")]
    List,
    #[command(about = "Stop the local session daemon")]
    Stop,
    #[command(hide = true)]
    Serve,
}

#[derive(Debug, Args)]
pub struct SessionStartArgs {
    #[arg(
        long,
        conflicts_with = "prompt_file",
        help = "Inline user prompt sent to the Plan-mode turn"
    )]
    pub prompt: Option<String>,
    #[arg(
        long,
        help = "File containing the user prompt sent to the Plan-mode turn"
    )]
    pub prompt_file: Option<PathBuf>,
    #[arg(long, help = "Set a thread goal before turn/start")]
    pub objective: Option<String>,
    #[arg(
        long,
        default_value_t = TokenBudget::Unlimited,
        help = "Goal token budget used only when --objective is set"
    )]
    pub token_budget: TokenBudget,
    #[arg(
        long,
        visible_alias = "reasoning-effort",
        value_enum,
        default_value_t = Effort::Medium,
        help = "Reasoning effort for this session turn: low, medium, high, or xhigh"
    )]
    pub effort: Effort,
    #[arg(
        long,
        help = "Override model used in collaborationMode settings; run `codexctl models` first"
    )]
    pub model: Option<String>,
    #[arg(
        long = "timeout",
        visible_alias = "timeout-secs",
        default_value_t = RunTimeout::Unlimited,
        help = "Maximum wait time for this session command to pause: seconds or unlimited"
    )]
    pub timeout: RunTimeout,
    #[arg(
        long,
        help = "Return immediately after submitting the turn; inspect progress with codexctl view --run-id"
    )]
    pub detach: bool,
    #[arg(
        long,
        value_enum,
        help = "Compatibility option; daemon sessions always surface questions for session answer"
    )]
    pub question_mode: Option<QuestionMode>,
    #[arg(
        long,
        value_name = "DIR",
        help = "Compatibility option accepted for older scripts; no files are written by this flag"
    )]
    pub version_dir: Option<PathBuf>,
    #[command(flatten)]
    pub runtime: RuntimeArgs,
}

#[derive(Debug, Args)]
pub struct SessionAnswerArgs {
    #[arg(long, help = "Run id returned by codexctl session start")]
    pub run_id: String,
    #[arg(
        long = "answer",
        help = "Answer in QUESTION_ID=SELECTED_LABEL form; repeat for multiple questions"
    )]
    pub answers: Vec<String>,
    #[arg(
        long,
        conflicts_with = "answers_file",
        help = "Raw answers JSON object"
    )]
    pub answers_json: Option<String>,
    #[arg(long, help = "File containing raw answers JSON object")]
    pub answers_file: Option<PathBuf>,
    #[arg(
        long,
        conflicts_with_all = ["answers", "answers_json", "answers_file"],
        help = "Answer pending questions by option index list, first, or recommended"
    )]
    pub pick: Option<String>,
    #[arg(
        long = "timeout",
        visible_alias = "timeout-secs",
        default_value_t = RunTimeout::Unlimited,
        help = "Maximum wait time for this session command to pause: seconds or unlimited"
    )]
    pub timeout: RunTimeout,
    #[arg(
        long,
        help = "Return immediately after submitting the answer; inspect progress with codexctl view --run-id"
    )]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct SessionSendArgs {
    #[arg(long, help = "Run id returned by codexctl session start")]
    pub run_id: String,
    #[arg(long, conflicts_with = "prompt_file", help = "Inline follow-up prompt")]
    pub prompt: Option<String>,
    #[arg(long, help = "File containing the follow-up prompt")]
    pub prompt_file: Option<PathBuf>,
    #[arg(
        long,
        visible_alias = "reasoning-effort",
        value_enum,
        default_value_t = Effort::Medium,
        help = "Reasoning effort for this follow-up turn: low, medium, high, or xhigh"
    )]
    pub effort: Effort,
    #[arg(
        long,
        help = "Override model used in collaborationMode settings; run `codexctl models` first"
    )]
    pub model: Option<String>,
    #[arg(
        long = "timeout",
        visible_alias = "timeout-secs",
        default_value_t = RunTimeout::Unlimited,
        help = "Maximum wait time for this session command to pause: seconds or unlimited"
    )]
    pub timeout: RunTimeout,
    #[arg(
        long,
        help = "Return immediately after submitting the prompt; inspect progress with codexctl view --run-id"
    )]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct SessionExecuteArgs {
    #[arg(long, help = "Run id returned by codexctl session start or resume")]
    pub run_id: String,
    #[arg(long, conflicts_with = "prompt_file", help = "Inline execution prompt")]
    pub prompt: Option<String>,
    #[arg(long, help = "File containing the execution prompt")]
    pub prompt_file: Option<PathBuf>,
    #[arg(
        long,
        visible_alias = "reasoning-effort",
        value_enum,
        default_value_t = Effort::Medium,
        help = "Reasoning effort for this execution turn: low, medium, high, or xhigh"
    )]
    pub effort: Effort,
    #[arg(
        long,
        help = "Override model used in collaborationMode settings; run `codexctl models` first"
    )]
    pub model: Option<String>,
    #[arg(
        long = "timeout",
        visible_alias = "timeout-secs",
        default_value_t = RunTimeout::Unlimited,
        help = "Maximum wait time for this session command to pause: seconds or unlimited"
    )]
    pub timeout: RunTimeout,
    #[arg(
        long,
        help = "Return immediately after submitting the execution turn; inspect progress with codexctl view --run-id"
    )]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct SessionResumeArgs {
    #[arg(long, help = "Existing Codex app-server thread id to attach")]
    pub thread_id: String,
    #[arg(
        long,
        value_enum,
        default_value_t = Effort::Medium,
        visible_alias = "reasoning-effort",
        help = "Reasoning effort for future turns on this resumed run"
    )]
    pub effort: Effort,
    #[arg(
        long,
        help = "Override model used for future turns; run `codexctl models` first"
    )]
    pub model: Option<String>,
    #[command(flatten)]
    pub runtime: RuntimeArgs,
}

#[derive(Debug, Args)]
pub struct SessionReadArgs {
    #[arg(long, help = "Run id returned by codexctl session start or resume")]
    pub run_id: String,
    #[arg(
        long,
        help = "Return full run state instead of the default compact summary"
    )]
    pub full: bool,
}

#[derive(Debug, Args)]
pub struct SessionWatchArgs {
    #[arg(long, help = "Run id returned by codexctl session start or resume")]
    pub run_id: String,
    #[arg(
        long,
        default_value_t = 1000,
        help = "Polling interval in milliseconds"
    )]
    pub interval_ms: u64,
    #[arg(long, help = "Print full run state on every JSONL line")]
    pub full: bool,
}

#[derive(Debug, Args)]
pub struct SessionListArgs {
    #[arg(
        long,
        help = "Also include recent persisted app-server threads from thread/list"
    )]
    pub threads: bool,
    #[arg(
        long,
        default_value_t = 20,
        help = "Maximum persisted threads to return with --threads"
    )]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct SessionRunIdArgs {
    #[arg(long, help = "Run id returned by codexctl session start")]
    pub run_id: String,
}

#[derive(Debug, Args)]
pub struct RuntimeArgs {
    #[arg(
        long,
        help = "Working directory for Codex; defaults to the current shell directory"
    )]
    pub cwd: Option<PathBuf>,
    #[arg(
        long,
        value_enum,
        default_value_t = SandboxMode::ReadOnly,
        help = "Sandbox mode passed to thread/start"
    )]
    pub sandbox: SandboxMode,
    #[arg(
        long,
        value_enum,
        default_value_t = ApprovalPolicy::Never,
        help = "Approval policy passed to thread/start"
    )]
    pub approval_policy: ApprovalPolicy,
    #[arg(
        long,
        visible_alias = "dangerously-full-access",
        help = "Highest Codex permission mode: --sandbox danger-full-access --approval-policy never",
        long_help = "Highest Codex permission mode. When set, the wrapper starts the Codex thread with sandbox=danger-full-access and approvalPolicy=never, overriding --sandbox and --approval-policy. Alias: --dangerously-full-access."
    )]
    pub full_auto: bool,
}

#[derive(Debug, Args)]
pub struct AnswerArgs {
    #[arg(long, help = "Question id from request_user_input")]
    pub question: String,
    #[arg(
        long,
        help = "Selected option label or free-form answer; repeat for multiple answers"
    )]
    pub answer: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogMode {
    Off,
    Summary,
    Full,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum QuestionMode {
    AutoRecommended,
    AutoFirst,
    Interactive,
    External,
    Fail,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ApprovalPolicy {
    Untrusted,
    OnFailure,
    OnRequest,
    Never,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Effort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GoalStatus {
    Active,
    Paused,
    BudgetLimited,
    Complete,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TokenBudget {
    #[default]
    Unlimited,
    Limited(NonZeroU64),
}

impl TokenBudget {
    pub fn as_protocol_value(self) -> Option<u64> {
        match self {
            Self::Unlimited => None,
            Self::Limited(value) => Some(value.get()),
        }
    }
}

impl fmt::Display for TokenBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlimited => write!(f, "unlimited"),
            Self::Limited(value) => write!(f, "{value}"),
        }
    }
}

impl FromStr for TokenBudget {
    type Err = anyhow::Error;

    fn from_str(raw: &str) -> Result<Self> {
        parse_limit(raw, "token budget").map(|value| match value {
            Some(value) => Self::Limited(value),
            None => Self::Unlimited,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RunTimeout {
    #[default]
    Unlimited,
    Seconds(NonZeroU64),
}

impl RunTimeout {
    pub fn duration(self) -> Option<Duration> {
        match self {
            Self::Unlimited => None,
            Self::Seconds(value) => Some(Duration::from_secs(value.get())),
        }
    }
}

impl fmt::Display for RunTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlimited => write!(f, "unlimited"),
            Self::Seconds(value) => write!(f, "{value}"),
        }
    }
}

impl FromStr for RunTimeout {
    type Err = anyhow::Error;

    fn from_str(raw: &str) -> Result<Self> {
        parse_limit(raw, "timeout").map(|value| match value {
            Some(value) => Self::Seconds(value),
            None => Self::Unlimited,
        })
    }
}

fn parse_limit(raw: &str, name: &str) -> Result<Option<NonZeroU64>> {
    let normalized = raw.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "unlimited" | "infinite" | "infinity" | "none" | "off" | "0"
    ) {
        return Ok(None);
    }
    let value = normalized
        .parse::<u64>()
        .with_context(|| format!("{name} must be a positive integer or unlimited"))?;
    let Some(value) = NonZeroU64::new(value) else {
        return Ok(None);
    };
    if value.get() > 10_000_000_000 {
        bail!("{name} is too large to be useful: {value}");
    }
    Ok(Some(value))
}

impl SandboxMode {
    pub fn as_protocol(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

impl ApprovalPolicy {
    pub fn as_protocol(self) -> &'static str {
        match self {
            Self::Untrusted => "untrusted",
            Self::OnFailure => "on-failure",
            Self::OnRequest => "on-request",
            Self::Never => "never",
        }
    }
}

impl Effort {
    pub fn as_protocol(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        }
    }
}

impl GoalStatus {
    pub fn as_protocol(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::BudgetLimited => "budgetLimited",
            Self::Complete => "complete",
        }
    }
}

impl RuntimeArgs {
    pub fn effective(self) -> Result<EffectiveRuntime> {
        let cwd = match self.cwd {
            Some(path) => path,
            None => std::env::current_dir().context("read current directory failed")?,
        };
        let sandbox = if self.full_auto {
            SandboxMode::DangerFullAccess
        } else {
            self.sandbox
        };
        let approval_policy = if self.full_auto {
            ApprovalPolicy::Never
        } else {
            self.approval_policy
        };
        Ok(EffectiveRuntime {
            cwd: cwd.display().to_string(),
            sandbox: sandbox.as_protocol().to_string(),
            approval_policy: approval_policy.as_protocol().to_string(),
        })
    }
}

#[derive(Debug)]
pub struct EffectiveRuntime {
    pub cwd: String,
    pub sandbox: String,
    pub approval_policy: String,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::str::FromStr;

    use clap::Parser;

    use super::{Cli, Commands, SessionCommand};
    use super::{RunTimeout, TokenBudget};

    #[test]
    fn default_codex_home_is_unset() {
        let cli = Cli::try_parse_from(["codexctl", "doctor"]).unwrap();
        assert!(cli.codex_home.is_none());
        assert!(matches!(cli.command, Commands::Doctor));
    }

    #[test]
    fn parses_guide_command() {
        let cli = Cli::try_parse_from(["codexctl", "guide"]).unwrap();
        assert!(matches!(cli.command, Commands::Guide));
    }

    #[test]
    fn parses_codex_home_alias() {
        let cli = Cli::try_parse_from(["codexctl", "--account-home", "/tmp/codex-work", "doctor"])
            .unwrap();
        assert_eq!(cli.codex_home, Some(PathBuf::from("/tmp/codex-work")));
    }

    #[test]
    fn parses_session_start_detach() {
        let cli = Cli::try_parse_from([
            "codexctl", "session", "start", "--prompt", "hello", "--detach",
        ])
        .unwrap();
        let Commands::Session(SessionCommand::Start(args)) = cli.command else {
            panic!("expected session start command");
        };
        assert!(args.detach);
    }

    #[test]
    fn parses_session_answer_pick_and_execute_aliases() {
        let answer = Cli::try_parse_from([
            "codexctl",
            "session",
            "answer",
            "--run-id",
            "run-1",
            "--pick",
            "recommended",
        ])
        .unwrap();
        let Commands::Session(SessionCommand::Answer(args)) = answer.command else {
            panic!("expected session answer command");
        };
        assert_eq!(args.pick.as_deref(), Some("recommended"));

        let execute = Cli::try_parse_from([
            "codexctl",
            "session",
            "execute",
            "--run-id",
            "run-1",
            "--reasoning-effort",
            "high",
        ])
        .unwrap();
        assert!(matches!(
            execute.command,
            Commands::Session(SessionCommand::Execute(_))
        ));

        let read =
            Cli::try_parse_from(["codexctl", "session", "read", "--run-id", "run-1"]).unwrap();
        assert!(matches!(
            read.command,
            Commands::Session(SessionCommand::Read(_))
        ));

        let watch =
            Cli::try_parse_from(["codexctl", "session", "watch", "--run-id", "run-1"]).unwrap();
        assert!(matches!(
            watch.command,
            Commands::Session(SessionCommand::Watch(_))
        ));
    }

    #[test]
    fn parses_view_file() {
        let cli = Cli::try_parse_from([
            "codexctl",
            "view",
            "sample-session.jsonl",
            "--no-open",
            "--out",
            "target/view.html",
        ])
        .unwrap();
        let Commands::View(args) = cli.command else {
            panic!("expected view command");
        };
        assert_eq!(args.file, Some(PathBuf::from("sample-session.jsonl")));
        assert_eq!(args.out, Some(PathBuf::from("target/view.html")));
        assert!(args.viewer_html.is_none());
        assert!(args.no_open);
    }

    #[test]
    fn parses_external_viewer_template() {
        let cli = Cli::try_parse_from([
            "codexctl",
            "view",
            "sample-session.jsonl",
            "--viewer-html",
            "viewer.html",
        ])
        .unwrap();
        let Commands::View(args) = cli.command else {
            panic!("expected view command");
        };
        assert_eq!(args.viewer_html, Some(PathBuf::from("viewer.html")));
    }

    #[test]
    fn parses_view_run_id() {
        let cli =
            Cli::try_parse_from(["codexctl", "view", "--run-id", "run-1", "--no-open"]).unwrap();
        let Commands::View(args) = cli.command else {
            panic!("expected view command");
        };
        assert_eq!(args.run_id.as_deref(), Some("run-1"));
        assert!(args.file.is_none());
    }

    #[test]
    fn parses_unlimited_token_budget() {
        assert_eq!(
            TokenBudget::from_str("unlimited").unwrap(),
            TokenBudget::Unlimited
        );
        assert_eq!(TokenBudget::from_str("0").unwrap(), TokenBudget::Unlimited);
    }

    #[test]
    fn parses_limited_token_budget() {
        assert_eq!(
            TokenBudget::from_str("6000").unwrap().as_protocol_value(),
            Some(6000)
        );
    }

    #[test]
    fn parses_unlimited_runtime_timeout() {
        assert_eq!(
            RunTimeout::from_str("infinite").unwrap(),
            RunTimeout::Unlimited
        );
        assert!(RunTimeout::from_str("0").unwrap().duration().is_none());
    }

    #[test]
    fn parses_limited_runtime_timeout() {
        assert_eq!(
            RunTimeout::from_str("120")
                .unwrap()
                .duration()
                .unwrap()
                .as_secs(),
            120
        );
    }
}
