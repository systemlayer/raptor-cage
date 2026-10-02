use anyhow::Context;
use std::{collections::HashMap, env};

fn format_env_error(name: &str) -> String {
  format!("environment variable '{}' is not set or invalid", name)
}

/// Retrieves an env variable, the difference between this method and using
/// env::var directly, is that this method includes the name in the error.
pub fn get_env_var(name: &str) -> anyhow::Result<String> {
  env::var(name).with_context(|| format_env_error(name))
}

pub struct RuntimeEnv {
  pub home_dir: String,
  /// The user name that will be passed to the sandbox, it's initially based on
  /// the `USER` env variable, however its value can be set the the Wine user
  /// if available.
  pub user_name: String,
  pub lang: String,
  /// Path that contains socket and lock files. This includes the Wayland socket.
  pub xdg_runtime_dir: String,
  /// Represents the unmodified value of the PATH variable.
  pub original_path: String,
  /// X11 display address, can look like `:0`, `:1` or `localhost:0.0`.
  x11_display: Option<String>,
  /// Needed on X11 sessions, and by Gamescope.
  xauthority_file: Option<String>,
  /// Wayland display socket name, looks like `wayland-0`.
  wayland_display: Option<String>,
  /// Tells programs how to control the display (e.g., colors, cursor movement).
  pub term: String,
  /// Default command-line interpreter.
  pub shell: String,
  /// Additional env variables set (e.g. set by the user or Bottles).
  pub overrides: Option<HashMap<String, String>>,
}

impl RuntimeEnv {
  pub fn from_env() -> anyhow::Result<Self> {
    let home_dir = get_env_var("HOME")?;
    let user_name = get_env_var("USER")?;
    let xdg_runtime_dir = get_env_var("XDG_RUNTIME_DIR")?;
    let original_path = get_env_var("PATH")?;
    Ok(Self {
      home_dir,
      user_name,
      lang: env::var("LANG").unwrap_or("en_US.UTF-8".to_owned()),
      xdg_runtime_dir,
      original_path,
      x11_display: env::var("DISPLAY").ok(),
      xauthority_file: env::var("XAUTHORITY").ok(),
      wayland_display: env::var("WAYLAND_DISPLAY").ok(),
      term: env::var("TERM").unwrap_or("xterm-256color".into()),
      shell: env::var("SHELL").unwrap_or("bash".into()),
      overrides: None,
    })
  }

  pub fn x11_display(&self) -> anyhow::Result<&str> {
    self
      .x11_display
      .as_deref()
      .with_context(|| format_env_error("DISPLAY"))
  }

  pub fn xauthority_file(&self) -> anyhow::Result<&str> {
    self
      .xauthority_file
      .as_deref()
      .with_context(|| format_env_error("XAUTHORITY"))
  }

  pub fn wayland_display(&self) -> anyhow::Result<&str> {
    self
      .wayland_display
      .as_deref()
      .with_context(|| format_env_error("WAYLAND_DISPLAY"))
  }
}
