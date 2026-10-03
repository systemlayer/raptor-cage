use crate::{
  config, inhibitor,
  sandbox::{
    bottles, bwrap,
    config::{DeviceAccess, DisplayProtocol, NetworkMode, SandboxConfig},
    mount::{MountConfig, MountMapping},
    placeholder::replace_placeholders,
    sandbox::{LaunchConfig, LaunchParams},
    sandbox_config::{INNER_APP_DIR, INNER_WINE_PREFIX},
    user_mapping::UserMapping,
    wine::{SyncMode, UpscaleMode, WinePrefixInfo},
  },
  system::env::{RuntimeEnv, get_env_var},
};
use std::{collections::HashMap, path::PathBuf, str::FromStr};

// TODO: deny "/" and other important dirs.
fn parse_mappings(
  volumes: &[String],
  placeholder_values: &HashMap<String, String>,
) -> anyhow::Result<Vec<MountMapping>> {
  let mut mappings: Vec<MountMapping> = Vec::with_capacity(volumes.len());
  for volume in volumes {
    let vol = replace_placeholders(volume, placeholder_values)?;
    let mapping =
      MountMapping::from_str(&vol).map_err(|e| anyhow::anyhow!("volume error: {}", e))?;
    mappings.push(mapping);
  }
  Ok(mappings)
}

fn parse_app_dir(
  app_dir: Option<String>,
  placeholder_values: &HashMap<String, String>,
) -> anyhow::Result<(Option<String>, bool)> {
  let Some(dir) = app_dir else {
    return Ok((None, true));
  };
  let resolved_dir = replace_placeholders(&dir, placeholder_values)?;
  let mount_config = MountConfig::from_str(&resolved_dir).map_err(|e| anyhow::anyhow!("{}", e))?;
  Ok((Some(mount_config.path.to_string_lossy().into_owned()), !mount_config.writable))
}

fn get_placeholder_values(
  launch_config: &LaunchConfig,
  custom_values: HashMap<String, String>,
) -> anyhow::Result<HashMap<String, String>> {
  let mut values: HashMap<String, String> = HashMap::from([
    ("APP_DIR".into(), INNER_APP_DIR.into()),
    ("HOST_HOME".into(), get_env_var("HOME")?),
    ("HOST_USER".into(), get_env_var("USER")?),
  ]);
  if let Some(runner_path) = &launch_config.runner_path {
    values.insert("RUNNER_PATH".into(), runner_path.to_string_lossy().into_owned());
  }
  let inner_prefix = PathBuf::from(INNER_WINE_PREFIX);
  values.insert("WINE_PREFIX".into(), inner_prefix.to_string_lossy().into_owned());
  if let Some(prefix_info) = &launch_config.prefix_info {
    values.insert(
      "WINE_HOME".into(),
      inner_prefix
        .join("drive_c")
        .join("users")
        .join(prefix_info.user.clone())
        .to_string_lossy()
        .into_owned(),
    );
    values.insert("OUTER_WINE_PREFIX".into(), prefix_info.path.to_string_lossy().into_owned());
    values.insert("OUTER_WINE_HOME".into(), prefix_info.home.to_string_lossy().into_owned());
    values.insert("WINE_USER".into(), prefix_info.user.clone());
  }
  values.extend(custom_values);
  Ok(values)
}

