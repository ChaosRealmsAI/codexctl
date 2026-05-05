use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};

const DEFAULT_TIMEOUT_SECS: u64 = 180;

#[derive(Debug, Parser)]
#[command(
    name = "codex-app",
    version,
    about = "Codex app-server friendly CLI wrapper",
    after_help = "Examples:
  codex-app doctor
  codex-app modes
  codex-app raw collaborationMode/list --params '{}'
  codex-app goal set --objective 'Ship a small CLI'
  codex-app plan --prompt 'Plan only. Ask one question first.' --question-mode auto-recommended
  codex-app plan --prompt-file input.md --full-auto --log-mode summary
  codex-app answer --question first_version_scope --answer 'A 首版只做 doctor/modes (Recommended)'"
)]
struct Cli {
    #[arg(
        long,
        default_value = "codex",
        global = true,
        help = "Codex CLI executable"
    )]
    codex_bin: String,
    #[arg(long, global = true, help = "Fixed log directory")]
    log_dir: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = LogMode::Summary, global = true)]
    log_mode: LogMode,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Check Codex CLI and app-server JSONL connectivity")]
    Doctor,
    #[command(about = "List known app-server method names exposed through raw")]
    Methods,
    #[command(about = "List app-server collaboration modes")]
    Modes,
    #[command(about = "List app-server experimental features")]
    Features,
    #[command(about = "Call any app-server JSON-RPC method")]
    Raw(RawArgs),
    #[command(subcommand, about = "Set, get, or clear a Codex thread goal")]
    Goal(GoalCommand),
    #[command(about = "Run a native Plan-mode turn with optional Goal and question handling")]
    Plan(PlanArgs),
    #[command(about = "Build a request_user_input answer payload")]
    Answer(AnswerArgs),
}

#[derive(Debug, Args)]
struct RawArgs {
    #[arg(help = "app-server method, e.g. thread/start or model/list")]
    method: String,
    #[arg(long, help = "JSON params object; defaults to {}")]
    params: Option<String>,
    #[arg(long, help = "Read JSON params from file")]
    params_file: Option<PathBuf>,
    #[arg(long, help = "Also include notifications seen before the response")]
    include_events: bool,
}

#[derive(Debug, Subcommand)]
enum GoalCommand {
    #[command(about = "Start a thread if needed, then set a goal")]
    Set(GoalSetArgs),
    #[command(about = "Read a goal from an existing thread")]
    Get(ThreadIdArgs),
    #[command(about = "Clear a goal from an existing thread")]
    Clear(ThreadIdArgs),
}

#[derive(Debug, Args)]
struct GoalSetArgs {
    #[arg(long)]
    thread_id: Option<String>,
    #[arg(long)]
    objective: String,
    #[arg(long)]
    token_budget: Option<u64>,
    #[arg(long, value_enum)]
    status: Option<GoalStatus>,
    #[command(flatten)]
    runtime: RuntimeArgs,
}

#[derive(Debug, Args)]
struct ThreadIdArgs {
    #[arg(long)]
    thread_id: String,
}

#[derive(Debug, Args)]
struct PlanArgs {
    #[arg(long, conflicts_with = "prompt_file")]
    prompt: Option<String>,
    #[arg(long)]
    prompt_file: Option<PathBuf>,
    #[arg(long, help = "Set a thread goal before turn/start")]
    objective: Option<String>,
    #[arg(long)]
    token_budget: Option<u64>,
    #[arg(long, value_enum, default_value_t = QuestionMode::AutoRecommended)]
    question_mode: QuestionMode,
    #[arg(long, value_enum, default_value_t = Effort::Medium)]
    effort: Effort,
    #[arg(long, help = "Override model used in collaborationMode settings")]
    model: Option<String>,
    #[arg(long, help = "Write events as JSONL while running")]
    jsonl: bool,
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECS)]
    timeout_secs: u64,
    #[command(flatten)]
    runtime: RuntimeArgs,
}

#[derive(Debug, Args)]
struct RuntimeArgs {
    #[arg(long, help = "Working directory for Codex")]
    cwd: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = SandboxMode::ReadOnly)]
    sandbox: SandboxMode,
    #[arg(long, value_enum, default_value_t = ApprovalPolicy::Never)]
    approval_policy: ApprovalPolicy,
    #[arg(
        long,
        help = "Shortcut for --sandbox danger-full-access --approval-policy never"
    )]
    full_auto: bool,
}

