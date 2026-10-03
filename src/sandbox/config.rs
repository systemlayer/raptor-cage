use super::user_mapping::UserMapping;
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
      _ => Err(format!("invalid display protocol: {}", s)),
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
      _ => Err(format!("invalid network mode: {}", s)),
    }
  }
}

/// Controls which host devices are accessible inside the sandbox.
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
      _ => Err(format!("invalid device access mode: {}", s)),
    }
  }
}

/// Defines isolation and access policies for sandboxed programs.
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
