use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(windows)]
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value, json};

use crate::app_server::AppServer;
use crate::cli::{
    DaemonCommand, EffectiveRuntime, LogMode, SessionAnswerArgs, SessionCommand,
    SessionExecuteArgs, SessionListArgs, SessionResumeArgs, SessionRunIdArgs, SessionSendArgs,
    SessionStartArgs,
};
use crate::commands::start_thread;
use crate::util::{
    extract_proposed_plan, is_response_id, read_params, read_prompt, resume_command,
};

const DEFAULT_THREAD_MODEL: &str = "gpt-5.5";

pub(crate) fn default_socket_path() -> PathBuf {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string());
    let suffix = if cfg!(windows) { "endpoint" } else { "sock" };
    std::env::temp_dir().join(format!("codexctl-{user}.{suffix}"))
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
        SessionCommand::Execute(args) => session_execute_request(args)?,
        SessionCommand::Resume(args) => session_resume_request(config, args)?,
        SessionCommand::Interrupt(args) => run_id_request("session_interrupt", args),
        SessionCommand::List(args) => session_list_request(config, args),
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
        "detach": args.detach,
        "runtime": {
            "cwd": runtime.cwd,
            "sandbox": runtime.sandbox,
            "approvalPolicy": runtime.approval_policy,
        }
    }))
}

fn session_answer_request(args: SessionAnswerArgs) -> Result<Value> {
    let answer = if args.pick.is_some() {
        Value::Null
    } else {
        build_session_answer(args.answers, args.answers_json, args.answers_file)?
    };
    Ok(json!({
        "type": "session_answer",
        "runId": args.run_id,
        "answer": answer,
        "pick": args.pick,
        "timeoutSecs": args.timeout.duration().map(|value| value.as_secs()),
        "detach": args.detach,
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
        "detach": args.detach,
    }))
}

fn session_execute_request(args: SessionExecuteArgs) -> Result<Value> {
    let prompt = match (args.prompt, args.prompt_file) {
        (Some(prompt), None) => prompt,
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("read prompt file {}", path.display()))?,
        (None, None) => {
            "Implement the approved plan. Continue until verified, then summarize results."
                .to_string()
        }
        (Some(_), Some(_)) => bail!("use either --prompt or --prompt-file, not both"),
    };
    Ok(json!({
        "type": "session_execute",
        "runId": args.run_id,
        "prompt": prompt,
        "effort": args.effort.as_protocol(),
        "model": args.model,
        "timeoutSecs": args.timeout.duration().map(|value| value.as_secs()),
        "detach": args.detach,
    }))
}

fn session_resume_request(config: Value, args: SessionResumeArgs) -> Result<Value> {
    let runtime = args.runtime.effective()?;
    Ok(json!({
        "type": "session_resume",
        "config": config,
        "threadId": args.thread_id,
        "effort": args.effort.as_protocol(),
        "model": args.model,
        "runtime": {
            "cwd": runtime.cwd,
            "sandbox": runtime.sandbox,
            "approvalPolicy": runtime.approval_policy,
        }
    }))
}

fn session_list_request(config: Value, args: SessionListArgs) -> Value {
    json!({
        "type": "session_list",
        "config": config,
        "threads": args.threads,
        "limit": args.limit,
    })
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
    let mut command = Command::new(exe);
    command
        .arg("--session-socket")
        .arg(socket_path)
        .arg("daemon")
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    command.spawn().context("spawn codexctl daemon")?;
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

#[cfg(unix)]
fn send_request(socket_path: &Path, request: Value) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connect daemon socket {}", socket_path.display()))?;
    send_request_on_stream(&mut stream, request)
}

#[cfg(windows)]
fn send_request(socket_path: &Path, request: Value) -> Result<Value> {
    let endpoint = fs::read_to_string(socket_path)
        .with_context(|| format!("read daemon endpoint {}", socket_path.display()))?;
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        bail!("daemon endpoint file is empty: {}", socket_path.display());
    }
    let mut stream = TcpStream::connect(endpoint)
        .with_context(|| format!("connect daemon endpoint {endpoint}"))?;
    send_request_on_stream(&mut stream, request)
}