fn join_args(args: &[String]) -> String {
  args
    .iter()
    .map(|s| {
      let needs_quotes = s
        .chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\\');
      if needs_quotes {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        return format!("\"{}\"", escaped);
      }
      s.clone()
    })
    .collect::<Vec<_>>()
    .join(" ")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn launch_config(
    runner_path: Option<PathBuf>,
    prefix_info: Option<WinePrefixInfo>,
  ) -> LaunchConfig {
    LaunchConfig {
      runner_path,
      prefix_info,
      launch_params: LaunchParams::Unconfigured,
      upscale_mode: None,
      sync_mode: None,
    }
  }

  #[test]
  fn test_placeholder_values_include_host_environment() {
    let values = get_placeholder_values(&launch_config(None, None), HashMap::new()).unwrap();
    assert_eq!(values.get("APP_DIR"), Some(&INNER_APP_DIR.to_string()));
    assert_eq!(values.get("HOST_HOME"), Some(&get_env_var("HOME").unwrap()));
    assert_eq!(values.get("HOST_USER"), Some(&get_env_var("USER").unwrap()));
  }

  #[test]
  fn test_placeholder_values_include_runner_path() {
    let runner_path = PathBuf::from("/opt/wine runners/custom");
    let values =
      get_placeholder_values(&launch_config(Some(runner_path.clone()), None), HashMap::new())
        .unwrap();
    assert_eq!(values.get("RUNNER_PATH"), Some(&runner_path.to_string_lossy().into_owned()));
    assert_eq!(values.get("WINE_PREFIX"), Some(&INNER_WINE_PREFIX.to_string()));
    assert!(!values.contains_key("OUTER_WINE_PREFIX"));
  }

  #[test]
  fn test_placeholder_values_include_custom_values() {
    let custom_values = HashMap::from([("GAME_ROOT".into(), "/home/test/my_games".into())]);
    let values = get_placeholder_values(&launch_config(None, None), custom_values).unwrap();
    assert_eq!(values.get("GAME_ROOT"), Some(&"/home/test/my_games".to_string()));
  }

  #[test]
  fn test_placeholder_values_include_wine_prefix_details() {
    let prefix_path = PathBuf::from("/games/prefixes/example");
    let wine_home = prefix_path.join("drive_c/users/player");
    let prefix_info = WinePrefixInfo {
      path: prefix_path.clone(),
      user: "player".into(),
      home: wine_home.clone(),
    };
    let values =
      get_placeholder_values(&launch_config(None, Some(prefix_info)), HashMap::new()).unwrap();
    assert_eq!(values.get("WINE_PREFIX"), Some(&INNER_WINE_PREFIX.to_string()));
    assert_eq!(
      values.get("WINE_HOME"),
      Some(
        &PathBuf::from(INNER_WINE_PREFIX)
          .join("drive_c/users/player")
          .to_string_lossy()
          .into_owned()
      )
    );
    assert_eq!(values.get("OUTER_WINE_PREFIX"), Some(&prefix_path.to_string_lossy().into_owned()));
    assert_eq!(values.get("OUTER_WINE_HOME"), Some(&wine_home.to_string_lossy().into_owned()));
    assert_eq!(values.get("WINE_USER"), Some(&"player".to_string()));
    assert!(!values.contains_key("RUNNER_PATH"));
  }

  #[test]
  fn test_parse_mappings_replaces_placeholders() {
    let volumes = vec!["{{SOURCE}}/games:{{TARGET}}/games:rw".to_string()];
    let values = HashMap::from([
      ("SOURCE".to_string(), "/host".to_string()),
      ("TARGET".to_string(), "/sandbox".to_string()),
    ]);
    let mappings = parse_mappings(&volumes, &values).unwrap();
    assert_eq!(
      mappings,
      vec![MountMapping {
        source_path: PathBuf::from("/host/games"),
        target_config: MountConfig {
          path: PathBuf::from("/sandbox/games"),
          writable: true,
        },
      }]
    );
  }

  #[test]
  fn test_parse_mappings_validates_resolved_paths() {
    let volumes = vec!["{{SOURCE}}:/sandbox".to_string()];
    let values = HashMap::from([("SOURCE".to_string(), "/".to_string())]);
    let error = parse_mappings(&volumes, &values).unwrap_err();
    assert_eq!(error.to_string(), "volume error: path is not allowed: /");
  }

  #[test]
  fn test_parse_mappings_rejects_unknown_placeholders() {
    let volumes = vec!["{{UNKNOWN}}:/sandbox".to_string()];
    let error = parse_mappings(&volumes, &HashMap::new()).unwrap_err();
    assert_eq!(error.to_string(), "unknown placeholder: {{UNKNOWN}}");
  }

  #[test]
  fn test_parse_app_dir_replaces_placeholders() {
    let values = HashMap::from([("GAMES_ROOT".to_string(), "/mnt/games".to_string())]);
    let (app_dir, read_only) =
      parse_app_dir(Some("{{GAMES_ROOT}}/mygame:rw".to_string()), &values).unwrap();
    assert_eq!(app_dir, Some("/mnt/games/mygame".to_string()));
    assert!(!read_only);
  }

  #[test]
  fn test_no_quotes_needed() {
    let input = vec!["hello".to_string(), "world".to_string()];
    let result = join_args(&input);
    assert_eq!(result, "hello world");
  }

  #[test]
  fn test_with_spaces() {
    let input = vec!["foo bar".to_string(), "baz".to_string()];
    let result = join_args(&input);
    assert_eq!(result, r#""foo bar" baz"#);
  }

  #[test]
  fn test_with_quotes() {
    let input = vec!["a\"b".to_string()];
    let result = join_args(&input);
    assert_eq!(result, r#""a\"b""#);
  }

  #[test]
  fn test_with_backslashes() {
    let input = vec![r#"a\b\c"#.to_string()];
    let result = join_args(&input);
    assert_eq!(result, r#""a\\b\\c""#);
  }

  #[test]
  fn test_mixed_values() {
    let input = vec![
      "simple".to_string(),
      "foo bar".to_string(),
      r#"x"y"#.to_string(),
    ];
    let result = join_args(&input);
    assert_eq!(result, r#"simple "foo bar" "x\"y""#);
  }
}

pub async fn run(
  environment: &[String],
  volumes: &[String],
  no_namespace_isolation: bool,
  user_mapping: UserMapping,
  display_protocol: DisplayProtocol,
  network_mode: NetworkMode,
  device_access: DeviceAccess,
  verbose: bool,
  upscale_mode: UpscaleMode,
  sync_mode: SyncMode,
  process_names: Option<Vec<String>>,
  runner_path: Option<PathBuf>,
  prefix_path: Option<PathBuf>,
  app_dir: Option<String>,
  app_bin: Option<String>,
  app_args: Option<Vec<String>>,
) -> anyhow::Result<()> {
  if runner_path.as_ref().xor(prefix_path.as_ref()).is_some() {
    anyhow::bail!("either both runner and prefix paths are required, or neither");
  }
  let config = config::load()?;
  let sandbox_config = SandboxConfig {
    namespace_isolation: !no_namespace_isolation,
    user_mapping,
    display_protocol,
    network_mode,
    device_access,
    verbose,
  };
  let mut runtime_env = RuntimeEnv::from_env()?;
  let runner_path = runner_path
    .map(|path| bottles::resolve_path(path, "runners"))
    .transpose()?;
  let prefix_info = prefix_path
    .map(|path| bottles::resolve_path(path, "bottles"))
    .transpose()?
    .map(|path| WinePrefixInfo::new(path, &runtime_env.user_name))
    .transpose()?;
  let mut launch_config = LaunchConfig {
    runner_path,
    prefix_info,
    launch_params: LaunchParams::Unconfigured,
    upscale_mode: Some(upscale_mode),
    sync_mode: Some(sync_mode),
  };
  let placeholder_values = get_placeholder_values(&launch_config, config.placeholders)?;
  let (app_dir, read_only) = parse_app_dir(app_dir, &placeholder_values)?;
  launch_config.launch_params =
    LaunchParams::from_options(read_only, app_dir, app_bin, app_args, process_names);
  let env_overrides: HashMap<String, String> = environment
    .into_iter()
    .map(|item| {
      let (key, val) = item.split_once('=').unwrap_or((&item, ""));
      (key.to_string(), val.to_string())
    })
    .collect();
  if let Some(prefix_info) = &launch_config.prefix_info {
    // Need to set current user because some games rely on this variable to build the save/config
    // path. If not set, the behavior depends on the game, common symptoms include saving data to
    // the root path (e.g., "/My Games"), silently failing to save settings or broken UI.
    runtime_env.user_name.clone_from(&prefix_info.user);
  }
  runtime_env.overrides = Some(env_overrides);
  let mount_mappings = parse_mappings(volumes, &placeholder_values)?;
  // Inhibit the system so screen does not dim while running a game, inhibition will be
  // automatically released when inhibit_handle is dropped.
  let inhibit_handle = inhibitor::inhibit_idle().await;
  if let Err(inhibit_error) = &inhibit_handle {
    println!("Inhibition failed: {}", inhibit_error);
  }
  // Need to prefix _temp_file to acknowledge is not being used, if "_" is used alone, it will be
  // dropped immediately, thus the temporary file will be removed.
  let (args, _temp_file) =
    bwrap::prepare_args(&sandbox_config, &launch_config, &runtime_env, &mount_mappings)?;
  if verbose {
    println!("Arguments: {}", join_args(&args));
  }
  bwrap::run(&args)
}
