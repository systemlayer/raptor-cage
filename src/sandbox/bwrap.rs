use super::config::{DeviceAccess, DisplayProtocol, NetworkMode, SandboxConfig};
use super::launch::{LaunchConfig, LaunchParams};
use super::mount::MountMapping;
use super::paths::{INNER_APP_DIR, INNER_WINE_PREFIX, INNER_WINE_ROOT};
use super::wine::{SyncMode, UpscaleMode};
use crate::system::devices::find_nvidia_devices;
use crate::system::display::X11Display;
use crate::system::env::RuntimeEnv;
use anyhow::Context;
use std::{
  path::PathBuf,
  process::{Command, Stdio},
};
use tempfile::NamedTempFile;

fn current_timestamp_hex() -> String {
  let start = std::time::SystemTime::now();
  let since_epoch = start.duration_since(std::time::UNIX_EPOCH).unwrap();
  let seconds = since_epoch.as_secs();
  format!("{:x}", seconds)
}

/// Builds device binding arguments, rejecting paths that cannot be represented as UTF-8.
fn get_device_bind_args(devices: &[PathBuf]) -> anyhow::Result<Vec<String>> {
  let mut args = Vec::with_capacity(devices.len() * 3);
  for device in devices {
    let path = device
      .to_str()
      .with_context(|| format!("device path is not valid utf-8: {}", device.display()))?;
    args.extend(["--dev-bind".to_owned(), path.to_owned(), path.to_owned()]);
  }
  Ok(args)
}

/// Gets the corresponding bwrap parameters for the selected DeviceAccess option.
pub fn get_device_args(device_access: &DeviceAccess) -> anyhow::Result<Vec<String>> {
  match device_access {
    DeviceAccess::All => {
      // NOTE: "bwrap --dev /dev ..." does not work as expected, so using "--dev-bind" instead.
      let args = vec!["--dev-bind", "/dev", "/dev"];
      Ok(args.into_iter().map(String::from).collect())
    }
    DeviceAccess::Minimal => {
      let nvidia_devices = find_nvidia_devices()?;
      // TODO: check if /dev/snd/seq is needed.
      let mut devices: Vec<PathBuf> = vec!["/dev/input", "/dev/uinput", "/dev/dri"]
        .into_iter()
        .map(PathBuf::from)
        .collect();
      devices.extend(nvidia_devices);
      get_device_bind_args(&devices)
    }
  }
}

fn get_mount_args(mount_mappings: &[MountMapping]) -> Vec<String> {
  let mut args: Vec<String> = Vec::with_capacity(mount_mappings.len() * 3);
  for mapping in mount_mappings {
    let bind_param = if mapping.target_config.writable {
      "--bind"
    } else {
      "--ro-bind"
    };
    let source = mapping.source_path.to_string_lossy();
    let target = mapping.target_config.path.to_string_lossy();
    args.extend(vec![bind_param.into(), source.into(), target.into()]);
  }
  args
}

fn get_app_dir_args(read_only: bool, app_dir: String) -> Vec<String> {
  // Most games can work without issues when mounted as read-only. This also prevents polluting
  // the game directory. Also, setting the working directory is important for many games.
  vec![
    if read_only {
      "--ro-bind".into()
    } else {
      "--bind".into()
    },
    app_dir,
    INNER_APP_DIR.into(),
    "--chdir".into(),
    INNER_APP_DIR.into(),
  ]
}

fn get_display_args(
  display_protocol: &DisplayProtocol,
  runtime_env: &RuntimeEnv,
) -> anyhow::Result<Vec<String>> {
  match display_protocol {
    DisplayProtocol::X11 => {
      let x11_display = runtime_env.x11_display()?;
      let xauthority_file = runtime_env.xauthority_file()?;
      let display = X11Display::from_str(&x11_display)?;
      let x11_socket = display.get_socket_path();
      // Mount X11 socket to allow running GUI apps. Using the same X11 display number as the host
      // because using a different number will not work despite being the first recommendation in the
      // ArchWiki: https://wiki.archlinux.org/title/Bubblewrap#Using_X11.
      Ok(vec![
        "--setenv".into(),
        "DISPLAY".into(),
        x11_display.into(),
        "--setenv".into(),
        "XAUTHORITY".into(),
        xauthority_file.into(),
        "--bind".into(),
        x11_socket.clone(),
        x11_socket,
        "--ro-bind".into(),
        xauthority_file.into(),
        xauthority_file.into(),
      ])
    }
    DisplayProtocol::Wayland => {
      let wayland_display = runtime_env.wayland_display()?;
      let wayland_socket = format!("{}/{}", runtime_env.xdg_runtime_dir, wayland_display);
      // The DISPLAY env variable must not be set to tell wine to use Wayland.
      // https://gitlab.winehq.org/wine/wine/-/releases/wine-10.0#wayland-driver.
      // Also, the XDG_RUNTIME_DIR env variable (set previously) is required because the Wayland
      // socket is located under this path.
      Ok(vec![
        "--setenv".into(),
        "WAYLAND_DISPLAY".into(),
        wayland_display.into(),
        "--bind".into(),
        wayland_socket.clone(),
        wayland_socket,
      ])
    }
  }
}

