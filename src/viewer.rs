use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::cli::{LogMode, SessionCommand, SessionListArgs, ViewArgs};
use crate::session::run_session_command;

const VIEWER_HTML: &str = include_str!("../viewer.html");

pub(crate) fn run_viewer(
    socket_path: PathBuf,
    codex_bin: String,
    codex_home: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    log_mode: LogMode,
    args: ViewArgs,
) -> Result<Value> {
    let source = load_source(
        &socket_path,
        &codex_bin,
        &codex_home,
        &log_dir,
        log_mode,
        &args,
    )?;
    let viewer_path = write_viewer_html(args.out.as_deref(), args.viewer_html.as_deref(), &source)?;
    let opened = if args.no_open {
        false
    } else {
        open_in_browser(&viewer_path)?
    };
    Ok(json!({
        "ok": true,
        "opened": opened,
        "viewer_path": viewer_path,
        "source": {
            "kind": source.kind,
            "name": source.name,
            "path": source.path,
            "run_id": source.run_id,
            "codex_bin": source.codex_bin,
            "codex_home": source.codex_home,
            "bytes": source.text.len(),
        }
    }))
}

fn load_source(
    socket_path: &Path,
    codex_bin: &str,
    codex_home: &Option<PathBuf>,
    log_dir: &Option<PathBuf>,
    log_mode: LogMode,
    args: &ViewArgs,
) -> Result<ViewerSource> {
    if let Some(file) = &args.file {
        return read_jsonl_source(
            "file",
            file,
            None,
            codex_bin.to_string(),
            codex_home.clone(),
        );
    }

    let Some(run_id) = &args.run_id else {
        bail!("view requires FILE or --run-id. Try `codexctl view --help`.");
    };
    let thread_path = thread_path_for_run(
        socket_path,
        codex_bin,
        codex_home,
        log_dir,
        log_mode,
        run_id,
    )?;
    read_jsonl_source(
        "run-jsonl",
        &thread_path,
        Some(run_id.clone()),
        codex_bin.to_string(),
        codex_home.clone(),
    )
}

fn thread_path_for_run(
    socket_path: &Path,
    codex_bin: &str,
    codex_home: &Option<PathBuf>,
    log_dir: &Option<PathBuf>,
    log_mode: LogMode,
    run_id: &str,
) -> Result<PathBuf> {
    let response = run_session_command(
        socket_path.to_path_buf(),
        codex_bin.to_string(),
        codex_home.clone(),
        log_dir.clone(),
        log_mode,
        SessionCommand::List(SessionListArgs {
            threads: false,
            limit: 20,
        }),
    )?;
    let runs = response
        .get("runs")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("daemon did not return a runs list"))?;
    let run = runs
        .iter()
        .find(|run| run.get("run_id").and_then(Value::as_str) == Some(run_id))
        .ok_or_else(|| {
            anyhow!(
                "unknown in-memory run id: {run_id}. Run `codexctl session list` while the daemon is alive, or open the rollout JSONL file directly with `codexctl view <path>`."
            )
        })?;
    let path = run
        .get("thread_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("run {run_id} has no local thread_path yet"))?;
    Ok(PathBuf::from(path))
}

fn read_jsonl_source(
    kind: &'static str,
    file: &Path,
    run_id: Option<String>,
    codex_bin: String,
    codex_home: Option<PathBuf>,
) -> Result<ViewerSource> {
    let text = fs::read_to_string(file)
        .with_context(|| format!("read viewer source file {}", file.display()))?;
    let path = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let codex_home = codex_home.or_else(|| derive_codex_home_from_rollout_path(&path));
    Ok(ViewerSource {
        kind,
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("rollout.jsonl")
            .to_string(),
        path: Some(path),
        run_id,
        codex_bin,
        codex_home,
        text,
    })
}

fn write_viewer_html(
    out: Option<&Path>,
    viewer_html: Option<&Path>,
    source: &ViewerSource,
) -> Result<PathBuf> {
    let path = match out {
        Some(path) => path.to_path_buf(),
        None => default_viewer_path()?,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create viewer output dir {}", parent.display()))?;
    }
    let context = json!({
        "name": source.name,
        "sourcePath": source.path,
        "runId": source.run_id,
        "codexBin": source.codex_bin,
        "codexHome": source.codex_home,
    });
    let boot = format!(
        r#"<script>
window.addEventListener('DOMContentLoaded', () => {{
  processSession({}, {});
}});
</script>
"#,
        js_string(&source.text)?,
        serde_json::to_string(&context)?,
    );
    let template = match viewer_html {
        Some(path) => fs::read_to_string(path)
            .with_context(|| format!("read viewer html {}", path.display()))?,
        None => VIEWER_HTML.to_string(),
    };
    let html = if template.contains("</body>") {
        template.replace("</body>", &format!("{boot}</body>"))
    } else {
        format!("{template}\n{boot}")
    };
    fs::write(&path, html).with_context(|| format!("write viewer html {}", path.display()))?;
    Ok(path)
}

fn default_viewer_path() -> Result<PathBuf> {
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(".codexctl")
        .join("viewer");
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX_EPOCH")?
        .as_millis();
    Ok(base.join(format!("view-{millis}-{}.html", std::process::id())))
}

fn derive_codex_home_from_rollout_path(path: &Path) -> Option<PathBuf> {
    let raw = path.to_string_lossy();
    raw.find("/sessions/")
        .map(|index| PathBuf::from(raw[..index].to_string()))
}

fn js_string(value: &str) -> Result<String> {
    let escaped = serde_json::to_string(value)?;
    Ok(escaped
        .replace("</script", "<\\/script")
        .replace("<!--", "<\\!--"))
}

fn open_in_browser(path: &Path) -> Result<bool> {
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .status()
    } else {
        Command::new("xdg-open").arg(path).status()
    }
    .with_context(|| format!("open viewer {}", path.display()))?;
    if !status.success() {
        bail!("open viewer failed with status {status}");
    }
    Ok(true)
}

struct ViewerSource {
    kind: &'static str,
    name: String,
    path: Option<PathBuf>,
    run_id: Option<String>,
    codex_bin: String,
    codex_home: Option<PathBuf>,
    text: String,
}