fn send_request_on_stream<S: Read + Write>(stream: &mut S, request: Value) -> Result<Value> {
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

#[cfg(unix)]
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

#[cfg(windows)]
fn serve(socket_path: PathBuf) -> Result<Value> {
    if socket_path.exists() {
        fs::remove_file(&socket_path)
            .with_context(|| format!("remove stale endpoint {}", socket_path.display()))?;
    }
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create endpoint dir {}", parent.display()))?;
    }
    let listener = TcpListener::bind("127.0.0.1:0").context("bind daemon TCP listener")?;
    let endpoint = listener
        .local_addr()
        .context("read daemon TCP listener address")?
        .to_string();
    fs::write(&socket_path, &endpoint)
        .with_context(|| format!("write daemon endpoint {}", socket_path.display()))?;
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
    runs: HashMap<String, RunHandle>,
}

impl SessionDaemon {
    fn handle_stream<S: Read + Write>(&mut self, mut stream: S) -> Result<bool> {
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
                    "run_count": self.runs.len(),
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
            Some("session_resume") => (self.handle_resume(request)?, false),
            Some("session_answer") => (self.handle_answer(request)?, false),
            Some("session_send") => (self.handle_send(request)?, false),
            Some("session_execute") => (self.handle_execute(request)?, false),
            Some("session_interrupt") => (self.handle_interrupt(request)?, false),
            Some("session_list") => (self.handle_list(request)?, false),
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
            .unwrap_or_else(|| DEFAULT_THREAD_MODEL.to_string());
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
        let state = Arc::new(Mutex::new(RunState::new(RunStateInit {
            run_id: run_id.clone(),
            thread_id,
            thread_model,
            approval_policy: runtime.approval_policy,
            codex_home,
            thread_path,
            log_path,
            goal,
        })));
        let handle = spawn_run_worker(server, Arc::clone(&state));
        submit_start_turn(
            &handle,
            prompt,
            string_at(&request, "effort")?,
            request
                .get("model")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            TurnMode::Plan,
        )?;
        let response = if bool_at(&request, "detach") {
            run_response(&state)?
        } else {
            wait_until_pause(&state, timeout)?
        };
        self.runs.insert(run_id, handle);
        Ok(response)
    }

    fn handle_answer(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let timeout = timeout_at(&request);
        let handle = self
            .runs
            .get(&run_id)
            .ok_or_else(|| anyhow!("unknown run id: {run_id}"))?;
        let answer = if let Some(pick) = request.get("pick").and_then(Value::as_str) {
            build_pick_answer(&handle.state, pick)?
        } else {
            request
                .get("answer")
                .cloned()
                .filter(|value| !value.is_null())
                .ok_or_else(|| anyhow!("session_answer missing answer or pick"))?
        };
        submit_answer(handle, answer)?;
        if bool_at(&request, "detach") {
            run_response(&handle.state)
        } else {
            wait_until_pause(&handle.state, timeout)
        }
    }

    fn handle_send(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let timeout = timeout_at(&request);
        let handle = self
            .runs
            .get(&run_id)
            .ok_or_else(|| anyhow!("unknown run id: {run_id}"))?;
        let prompt = string_at(&request, "prompt")?;
        submit_start_turn(
            handle,
            prompt,
            string_at(&request, "effort")?,
            request
                .get("model")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            TurnMode::Plan,
        )?;
        if bool_at(&request, "detach") {
            run_response(&handle.state)
        } else {
            wait_until_pause(&handle.state, timeout)
        }
    }

    fn handle_execute(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let timeout = timeout_at(&request);
        let handle = self
            .runs
            .get(&run_id)
            .ok_or_else(|| anyhow!("unknown run id: {run_id}"))?;
        let prompt = string_at(&request, "prompt")?;
        submit_start_turn(
            handle,
            prompt,
            string_at(&request, "effort")?,
            request
                .get("model")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            TurnMode::Default,
        )?;
        if bool_at(&request, "detach") {
            run_response(&handle.state)
        } else {
            wait_until_pause(&handle.state, timeout)
        }
    }