fn build_args(
  sandbox_config: &SandboxConfig,
  launch_config: &LaunchConfig,
  runtime_env: &RuntimeEnv,
  mount_mappings: &[MountMapping],
  empty_file_path: &str,
) -> anyhow::Result<Vec<String>> {
  let mut args: Vec<String> = vec![
    // Kill processes in sandbox when bwrap dies.
    "--die-with-parent".into(),
    // Allow adjusting process niceness, can be checked with "capsh --print".
    "--cap-add".into(),
    "CAP_SYS_NICE".into(),
  ];
  // Re-assign uid and gid if needed, do not confuse with --unshare-user, the later is to unshare
  // the current user namespace.
  if let Some((uid, gid)) = sandbox_config.user_mapping.get_uid_gid_string() {
    args.extend(["--uid".into(), uid, "--gid".into(), gid]);
  }
  if sandbox_config.namespace_isolation {
    // Need to keep IPC namespace (i.e. no --unshare-ipc) in X11 because it breaks some GUI apps
    // i.e. when quickly moving the mouse cursor over the WinRAR menu bar, the application will
    // crash with a "X Error of failed request:  BadValue (integer parameter out of range for
    // operation)" error.
    args.extend([
      "--unshare-pid".into(),
      "--unshare-cgroup".into(),
      "--unshare-user".into(),
    ]);
    if matches!(sandbox_config.display_protocol, DisplayProtocol::Wayland) {
      args.push("--unshare-ipc".into());
    }
  }
  // Use a new UTS space, and a hostname based on the current timestamp.
  args.extend([
    "--unshare-uts".into(),
    "--hostname".into(),
    current_timestamp_hex(),
  ]);
  // Share devices, if NVIDIA devices are missing, weird/misleading gstreamer errors may appear when
  // playing games, like telling you that a gst plugin is missing.
  args.extend(get_device_args(&sandbox_config.device_access)?);
  // System binaries and libraries.
  args.extend([
    "--ro-bind".into(),
    "/bin".into(),
    "/bin".into(),
    "--ro-bind".into(),
    "/lib64".into(),
    "/lib64".into(),
    "--ro-bind".into(),
    "/sbin".into(),
    "/sbin".into(),
    "--ro-bind".into(),
    "/usr".into(),
    "/usr".into(),
    "--symlink".into(),
    "/usr/lib".into(),
    "/lib".into(),
  ]);
  // While --dir itself doesn't inherently leak data from the host, it provides less protection
  // because it allows the container to manage files on a persistent basis (even if those files are
  // contained within the sandbox), i.e., it has greater attack surface. In contrast, --tmpfs
  // provides no chance of interaction with the host filesystem.
  args.extend([
    "--tmpfs".into(),
    "/var".into(),
    "--proc".into(),
    "proc".into(),
    "--tmpfs".into(),
    runtime_env.home_dir.clone(),
    // Some programs may fail to start or crash if /tmp or /dev/shm are not available
    // e.g., X11 apps, Electron apps, wine with Fsync/Esync.
    "--tmpfs".into(),
    "/tmp".into(),
    "--tmpfs".into(),
    "/dev/shm".into(),
  ]);
  // Need to bind /run because it allows D-Bus to work, also some apps that directly or indirectly
  // rely on libudev may fail to access devices like gamepads if /run/udev/data is not accessible.
  // Binding /run works but it exposes more than we need, so only bind D-Bus related paths,
  // i.e. sandboxed apps shouldn't be able to run "DOCKER_HOST=unix:///run/docker.sock docker ps",
  // the aforementioned command works even if --ro-bind was used.
  // Access to /sys is needed for apps to be able to retrieve kernel and hardware information.
  let pulse_cookie = format!("{}/.config/pulse/cookie", runtime_env.home_dir);
  let pulse_socket = format!("{}/pulse/native", runtime_env.xdg_runtime_dir);
  let pipewire_socket = format!("{}/pipewire-0", runtime_env.xdg_runtime_dir);
  args.extend([
    "--ro-bind".into(),
    "/run/dbus".into(),
    "/run/dbus".into(),
    // Mount an empty writable directory as the XDG_RUNTIME_DIR because if "/run/user/USER_ID" is
    // mounted, it would expose sockets like vscode, ssh, kwallet; also needs to be writable because
    // some programs like gamescope may create lock files under this path.
    // TODO: need more testing to see if mounting /run/udev/data makes a meaningful difference, this
    // requires to unset SDL_JOYSTICK_DISABLE_UDEV, a game that has gamepad issues and said gamepad
    // issues not to be related to Steam Input. The expected result is to have a previously
    // non-working gamepad working and to have gamepad hotplugging unaffected.
    // TODO: investigate "0090:err:hid:udev_bus_init UDEV monitor creation failed" errors. Happens
    // with wine-ge-proton8-26.
    "--tmpfs".into(),
    runtime_env.xdg_runtime_dir.clone(),
    "--ro-bind-try".into(),
    pulse_cookie.clone(),
    pulse_cookie,
    "--ro-bind-try".into(),
    pulse_socket.clone(),
    pulse_socket,
    "--ro-bind-try".into(),
    pipewire_socket.clone(),
    pipewire_socket,
    "--ro-bind".into(),
    "/sys".into(),
    "/sys".into(),
  ]);
  // There are just so many things that could be needed under /etc to the point
  // that is not reliable to selectively mount directories under /etc
  // (e.g. DOOM 2016 will fail if no /etc/vulkan is present), so mount all /etc.
  args.extend([
    "--bind".into(),
    "/etc".into(),
    "/etc".into(),
    "--ro-bind".into(),
    empty_file_path.into(),
    "/etc/hostname".into(),
    // Application shared data e.g., "/usr/share/vulkan/icd.d".
    "--ro-bind".into(),
    "/usr/share".into(),
    "/usr/share".into(),
  ]);
  // Setup networking, the bwrap default is enabled, our default will be to have it disabled.
  match sandbox_config.network_mode {
    NetworkMode::FullAccess => (), // No extra arguments required
    NetworkMode::RestrictedAccess => {
      args.extend([
        "--tmpfs".into(),
        "/etc/ca-certificates".into(),
        "--tmpfs".into(),
        "/etc/ssl".into(),
        "--tmpfs".into(),
        "/etc/NetworkManager".into(),
        "--ro-bind".into(),
        empty_file_path.into(),
        "/etc/resolv.conf".into(),
        "--ro-bind".into(),
        empty_file_path.into(),
        "/etc/nsswitch.conf".into(),
        "--ro-bind".into(),
        empty_file_path.into(),
        "/etc/hosts".into(),
      ]);
    }
    NetworkMode::NoAccess => {
      args.push("--unshare-net".into());
    }
  }
  // Mount the directory that contains the Wine binaries and libraries (a.k.a. runner), the Wine
  // version to be mounted must be statically compiled in order to not rely on any host library
  // i.e. the runners downloaded by Bottles are statically compiled.
  if let Some(runner_path) = &launch_config.runner_path {
    args.extend([
      "--tmpfs".into(),
      "/opt".into(),
      "--ro-bind".into(),
      runner_path.to_str().context("bad runner path")?.into(),
      INNER_WINE_ROOT.into(),
    ]);
  }
  // Prefix needs to be read-write because some dependencies may be installed or system files change
  // while wine is running, even changing the registry requires write access.
  if let Some(prefix_info) = &launch_config.prefix_info {
    args.extend([
      "--bind".into(),
      prefix_info.path.to_str().context("bad prefix path")?.into(),
      INNER_WINE_PREFIX.into(),
    ]);
  }
  // Clear env and set minimal required variables, we need to make sure that all needed variables
  // are being passed otherwise games may crash or have no sound.
  // The USER and LANG variables are needed for some games in order to be able to save settings and
  // progress, if not set, the affected game may behave strangely.
  args.extend([
    "--clearenv".into(),
    "--setenv".into(),
    "HOME".into(),
    runtime_env.home_dir.clone(),
    "--setenv".into(),
    "USER".into(),
    runtime_env.user_name.clone(),
    "--setenv".into(),
    "LANG".into(),
    runtime_env.lang.clone(),
    "--setenv".into(),
    "XDG_RUNTIME_DIR".into(),
    runtime_env.xdg_runtime_dir.clone(),
    "--setenv".into(),
    "WINEPREFIX".into(),
    INNER_WINE_PREFIX.into(),
    "--setenv".into(),
    "WINEDLLOVERRIDES".into(),
    "winemenubuilder=''".into(),
    "--setenv".into(),
    "WINE_LARGE_ADDRESS_AWARE".into(),
    "1".into(),
  ]);
  // Allow gamepad hotplugging, otherwise network access or --share-net would be required.
  args.extend([
    "--setenv".into(),
    "SDL_JOYSTICK_DISABLE_UDEV".into(),
    "1".into(),
  ]);
  // Extend the PATH to have access to the Wine binaries without full paths.
  args.extend([
    "--setenv".into(),
    "PATH".into(),
    format!("{}/bin:{}", INNER_WINE_ROOT, runtime_env.original_path),
  ]);
  // GPU cache is saved in the game directory by default, this is undesired because most of the time
  // this directory will be read-only, so put the caches under the prefix (Bottles does the same).
  args.extend([
    "--setenv".into(),
    "__GL_SHADER_DISK_CACHE".into(),
    "1".into(),
    "--setenv".into(),
    "__GL_SHADER_DISK_CACHE_PATH".into(),
    format!("{}/cache/gl_shader", INNER_WINE_PREFIX),
    "--setenv".into(),
    "DXVK_STATE_CACHE_PATH".into(),
    format!("{}/cache/dxvk_state", INNER_WINE_PREFIX),
    "--setenv".into(),
    "MESA_SHADER_CACHE_DIR".into(),
    format!("{}/cache/mesa_shader", INNER_WINE_PREFIX),
    "--setenv".into(),
    "VKD3D_SHADER_CACHE_PATH".into(),
    format!("{}/cache/vkd3d_shader", INNER_WINE_PREFIX),
  ]);
  args.extend(get_display_args(&sandbox_config.display_protocol, runtime_env)?);
  // Configure upscale mode.
  match &launch_config.upscale_mode {
    None | Some(UpscaleMode::None) => (),
    Some(UpscaleMode::Fsr { mode, strength }) => {
      args.extend([
        "--setenv".into(),
        "WINE_FULLSCREEN_FSR".into(),
        "1".into(),
        "--setenv".into(),
        "WINE_FULLSCREEN_FSR_MODE".into(),
        mode.to_string(),
        "--setenv".into(),
        "WINE_FULLSCREEN_FSR_STRENGTH".into(),
        strength.to_string(),
      ]);
    }
    Some(UpscaleMode::Dlss) => {
      args.extend([
        "--setenv".into(),
        "DXVK_NVAPIHACK".into(),
        "0".into(),
        "--setenv".into(),
        "DXVK_ENABLE_NVAPI".into(),
        "1".into(),
      ]);
    }
  }
  // Configure Wine sync mode, only one mode can be set at time.
  match launch_config.sync_mode {
    None | Some(SyncMode::None) => (),
    Some(SyncMode::Fsync) => {
      args.extend(["--setenv".into(), "WINEFSYNC".into(), "1".into()]); // Default for soda runner
    }
    Some(SyncMode::Esync) => {
      args.extend(["--setenv".into(), "WINEESYNC".into(), "1".into()]);
    }
  }
  // Configure verbosity.
  if !sandbox_config.verbose {
    args.extend([
      "--setenv".into(),
      "WINEDEBUG".into(),
      "fixme-all".into(),
      "--setenv".into(),
      "DXVK_LOG_LEVEL".into(),
      "warn".into(),
    ]);
  }
  // Set custom environment variables overrides. If there are 2 variables with the same name set by
  // --setenv, bwrap will use the rightmost one.
  if let Some(env_overrides) = &runtime_env.overrides {
    for (key, value) in env_overrides.into_iter() {
      args.extend(["--setenv".into(), key.clone(), value.clone()])
    }
  }
  let shell_params: Vec<String> = vec![
    "--setenv".into(),
    "TERM".into(),
    runtime_env.term.clone(),
    runtime_env.shell.clone(),
  ];
  // Mount the app directory before additional volumes so volume mappings can override paths inside
  // the app directory.
  match &launch_config.launch_params {
    LaunchParams::AppDirOnly { read_only, app_dir }
    | LaunchParams::AppDirWithCommand {
      read_only, app_dir, ..
    } => {
      args.extend(get_app_dir_args(*read_only, app_dir.to_owned()));
    }
    LaunchParams::Unconfigured => (),
  }
  // Additional mounts.
  args.extend(get_mount_args(mount_mappings));
  // Depending on the launch params, add the necessary arguments to start a regular shell or execute
  // the specified command.
  match &launch_config.launch_params {
    LaunchParams::Unconfigured => {
      // No launch params, so start with a regular shell.
      args.extend(["--chdir".into(), "/".into()]);
      args.extend(shell_params);
    }
    LaunchParams::AppDirOnly { .. } => {
      // Only app_dir was set (not app_bin), so start with default shell (useful for maintenance).
      args.extend(shell_params);
    }
    LaunchParams::AppDirWithCommand {
      app_bin, app_args, ..
    } => {
      args.push(app_bin.to_owned());
      args.extend(app_args.clone());
    }
  }
  Ok(args)
}

