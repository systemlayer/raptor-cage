use crate::{
  list::Category,
  sandbox::{
    config::{DeviceAccess, DisplayProtocol, NetworkMode},
    user_mapping::UserMapping,
    wine::{SyncMode, UpscaleMode},
  },
};
use clap::{ArgAction, Parser};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub enum Commands {
  /// Run application sandboxed.
  #[command(arg_required_else_help = true)]
  Run {
    /// Environment variable overrides.
    #[arg(short = 'e', long = "setenv", value_name="KEY=VALUE", action = ArgAction::Append)]
    environment: Vec<String>,
    /// Additional mount points.
    #[arg(short = 'v', long = "volume", value_name="PATH", action = ArgAction::Append)]
    volumes: Vec<String>,
    /// Disable namespace isolation.
    #[arg(long, default_value = "false")]
    no_namespace_isolation: bool,
    /// Use specific user and group id.
    #[arg(
      short = 'u',
      long,
      value_name = "UID:GID",
      default_value = "random",
      value_parser
    )]
    user_mapping: UserMapping,
    /// Display protocol.
    #[arg(
      short = 'o',
      long,
      value_name = "PRT",
      default_value = "x11",
      value_parser
    )]
    display_protocol: DisplayProtocol,
    /// Configure network access.
    #[arg(long, value_name = "MODE", default_value = "no_access", value_parser)]
    network_mode: NetworkMode,
    /// Sandbox device access.
    #[arg(long, value_name = "ACCESS", default_value = "minimal", value_parser)]
    device_access: DeviceAccess,
    /// Print additional troubleshooting information.
    #[arg(long, default_value = "false")]
    verbose: bool,
    /// One of none, dlss, fsr:mode:stre.
    #[arg(long, value_name = "MODE", default_value = "none", value_parser)]
    upscale_mode: UpscaleMode,
    /// Configure Wine sync mode.
    #[arg(long, value_name = "MODE", default_value = "none", value_parser)]
    sync_mode: SyncMode,
    /// Process names to wait for before exiting.
    #[arg(
      short = 'w',
      long = "process-names",
      value_name = "NAMES",
      value_delimiter = ','
    )]
    process_names: Option<Vec<String>>,
    /// Path of the Wine runner.
    #[arg(short, long = "runner", value_name = "PATH")]
    runner_path: Option<PathBuf>,
    /// Path of the Wine prefix.
    #[arg(short, long = "prefix", value_name = "PATH")]
    prefix_path: Option<PathBuf>,
    /// Path that contains the application files.
    #[arg(short = 'd', long = "appdir", value_name = "PATH")]
    app_dir: Option<String>,
    /// Path of the executable file relative to appdir.
    #[arg(short = 'b', long = "appbin", value_name = "BIN")]
    app_bin: Option<String>,
    /// Optional game arguments, need to be placed after double dash.
    app_args: Option<Vec<String>>,
  },
  /// List installed runners and prefixes.
  List {
    #[arg(long, value_name = "CATEGORY", default_value = "all", value_parser)]
    category: Category,
  },
  /// Runs a process then waits for one or more processes to stop.
  Wait {
    /// Waits for all comma-separated processes to exit.
    #[arg(
      short = 'w',
      long,
      value_name = "NAMES",
      value_delimiter = ',',
      required = true
    )]
    process_names: Vec<String>,
    /// Program to launch (usually "wine").
    program: String,
    /// Optional program arguments, need to be placed after double dash.
    args: Option<Vec<String>>,
  },
}

#[derive(Debug, Parser)]
#[command(about = env!("CARGO_PKG_DESCRIPTION"), long_about = None)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Commands,
}