    fn handle_resume(&mut self, request: Value) -> Result<Value> {
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
        let thread_id = string_at(&request, "threadId")?;
        let model = request
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let mut server = AppServer::spawn(&codex_bin, codex_home, log_dir, log_mode)?;
        server.initialize()?;
        let mut params = json!({ "threadId": thread_id });
        if let Some(model) = &model {
            params["model"] = json!(model);
        }
        let response = server.call("thread/resume", params, false)?;
        if let Some(error) = response.get("error") {
            bail!("thread/resume failed: {error}");
        }
        let thread_path = response
            .pointer("/result/thread/path")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let thread_model = response
            .pointer("/result/model")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or(model)
            .unwrap_or_else(|| DEFAULT_THREAD_MODEL.to_string());
        let run_id = new_run_id();
        let log_path = server.log_path();
        let codex_home = server.codex_home.clone();
        let state = Arc::new(Mutex::new(RunState::new(RunStateInit {
            run_id: run_id.clone(),
            thread_id,
            thread_model,
            approval_policy: runtime.approval_policy,
            codex_home,
            thread_path,
            log_path,
            goal: None,
        })));
        if let Ok(mut state) = state.lock() {
            state.set_status("completed", "resumed");
        }
        let response = run_response(&state)?;
        self.runs.insert(run_id, spawn_run_worker(server, state));
        Ok(response)
    }

    fn handle_interrupt(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let handle = self
            .runs
            .get(&run_id)
            .ok_or_else(|| anyhow!("unknown run id: {run_id}"))?;
        submit_interrupt(handle)?;
        run_response(&handle.state)
    }

    fn handle_list(&mut self, request: Value) -> Result<Value> {
        let runs = self
            .runs
            .values()
            .filter_map(|handle| run_summary(&handle.state).ok())
            .collect::<Vec<_>>();
        let threads = if bool_at(&request, "threads") {
            let config = &request["config"];
            let codex_bin = string_at(config, "codexBin")?;
            let codex_home = path_at(config, "codexHome");
            let log_dir = path_at(config, "logDir");
            let log_mode = parse_log_mode(string_at(config, "logMode")?)?;
            let mut server = AppServer::spawn(&codex_bin, codex_home, log_dir, log_mode)?;
            server.initialize()?;
            let limit = request.get("limit").and_then(Value::as_u64).unwrap_or(20);
            let response = server.call("thread/list", json!({ "limit": limit }), false)?;
            response
                .pointer("/result/data")
                .cloned()
                .unwrap_or_else(|| json!([]))
        } else {
            json!([])
        };
        Ok(json!({
            "ok": true,
            "runs": runs,
            "threads": threads,
        }))
    }

    fn handle_stop(&mut self, request: Value) -> Result<Value> {
        let run_id = string_at(&request, "runId")?;
        let existed = self.runs.remove(&run_id);
        if let Some(handle) = &existed {
            let _ = handle.control.send(RunCommand::Stop);
            if let Ok(mut state) = handle.state.lock() {
                state.set_status("stopped", "stopped");
            }
        }
        Ok(json!({
            "ok": existed.is_some(),
            "status": if existed.is_some() { "stopped" } else { "not_found" },
            "run_id": run_id,
        }))
    }
}

struct RunHandle {
    state: Arc<Mutex<RunState>>,
    control: mpsc::Sender<RunCommand>,
}

enum RunCommand {
    StartTurn {
        prompt: String,
        effort: String,
        model: Option<String>,
        mode: TurnMode,
        response: mpsc::Sender<Result<(), String>>,
    },
    Answer {
        answer: Value,
        response: mpsc::Sender<Result<(), String>>,
    },
    Interrupt {
        response: mpsc::Sender<Result<(), String>>,
    },
    Stop,
}

#[derive(Debug, Clone, Copy)]
enum TurnMode {
    Plan,
    Default,
}

impl TurnMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Default => "default",
        }
    }
}

