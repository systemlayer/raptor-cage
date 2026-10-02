use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

// https://github.com/rust-lang/cargo/blob/4c06c57d0dc303b2bc93a5a52f5b962cae48bbce/crates/cargo-util/src/paths.rs#L84.
fn normalize_path<P>(path: P) -> PathBuf
where
  P: AsRef<Path>,
{
  let mut components = path.as_ref().components().peekable();
  let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
    components.next();
    PathBuf::from(c.as_os_str())
  } else {
    PathBuf::new()
  };
  for component in components {
    match component {
      Component::Prefix(..) => unreachable!(),
      Component::RootDir => {
        ret.push(Component::RootDir);
      }
      Component::CurDir => {}
      Component::ParentDir => {
        if ret.ends_with(Component::ParentDir) {
          ret.push(Component::ParentDir);
        } else {
          let popped = ret.pop();
          if !popped && !ret.has_root() {
            ret.push(Component::ParentDir);
          }
        }
      }
      Component::Normal(c) => {
        ret.push(c);
      }
    }
  }
  ret
}

#[derive(Debug, PartialEq)]
pub enum MountError {
  EmptyPath,
  DisallowedPath(PathBuf),
  InvalidFormat(String),
}

impl fmt::Display for MountError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      MountError::EmptyPath => write!(f, "path must not be empty"),
      MountError::DisallowedPath(path) => {
        write!(f, "path is not allowed: {}", path.to_string_lossy())
      }
      MountError::InvalidFormat(value) => write!(f, "invalid format: {}", value),
    }
  }
}

fn resolve_mount_path(mount_path: &str) -> Result<PathBuf, MountError> {
  let path = PathBuf::from(mount_path);
  // This will help us identify paths that we want to be forbidden to mount (such as "/"), bwrap
  // will take care of making sure that the path is valid. Avoid using fs::canonicalize because
  // that one needs the paths to exist on the filesystem.
  if normalize_path(&path).to_string_lossy().to_string() == "/" {
    return Err(MountError::DisallowedPath(path));
  }
  Ok(path)
}

/// Contains the configuration for a single path and flags with the `path:flags`
/// syntax. For multiple paths see `MountMapping`.
#[derive(Debug, PartialEq)]
pub struct MountConfig {
  pub path: PathBuf,
  pub writable: bool,
}

impl FromStr for MountConfig {
  type Err = MountError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut parts: Vec<&str> = s.rsplitn(2, ":").collect();
    parts.reverse();
    if parts[0].is_empty() {
      return Err(MountError::EmptyPath);
    }
    let path = resolve_mount_path(parts[0])?;
    let writable = parts.len() > 1 && parts[1].split(',').any(|flag| flag == "rw");
    Ok(Self { path, writable })
  }
}

/// Represents a source path, target path and mount flags, valid values look
/// like `source:target:flags`.
#[derive(Debug, PartialEq)]
pub struct MountMapping {
  pub source_path: PathBuf,
  pub target_config: MountConfig,
}

impl FromStr for MountMapping {
  type Err = MountError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let parts: Vec<&str> = s.splitn(2, ":").collect();
    if parts[0].is_empty() {
      return Err(MountError::EmptyPath);
    }
    let source_path = resolve_mount_path(parts[0])?;
    let target_part = parts
      .get(1)
      .ok_or_else(|| MountError::InvalidFormat(s.to_owned()))?;
    let target_config = MountConfig::from_str(target_part)?;
    Ok(Self {
      source_path,
      target_config,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;
  use std::str::FromStr;

  #[test]
  fn test_absolute_path() {
    let result = normalize_path("/home/user/../user/docs");
    assert_eq!(result, PathBuf::from("/home/user/docs"));
  }

  #[test]
  fn test_relative_path() {
    let result = normalize_path("docs/../user/docs");
    assert_eq!(result, PathBuf::from("user/docs"));
  }

  #[test]
  fn test_redundant_slashes() {
    let result = normalize_path("/////home//user///docs///file.txt");
    assert_eq!(result, PathBuf::from("/home/user/docs/file.txt"));
  }

  #[test]
  fn test_current_directory() {
    let result = normalize_path("docs/./user/././file.txt");
    assert_eq!(result, PathBuf::from("docs/user/file.txt"));
  }

  #[test]
  fn test_empty_path() {
    let result = normalize_path("");
    assert_eq!(result, PathBuf::from(""));
  }

  #[test]
  fn test_parent_directory() {
    let result = normalize_path("docs/../user/../file.txt");
    assert_eq!(result, PathBuf::from("file.txt"));
  }

  #[test]
  fn test_mount_config_parsing() {
    let test_cases = vec![
      ("", Err(MountError::EmptyPath)),
      (":", Err(MountError::EmptyPath)),
      (
        "/usr/bin",
        Ok(MountConfig {
          path: PathBuf::from("/usr/bin"),
          writable: false,
        }),
      ),
      (
        "/usr/bin:",
        Ok(MountConfig {
          path: PathBuf::from("/usr/bin"),
          writable: false,
        }),
      ),
      (
        "/usr/bin:rw",
        Ok(MountConfig {
          path: PathBuf::from("/usr/bin"),
          writable: true,
        }),
      ),
      (
        "/usr/bin:noexec,rw",
        Ok(MountConfig {
          path: PathBuf::from("/usr/bin"),
          writable: true,
        }),
      ),
      (
        "/does/not/exist",
        Ok(MountConfig {
          path: PathBuf::from("/does/not/exist"),
          writable: false,
        }),
      ),
      ("/", Err(MountError::DisallowedPath(PathBuf::from("/")))),
      ("/./", Err(MountError::DisallowedPath(PathBuf::from("/")))),
    ];
    for (input, expected) in test_cases {
      let result = MountConfig::from_str(input);
      assert_eq!(result, expected);
    }
  }

  #[test]
  fn test_mount_mapping_parsing() {
    let test_cases = vec![
      ("", Err(MountError::EmptyPath)),
      (":", Err(MountError::EmptyPath)),
      (":rw", Err(MountError::EmptyPath)),
      ("::rw", Err(MountError::EmptyPath)),
      ("/test:", Err(MountError::EmptyPath)),
      (":/test", Err(MountError::EmptyPath)),
      ("/:/test", Err(MountError::DisallowedPath(PathBuf::from("/")))),
      ("/./:/test", Err(MountError::DisallowedPath(PathBuf::from("/")))),
      ("/data/:/", Err(MountError::DisallowedPath(PathBuf::from("/")))),
      ("/data/:/./", Err(MountError::DisallowedPath(PathBuf::from("/")))),
      ("/data/:/./:", Err(MountError::DisallowedPath(PathBuf::from("/")))),
      ("/data/:/./:rw", Err(MountError::DisallowedPath(PathBuf::from("/")))),
      ("data", Err(MountError::InvalidFormat("data".into()))),
      (
        "./:/test",
        Ok(MountMapping {
          source_path: PathBuf::from("./"),
          target_config: MountConfig {
            path: PathBuf::from("/test"),
            writable: false,
          },
        }),
      ),
      (
        "/data:/test/.:rw",
        Ok(MountMapping {
          source_path: PathBuf::from("/data"),
          target_config: MountConfig {
            path: PathBuf::from("/test"),
            writable: true,
          },
        }),
      ),
    ];
    for (input, expected) in test_cases {
      let result = MountMapping::from_str(input);
      assert_eq!(result, expected);
    }
  }
}
