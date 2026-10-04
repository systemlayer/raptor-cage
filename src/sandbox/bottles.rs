use anyhow::Context;
use std::{
  env, fs,
  path::{Path, PathBuf},
};

fn list_directories(path: &Path) -> anyhow::Result<Vec<String>> {
  let mut result: Vec<String> = Vec::new();
  let entries = fs::read_dir(path)
    .with_context(|| format!("failed to read directory: {}", path.to_string_lossy()))?;
  for entry in entries {
    let entry =
      entry.with_context(|| format!("failed to read entry under: {}", path.to_string_lossy()))?;
    if entry.path().is_dir() {
      if let Some(dir_name) = entry.path().file_name() {
        result.push(dir_name.to_string_lossy().to_string());
      }
    }
  }
  Ok(result)
}

/// Get the path where Bottles stores it's data. Keep in mind that `data` is a generic Flatpak
/// location, but the directory that will be returned is `bottles` (the one located under `data`).
/// ```txt
/// ~/.var/app/com.usebottles.bottles/data
/// └── bottles
///     ├── bottles
///     ├── runners
///     ├── runtimes
///     ├── templates
///     ├── data.yml
///     └── ...
/// ```
pub fn get_data_root() -> anyhow::Result<PathBuf> {
  let home_dir = env::var("HOME").context("failed to retrieve $HOME variable")?;
  let data_path = Path::new(&home_dir).join(".var/app/com.usebottles.bottles/data/bottles");
  Ok(data_path)
}

/// Resolves a path relative to a directory in the Bottles data root.
pub fn resolve_path(path: PathBuf, directory: &str) -> anyhow::Result<PathBuf> {
  if path.is_absolute() {
    return Ok(path);
  }
  Ok(get_data_root()?.join(directory).join(path))
}

pub fn list_prefixes(data_root: &Path) -> anyhow::Result<Vec<String>> {
  let prefixes_dir = data_root.join("bottles");
  let result = list_directories(&prefixes_dir);
  result
}

pub fn list_runners(data_root: &Path) -> anyhow::Result<Vec<String>> {
  let runners_dir = data_root.join("runners");
  let result = list_directories(&runners_dir);
  result
}
