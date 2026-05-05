use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value, json};

use crate::app_server::AppServer;
use crate::cli::{
    DaemonCommand, EffectiveRuntime, LogMode, SessionAnswerArgs, SessionCommand, SessionRunIdArgs,
    SessionSendArgs, SessionStartArgs,
};
use crate::commands::start_thread;
use crate::util::{
    extract_proposed_plan, is_response_id, read_params, read_prompt, resume_command,
};

pub(crate) fn default_socket_path() -> PathBuf {
    let user = std::env::var("USER").unwrap_or_else(|_| "default".to_string());
    std::env::temp_dir().join(format!("codex-app-{user}.sock"))
}

pub(crate) fn run_daemon_command(socket_path: PathBuf, command: DaemonCommand) -> Result<Value> {
    match command {
        DaemonCommand::Start => ensure_daemon(&socket_path),
        DaemonCommand::Status => {
            match send_request(&socket_path, json!({ "type": "daemon_status" })) {
                Ok(response) => Ok(response),
                Err(error) => Ok(json!({
                    "ok": false,
                    "status": "not_running",
                    "socket_path": socket_path,
                    "error": error.to_string(),
                })),
            }
        }
        DaemonCommand::Stop => send_request(&socket_path, json!({ "type": "daemon_stop" })),
        DaemonCommand::Serve => serve(socket_path),
    }
}

pub(crate) fn run_session_command(
    socket_path: PathBuf,
    codex_bin: String,
    codex_home: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    log_mode: LogMode,
    command: SessionCommand,
) -> Result<Value> {
    ensure_daemon(&socket_path)?;
    let config = json!({
        "codexBin": codex_bin,
        "codexHome": codex_home,
        "logDir": log_dir,
        "logMode": log_mode_name(log_mode),
    });
    let request = match command {
        SessionCommand::Start(args) => session_start_request(config, args)?,
        SessionCommand::Answer(args) => session_answer_request(args)?,
        SessionCommand::Send(args) => session_send_request(args)?,
        SessionCommand::Read(args) => run_id_request("session_read", args),
        SessionCommand::Stop(args) => run_id_request("session_stop", args),
    };
    send_request(&socket_path, request)
}

fn session_start_request(config: Value, args: SessionStartArgs) -> Result<Value> {
    let prompt = read_prompt(args.prompt, args.prompt_file)?;
    let runtime = args.runtime.effective()?;
    Ok(json!({
        "type": "session_start",
        "config": config,
        "prompt": prompt,
        "objective": args.objective,
        "tokenBudget": args.token_budget.as_protocol_value(),
        "effort": args.effort.as_protocol(),
        "model": args.model,
        "timeoutSecs": args.timeout.duration().map(|value| value.as_secs()),
        "runtime": {
            "cwd": runtime.cwd,
            "sandbox": runtime.sandbox,
            "approvalPolicy": runtime.approval_policy,
        }
    }))
}

fn session_answer_request(args: SessionAnswerArgs) -> Result<Value> {
    Ok(json!({
        "type": "session_answer",
        "runId": args.run_id,
        "answer": build_session_answer(args.answers, args.answers_json, args.answers_file)?,
        "timeoutSecs": args.timeout.duration().map(|value| value.as_secs()),
    }))
}

fn session_send_request(args: SessionSendArgs) -> Result<Value> {
    let prompt = read_prompt(args.prompt, args.prompt_file)?;
    Ok(json!({
        "type": "session_send",
        "runId": args.run_id,
        "prompt": prompt,
        "effort": args.effort.as_protocol(),
        "model": args.model,
        "timeoutSecs": args.timeout.duration().map(|value| value.as_secs()),
    }))
}

fn run_id_request(kind: &str, args: SessionRunIdArgs) -> Value {
    json!({
        "type": kind,
        "runId": args.run_id,
    })
}