pub fn prepare_args(
  sandbox_config: &SandboxConfig,
  launch_config: &LaunchConfig,
  runtime_env: &RuntimeEnv,
  mount_mappings: &[MountMapping],
) -> anyhow::Result<(Vec<String>, NamedTempFile)> {
  // Temporary file will be automatically removed when variable goes out of scope.
  let temp_file = NamedTempFile::new()?;
  let temp_file_path = temp_file
    .path()
    .to_str()
    .context("could not get temporary file path")?;
  let args =
    build_args(sandbox_config, launch_config, runtime_env, mount_mappings, temp_file_path)?;
  Ok((args, temp_file))
}

/// Execute a program under a restricted Bubblewrap container, the output will be inherited by the
/// current terminal and printed in real-time. See detailed parameter information at
/// https://man.archlinux.org/man/extra/bubblewrap/bwrap.1.en.
/// **NOTE:** keep in mind that even if runners (Wine custom builds downloaded through Bottles) are
/// statically compiled, it does not mean they will run without additional dependencies, they are
/// kinda independent of glibc and similar lower level stuff, however they still need the OS to
/// provide the right dependencies, otherwise not even `notepad.exe` will run, to install these
/// dependencies, just install `steam-native-runtime` on Arch/Manjaro.
pub fn run(args: &[String]) -> anyhow::Result<()> {
  let mut cmd = Command::new("bwrap")
    .args(args)
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()
    .map_err(|e| anyhow::anyhow!("could not spawn bwrap: {}", e))?;
  let status = cmd.wait()?;
  if status.success() {
    return Ok(());
  }
  Err(anyhow::anyhow!("the bwrap command exited with non-zero exit code"))
}

