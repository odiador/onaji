mod cli;
mod commands;
mod util;

use clap::Parser;
use cli::{Cli, Commands};
use commands::*;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Onboard => handle_onboard(&cli.config),
        Commands::Init { force } => handle_init(&cli.config, force),
        Commands::Watch {
            debounce,
            tests,
            env,
            dashboard,
        } => handle_watch(&cli.config, debounce, tests, env, dashboard).await,
        Commands::Run {
            resource,
            ci,
            output,
            fail_fast,
            trace,
            trace_format,
            trace_output,
            since,
            tests,
            remote,
            workers,
            env,
        } => {
            handle_run(
                &cli.config,
                resource,
                since,
                tests,
                ci,
                output,
                fail_fast,
                trace,
                trace_format,
                trace_output,
                remote,
                workers,
                env,
            )
            .await
        }
        Commands::Affected {
            since,
            uncommitted,
            tests,
            run,
            fail_fast,
            output,
            remote,
            workers,
            env,
        } => {
            handle_affected(
                &cli.config,
                &since,
                uncommitted,
                tests,
                run,
                fail_fast,
                output,
                remote,
                workers,
                env,
            )
            .await
        }
        Commands::Explain { task } => handle_explain(&cli.config, &task),
        Commands::Worker { command } => handle_worker(&cli.config, command).await,
        Commands::Daemon { socket, stdio } => handle_daemon(&cli.config, socket, stdio).await,
        Commands::Rpc {
            method,
            params,
            socket,
        } => handle_rpc(&cli.config, method, params, socket).await,
        Commands::Inspect { target } => handle_inspect(&cli.config, target),
        Commands::Validate { target } => handle_validate(&cli.config, target),
        Commands::Graph { format } => handle_graph(&cli.config, format),
        Commands::Doctor => handle_doctor(&cli.config),
        Commands::Check { ci, output } => handle_check(&cli.config, ci, output),
        Commands::Impact {
            resource,
            ci,
            output,
        } => handle_impact(&cli.config, &resource, ci, output),
        Commands::Plugin { command } => handle_plugin(&cli.config, command).await,
        Commands::Workspace { command } => handle_workspace(&cli.config, command).await,
        Commands::Security { command } => handle_security(&cli.config, command),
        Commands::Release { command } => handle_release(command),
        Commands::Dashboard { port, open } => handle_dashboard(&cli.config, port, open).await,
        Commands::Completions { shell } => handle_completions(shell),
        Commands::Clean {
            all,
            traces,
            task,
            max_size,
        } => handle_clean(&cli.config, all, traces, task, max_size),
        Commands::Hook { command } => handle_hook(&cli.config, command),
    }
}