fn build_session_answer(
    pairs: Vec<String>,
    answers_json: Option<String>,
    answers_file: Option<PathBuf>,
) -> Result<Value> {
    if answers_json.is_some() || answers_file.is_some() {
        if !pairs.is_empty() {
            bail!("use either --answer or --answers-json/--answers-file, not both");
        }
        let value = read_params(answers_json, answers_file)?;
        return if value.get("answers").is_some() {
            Ok(value)
        } else {
            Ok(json!({ "answers": value }))
        };
    }
    if pairs.is_empty() {
        bail!("session answer requires at least one --answer QUESTION_ID=VALUE");
    }
    let mut answers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pair in pairs {
        let Some((question_id, answer)) = pair.split_once('=') else {
            bail!("answer must be QUESTION_ID=VALUE, got {pair}");
        };
        if question_id.trim().is_empty() || answer.trim().is_empty() {
            bail!("answer must be QUESTION_ID=VALUE, got {pair}");
        }
        answers
            .entry(question_id.trim().to_string())
            .or_default()
            .push(answer.trim().to_string());
    }
    let mut object = Map::new();
    for (question_id, answers) in answers {
        object.insert(question_id, json!({ "answers": answers }));
    }
    Ok(json!({ "answers": object }))
}

fn ensure_daemon(socket_path: &Path) -> Result<Value> {
    if let Ok(response) = send_request(socket_path, json!({ "type": "daemon_status" })) {
        return Ok(response);
    }
    if socket_path.exists() {
        let _ = fs::remove_file(socket_path);
    }
    let exe = std::env::current_exe().context("read current executable path")?;
    let command = format!(
        "nohup {} --session-socket {} daemon serve >/dev/null 2>&1 &",
        shell_quote(&exe.display().to_string()),
        shell_quote(&socket_path.display().to_string())
    );
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn codex-app daemon")?;
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if let Ok(response) = send_request(socket_path, json!({ "type": "daemon_status" })) {
            return Ok(json!({
                "ok": true,
                "status": "running",
                "started": true,
                "socket_path": socket_path,
                "daemon": response,
            }));
        }
    }
    bail!("daemon did not start at {}", socket_path.display())
}

fn send_request(socket_path: &Path, request: Value) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connect daemon socket {}", socket_path.display()))?;
    writeln!(stream, "{request}")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.trim().is_empty() {
        bail!("daemon returned empty response");
    }
    serde_json::from_str(line.trim()).context("daemon returned invalid JSON")
}

fn serve(socket_path: PathBuf) -> Result<Value> {
    if socket_path.exists() {
        fs::remove_file(&socket_path)
            .with_context(|| format!("remove stale socket {}", socket_path.display()))?;
    }
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("bind daemon socket {}", socket_path.display()))?;
    let mut daemon = SessionDaemon::default();
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let should_stop = daemon.handle_stream(stream).unwrap_or_default();
        if should_stop {
            break;
        }
    }
    let _ = fs::remove_file(&socket_path);
    Ok(json!({ "ok": true, "status": "stopped" }))
}

#[derive(Default)]
struct SessionDaemon {
    runs: HashMap<String, RunState>,
}

impl SessionDaemon {
    fn handle_stream(&mut self, mut stream: UnixStream) -> Result<bool> {
        let mut line = String::new();
        {
            let mut reader = BufReader::new(&mut stream);
            reader.read_line(&mut line)?;
        }
        let outcome = serde_json::from_str(line.trim())
            .context("invalid daemon request")
            .and_then(|request| self.dispatch(request));
        let (response, should_stop) = match outcome {
            Ok(outcome) => outcome,
            Err(error) => (
                json!({
                    "ok": false,
                    "status": "failed",
                    "error": error.to_string(),
                }),
                false,
            ),
        };
        writeln!(stream, "{response}")?;
        stream.flush()?;
        Ok(should_stop)
    }

