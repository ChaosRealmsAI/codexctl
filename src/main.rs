mod app_server;
mod cli;
mod commands;
mod help;
mod logging;
mod methods;
mod session;
mod util;

use anyhow::Result;
use clap::Parser;
use serde_json::json;

use cli::{Cli, Commands, DaemonCommand};
use commands::{
    build_answer, compact_thread_read, initialized_server, run_account, run_doctor, run_goal,
    run_models, run_plan, run_quota,
};
use methods::KNOWN_METHODS;
use session::{default_socket_path, run_daemon_command, run_session_command};
use util::{print_json, read_params};

fn main() -> Result<()> {
    let Cli {
        codex_bin,
        codex_home,
        log_dir,
        log_mode,
        session_socket,
        command,
    } = Cli::parse();
    let socket_path = session_socket.unwrap_or_else(default_socket_path);
    match command {
        Commands::Doctor => print_json(run_doctor(&codex_bin, codex_home, log_dir, log_mode)?),
        Commands::Methods => print_json(json!({ "ok": true, "methods": KNOWN_METHODS })),
        Commands::Modes => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            print_json(server.call("collaborationMode/list", json!({}), false)?)
        }
        Commands::Features => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            print_json(server.call("experimentalFeature/list", json!({}), false)?)
        }
        Commands::Account => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            print_json(run_account(&mut server)?)
        }
        Commands::Quota => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            print_json(run_quota(&mut server)?)
        }
        Commands::Models => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            print_json(run_models(&mut server)?)
        }
        Commands::Status => {
            let daemon = run_daemon_command(socket_path, DaemonCommand::Status)?;
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            let account = run_account(&mut server)?;
            let quota = run_quota(&mut server)?;
            let models = run_models(&mut server)?;
            print_json(json!({
                "ok": true,
                "daemon": daemon,
                "account": account.get("account").cloned().unwrap_or_default(),
                "rate_limits": quota.get("rate_limits").cloned().unwrap_or_default(),
                "models": models.get("models").cloned().unwrap_or_default(),
                "codex_home": server.codex_home,
                "log_path": server.log_path(),
            }))
        }
        Commands::Read(args) => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            let response = server.call(
                "thread/read",
                json!({
                    "threadId": args.thread_id,
                    "includeTurns": !args.metadata_only,
                }),
                false,
            )?;
            if args.compact {
                print_json(compact_thread_read(response))
            } else {
                print_json(response)
            }
        }
        Commands::Raw(args) => {
            let params = read_params(args.params, args.params_file)?;
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            print_json(server.call(&args.method, params, args.include_events)?)
        }
        Commands::Goal(command) => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            print_json(run_goal(&mut server, command)?)
        }
        Commands::Plan(args) => {
            let mut server = initialized_server(&codex_bin, codex_home, log_dir, log_mode)?;
            let result = run_plan(&mut server, args)?;
            print_json(result)
        }
        Commands::Answer(args) => print_json(build_answer(args)?),
        Commands::Session(command) => print_json(run_session_command(
            socket_path,
            codex_bin,
            codex_home,
            log_dir,
            log_mode,
            command,
        )?),
        Commands::Daemon(command) => print_json(run_daemon_command(socket_path, command)?),
    }
}
