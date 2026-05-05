use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::cli::{DEFAULT_TIMEOUT_SECS, LogMode};
use crate::logging::RunLogger;
use crate::util::is_response_id;

pub(crate) struct AppServer {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    rx: Receiver<ServerLine>,
    next_id: u64,
    logger: RunLogger,
    pub(crate) codex_home: Option<String>,
    pub(crate) last_thread_model: Option<String>,
    pub(crate) last_thread_path: Option<String>,
}

impl AppServer {
    pub(crate) fn spawn(
        codex_bin: &str,
        codex_home: Option<PathBuf>,
        log_dir: Option<PathBuf>,
        log_mode: LogMode,
    ) -> Result<Self> {
        let mut command = Command::new(codex_bin);
        command
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match &codex_home {
            Some(codex_home) => {
                command.env("CODEX_HOME", codex_home);
            }
            None => {
                command.env_remove("CODEX_HOME");
            }
        }
        let mut child = command
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

    pub(crate) fn log_path(&self) -> Option<PathBuf> {
        self.logger.path()
    }

    pub(crate) fn latest_log_path(&self) -> Option<PathBuf> {
        self.logger.latest_path()
    }

    pub(crate) fn initialize(&mut self) -> Result<()> {
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
        let response = self.wait_response(id, Some(Duration::from_secs(20)))?;
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

    pub(crate) fn call(
        &mut self,
        method: &str,
        params: Value,
        include_events: bool,
    ) -> Result<Value> {
        let id = self.send_request(method, params)?;
        let mut events = Vec::new();
        loop {
            let message = self.recv(Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)))?;
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

    pub(crate) fn send_request(&mut self, method: &str, params: Value) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_value(json!({ "id": id, "method": method, "params": params }))?;
        Ok(id)
    }

    pub(crate) fn send_response(&mut self, id: Value, result: Value) -> Result<()> {
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

    fn wait_response(&mut self, id: u64, timeout: Option<Duration>) -> Result<Value> {
        loop {
            let message = self.recv(timeout)?;
            if is_response_id(&message, id) {
                return Ok(message);
            }
        }
    }

    pub(crate) fn recv(&mut self, timeout: Option<Duration>) -> Result<Value> {
        loop {
            let line = match timeout {
                Some(timeout) => self.rx.recv_timeout(timeout).map_err(|_| {
                    anyhow!("timed out waiting for app-server event after {timeout:?}")
                })?,
                None => self
                    .rx
                    .recv()
                    .map_err(|_| anyhow!("app-server stream closed"))?,
            };
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