    fn dispatch(&mut self, request: Value) -> Result<(Value, bool)> {
        Ok(match request.get("type").and_then(Value::as_str) {
            Some("daemon_status") => (
                json!({
                    "ok": true,
                    "status": "running",
                    "runs": self.runs.keys().cloned().collect::<Vec<_>>(),
                }),
                false,
            ),
            Some("daemon_stop") => (
                json!({
                    "ok": true,
                    "status": "stopping",
                    "stopped_runs": self.runs.len(),
                }),
                true,
            ),
            Some("session_start") => (self.handle_start(request)?, false),
            Some("session_answer") => (self.handle_answer(request)?, false),
            Some("session_send") => (self.handle_send(request)?, false),
            Some("session_read") => (self.handle_read(request)?, false),
            Some("session_stop") => (self.handle_stop(request)?, false),
            other => (
                json!({
                    "ok": false,
                    "status": "failed",
                    "error": format!("unknown daemon request type: {other:?}"),
                }),
                false,
            ),
        })
    }

    fn handle_start(&mut self, request: Value) -> Result<Value> {
        let config = &request["config"];
        let codex_bin = string_at(config, "codexBin")?;
        let codex_home = path_at(config, "codexHome");
        let log_dir = path_at(config, "logDir");
        let log_mode = parse_log_mode(string_at(config, "logMode")?)?;
        let runtime = EffectiveRuntime {
            cwd: string_at(&request["runtime"], "cwd")?,
            sandbox: string_at(&request["runtime"], "sandbox")?,
            approval_policy: string_at(&request["runtime"], "approvalPolicy")?,
        };
        let prompt = string_at(&request, "prompt")?;
        let timeout = timeout_at(&request);
        let model = request
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let mut server = AppServer::spawn(&codex_bin, codex_home, log_dir, log_mode)?;
        server.initialize()?;
        let thread_id = start_thread(&mut server, &runtime, model.as_deref())?;
        let thread_model = server
            .last_thread_model
            .clone()
            .or(model)
            .unwrap_or_else(|| "gpt-5.5".to_string());
        let goal = if let Some(objective) = request.get("objective").and_then(Value::as_str) {
            let mut params = json!({
                "threadId": thread_id,
                "objective": objective,
                "status": "active",
            });
            if let Some(token_budget) = request.get("tokenBudget").and_then(Value::as_u64) {
                params["tokenBudget"] = json!(token_budget);
            }
            Some(server.call("thread/goal/set", params, false)?["result"].clone())
        } else {
            None
        };
        let run_id = new_run_id();
        let log_path = server.log_path();
        let thread_path = server.last_thread_path.clone();
        let codex_home = server.codex_home.clone();
        let mut state = RunState {
            server,
            run_id: run_id.clone(),
            thread_id,
            thread_model,
            approval_policy: runtime.approval_policy,
            codex_home,
            thread_path,
            log_path,
            goal,
            status: "running".to_string(),
            turn_id: None,
            current_turn_request_id: None,
            pending_request_id: None,
            questions: Vec::new(),
            answers: Vec::new(),
            plans: Vec::new(),
            agent_messages: Vec::new(),
            agent_deltas: Vec::new(),
            usage: None,
            completed: None,
            warnings: Vec::new(),
            errors: Vec::new(),
            items: Vec::new(),
        };
        let request_id = state.server.send_request(
            "turn/start",
            json!({
                "threadId": state.thread_id,
                "input": [{"type": "text", "text": prompt, "text_elements": []}],
                "approvalPolicy": state.approval_policy,
                "collaborationMode": {
                    "mode": "plan",
                    "settings": {
                        "model": state.thread_model,
                        "reasoning_effort": string_at(&request, "effort")?,
                        "developer_instructions": Value::Null,
                    }
                }
            }),
        )?;
        state.pump_until_pause(request_id, timeout)?;
        let response = state.snapshot();
        self.runs.insert(run_id, state);
        Ok(response)
    }

