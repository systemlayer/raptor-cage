use serde::Deserialize;
use std::{
  collections::HashMap,
  fs,
  io::ErrorKind,
  path::{Path, PathBuf},
};

const CONFIG_FILE_NAME: &str = "rcage.toml";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
  #[serde(default)]
  pub placeholders: HashMap<String, String>,
}

fn config_paths() -> Vec<PathBuf> {
  let mut paths = vec![PathBuf::from(CONFIG_FILE_NAME)];
  if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
    paths.push(PathBuf::from(config_home).join(CONFIG_FILE_NAME));
  }
  if let Some(home) = std::env::var_os("HOME") {
    paths.push(PathBuf::from(home).join(".config").join(CONFIG_FILE_NAME));
  }
  paths
}

fn config_error(path: &Path, error: impl std::fmt::Display) -> anyhow::Error {
  anyhow::anyhow!("could not load configuration {}: {}", path.display(), error)
}

fn load_from_paths<I>(paths: I) -> anyhow::Result<Config>
where
  I: IntoIterator<Item = PathBuf>,
{
  for path in paths {
    let contents = match fs::read_to_string(&path) {
      Ok(contents) => contents,
      Err(error) if error.kind() == ErrorKind::NotFound => continue,
      Err(error) => return Err(config_error(&path, error)),
    };
    return toml::from_str(&contents).map_err(|error| config_error(&path, error));
  }
  Ok(Config::default())
}

pub fn load() -> anyhow::Result<Config> {
  load_from_paths(config_paths())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn write_config(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
  }

  #[test]
  fn missing_configuration_is_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = load_from_paths([temp_dir.path().join("missing.toml")]).unwrap();
    assert!(config.placeholders.is_empty());
  }

  #[test]
  fn loads_placeholder_definitions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join(CONFIG_FILE_NAME);
    write_config(
      &path,
      r#"
        [placeholders]
        GAME_ROOT = "/home/test/my_games"
      "#,
    );
    let config = load_from_paths([path]).unwrap();
    assert_eq!(config.placeholders.get("GAME_ROOT").unwrap(), "/home/test/my_games");
  }

  #[test]
  fn uses_first_existing_configuration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let first = temp_dir.path().join("first.toml");
    let second = temp_dir.path().join("second.toml");
    write_config(&first, "[placeholders]\nSOURCE = 'first'");
    write_config(&second, "[placeholders]\nSOURCE = 'second'");
    let config = load_from_paths([first, second]).unwrap();
    assert_eq!(config.placeholders.get("SOURCE").unwrap(), "first");
  }

  #[test]
  fn invalid_configuration_returns_an_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join(CONFIG_FILE_NAME);
    write_config(&path, "[placeholders\n");
    let error = load_from_paths([path.clone()]).unwrap_err();
    assert!(error.to_string().contains(path.to_string_lossy().as_ref()));
  }
}
