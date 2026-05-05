use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::Value;

pub(crate) fn read_params(params: Option<String>, params_file: Option<PathBuf>) -> Result<Value> {
    let raw = match (params, params_file) {
        (Some(_), Some(_)) => bail!("use either --params or --params-file, not both"),
        (Some(text), None) => text,
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("read params file {}", path.display()))?,
        (None, None) => "{}".to_string(),
    };
    serde_json::from_str(&raw).with_context(|| format!("invalid JSON params: {raw}"))
}

pub(crate) fn read_prompt(prompt: Option<String>, prompt_file: Option<PathBuf>) -> Result<String> {
    match (prompt, prompt_file) {
        (Some(_), Some(_)) => bail!("use either --prompt or --prompt-file, not both"),
        (Some(text), None) => Ok(text),
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("read prompt file {}", path.display())),
        (None, None) => bail!("plan requires --prompt or --prompt-file"),
    }
}

pub(crate) fn response_payload(value: Value) -> Value {
    value.get("result").cloned().unwrap_or(value)
}

pub(crate) fn resume_command(codex_home: Option<&str>, thread_id: &str) -> String {
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

pub(crate) fn is_response_id(value: &Value, id: u64) -> bool {
    value.get("id").and_then(Value::as_u64) == Some(id)
}

pub(crate) fn extract_proposed_plan(text: &str) -> Option<String> {
    let start_tag = "<proposed_plan>";
    let end_tag = "</proposed_plan>";
    let start = text.find(start_tag)? + start_tag.len();
    let end = text[start..].find(end_tag)? + start;
    Some(text[start..end].trim().to_string())
}

pub(crate) fn print_json(value: Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

pub(crate) fn print_json_line(value: &Value) {
    println!("{value}");
}

#[cfg(test)]
mod tests {
    use super::extract_proposed_plan;

    #[test]
    fn extracts_proposed_plan() {
        let text = "x<proposed_plan>\n1. A\n2. B\n</proposed_plan>y";
        assert_eq!(extract_proposed_plan(text).unwrap(), "1. A\n2. B");
    }
}