    fn handle_answer(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let timeout = timeout_at(&request);
        let state = self
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| anyhow!("unknown run id: {run_id}"))?;
        let Some(request_id) = state.pending_request_id.take() else {
            bail!("run {run_id} has no pending structured question");
        };
        let answer = request
            .get("answer")
            .cloned()
            .ok_or_else(|| anyhow!("session_answer missing answer"))?;
        state.answers.push(answer.clone());
        state.questions.clear();
        state.server.send_response(request_id, answer)?;
        state.status = "running".to_string();
        let request_id = state.current_turn_request_id.take().unwrap_or_default();
        state.pump_until_pause(request_id, timeout)?;
        Ok(state.snapshot())
    }

    fn handle_send(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let timeout = timeout_at(&request);
        let state = self
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| anyhow!("unknown run id: {run_id}"))?;
        if state.pending_request_id.is_some() {
            bail!("run {run_id} has a pending structured question; call session answer first");
        }
        let prompt = string_at(&request, "prompt")?;
        if let Some(model) = request.get("model").and_then(Value::as_str) {
            state.thread_model = model.to_string();
        }
        state.reset_turn_fields();
        let request_id = state.server.send_request(
            "turn/start",
            json!({
                "threadId": state.thread_id,
                "input": [{"type": "text", "text": prompt, "text_elements": []}],
                "approvalPolicy": state.approval_policy,
                "collaborationMode": {
                    "mode": "plan",
                    "settings": {
                        "model": state.thread_model,
                        "reasoning_effort": string_at(&request, "effort")?,
                        "developer_instructions": Value::Null,
                    }
                }
            }),
        )?;
        state.pump_until_pause(request_id, timeout)?;
        Ok(state.snapshot())
    }

    fn handle_read(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let state = self
            .runs
            .get(&run_id)
            .ok_or_else(|| anyhow!("unknown run id: {run_id}"))?;
        Ok(state.snapshot())
    }

    fn handle_stop(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let existed = self.runs.remove(&run_id).is_some();
        Ok(json!({
            "ok": existed,
            "status": if existed { "stopped" } else { "not_found" },
            "run_id": run_id,
        }))
    }
}

struct RunState {
    server: AppServer,
    run_id: String,
    thread_id: String,
    thread_model: String,
    approval_policy: String,
    codex_home: Option<String>,
    thread_path: Option<String>,
    log_path: Option<PathBuf>,
    goal: Option<Value>,
    status: String,
    turn_id: Option<String>,
    current_turn_request_id: Option<u64>,
    pending_request_id: Option<Value>,
    questions: Vec<Value>,
    answers: Vec<Value>,
    plans: Vec<String>,
    agent_messages: Vec<String>,
    agent_deltas: Vec<String>,
    usage: Option<Value>,
    completed: Option<Value>,
    warnings: Vec<Value>,
    errors: Vec<Value>,
    items: Vec<Value>,
}

impl RunState {
    fn pump_until_pause(
        &mut self,
        turn_start_request_id: u64,
        timeout: Option<Duration>,
    ) -> Result<()> {
        self.current_turn_request_id = Some(turn_start_request_id);
        loop {
            let message = self.server.recv(timeout)?;
            if is_response_id(&message, turn_start_request_id) {
                if let Some(error) = message.get("error") {
                    self.status = "failed".to_string();
                    self.errors.push(error.clone());
                    return Ok(());
                }
                if let Some(turn_id) = message
                    .pointer("/result/turn/id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                {
                    self.turn_id = Some(turn_id);
                }
                continue;
            }
            if message.get("method").and_then(Value::as_str) == Some("item/tool/requestUserInput") {
                self.status = "needs_input".to_string();
                self.pending_request_id = message.get("id").cloned();
                self.questions = message
                    .pointer("/params/questions")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                return Ok(());
            }
            if let Some(method) = message.get("method").and_then(Value::as_str) {
                match method {
                    "item/completed" => self.collect_item(&message),
                    "item/agentMessage/delta" => {
                        if let Some(delta) =
                            message.pointer("/params/delta").and_then(Value::as_str)
                        {
                            self.agent_deltas.push(delta.to_string());
                        }
                    }
                    "thread/tokenUsage/updated" => self.usage = Some(message["params"].clone()),
                    "turn/completed" => {
                        self.status = "completed".to_string();
                        self.completed = Some(message["params"].clone());
                        self.pending_request_id = None;
                        self.current_turn_request_id = None;
                        return Ok(());
                    }
                    "error" => {
                        self.status = "failed".to_string();
                        self.errors.push(message.clone());
                        return Ok(());
                    }
                    "warning" | "guardianWarning" => self.warnings.push(message.clone()),
                    _ => self.items.push(message.clone()),
                }
            }
        }
    }

