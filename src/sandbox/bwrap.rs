use super::config::{DeviceAccess, DisplayProtocol, NetworkMode, SandboxConfig};
use super::mount::MountMapping;
use super::sandbox::{LaunchConfig, LaunchParams};
use super::sandbox_config::{
  INNER_APP_DIR, INNER_WINE_PREFIX, INNER_WINE_ROOT, current_timestamp_hex, find_nvidia_devices,
};
use super::wine::{SyncMode, UpscaleMode};
use crate::system::display::X11Display;
use crate::system::env::RuntimeEnv;
use anyhow::Context;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

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
      let mut devices: Vec<String> = vec!["/dev/input", "/dev/uinput", "/dev/dri"]
        .into_iter()
        .map(String::from)
        .collect();
      devices.extend(nvidia_devices);
      let args: Vec<String> = devices
        .into_iter()
        .flat_map(|d| vec!["--dev-bind".to_string(), d.to_owned(), d.to_owned()])
        .collect();
      Ok(args)
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
  let mut args = vec![
    // Kill processes in sandbox when bwrap dies.
    "--die-with-parent",
    // Allow adjusting process niceness, can be checked with "capsh --print".
    "--cap-add",
    "CAP_SYS_NICE",
  ];
  let uid: String;
  let gid: String;
  // Re-assign uid and gid if needed, do not confuse with --unshare-user, the later is to unshare
  // the current user namespace.
  if let Some(uid_gid) = sandbox_config.user_mapping.get_uid_gid_string() {
    (uid, gid) = uid_gid;
    args.extend(["--uid", &uid, "--gid", &gid]);
  }
  if sandbox_config.namespace_isolation {
    // Need to keep IPC namespace (i.e. no --unshare-ipc) in X11 because it breaks some GUI apps
    // i.e. when quickly moving the mouse cursor over the WinRAR menu bar, the application will
    // crash with a "X Error of failed request:  BadValue (integer parameter out of range for
    // operation)" error.
    args.extend(["--unshare-pid", "--unshare-cgroup", "--unshare-user"]);
    if matches!(sandbox_config.display_protocol, DisplayProtocol::Wayland) {
      args.push("--unshare-ipc");
    }
  }
  // Use a new UTS space, and a hostname based on the current timestamp.
  let timestamp = current_timestamp_hex();
  args.extend(["--unshare-uts", "--hostname", &timestamp]);
  // Share devices, if NVIDIA devices are missing, weird/misleading gstreamer errors may appear when
  // playing games, like telling you that a gst plugin is missing.
  let device_args = get_device_args(&sandbox_config.device_access)?;
  args.extend(device_args.iter().map(|a| a.as_str()));
  // System binaries and libraries.
  args.extend([
    "--ro-bind",
    "/bin",
    "/bin",
    "--ro-bind",
    "/lib64",
    "/lib64",
    "--ro-bind",
    "/sbin",
    "/sbin",
    "--ro-bind",
    "/usr",
    "/usr",
    "--symlink",
    "/usr/lib",
    "/lib",
  ]);
  // While --dir itself doesn't inherently leak data from the host, it provides less protection
  // because it allows the container to manage files on a persistent basis (even if those files are
  // contained within the sandbox), i.e., it has greater attack surface. In contrast, --tmpfs
  // provides no chance of interaction with the host filesystem.
  args.extend([
    "--tmpfs",
    "/var",
    "--proc",
    "proc",
    "--tmpfs",
    &runtime_env.home_dir,
    // Some programs may fail to start or crash if /tmp or /dev/shm are not available
    // e.g., X11 apps, Electron apps, wine with Fsync/Esync.
    "--tmpfs",
    "/tmp",
    "--tmpfs",
    "/dev/shm",
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
    "--ro-bind",
    "/run/dbus",
    "/run/dbus",
    // Mount an empty writable directory as the XDG_RUNTIME_DIR because if "/run/user/USER_ID" is
    // mounted, it would expose sockets like vscode, ssh, kwallet; also needs to be writable because
    // some programs like gamescope may create lock files under this path.
    // TODO: need more testing to see if mounting /run/udev/data makes a meaningful difference, this
    // requires to unset SDL_JOYSTICK_DISABLE_UDEV, a game that has gamepad issues and said gamepad
    // issues not to be related to Steam Input. The expected result is to have a previously
    // non-working gamepad working and to have gamepad hotplugging unaffected.
    // TODO: investigate "0090:err:hid:udev_bus_init UDEV monitor creation failed" errors. Happens
    // with wine-ge-proton8-26.
    "--tmpfs",
    &runtime_env.xdg_runtime_dir,
    "--ro-bind-try",
    &pulse_cookie,
    &pulse_cookie,
    "--ro-bind-try",
    &pulse_socket,
    &pulse_socket,
    "--ro-bind-try",
    &pipewire_socket,
    &pipewire_socket,
    "--ro-bind",
    "/sys",
    "/sys",
  ]);
  // There are just so many things that could be needed under /etc to the point
  // that is not reliable to selectively mount directories under /etc
  // (e.g. DOOM 2016 will fail if no /etc/vulkan is present), so mount all /etc.
  args.extend([
    "--bind",
    "/etc",
    "/etc",
    "--ro-bind",
    empty_file_path,
    "/etc/hostname",
    // Application shared data e.g., "/usr/share/vulkan/icd.d".
    "--ro-bind",
    "/usr/share",
    "/usr/share",
  ]);
  // Setup networking, the bwrap default is enabled, our default will be to have it disabled.
  match sandbox_config.network_mode {
    NetworkMode::FullAccess => (), // No extra arguments required
    NetworkMode::RestrictedAccess => {
      args.extend([
        "--tmpfs",
        "/etc/ca-certificates",
        "--tmpfs",
        "/etc/ssl",
        "--tmpfs",
        "/etc/NetworkManager",
        "--ro-bind",
        empty_file_path,
        "/etc/resolv.conf",
        "--ro-bind",
        empty_file_path,
        "/etc/nsswitch.conf",
        "--ro-bind",
        empty_file_path,
        "/etc/hosts",
      ]);
    }
    NetworkMode::NoAccess => {
      args.push("--unshare-net");
    }
  }
  // Mount the directory that contains the Wine binaries and libraries (a.k.a. runner), the Wine
  // version to be mounted must be statically compiled in order to not rely on any host library
  // i.e. the runners downloaded by Bottles are statically compiled.
  if let Some(runner_path) = &launch_config.runner_path {
    args.extend([
      "--tmpfs",
      "/opt",
      "--ro-bind",
      runner_path.to_str().context("bad runner path")?,
      INNER_WINE_ROOT,
    ]);
  }
  // Prefix needs to be read-write because some dependencies may be installed or system files change
  // while wine is running, even changing the registry requires write access.
  if let Some(prefix_info) = &launch_config.prefix_info {
    args.extend([
      "--bind",
      prefix_info.path.to_str().context("bad prefix path")?,
      INNER_WINE_PREFIX,
    ]);
  }
  // Clear env and set minimal required variables, we need to make sure that all needed variables
  // are being passed otherwise games may crash or have no sound.
  // The USER and LANG variables are needed for some games in order to be able to save settings and
  // progress, if not set, the affected game may behave strangely.
  args.extend([
    "--clearenv",
    "--setenv",
    "HOME",
    &runtime_env.home_dir,
    "--setenv",
    "USER",
    &runtime_env.user_name,
    "--setenv",
    "LANG",
    &runtime_env.lang,
    "--setenv",
    "XDG_RUNTIME_DIR",
    &runtime_env.xdg_runtime_dir,
    "--setenv",
    "WINEPREFIX",
    INNER_WINE_PREFIX,
    "--setenv",
    "WINEDLLOVERRIDES",
    "winemenubuilder=''",
    "--setenv",
    "WINE_LARGE_ADDRESS_AWARE",
    "1",
  ]);
  // Allow gamepad hotplugging, otherwise network access or --share-net would be required.
  args.extend(["--setenv", "SDL_JOYSTICK_DISABLE_UDEV", "1"]);
  // Extend the PATH to have access to the Wine binaries without full paths.
  let new_path = format!("{}/bin:{}", INNER_WINE_ROOT, runtime_env.original_path);
  args.extend(["--setenv", "PATH", &new_path]);
  // GPU cache is saved in the game directory by default, this is undesired because most of the time
  // this directory will be read-only, so put the caches under the prefix (Bottles does the same).
  let gl_cache_path = format!("{}/cache/gl_shader", INNER_WINE_PREFIX);
  let dxvk_cache_path = format!("{}/cache/dxvk_state", INNER_WINE_PREFIX);
  let mesa_cache_path = format!("{}/cache/mesa_shader", INNER_WINE_PREFIX);
  let vkd3d_cache_path = format!("{}/cache/vkd3d_shader", INNER_WINE_PREFIX);
  args.extend([
    "--setenv",
    "__GL_SHADER_DISK_CACHE",
    "1",
    "--setenv",
    "__GL_SHADER_DISK_CACHE_PATH",
    &gl_cache_path,
    "--setenv",
    "DXVK_STATE_CACHE_PATH",
    &dxvk_cache_path,
    "--setenv",
    "MESA_SHADER_CACHE_DIR",
    &mesa_cache_path,
    "--setenv",
    "VKD3D_SHADER_CACHE_PATH",
    &vkd3d_cache_path,
  ]);
  let display_args = get_display_args(&sandbox_config.display_protocol, runtime_env)?;
  args.extend(display_args.iter().map(|a| a.as_str()));
  // Configure upscale mode.
  let fsr_mode: String;
  let fsr_strength: String;
  match &launch_config.upscale_mode {
    None | Some(UpscaleMode::None) => (),
    Some(UpscaleMode::Fsr { mode, strength }) => {
      fsr_mode = mode.to_string();
      fsr_strength = strength.to_string();
      args.extend([
        "--setenv",
        "WINE_FULLSCREEN_FSR",
        "1",
        "--setenv",
        "WINE_FULLSCREEN_FSR_MODE",
        &fsr_mode,
        "--setenv",
        "WINE_FULLSCREEN_FSR_STRENGTH",
        &fsr_strength,
      ]);
    }
    Some(UpscaleMode::Dlss) => {
      args.extend([
        "--setenv",
        "DXVK_NVAPIHACK",
        "0",
        "--setenv",
        "DXVK_ENABLE_NVAPI",
        "1",
      ]);
    }
  }
  // Configure Wine sync mode, only one mode can be set at time.
  match launch_config.sync_mode {
    None | Some(SyncMode::None) => (),
    Some(SyncMode::Fsync) => {
      args.extend(["--setenv", "WINEFSYNC", "1"]); // Default for soda runner
    }
    Some(SyncMode::Esync) => {
      args.extend(["--setenv", "WINEESYNC", "1"]);
    }
  }
  // Configure verbosity.
  if !sandbox_config.verbose {
    args.extend([
      "--setenv",
      "WINEDEBUG",
      "fixme-all",
      "--setenv",
      "DXVK_LOG_LEVEL",
      "warn",
    ]);
  }
  // Set custom environment variables overrides. If there are 2 variables with the same name set by
  // --setenv, bwrap will use the rightmost one.
  if let Some(env_overrides) = &runtime_env.overrides {
    for (key, value) in env_overrides.into_iter() {
      args.extend(["--setenv", &key, &value])
    }
  }
  // Method return contains a Vec<String> because it needs to own each element, we initially declare
  // args as Vec<&str> to make it easier to add elements (so we avoid String::from() or .into() on
  // each element), however args still needs to be converted to Vec<String> at the end.
  let mut final_args: Vec<String> = args.into_iter().map(String::from).collect();
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
      let app_dir_args = get_app_dir_args(*read_only, app_dir.to_owned());
      final_args.extend(app_dir_args);
    }
    LaunchParams::Unconfigured => (),
  }
  // Additional mounts.
  let mount_args = get_mount_args(mount_mappings);
  final_args.extend(mount_args);
  // Depending on the launch params, add the necessary arguments to start a regular shell or execute
  // the specified command.
  match &launch_config.launch_params {
    LaunchParams::Unconfigured => {
      // No launch params, so start with a regular shell.
      final_args.extend(["--chdir".into(), "/".into()]);
      final_args.extend(shell_params);
    }
    LaunchParams::AppDirOnly { .. } => {
      // Only app_dir was set (not app_bin), so start with default shell (useful for maintenance).
      final_args.extend(shell_params);
    }
    LaunchParams::AppDirWithCommand {
      app_bin, app_args, ..
    } => {
      final_args.push(app_bin.to_owned());
      final_args.extend(app_args.clone());
    }
  }
  Ok(final_args)
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
