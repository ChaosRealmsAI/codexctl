mod app_server;
mod cli;
mod commands;
mod help;
mod logging;
mod methods;
mod util;

use anyhow::Result;
use clap::Parser;
use serde_json::json;

use cli::{Cli, Commands};
use commands::{
    build_answer, compact_thread_read, initialized_server, run_doctor, run_goal, run_plan,
};
use methods::KNOWN_METHODS;
use util::{print_json, read_params};

fn main() -> Result<()> {
    let Cli {
        codex_bin,
        codex_home,
        log_dir,
        log_mode,
        command,
    } = Cli::parse();
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
    }
}