    fn collect_item(&mut self, message: &Value) {
        let item = &message["params"]["item"];
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        match item_type {
            "plan" => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    self.plans.push(text.to_string());
                }
            }
            "agentMessage" | "agent_message" => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    self.agent_messages.push(text.to_string());
                    if let Some(plan) = extract_proposed_plan(text) {
                        self.plans.push(plan);
                    }
                }
            }
            _ => self.items.push(item.clone()),
        }
    }

    fn reset_turn_fields(&mut self) {
        self.status = "running".to_string();
        self.turn_id = None;
        self.pending_request_id = None;
        self.questions.clear();
        self.answers.clear();
        self.plans.clear();
        self.agent_messages.clear();
        self.agent_deltas.clear();
        self.usage = None;
        self.completed = None;
        self.warnings.clear();
        self.errors.clear();
        self.items.clear();
    }

    fn snapshot(&self) -> Value {
        json!({
            "ok": self.status != "failed",
            "status": self.status,
            "run_id": self.run_id,
            "thread_id": self.thread_id,
            "turn_id": self.turn_id,
            "codex_home": self.codex_home,
            "thread_path": self.thread_path,
            "resume_command": resume_command(self.codex_home.as_deref(), &self.thread_id),
            "pending_request_id": self.pending_request_id,
            "questions": self.questions,
            "answers": self.answers,
            "goal": self.goal,
            "plans": self.plans,
            "agent_messages": self.agent_messages,
            "usage": self.usage,
            "completed": self.completed,
            "warnings": self.warnings,
            "errors": self.errors,
            "items": self.items,
            "log_path": self.log_path,
        })
    }
}

fn string_at(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing string field {key}"))
}

fn path_at(value: &Value, key: &str) -> Option<PathBuf> {
    value.get(key).and_then(Value::as_str).map(PathBuf::from)
}

fn timeout_at(value: &Value) -> Option<Duration> {
    value
        .get("timeoutSecs")
        .and_then(Value::as_u64)
        .map(Duration::from_secs)
}

fn log_mode_name(mode: LogMode) -> &'static str {
    match mode {
        LogMode::Off => "off",
        LogMode::Summary => "summary",
        LogMode::Full => "full",
    }
}

fn parse_log_mode(value: String) -> Result<LogMode> {
    match value.as_str() {
        "off" => Ok(LogMode::Off),
        "summary" => Ok(LogMode::Summary),
        "full" => Ok(LogMode::Full),
        _ => bail!("unknown log mode {value}"),
    }
}

fn new_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("run-{millis}-{}", std::process::id())
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

#[cfg(test)]
mod tests {
    use super::build_session_answer;

    #[test]
    fn builds_session_answer_from_pairs() {
        let answer = build_session_answer(
            vec![
                "scope=A Small plan (Recommended)".to_string(),
                "mode=Plan".to_string(),
            ],
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            answer["answers"]["scope"]["answers"][0],
            "A Small plan (Recommended)"
        );
        assert_eq!(answer["answers"]["mode"]["answers"][0], "Plan");
    }

    #[test]
    fn wraps_raw_session_answer_json() {
        let answer = build_session_answer(
            Vec::new(),
            Some(r#"{"scope":{"answers":["A"]}}"#.to_string()),
            None,
        )
        .unwrap();
        assert_eq!(answer["answers"]["scope"]["answers"][0], "A");
    }
}