/// Checks device binding arguments and temporary-file ownership during argument preparation.
#[cfg(test)]
mod tests {
  use super::*;
  use crate::sandbox::user_mapping::UserMapping;
  use std::{collections::HashMap, ffi::OsStr, os::unix::ffi::OsStrExt};

  #[test]
  fn device_bind_args_preserve_paths_and_order() -> anyhow::Result<()> {
    let devices = [PathBuf::from("/dev/input"), PathBuf::from("/dev/nvidia0")];
    assert_eq!(
      get_device_bind_args(&devices)?,
      vec![
        "--dev-bind",
        "/dev/input",
        "/dev/input",
        "--dev-bind",
        "/dev/nvidia0",
        "/dev/nvidia0"
      ]
    );
    Ok(())
  }

  #[test]
  fn device_bind_args_reject_non_utf8_paths() {
    let device = PathBuf::from(OsStr::from_bytes(b"/dev/nvidia\xff"));
    let error = get_device_bind_args(&[device.clone()]).unwrap_err();
    assert_eq!(error.to_string(), format!("device path is not valid utf-8: {}", device.display()));
  }

  #[test]
  fn prepared_args_keep_empty_file_alive_until_handle_is_dropped() -> anyhow::Result<()> {
    let sandbox = SandboxConfig {
      namespace_isolation: true,
      user_mapping: UserMapping::None,
      display_protocol: DisplayProtocol::Wayland,
      network_mode: NetworkMode::NoAccess,
      device_access: DeviceAccess::All,
      verbose: false,
    };
    let launch = LaunchConfig {
      runner_path: None,
      prefix_info: None,
      launch_params: LaunchParams::Unconfigured,
      upscale_mode: None,
      sync_mode: None,
    };
    let vars = HashMap::from([
      ("HOME".into(), "/home/test".into()),
      ("USER".into(), "test".into()),
      ("XDG_RUNTIME_DIR".into(), "/run/user/1000".into()),
      ("PATH".into(), "/usr/bin:/bin".into()),
      ("WAYLAND_DISPLAY".into(), "wayland-0".into()),
    ]);
    let runtime_env = RuntimeEnv::from_map(&vars)?;
    let (args, file) = prepare_args(&sandbox, &launch, &runtime_env, &[])?;
    let path = file.path().to_owned();
    assert!(
      args
        .windows(3)
        .any(|args| args == ["--ro-bind", path.to_str().unwrap(), "/etc/hostname"])
    );
    assert_eq!(std::fs::read(&path)?, Vec::<u8>::new());
    drop(file);
    assert!(!path.exists());
    Ok(())
  }
}
