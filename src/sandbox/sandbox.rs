use super::user_mapping::UserMapping;
use super::wine::{SyncMode, UpscaleMode, WinePrefixInfo, is_windows_binary};
use std::path::PathBuf;
use std::str::FromStr;

/// Defines the necessary environment variables and mount paths for each driver.
#[derive(Debug, Clone)]
pub enum DisplayProtocol {
  X11,
  Wayland,
}

impl FromStr for DisplayProtocol {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "x11" | "x" => Ok(DisplayProtocol::X11),
      "wayland" | "w" => Ok(DisplayProtocol::Wayland),
      _ => Err(format!("Invalid display protocol: {}", s)),
    }
  }
}

/// Represents network configuration options.
#[derive(Debug, Clone)]
pub enum NetworkMode {
  /// Allows complete network access.
  FullAccess,
  /// Allows network access but denies certain features like DNS resolving and SSL certificate
  /// access, useful for some games that require LAN.
  RestrictedAccess,
  /// Denies complete network access (recommended).
  NoAccess,
}

impl FromStr for NetworkMode {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "full_access" | "full" | "f" => Ok(NetworkMode::FullAccess),
      "restricted_access" | "restricted" | "r" => Ok(NetworkMode::RestrictedAccess),
      "no_access" | "no" | "n" => Ok(NetworkMode::NoAccess),
      _ => Err(format!("Invalid network mode: {}", s)),
    }
  }
}

#[derive(Debug, Clone)]
pub enum DeviceAccess {
  /// Allow access to all devices.
  All,
  /// Minimal set of input and GPU devices for games to work.
  Minimal,
}

impl FromStr for DeviceAccess {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "all" | "a" => Ok(DeviceAccess::All),
      "minimal" | "m" => Ok(DeviceAccess::Minimal),
      _ => Err(format!("Invalid device access mode: {}", s)),
    }
  }
}

pub struct SandboxConfig {
  /// Controls whether `--unshare-{ipc,pid,cgroup,uts}` is used or not.
  pub namespace_isolation: bool,
  /// Controls the user and group id inside the sandbox.
  pub user_mapping: UserMapping,
  /// Controls what environment variables will be set and what sockets will be mounted.
  pub display_protocol: DisplayProtocol,
  /// Controls network access for sandboxed programs (e.g. internet access), the bwrap default is to
  /// allow network connections, our default is to deny connections by using a separate network
  /// namespace.
  pub network_mode: NetworkMode,
  /// Controls what devices are accessible from within the sandbox.
  pub device_access: DeviceAccess,
  /// Configures various options such as WINEDEBUG and DXVK_LOG_LEVEL.
  pub verbose: bool,
}

impl Default for SandboxConfig {
  fn default() -> Self {
    SandboxConfig {
      namespace_isolation: true,
      user_mapping: UserMapping::Random,
      display_protocol: DisplayProtocol::X11,
      network_mode: NetworkMode::NoAccess,
      device_access: DeviceAccess::Minimal,
      verbose: false,
    }
  }
}

fn map_wine_app(app_bin: String, app_args: Option<Vec<String>>) -> (String, Vec<String>) {
  if !is_windows_binary(&app_bin) {
    return (app_bin, app_args.unwrap_or_default());
  }
  let new_args: Vec<String> = if let Some(mut args) = app_args {
    args.insert(0, app_bin);
    args
  } else {
    vec![app_bin]
  };
  return ("wine".to_string(), new_args);
}

fn map_wait_command(
  app_bin: String,
  app_args: Vec<String>,
  process_names: Option<Vec<String>>,
) -> (String, Vec<String>) {
  match process_names {
    Some(process_names) => {
      let current_exe = std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .expect("Failed to get executable name");
      let mut new_args: Vec<String> = vec![
        "wait".into(),
        "-w".into(),
        process_names.join(","),
        app_bin,
        "--".into(),
      ];
      new_args.extend(app_args);
      (current_exe, new_args)
    }
    None => (app_bin, app_args),
  }
}

pub enum LaunchParams {
  /// The root directory will be mounted without any app directory.
  Unconfigured,
  /// Only the app directory is mounted; no command will be executed.
  AppDirOnly { read_only: bool, app_dir: String },
  /// App directory is mounted and specified command will be executed.
  AppDirWithCommand {
    read_only: bool,
    app_dir: String,
    app_bin: String,
    app_args: Vec<String>,
  },
}

impl LaunchParams {
  pub fn from_options(
    read_only: bool,
    app_dir: Option<String>,
    app_bin: Option<String>,
    app_args: Option<Vec<String>>,
    process_names: Option<Vec<String>>,
  ) -> Self {
    let Some(app_dir) = app_dir else {
      return LaunchParams::Unconfigured;
    };
    let Some(app_bin) = app_bin else {
      return LaunchParams::AppDirOnly { read_only, app_dir };
    };
    let (bin, args) = map_wine_app(app_bin, app_args);
    let (bin, args) = map_wait_command(bin, args, process_names);
    LaunchParams::AppDirWithCommand {
      read_only,
      app_dir,
      app_bin: bin,
      app_args: args,
    }
  }
}

// TODO: detect GPUs and configure environment to use the dedicated GPU, currently bottles uses
// lspci and grep, however it does not seem to work in many scenarios. See
// https://github.com/bottlesdevs/Bottles/blob/540f6fc0d4c2853e2a62cab98548ce3210c7352a/bottles/backend/utils/gpu.py.
pub struct LaunchConfig {
  /// Full path to the wine runner.
  pub runner_path: Option<PathBuf>,
  /// Paths and user information for the Wine prefix.
  pub prefix_info: Option<WinePrefixInfo>,
  /// Application to execute inside the sandbox, if not set, a shell will be started instead.
  pub launch_params: LaunchParams,
  /// Optional upscale mode (needs to be supported by the runner).
  pub upscale_mode: Option<UpscaleMode>,
  /// Optional Wine sync mode.
  pub sync_mode: Option<SyncMode>,
}
