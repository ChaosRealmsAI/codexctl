use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::cli::LogMode;

pub(crate) struct RunLogger {
    mode: LogMode,
    path: Option<PathBuf>,
    latest_path: Option<PathBuf>,
    run_file: Option<File>,
    latest_file: Option<File>,
}

impl RunLogger {
    pub(crate) fn new(log_dir: Option<PathBuf>, mode: LogMode) -> Result<Self> {
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

    pub(crate) fn path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    pub(crate) fn latest_path(&self) -> Option<PathBuf> {
        self.latest_path.clone()
    }

    pub(crate) fn log(&mut self, direction: &str, value: &Value) -> Result<()> {
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

pub(crate) fn summarize(value: &Value) -> Value {
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
        .join(".codexctl")
        .join("logs")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::summarize;

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
