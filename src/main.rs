mod cli;
mod config;
mod inhibitor;
mod invoker;
mod list;
mod sandbox;
mod subprocess;
mod system;

use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let args = Cli::parse();
  match args.command {
    Commands::Run {
      environment,
      volumes,
      no_namespace_isolation,
      user_mapping,
      display_protocol,
      network_mode,
      device_access,
      verbose,
      upscale_mode,
      sync_mode,
      process_names,
      runner_path,
      prefix_path,
      app_dir,
      app_bin,
      app_args,
    } => {
      invoker::run(
        &environment,
        &volumes,
        no_namespace_isolation,
        user_mapping,
        display_protocol,
        network_mode,
        device_access,
        verbose,
        upscale_mode,
        sync_mode,
        process_names,
        runner_path,
        prefix_path,
        app_dir,
        app_bin,
        app_args,
      )
      .await
    }
    Commands::List { category } => list::list(category),
    Commands::Wait {
      process_names,
      program,
      args,
    } => subprocess::run(process_names, program, args),
  }
}
