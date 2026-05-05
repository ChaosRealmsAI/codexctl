use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::app_server::AppServer;
use crate::cli::{AnswerArgs, EffectiveRuntime, GoalCommand, LogMode, PlanArgs, QuestionMode};
use crate::util::{
    extract_proposed_plan, is_response_id, print_json_line, read_prompt, response_payload,
    resume_command,
};

pub(crate) fn run_doctor(
    codex_bin: &str,
    codex_home: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    log_mode: LogMode,
) -> Result<Value> {
    let mut version_command = Command::new(codex_bin);
    version_command.arg("--version");
    match &codex_home {
        Some(codex_home) => {
            version_command.env("CODEX_HOME", codex_home);
        }
        None => {
            version_command.env_remove("CODEX_HOME");
        }
    }
    let version = version_command
        .output()
        .with_context(|| format!("failed to execute {codex_bin} --version"))?;
    let version_json = json!({
        "ok": version.status.success(),
        "status": version.status.code(),
        "stdout": String::from_utf8_lossy(&version.stdout).trim(),
        "stderr": String::from_utf8_lossy(&version.stderr).trim(),
    });
    let mut server = initialized_server(codex_bin, codex_home, log_dir, log_mode)?;
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

pub(crate) fn initialized_server(
    codex_bin: &str,
    codex_home: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    log_mode: LogMode,
) -> Result<AppServer> {
    let mut server = AppServer::spawn(codex_bin, codex_home, log_dir, log_mode)?;
    server.initialize()?;
    Ok(server)
}

pub(crate) fn run_goal(server: &mut AppServer, command: GoalCommand) -> Result<Value> {
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
            if let Some(token_budget) = args.token_budget.as_protocol_value() {
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

pub(crate) fn run_account(server: &mut AppServer) -> Result<Value> {
    let response = server.call("account/read", json!({}), false)?;
    let account = response
        .pointer("/result/account")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(json!({
        "ok": response.get("error").is_none(),
        "account": account,
        "codex_home": server.codex_home,
        "log_path": server.log_path(),
        "raw": response,
    }))
}

pub(crate) fn run_quota(server: &mut AppServer) -> Result<Value> {
    let response = server.call("account/rateLimits/read", json!({}), false)?;
    let rate_limits = response
        .pointer("/result/rateLimits")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(json!({
        "ok": response.get("error").is_none(),
        "rate_limits": rate_limits,
        "codex_home": server.codex_home,
        "log_path": server.log_path(),
        "raw": response,
    }))
}

pub(crate) fn run_models(server: &mut AppServer) -> Result<Value> {
    let response = server.call("model/list", json!({}), false)?;
    let models = response
        .pointer("/result/data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|model| {
            json!({
                "id": model.get("id").cloned().unwrap_or(Value::Null),
                "model": model.get("model").cloned().unwrap_or(Value::Null),
                "display_name": model.get("displayName").cloned().unwrap_or(Value::Null),
                "description": model.get("description").cloned().unwrap_or(Value::Null),
                "is_default": model.get("isDefault").cloned().unwrap_or(Value::Bool(false)),
                "hidden": model.get("hidden").cloned().unwrap_or(Value::Bool(false)),
                "default_reasoning_effort": model.get("defaultReasoningEffort").cloned().unwrap_or(Value::Null),
                "supported_reasoning_efforts": model.get("supportedReasoningEfforts").cloned().unwrap_or(Value::Array(Vec::new())),
                "input_modalities": model.get("inputModalities").cloned().unwrap_or(Value::Array(Vec::new())),
                "upgrade": model.get("upgrade").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "ok": response.get("error").is_none(),
        "models": models,
        "codex_home": server.codex_home,
        "log_path": server.log_path(),
        "raw": response,
    }))
}

pub(crate) fn run_plan(server: &mut AppServer, args: PlanArgs) -> Result<Value> {
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
        if let Some(token_budget) = args.token_budget.as_protocol_value() {
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
    let timeout = args.timeout.duration();
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

pub(crate) fn build_answer(args: AnswerArgs) -> Result<Value> {
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

pub(crate) fn compact_thread_read(response: Value) -> Value {
    let thread = &response["result"]["thread"];
    let turns = thread
        .get("turns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut messages = Vec::new();
    for turn in &turns {
        let turn_id = turn.get("id").and_then(Value::as_str).unwrap_or("");
        for item in turn
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            let text = match item_type {
                "userMessage" => item
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| content.first())
                    .and_then(|content| content.get("text"))
                    .and_then(Value::as_str),
                "agentMessage" => item.get("text").and_then(Value::as_str),
                _ => None,
            };
            if let Some(text) = text {
                messages.push(json!({
                    "turn_id": turn_id,
                    "type": item_type,
                    "text": text,
                }));
            }
        }
    }
    json!({
        "ok": response.get("error").is_none(),
        "thread_id": thread.get("id"),
        "cwd": thread.get("cwd"),
        "source": thread.get("source"),
        "status": thread.get("status"),
        "path": thread.get("path"),
        "turn_count": turns.len(),
        "messages": messages,
    })
}

pub(crate) fn start_thread(
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::cli::AnswerArgs;

    use super::{build_answer, compact_thread_read, select_recommended};

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
    fn compact_thread_read_collects_user_and_agent_messages() {
        let response = json!({
            "result": {
                "thread": {
                    "id": "t1",
                    "cwd": "/tmp/project",
                    "source": "vscode",
                    "status": {"type": "notLoaded"},
                    "path": "/tmp/session.jsonl",
                    "turns": [{
                        "id": "turn1",
                        "items": [
                            {"type": "userMessage", "content": [{"type": "text", "text": "hello"}]},
                            {"type": "agentMessage", "text": "world"}
                        ]
                    }]
                }
            }
        });
        let compact = compact_thread_read(response);
        assert_eq!(compact["thread_id"], "t1");
        assert_eq!(compact["turn_count"], 1);
        assert_eq!(compact["messages"][0]["text"], "hello");
        assert_eq!(compact["messages"][1]["text"], "world");
    }
}