fn spawn_run_worker(mut server: AppServer, state: Arc<Mutex<RunState>>) -> RunHandle {
    let (control, rx) = mpsc::channel();
    let worker_state = Arc::clone(&state);
    thread::spawn(move || run_worker_loop(&mut server, worker_state, rx));
    RunHandle { state, control }
}

fn submit_start_turn(
    handle: &RunHandle,
    prompt: String,
    effort: String,
    model: Option<String>,
    mode: TurnMode,
) -> Result<()> {
    let (response, ack) = mpsc::channel();
    handle
        .control
        .send(RunCommand::StartTurn {
            prompt,
            effort,
            model,
            mode,
            response,
        })
        .context("send start-turn command to session worker")?;
    wait_worker_ack(ack)
}

fn submit_interrupt(handle: &RunHandle) -> Result<()> {
    let (response, ack) = mpsc::channel();
    handle
        .control
        .send(RunCommand::Interrupt { response })
        .context("send interrupt command to session worker")?;
    wait_worker_ack(ack)
}

fn submit_answer(handle: &RunHandle, answer: Value) -> Result<()> {
    let (response, ack) = mpsc::channel();
    handle
        .control
        .send(RunCommand::Answer { answer, response })
        .context("send answer command to session worker")?;
    wait_worker_ack(ack)
}

fn wait_worker_ack(ack: mpsc::Receiver<Result<(), String>>) -> Result<()> {
    match ack.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => bail!("{error}"),
        Err(_) => bail!("session worker did not acknowledge command"),
    }
}