#[derive(Debug, Args)]
struct AnswerArgs {
    #[arg(long, help = "Question id from request_user_input")]
    question: String,
    #[arg(long, help = "Selected option label or free-form answer")]
    answer: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogMode {
    Off,
    Summary,
    Full,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum QuestionMode {
    AutoRecommended,
    AutoFirst,
    Interactive,
    External,
    Fail,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ApprovalPolicy {
    Untrusted,
    OnFailure,
    OnRequest,
    Never,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Effort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GoalStatus {
    Active,
    Paused,
    BudgetLimited,
    Complete,
}

impl SandboxMode {
    fn as_protocol(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

impl ApprovalPolicy {
    fn as_protocol(self) -> &'static str {
        match self {
            Self::Untrusted => "untrusted",
            Self::OnFailure => "on-failure",
            Self::OnRequest => "on-request",
            Self::Never => "never",
        }
    }
}

impl Effort {
    fn as_protocol(self) -> &'static str {
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
    fn as_protocol(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::BudgetLimited => "budgetLimited",
            Self::Complete => "complete",
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Doctor => print_json(run_doctor(&cli.codex_bin, cli.log_dir, cli.log_mode)?),
        Commands::Methods => print_json(json!({ "ok": true, "methods": KNOWN_METHODS })),
        Commands::Modes => {
            let mut server = initialized_server(&cli.codex_bin, cli.log_dir, cli.log_mode)?;
            print_json(server.call("collaborationMode/list", json!({}), false)?)
        }
        Commands::Features => {
            let mut server = initialized_server(&cli.codex_bin, cli.log_dir, cli.log_mode)?;
            print_json(server.call("experimentalFeature/list", json!({}), false)?)
        }
        Commands::Raw(args) => {
            let params = read_params(args.params, args.params_file)?;
            let mut server = initialized_server(&cli.codex_bin, cli.log_dir, cli.log_mode)?;
            print_json(server.call(&args.method, params, args.include_events)?)
        }
        Commands::Goal(command) => {
            let mut server = initialized_server(&cli.codex_bin, cli.log_dir, cli.log_mode)?;
            print_json(run_goal(&mut server, command)?)
        }
        Commands::Plan(args) => {
            let mut server = initialized_server(&cli.codex_bin, cli.log_dir, cli.log_mode)?;
            let result = run_plan(&mut server, args)?;
            print_json(result)
        }
        Commands::Answer(args) => print_json(build_answer(args)?),
    }
}

fn run_doctor(codex_bin: &str, log_dir: Option<PathBuf>, log_mode: LogMode) -> Result<Value> {
    let version = Command::new(codex_bin)
        .arg("--version")
        .output()
        .with_context(|| format!("failed to execute {codex_bin} --version"))?;
    let version_json = json!({
        "ok": version.status.success(),
        "status": version.status.code(),
        "stdout": String::from_utf8_lossy(&version.stdout).trim(),
        "stderr": String::from_utf8_lossy(&version.stderr).trim(),
    });
    let mut server = initialized_server(codex_bin, log_dir, log_mode)?;
    let modes = server.call("collaborationMode/list", json!({}), false)?;
    Ok(json!({
        "ok": version.status.success() && modes.get("error").is_none(),
        "kind": "codex-app-cli-doctor",
        "codex": version_json,
        "app_server": {
            "initialized": true,
            "codex_home": server.codex_home,
            "modes": modes.get("result").cloned().unwrap_or(modes),
        },
        "log_path": server.log_path(),
        "latest_log_path": server.latest_log_path(),
    }))
}

fn initialized_server(
    codex_bin: &str,
    log_dir: Option<PathBuf>,
    log_mode: LogMode,
) -> Result<AppServer> {
    let mut server = AppServer::spawn(codex_bin, log_dir, log_mode)?;
    server.initialize()?;
    Ok(server)
}

fn run_goal(server: &mut AppServer, command: GoalCommand) -> Result<Value> {
    match command {
        GoalCommand::Set(args) => {
            let runtime = args.runtime.effective()?;
            let thread_id = match args.thread_id {
                Some(thread_id) => thread_id,
                None => start_thread(server, &runtime, None)?,
            };
            let mut params = json!({
                "threadId": thread_id,
                "objective": args.objective,
            });
            if let Some(token_budget) = args.token_budget {
                params["tokenBudget"] = json!(token_budget);
            }
            if let Some(status) = args.status {
                params["status"] = json!(status.as_protocol());
            }
            let set = server.call("thread/goal/set", params, false)?;
            let get = server.call("thread/goal/get", json!({ "threadId": thread_id }), false)?;
            Ok(json!({
                "ok": set.get("error").is_none() && get.get("error").is_none(),
                "thread_id": thread_id,
                "thread_path": server.last_thread_path,
                "codex_home": server.codex_home,
                "resume_command": resume_command(server.codex_home.as_deref(), &thread_id),
                "set": response_payload(set),
                "get": response_payload(get),
                "log_path": server.log_path(),
            }))
        }
        GoalCommand::Get(args) => server.call(
            "thread/goal/get",
            json!({ "threadId": args.thread_id }),
            false,
        ),
        GoalCommand::Clear(args) => server.call(
            "thread/goal/clear",
            json!({ "threadId": args.thread_id }),
            false,
        ),
    }
}

fn run_plan(server: &mut AppServer, args: PlanArgs) -> Result<Value> {
    let prompt = read_prompt(args.prompt, args.prompt_file)?;
    let runtime = args.runtime.effective()?;
    let thread_id = start_thread(server, &runtime, args.model.as_deref())?;
    let thread_model = server
        .last_thread_model
        .clone()
        .or(args.model.clone())
        .unwrap_or_else(|| "gpt-5.5".to_string());

    let goal_result = if let Some(objective) = args.objective {
        let mut params = json!({
            "threadId": thread_id,
            "objective": objective,
            "status": "active",
        });
        if let Some(token_budget) = args.token_budget {
            params["tokenBudget"] = json!(token_budget);
        }
        Some(response_payload(server.call(
            "thread/goal/set",
            params,
            false,
        )?))
    } else {
        None
    };

    let turn_id_request = server.send_request(
        "turn/start",
        json!({
            "threadId": thread_id,
            "input": [{"type": "text", "text": prompt, "text_elements": []}],
            "approvalPolicy": runtime.approval_policy,
            "collaborationMode": {
                "mode": "plan",
                "settings": {
                    "model": thread_model,
                    "reasoning_effort": args.effort.as_protocol(),
                    "developer_instructions": Value::Null,
                }
            }
        }),
    )?;

    let mut result = PlanRun::new(
        &thread_id,
        goal_result,
        server.log_path(),
        server.codex_home.clone(),
        server.last_thread_path.clone(),
    );
    let timeout = Duration::from_secs(args.timeout_secs);
    loop {
        let message = server.recv(timeout)?;
        if args.jsonl {
            print_json_line(&json!({ "type": "event", "event": message }));
        }
        if is_response_id(&message, turn_id_request) {
            if let Some(error) = message.get("error") {
                result.status = "failed".to_string();
                result.errors.push(error.clone());
                break;
            }
            if let Some(turn_id) = message
                .pointer("/result/turn/id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
            {
                result.turn_id = Some(turn_id);
            }
            continue;
        }
        if message.get("method").and_then(Value::as_str) == Some("item/tool/requestUserInput") {
            let request_id = message.get("id").cloned().unwrap_or(Value::Null);
            let questions = message
                .pointer("/params/questions")
                .cloned()
                .unwrap_or_else(|| json!([]));
            result.questions.push(json!({
                "request_id": request_id,
                "questions": questions,
            }));
            match answer_questions(args.question_mode, &message)? {
                QuestionAnswer::Answered(answer) => {
                    result.answers.push(answer.clone());
                    server.send_response(request_id, answer)?;
                    continue;
                }
                QuestionAnswer::NeedsInput(payload) => {
                    result.status = "needs_input".to_string();
                    result.needs_input = Some(payload);
                    break;
                }
            }
        }
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            match method {
                "item/completed" => collect_item(&mut result, &message),
                "item/agentMessage/delta" => collect_delta(&mut result, &message),
                "thread/tokenUsage/updated" => result.usage = Some(message["params"].clone()),
                "turn/completed" => {
                    result.status = "completed".to_string();
                    result.completed = Some(message["params"].clone());
                    break;
                }
                "error" | "warning" | "guardianWarning" => {
                    result.warnings.push(message.clone());
                }
                _ => {}
            }
        }
    }
    Ok(result.into_json())
}

fn collect_item(result: &mut PlanRun, message: &Value) {
    let item = &message["params"]["item"];
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
    match item_type {
        "plan" => {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                result.plans.push(text.to_string());
            }
        }
        "agentMessage" | "agent_message" => {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                result.agent_messages.push(text.to_string());
                if let Some(plan) = extract_proposed_plan(text) {
                    result.plans.push(plan);
                }
            }
        }
        _ => result.items.push(item.clone()),
    }
}

fn collect_delta(result: &mut PlanRun, message: &Value) {
    if let Some(delta) = message.pointer("/params/delta").and_then(Value::as_str) {
        result.agent_deltas.push(delta.to_string());
    }
}

fn answer_questions(mode: QuestionMode, message: &Value) -> Result<QuestionAnswer> {
    let request_id = message.get("id").cloned().unwrap_or(Value::Null);
    let questions = message
        .pointer("/params/questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    match mode {
        QuestionMode::Fail => Ok(QuestionAnswer::NeedsInput(json!({
            "request_id": request_id,
            "params": message.get("params").cloned().unwrap_or_else(|| json!({})),
            "answer_format": {"answers": {"<question_id>": {"answers": ["<selected option label>"]}}},
        }))),
        QuestionMode::External => {
            print_json_line(&json!({
                "type": "needs_input",
                "request_id": request_id,
                "params": message.get("params").cloned().unwrap_or_else(|| json!({})),
                "answer_format": {"answers": {"<question_id>": {"answers": ["<selected option label>"]}}},
            }));
            let mut line = String::new();
            io::stdin()
                .read_line(&mut line)
                .context("failed to read external answer JSON from stdin")?;
            let answer: Value = serde_json::from_str(line.trim()).context("invalid answer JSON")?;
            let result = answer
                .get("result")
                .cloned()
                .or_else(|| {
                    answer
                        .get("answers")
                        .cloned()
                        .map(|answers| json!({ "answers": answers }))
                })
                .ok_or_else(|| anyhow!("answer JSON must contain result or answers"))?;
            Ok(QuestionAnswer::Answered(result))
        }
        QuestionMode::AutoRecommended | QuestionMode::AutoFirst | QuestionMode::Interactive => {
            let mut answers = BTreeMap::new();
            for question in questions {
                let id = question
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("request_user_input question missing id"))?;
                let selected = match mode {
                    QuestionMode::AutoRecommended => select_recommended(&question)
                        .or_else(|| select_first(&question))
                        .ok_or_else(|| anyhow!("question {id} has no selectable option"))?,
                    QuestionMode::AutoFirst => select_first(&question)
                        .ok_or_else(|| anyhow!("question {id} has no selectable option"))?,
                    QuestionMode::Interactive => select_interactive(&question)?,
                    _ => unreachable!(),
                };
                answers.insert(id.to_string(), json!({ "answers": [selected] }));
            }
            Ok(QuestionAnswer::Answered(json!({ "answers": answers })))
        }
    }
}

fn select_recommended(question: &Value) -> Option<String> {
    options(question)
        .into_iter()
        .find(|label| label.to_ascii_lowercase().contains("(recommended)"))
}

fn select_first(question: &Value) -> Option<String> {
    options(question).into_iter().next()
}

fn select_interactive(question: &Value) -> Result<String> {
    let header = question.get("header").and_then(Value::as_str).unwrap_or("");
    let text = question
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("Choose an option");
    eprintln!("{header}: {text}");
    let labels = options(question);
    for (index, label) in labels.iter().enumerate() {
        let description = question
            .get("options")
            .and_then(Value::as_array)
            .and_then(|items| items.get(index))
            .and_then(|item| item.get("description"))
            .and_then(Value::as_str)
            .unwrap_or("");
        eprintln!("  {}. {} - {}", index + 1, label, description);
    }
    eprint!("Select number or type answer: ");
    io::stderr().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    if let Ok(index) = input.parse::<usize>()
        && let Some(label) = labels.get(index.saturating_sub(1))
    {
        return Ok(label.clone());
    }
    if input.is_empty() {
        bail!("empty interactive answer");
    }
    Ok(input.to_string())
}

fn options(question: &Value) -> Vec<String> {
    question
        .get("options")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("label").and_then(Value::as_str))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn build_answer(args: AnswerArgs) -> Result<Value> {
    if args.answer.is_empty() {
        bail!("answer requires at least one --answer value");
    }
    Ok(json!({
        "answers": {
            args.question: {
                "answers": args.answer
            }
        }
    }))
}

fn start_thread(
    server: &mut AppServer,
    runtime: &EffectiveRuntime,
    model: Option<&str>,
) -> Result<String> {
    let mut params = json!({
        "cwd": runtime.cwd,
        "approvalPolicy": runtime.approval_policy,
        "sandbox": runtime.sandbox,
        "experimentalRawEvents": false,
        "persistExtendedHistory": true,
    });
    if let Some(model) = model {
        params["model"] = json!(model);
    }
    let response = server.call("thread/start", params, false)?;
    if let Some(error) = response.get("error") {
        bail!("thread/start failed: {error}");
    }
    let thread_id = response
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("thread/start response missing result.thread.id"))?
        .to_string();
    server.last_thread_model = response
        .pointer("/result/model")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    server.last_thread_path = response
        .pointer("/result/thread/path")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(thread_id)
}

impl RuntimeArgs {
    fn effective(self) -> Result<EffectiveRuntime> {
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
struct EffectiveRuntime {
    cwd: String,
    sandbox: String,
    approval_policy: String,
}

enum QuestionAnswer {
    Answered(Value),
    NeedsInput(Value),
}

struct PlanRun {
    status: String,
    thread_id: String,
    turn_id: Option<String>,
    codex_home: Option<String>,
    thread_path: Option<String>,
    goal: Option<Value>,
    plans: Vec<String>,
    agent_messages: Vec<String>,
    agent_deltas: Vec<String>,
    questions: Vec<Value>,
    answers: Vec<Value>,
    needs_input: Option<Value>,
    usage: Option<Value>,
    completed: Option<Value>,
    warnings: Vec<Value>,
    errors: Vec<Value>,
    items: Vec<Value>,
    log_path: Option<PathBuf>,
}

impl PlanRun {
    fn new(
        thread_id: &str,
        goal: Option<Value>,
        log_path: Option<PathBuf>,
        codex_home: Option<String>,
        thread_path: Option<String>,
    ) -> Self {
        Self {
            status: "running".to_string(),
            thread_id: thread_id.to_string(),
            turn_id: None,
            codex_home,
            thread_path,
            goal,
            plans: Vec::new(),
            agent_messages: Vec::new(),
            agent_deltas: Vec::new(),
            questions: Vec::new(),
            answers: Vec::new(),
            needs_input: None,
            usage: None,
            completed: None,
            warnings: Vec::new(),
            errors: Vec::new(),
            items: Vec::new(),
            log_path,
        }
    }

    fn into_json(self) -> Value {
        json!({
            "ok": self.status == "completed",
            "status": self.status,
            "thread_id": self.thread_id,
            "turn_id": self.turn_id,
            "codex_home": self.codex_home,
            "thread_path": self.thread_path,
            "resume_command": resume_command(self.codex_home.as_deref(), &self.thread_id),
            "goal": self.goal,
            "plans": self.plans,
            "agent_messages": self.agent_messages,
            "questions": self.questions,
            "answers": self.answers,
            "needs_input": self.needs_input,
            "usage": self.usage,
            "completed": self.completed,
            "warnings": self.warnings,
            "errors": self.errors,
            "items": self.items,
            "log_path": self.log_path,
        })
    }
}

struct AppServer {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    rx: Receiver<ServerLine>,
    next_id: u64,
    logger: RunLogger,
    codex_home: Option<String>,
    last_thread_model: Option<String>,
    last_thread_path: Option<String>,
}

impl AppServer {
    fn spawn(codex_bin: &str, log_dir: Option<PathBuf>, log_mode: LogMode) -> Result<Self> {
        let mut child = Command::new(codex_bin)
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn {codex_bin} app-server"))?;
        let stdin = child.stdin.take().context("app-server stdin missing")?;
        let stdout = child.stdout.take().context("app-server stdout missing")?;
        let stderr = child.stderr.take().context("app-server stderr missing")?;
        let (tx, rx) = mpsc::channel();
        let tx_stdout = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_stdout.send(ServerLine::Stdout(line));
            }
        });
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx.send(ServerLine::Stderr(line));
            }
        });
        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            rx,
            next_id: 1,
            logger: RunLogger::new(log_dir, log_mode)?,
            codex_home: None,
            last_thread_model: None,
            last_thread_path: None,
        })
    }

    fn log_path(&self) -> Option<PathBuf> {
        self.logger.path.clone()
    }

    fn latest_log_path(&self) -> Option<PathBuf> {
        self.logger.latest_path.clone()
    }

    fn initialize(&mut self) -> Result<()> {
        let id = self.send_request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "codex-app-cli",
                    "title": "codex-app CLI",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": {
                    "experimentalApi": true,
                    "optOutNotificationMethods": ["fs/changed"],
                },
            }),
        )?;
        let response = self.wait_response(id, Duration::from_secs(20))?;
        if let Some(error) = response.get("error") {
            bail!("initialize failed: {error}");
        }
        self.codex_home = response
            .pointer("/result/codexHome")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        self.send_notification("initialized", None)?;
        Ok(())
    }

    fn call(&mut self, method: &str, params: Value, include_events: bool) -> Result<Value> {
        let id = self.send_request(method, params)?;
        let mut events = Vec::new();
        loop {
            let message = self.recv(Duration::from_secs(DEFAULT_TIMEOUT_SECS))?;
            if is_response_id(&message, id) {
                return if include_events {
                    Ok(
                        json!({ "response": message, "events": events, "log_path": self.log_path() }),
                    )
                } else {
                    Ok(message)
                };
            }
            events.push(message);
        }
    }

    fn send_request(&mut self, method: &str, params: Value) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_value(json!({ "id": id, "method": method, "params": params }))?;
        Ok(id)
    }

    fn send_response(&mut self, id: Value, result: Value) -> Result<()> {
        self.send_value(json!({ "id": id, "result": result }))
    }

    fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        let value = match params {
            Some(params) => json!({ "method": method, "params": params }),
            None => json!({ "method": method }),
        };
        self.send_value(value)
    }

    fn send_value(&mut self, value: Value) -> Result<()> {
        self.logger.log("out", &value)?;
        writeln!(self.stdin, "{value}")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn wait_response(&mut self, id: u64, timeout: Duration) -> Result<Value> {
        loop {
            let message = self.recv(timeout)?;
            if is_response_id(&message, id) {
                return Ok(message);
            }
        }
    }

    fn recv(&mut self, timeout: Duration) -> Result<Value> {
        loop {
            let line = self
                .rx
                .recv_timeout(timeout)
                .map_err(|_| anyhow!("timed out waiting for app-server event after {timeout:?}"))?;
            match line {
                ServerLine::Stdout(line) => {
                    let value: Value = serde_json::from_str(&line)
                        .with_context(|| format!("app-server returned invalid JSON: {line}"))?;
                    self.logger.log("in", &value)?;
                    return Ok(value);
                }
                ServerLine::Stderr(line) => {
                    self.logger.log("stderr", &json!({ "line": line }))?;
                }
            }
        }
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

enum ServerLine {
    Stdout(String),
    Stderr(String),
}

struct RunLogger {
    mode: LogMode,
    path: Option<PathBuf>,
    latest_path: Option<PathBuf>,
    run_file: Option<File>,
    latest_file: Option<File>,
}

impl RunLogger {
    fn new(log_dir: Option<PathBuf>, mode: LogMode) -> Result<Self> {
        if matches!(mode, LogMode::Off) {
            return Ok(Self {
                mode,
                path: None,
                latest_path: None,
                run_file: None,
                latest_file: None,
            });
        }
        let dir = log_dir.unwrap_or_else(default_log_dir);
        fs::create_dir_all(&dir).with_context(|| format!("create log dir {}", dir.display()))?;
        let path = dir.join(format!("run-{}.jsonl", now_ms()));
        let latest_path = dir.join("latest.jsonl");
        let run_file =
            File::create(&path).with_context(|| format!("create log {}", path.display()))?;
        let latest_file = File::create(&latest_path)
            .with_context(|| format!("create log {}", latest_path.display()))?;
        Ok(Self {
            mode,
            path: Some(path),
            latest_path: Some(latest_path),
            run_file: Some(run_file),
            latest_file: Some(latest_file),
        })
    }

    fn log(&mut self, direction: &str, value: &Value) -> Result<()> {
        if matches!(self.mode, LogMode::Off) {
            return Ok(());
        }
        let payload = match self.mode {
            LogMode::Full => json!({
                "ts_ms": now_ms(),
                "direction": direction,
                "message": value,
            }),
            LogMode::Summary => json!({
                "ts_ms": now_ms(),
                "direction": direction,
                "summary": summarize(value),
            }),
            LogMode::Off => unreachable!(),
        };
        let line = format!("{payload}\n");
        if let Some(file) = &mut self.run_file {
            file.write_all(line.as_bytes())?;
            file.flush()?;
        }
        if let Some(file) = &mut self.latest_file {
            file.write_all(line.as_bytes())?;
            file.flush()?;
        }
        Ok(())
    }
}

fn summarize(value: &Value) -> Value {
    if let Some(method) = value.get("method").and_then(Value::as_str) {
        if method == "item/tool/requestUserInput" {
            return json!({
                "kind": "request_user_input",
                "id": value.get("id"),
                "thread_id": value.pointer("/params/threadId"),
                "turn_id": value.pointer("/params/turnId"),
                "questions": value.pointer("/params/questions").and_then(Value::as_array).map(|items| items.len()).unwrap_or(0),
            });
        }
        return json!({
            "kind": "method",
            "id": value.get("id"),
            "method": method,
        });
    }
    if value.get("result").is_some() {
        return json!({
            "kind": "response",
            "id": value.get("id"),
            "result_keys": value.get("result").and_then(Value::as_object).map(|obj| obj.keys().cloned().collect::<Vec<_>>()),
        });
    }
    if value.get("error").is_some() {
        return json!({
            "kind": "error",
            "id": value.get("id"),
            "error": value.get("error"),
        });
    }
    value.clone()
}

fn default_log_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex-app-cli")
        .join("logs")
}

fn read_params(params: Option<String>, params_file: Option<PathBuf>) -> Result<Value> {
    let raw = match (params, params_file) {
        (Some(_), Some(_)) => bail!("use either --params or --params-file, not both"),
        (Some(text), None) => text,
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("read params file {}", path.display()))?,
        (None, None) => "{}".to_string(),
    };
    serde_json::from_str(&raw).with_context(|| format!("invalid JSON params: {raw}"))
}