fn wait_until_pause(state: &Arc<Mutex<RunState>>, timeout: Option<Duration>) -> Result<Value> {
    let started = Instant::now();
    loop {
        let response = run_response(state)?;
        if response.get("status").and_then(Value::as_str) != Some("running") {
            return Ok(response);
        }
        if let Some(timeout) = timeout
            && started.elapsed() >= timeout
        {
            bail!("timed out waiting for session run to pause after {timeout:?}");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn run_response(state: &Arc<Mutex<RunState>>) -> Result<Value> {
    let state = state
        .lock()
        .map_err(|_| anyhow!("session run state lock poisoned"))?;
    Ok(state.response())
}

fn run_summary(state: &Arc<Mutex<RunState>>) -> Result<Value> {
    let state = state
        .lock()
        .map_err(|_| anyhow!("session run state lock poisoned"))?;
    Ok(json!({
        "run_id": state.run_id,
        "status": state.status,
        "current_phase": state.current_phase,
        "thread_id": state.thread_id,
        "thread_path": state.thread_path,
        "updated_at_ms": state.updated_at_ms,
        "started_at_ms": state.started_at_ms,
    }))
}

fn build_pick_answer(state: &Arc<Mutex<RunState>>, raw_pick: &str) -> Result<Value> {
    let questions = {
        let state = state
            .lock()
            .map_err(|_| anyhow!("session run state lock poisoned"))?;
        state.questions.clone()
    };
    if questions.is_empty() {
        bail!("run has no pending questions to pick from");
    }
    let pick = raw_pick.trim().to_ascii_lowercase();
    let mut answers = Map::new();
    if pick == "recommended" || pick == "first" {
        for question in &questions {
            let id = question_id(question)?;
            let selected = if pick == "recommended" {
                select_recommended(question).or_else(|| select_index(question, 1))
            } else {
                select_index(question, 1)
            }
            .ok_or_else(|| anyhow!("question {id} has no selectable option"))?;
            answers.insert(id, json!({ "answers": [selected] }));
        }
        return Ok(json!({ "answers": answers }));
    }
    let picks = raw_pick
        .split(',')
        .map(|value| {
            value
                .trim()
                .parse::<usize>()
                .with_context(|| format!("invalid --pick index: {value}"))
        })
        .collect::<Result<Vec<_>>>()?;
    if picks.len() != questions.len() {
        bail!(
            "--pick index count ({}) must match pending question count ({})",
            picks.len(),
            questions.len()
        );
    }
    for (question, index) in questions.iter().zip(picks) {
        let id = question_id(question)?;
        let selected = select_index(question, index)
            .ok_or_else(|| anyhow!("question {id} has no option #{index}"))?;
        answers.insert(id, json!({ "answers": [selected] }));
    }
    Ok(json!({ "answers": answers }))
}

fn question_id(question: &Value) -> Result<String> {
    question
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("request_user_input question missing id"))
}

fn select_recommended(question: &Value) -> Option<String> {
    question
        .get("options")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|option| option.get("label").and_then(Value::as_str))
        .find(|label| label.to_ascii_lowercase().contains("(recommended)"))
        .map(ToString::to_string)
}

fn select_index(question: &Value, one_based: usize) -> Option<String> {
    if one_based == 0 {
        return None;
    }
    question
        .get("options")
        .and_then(Value::as_array)?
        .get(one_based - 1)?
        .get("label")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn run_worker_loop(
    server: &mut AppServer,
    state: Arc<Mutex<RunState>>,
    rx: mpsc::Receiver<RunCommand>,
) {
    let mut active = false;
    loop {
        loop {
            match rx.try_recv() {
                Ok(command) => {
                    if !handle_worker_command(server, &state, command, &mut active) {
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        if active {
            match server.recv_maybe(Duration::from_millis(100)) {
                Ok(Some(message)) => {
                    active = process_server_message(&state, message);
                }
                Ok(None) => {}
                Err(error) => {
                    mark_failed(&state, error.to_string());
                    active = false;
                }
            }
            continue;
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(command) => {
                if !handle_worker_command(server, &state, command, &mut active) {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn handle_worker_command(
    server: &mut AppServer,
    state: &Arc<Mutex<RunState>>,
    command: RunCommand,
    active: &mut bool,
) -> bool {
    match command {
        RunCommand::StartTurn {
            prompt,
            effort,
            model,
            mode,
            response,
        } => {
            let result = start_turn(server, state, prompt, effort, model, mode);
            *active = result.is_ok();
            let _ = response.send(result.map_err(|error| error.to_string()));
            true
        }
        RunCommand::Answer { answer, response } => {
            let result = answer_question(server, state, answer);
            *active = result.is_ok();
            let _ = response.send(result.map_err(|error| error.to_string()));
            true
        }
        RunCommand::Interrupt { response } => {
            let result = interrupt_turn(server, state);
            *active = false;
            let _ = response.send(result.map_err(|error| error.to_string()));
            true
        }
        RunCommand::Stop => false,
    }
}

fn start_turn(
    server: &mut AppServer,
    state: &Arc<Mutex<RunState>>,
    prompt: String,
    effort: String,
    model: Option<String>,
    mode: TurnMode,
) -> Result<()> {
    let (thread_id, approval_policy, thread_model) = {
        let mut state = state
            .lock()
            .map_err(|_| anyhow!("session run state lock poisoned"))?;
        if state.pending_request_id.is_some() {
            bail!(
                "run {} has a pending structured question; call session answer first",
                state.run_id
            );
        }
        if state.status == "running" && state.current_turn_request_id.is_some() {
            bail!(
                "run {} is already running; wait for it to return needs_input or completed",
                state.run_id
            );
        }
        if let Some(model) = model {
            state.thread_model = model;
        }
        state.reset_turn_fields();
        (
            state.thread_id.clone(),
            state.approval_policy.clone(),
            state.thread_model.clone(),
        )
    };
    let request_id = server.send_request(
        "turn/start",
        turn_start_params(
            thread_id,
            prompt,
            approval_policy,
            thread_model,
            effort,
            mode,
        ),
    );
    match request_id {
        Ok(request_id) => {
            let mut state = state
                .lock()
                .map_err(|_| anyhow!("session run state lock poisoned"))?;
            state.current_turn_request_id = Some(request_id);
            state.set_status("running", "reasoning");
            Ok(())
        }
        Err(error) => {
            mark_failed(state, error.to_string());
            Err(error)
        }
    }
}

fn turn_start_params(
    thread_id: String,
    prompt: String,
    approval_policy: String,
    thread_model: String,
    effort: String,
    mode: TurnMode,
) -> Value {
    let mut settings = Map::new();
    settings.insert("model".to_string(), json!(thread_model));
    settings.insert("developer_instructions".to_string(), Value::Null);
    settings.insert("reasoning_effort".to_string(), json!(effort));
    json!({
        "threadId": thread_id,
        "input": [{"type": "text", "text": prompt, "text_elements": []}],
        "approvalPolicy": approval_policy,
        "collaborationMode": {
            "mode": mode.as_str(),
            "settings": settings,
        }
    })
}

fn interrupt_turn(server: &mut AppServer, state: &Arc<Mutex<RunState>>) -> Result<()> {
    let (thread_id, turn_id) = {
        let state = state
            .lock()
            .map_err(|_| anyhow!("session run state lock poisoned"))?;
        if state.status != "running" {
            bail!("run {} is not currently running", state.run_id);
        }
        let Some(turn_id) = state.turn_id.clone() else {
            bail!("run {} has no active turn id yet", state.run_id);
        };
        (state.thread_id.clone(), turn_id)
    };
    let response = server.call(
        "turn/interrupt",
        json!({
            "threadId": thread_id,
            "turnId": turn_id,
        }),
        false,
    )?;
    if let Some(error) = response.get("error") {
        bail!("turn/interrupt failed: {error}");
    }
    if let Ok(mut state) = state.lock() {
        state.set_status("completed", "interrupted");
        state.current_turn_request_id = None;
    }
    Ok(())
}

fn answer_question(
    server: &mut AppServer,
    state: &Arc<Mutex<RunState>>,
    answer: Value,
) -> Result<()> {
    let request_id = {
        let mut state = state
            .lock()
            .map_err(|_| anyhow!("session run state lock poisoned"))?;
        let Some(request_id) = state.pending_request_id.take() else {
            bail!("run {} has no pending structured question", state.run_id);
        };
        state.answers.push(answer.clone());
        state.questions.clear();
        state.set_status("running", "reasoning");
        request_id
    };
    if let Err(error) = server.send_response(request_id, answer) {
        mark_failed(state, error.to_string());
        return Err(error);
    }
    Ok(())
}

fn process_server_message(state: &Arc<Mutex<RunState>>, message: Value) -> bool {
    let Ok(mut state) = state.lock() else {
        return false;
    };
    state.process_server_message(message)
}

fn mark_failed(state: &Arc<Mutex<RunState>>, error: String) {
    if let Ok(mut state) = state.lock() {
        state.set_status("failed", "failed");
        state.errors.push(json!({ "error": error }));
    }
}

struct RunStateInit {
    run_id: String,
    thread_id: String,
    thread_model: String,
    approval_policy: String,
    codex_home: Option<String>,
    thread_path: Option<String>,
    log_path: Option<PathBuf>,
    goal: Option<Value>,
}

struct RunState {
    run_id: String,
    thread_id: String,
    thread_model: String,
    approval_policy: String,
    codex_home: Option<String>,
    thread_path: Option<String>,
    log_path: Option<PathBuf>,
    goal: Option<Value>,
    status: String,
    current_phase: String,
    started_at_ms: u64,
    updated_at_ms: u64,
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
    fn new(init: RunStateInit) -> Self {
        let now = now_millis();
        Self {
            run_id: init.run_id,
            thread_id: init.thread_id,
            thread_model: init.thread_model,
            approval_policy: init.approval_policy,
            codex_home: init.codex_home,
            thread_path: init.thread_path,
            log_path: init.log_path,
            goal: init.goal,
            status: "running".to_string(),
            current_phase: "starting".to_string(),
            started_at_ms: now,
            updated_at_ms: now,
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
        }
    }

    fn process_server_message(&mut self, message: Value) -> bool {
        if let Some(request_id) = self.current_turn_request_id
            && is_response_id(&message, request_id)
        {
            if let Some(error) = message.get("error") {
                self.set_status("failed", "failed");
                self.errors.push(error.clone());
                self.current_turn_request_id = None;
                return false;
            }
            if let Some(turn_id) = message
                .pointer("/result/turn/id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
            {
                self.turn_id = Some(turn_id);
                self.touch_phase("turn_started");
            }
            return true;
        }
        if message.get("method").and_then(Value::as_str) == Some("item/tool/requestUserInput") {
            self.set_status("needs_input", "needs_input");
            self.pending_request_id = message.get("id").cloned();
            self.questions = message
                .pointer("/params/questions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            return false;
        }
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            match method {
                "item/completed" => {
                    self.collect_item(&message);
                    self.touch_phase("item_completed");
                }
                "item/agentMessage/delta" => {
                    if let Some(delta) = message.pointer("/params/delta").and_then(Value::as_str) {
                        self.agent_deltas.push(delta.to_string());
                    }
                    self.touch_phase("agent_message");
                }
                "thread/tokenUsage/updated" => {
                    self.usage = Some(message["params"].clone());
                    self.touch_phase("token_usage");
                }
                "turn/completed" => {
                    self.set_status("completed", "completed");
                    self.completed = Some(message["params"].clone());
                    self.pending_request_id = None;
                    self.current_turn_request_id = None;
                    return false;
                }
                "error" => {
                    self.set_status("failed", "failed");
                    self.errors.push(message.clone());
                    self.current_turn_request_id = None;
                    return false;
                }
                "warning" | "guardianWarning" => {
                    self.warnings.push(message.clone());
                    self.touch_phase("warning");
                }
                _ => {
                    self.items.push(message.clone());
                    self.touch_phase(method);
                }
            }
        }
        true
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
        self.set_status("running", "starting");
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

    fn set_status(&mut self, status: &str, phase: &str) {
        self.status = status.to_string();
        self.touch_phase(phase);
    }

    fn touch_phase(&mut self, phase: &str) {
        self.current_phase = phase.to_string();
        self.updated_at_ms = now_millis();
    }

    fn response(&self) -> Value {
        let now = now_millis();
        json!({
            "ok": self.status != "failed",
            "status": self.status,
            "current_phase": self.current_phase,
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
            "agent_deltas": self.agent_deltas,
            "usage": self.usage,
            "completed": self.completed,
            "warnings": self.warnings,
            "errors": self.errors,
            "items": self.items,
            "log_path": self.log_path,
            "started_at_ms": self.started_at_ms,
            "updated_at_ms": self.updated_at_ms,
            "elapsed_ms": now.saturating_sub(self.started_at_ms),
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

fn bool_at(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
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
    format!("run-{}-{}", now_millis(), std::process::id())
}

fn now_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    millis.min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::{RunState, RunStateInit, build_pick_answer, build_session_answer};

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

    #[test]
    fn builds_pick_answer_from_recommended_and_indices() {
        let mut state = RunState::new(RunStateInit {
            run_id: "run-1".to_string(),
            thread_id: "thread-1".to_string(),
            thread_model: "gpt-5.5".to_string(),
            approval_policy: "never".to_string(),
            codex_home: None,
            thread_path: None,
            log_path: None,
            goal: None,
        });
        state.questions = vec![
            json!({
                "id": "scope",
                "options": [
                    {"label": "A Small (Recommended)"},
                    {"label": "B Large"}
                ]
            }),
            json!({
                "id": "mode",
                "options": [
                    {"label": "Plan"},
                    {"label": "Execute"}
                ]
            }),
        ];
        let state = Arc::new(Mutex::new(state));
        let recommended = build_pick_answer(&state, "recommended").unwrap();
        assert_eq!(
            recommended["answers"]["scope"]["answers"][0],
            "A Small (Recommended)"
        );
        assert_eq!(recommended["answers"]["mode"]["answers"][0], "Plan");

        let indexed = build_pick_answer(&state, "2,1").unwrap();
        assert_eq!(indexed["answers"]["scope"]["answers"][0], "B Large");
        assert_eq!(indexed["answers"]["mode"]["answers"][0], "Plan");
    }
}