fn read_prompt(prompt: Option<String>, prompt_file: Option<PathBuf>) -> Result<String> {
    match (prompt, prompt_file) {
        (Some(_), Some(_)) => bail!("use either --prompt or --prompt-file, not both"),
        (Some(text), None) => Ok(text),
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("read prompt file {}", path.display())),
        (None, None) => bail!("plan requires --prompt or --prompt-file"),
    }
}

fn response_payload(value: Value) -> Value {
    value.get("result").cloned().unwrap_or(value)
}

fn resume_command(codex_home: Option<&str>, thread_id: &str) -> String {
    match codex_home {
        Some(home) => format!(
            "CODEX_HOME={} codex resume --include-non-interactive {}",
            shell_quote(home),
            shell_quote(thread_id)
        ),
        None => format!(
            "codex resume --include-non-interactive {}",
            shell_quote(thread_id)
        ),
    }
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_response_id(value: &Value, id: u64) -> bool {
    value.get("id").and_then(Value::as_u64) == Some(id)
}

fn extract_proposed_plan(text: &str) -> Option<String> {
    let start_tag = "<proposed_plan>";
    let end_tag = "</proposed_plan>";
    let start = text.find(start_tag)? + start_tag.len();
    let end = text[start..].find(end_tag)? + start;
    Some(text[start..end].trim().to_string())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn print_json(value: Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn print_json_line(value: &Value) {
    println!("{value}");
}

static KNOWN_METHODS: &[&str] = &[
    "account/login/cancel",
    "account/login/start",
    "account/logout",
    "account/rateLimits/read",
    "account/read",
    "account/sendAddCreditsNudgeEmail",
    "app/list",
    "collaborationMode/list",
    "command/exec",
    "command/exec/resize",
    "command/exec/terminate",
    "command/exec/write",
    "config/batchWrite",
    "config/mcpServer/reload",
    "config/read",
    "config/value/write",
    "configRequirements/read",
    "device/key/create",
    "device/key/public",
    "device/key/sign",
    "experimentalFeature/enablement/set",
    "experimentalFeature/list",
    "externalAgentConfig/detect",
    "externalAgentConfig/import",
    "feedback/upload",
    "fs/copy",
    "fs/createDirectory",
    "fs/getMetadata",
    "fs/readDirectory",
    "fs/readFile",
    "fs/remove",
    "fs/unwatch",
    "fs/watch",
    "fs/writeFile",
    "fuzzyFileSearch",
    "fuzzyFileSearch/sessionStart",
    "fuzzyFileSearch/sessionStop",
    "fuzzyFileSearch/sessionUpdate",
    "getAuthStatus",
    "getConversationSummary",
    "gitDiffToRemote",
    "hooks/list",
    "initialize",
    "marketplace/add",
    "marketplace/remove",
    "marketplace/upgrade",
    "mcpServer/oauth/login",
    "mcpServer/resource/read",
    "mcpServer/tool/call",
    "mcpServerStatus/list",
    "memory/reset",
    "mock/experimentalMethod",
    "model/list",
    "modelProvider/capabilities/read",
    "plugin/install",
    "plugin/list",
    "plugin/read",
    "plugin/uninstall",
    "review/start",
    "skills/config/write",
    "skills/list",
    "thread/approveGuardianDeniedAction",
    "thread/archive",
    "thread/backgroundTerminals/clean",
    "thread/compact/start",
    "thread/decrement_elicitation",
    "thread/fork",
    "thread/goal/clear",
    "thread/goal/get",
    "thread/goal/set",
    "thread/increment_elicitation",
    "thread/inject_items",
    "thread/list",
    "thread/loaded/list",
    "thread/memoryMode/set",
    "thread/metadata/update",
    "thread/name/set",
    "thread/read",
    "thread/realtime/appendAudio",
    "thread/realtime/appendText",
    "thread/realtime/listVoices",
    "thread/realtime/start",
    "thread/realtime/stop",
    "thread/resume",
    "thread/rollback",
    "thread/shellCommand",
    "thread/start",
    "thread/turns/list",
    "thread/unarchive",
    "thread/unsubscribe",
    "turn/interrupt",
    "turn/start",
    "turn/steer",
    "windowsSandbox/setupStart",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_proposed_plan() {
        let text = "x<proposed_plan>\n1. A\n2. B\n</proposed_plan>y";
        assert_eq!(extract_proposed_plan(text).unwrap(), "1. A\n2. B");
    }

    #[test]
    fn selects_recommended_option() {
        let question = json!({
            "id": "q",
            "options": [
                {"label": "A", "description": ""},
                {"label": "B (Recommended)", "description": ""}
            ]
        });
        assert_eq!(select_recommended(&question).unwrap(), "B (Recommended)");
    }

    #[test]
    fn builds_answer_payload() {
        let payload = build_answer(AnswerArgs {
            question: "scope".to_string(),
            answer: vec!["A".to_string()],
        })
        .unwrap();
        assert_eq!(payload["answers"]["scope"]["answers"][0], "A");
    }

    #[test]
    fn summary_detects_user_input_request() {
        let value = json!({
            "id": 0,
            "method": "item/tool/requestUserInput",
            "params": {"threadId": "t", "turnId": "u", "questions": [{ "id": "q" }]}
        });
        let summary = summarize(&value);
        assert_eq!(summary["kind"], "request_user_input");
        assert_eq!(summary["questions"], 1);
    }
}
