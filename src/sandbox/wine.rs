use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::{fmt, fs, io};

const MIN_FSR_STRENGTH: u8 = 0;
const MAX_FSR_STRENGTH: u8 = 5;

#[derive(Debug)]
pub enum UpscaleModeError {
  InvalidUpscaleMode(String),
  InvalidFsrMode(String),
  InvalidFsrStrength(String),
  OutOfRangeFsrStrength(u8),
}

impl fmt::Display for UpscaleModeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      UpscaleModeError::InvalidUpscaleMode(s) => write!(f, "invalid upscale mode: {}", s),
      UpscaleModeError::InvalidFsrMode(s) => write!(f, "invalid FSR mode: {}", s),
      UpscaleModeError::InvalidFsrStrength(s) => write!(f, "invalid FSR strength: {}", s),
      UpscaleModeError::OutOfRangeFsrStrength(strength) => write!(
        f,
        "strength for FSR must be between {} and {}, but got {}",
        MIN_FSR_STRENGTH, MAX_FSR_STRENGTH, strength
      ),
    }
  }
}

impl std::error::Error for UpscaleModeError {}

/// Enum representing the different FSR modes.
/// See also https://github.com/sonic2kk/steamtinkerlaunch/wiki/Wine-FSR.
///
/// | FSR 2.0 Quality Mode | Description                                                                                      | Scale Factor           | Input Resolution | Output Resolution |
/// |----------------------|--------------------------------------------------------------------------------------------------|------------------------|------------------|-------------------|
/// | **Quality**           | Quality mode provides an image quality equal or superior to native rendering with a significant performance gain. | 1.5x per dimension (2.25x area scale) (67% screen resolution) | 1280 x 720 <br> 1706 x 960 <br> 2293 x 960 <br> 2560 x 1440 | 1920 x 1080 <br> 2560 x 1440 <br> 3440 x 1440 <br> 3840 x 2160 |
/// | **Balanced**          | Balanced mode offers an ideal compromise between image quality and performance gains.            | 1.7x per dimension (2.89x area scale) (59% screen resolution) | 1129 x 635 <br> 1506 x 847 <br> 2024 x 960 <br> 2259 x 1270 | 1920 x 1080 <br> 2560 x 1440 <br> 3440 x 1440 <br> 3840 x 2160 |
/// | **Performance**       | Performance mode provides an image quality similar to native rendering with a major performance gain. | 2.0x per dimension (4x area scale) (50% screen resolution)  | 960 x 540 <br> 1280 x 720 <br> 1720 x 960 <br> 1920 x 1080  | 1920 x 1080 <br> 2560 x 1440 <br> 3440 x 1440 <br> 3840 x 2160 |
/// | **Ultra Performance** | Ultra Performance mode provides the highest performance gain while still maintaining an image quality representative of native rendering. | 3.0x per dimension (9x area scale) (33% screen resolution) | 640 x 360 <br> 854 x 480 <br> 1147 x 480 <br> 1280 x 720    | 1920 x 1080 <br> 2560 x 1440 <br> 3440 x 1440 <br> 3840 x 2160 |
#[derive(Debug, Clone)]
pub enum FsrMode {
  None,
  Quality,
  Balanced,
  Performance,
  UltraPerformance,
}

impl fmt::Display for FsrMode {
  /// Converts the FsrMode enum variant to a String, the resulting strings will be the same as the
  /// ones used by `WINE_FULLSCREEN_FSR_MODE`.
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mode_str = match self {
      FsrMode::None => "none",
      FsrMode::Quality => "quality",
      FsrMode::Balanced => "balanced",
      FsrMode::Performance => "performance",
      FsrMode::UltraPerformance => "ultra",
    };
    write!(f, "{}", mode_str)
  }
}

impl FromStr for FsrMode {
  type Err = UpscaleModeError;
  /// Parse a mode from a string, single characters for each mode are also accepted for convenience.
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "none" | "n" => Ok(FsrMode::None),
      "quality" | "q" => Ok(FsrMode::Quality),
      "balanced" | "b" => Ok(FsrMode::Balanced),
      "performance" | "p" => Ok(FsrMode::Performance),
      "ultra" | "u" => Ok(FsrMode::UltraPerformance),
      _ => Err(UpscaleModeError::InvalidFsrMode(s.to_string())),
    }
  }
}

/// Enum representing the different upscale modes available.
#[derive(Debug, Clone)]
pub enum UpscaleMode {
  /// No upscaling applied.
  None,
  /// AMD FidelityFX Super Resolution (FSR). Only works on certain Wine variants i.e. kron4ek,
  /// any tkg variant, ge-proton.
  /// **Requires** the application or game to be configured to run in fullscreen, bear in mind that
  /// many games call fullscreen to borderless fullscreen, FSR requires true fullscreen, this
  /// is also a requirement on Gamescope. Works on DXVK (and maybe VK3D?), does NOT work on WINED3D.
  Fsr {
    mode: FsrMode,
    strength: u8, // Strength value from 0 to 5.
  },
  /// NVIDIA Deep Learning Super Sampling (DLSS) upscaling.
  Dlss,
}

impl FromStr for UpscaleMode {
  type Err = UpscaleModeError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_ascii_lowercase().as_str() {
      "none" | "n" => Ok(UpscaleMode::None),
      "dlss" | "d" => Ok(UpscaleMode::Dlss),
      _ => {
        // A valid upscale mode with FSR looks like "fsr:fsr_mode:fsr_strength".
        if let Some(parts) = s
          .strip_prefix("fsr:")
          .map(|v| v.splitn(2, ':').collect::<Vec<_>>())
        {
          if parts.len() == 2 {
            let mode = FsrMode::from_str(parts[0])?;
            let strength = parts[1]
              .parse::<u8>()
              .map_err(|_| UpscaleModeError::InvalidFsrStrength(parts[1].to_string()))?;
            if strength < MIN_FSR_STRENGTH || strength > MAX_FSR_STRENGTH {
              return Err(UpscaleModeError::OutOfRangeFsrStrength(strength));
            }
            return Ok(UpscaleMode::Fsr { mode, strength });
          }
        }
        return Err(UpscaleModeError::InvalidUpscaleMode(s.to_string()));
      }
    }
  }
}

/// Configures how synchronization operations are done, it may affect CPU
/// performance and game compatibility. Do **NOT** confuse with VSync.
/// TL;DR: Fsync and Ntsync yield similar performance (according to Valve),
/// only Ntsync is available on upstream Wine (Wine 10+ and kernel 6.14+), Fsync
/// does not need a newer kernel and is available in Proton and other variants.
/// https://github.com/lutris/lutris/wiki/How-to:-Esync/be48f27a4112271d0eb7c42eb14b57cea022f8c6.
/// https://github.com/Frogging-Family/wine-tkg-git/issues/936.
/// TODO: as of 2024-08 the latest and greatest option seems to be NTsync. It seems to require an
/// additional /dev/... mount, see https://www.phoronix.com/news/Linux-6.10-Merging-NTSYNC.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SyncMode {
  /// Will use the runner's default.
  None,
  /// Preferred over Esync, this is the default for the soda runner (as of 2024-08).
  Fsync,
  /// Old mode, used before Fsync was common.
  Esync,
}

impl FromStr for SyncMode {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "none" => Ok(SyncMode::None),
      "fsync" => Ok(SyncMode::Fsync),
      "esync" => Ok(SyncMode::Esync),
      _ => Err(format!("invalid sync mode: {}", s)),
    }
  }
}

/// Gets the wine user based on the directories available under "$WINEPREFIX/drive_c/users/", this
/// method looks for the first directory that is not known to be there by default.
/// Keep in mind that all (most?) wine variants create a user with the same name as the current
/// user, additionally some variants create extra users like "steamuser" or "wine".
pub fn get_wine_user(wine_prefix: &Path, fallback_user: &str) -> io::Result<String> {
  let blacklist = [
    "Public".to_lowercase(),
    "Default".to_lowercase(),
    fallback_user.to_lowercase(),
  ];
  let users_path = wine_prefix.join("drive_c").join("users");
  let mut user = fallback_user.to_string();
  for entry in fs::read_dir(&users_path)? {
    let path = entry?.path();
    if !path.is_dir() {
      continue;
    }
    if let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) {
      if !blacklist.contains(&dir_name.to_lowercase()) {
        user = dir_name.to_string();
        break;
      }
    }
  }
  let mut user_components = Path::new(&user).components();
  let first_component_is_invalid = !matches!(user_components.next(), Some(Component::Normal(_)));
  if first_component_is_invalid || user_components.next().is_some() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "wine user must be a single path component",
    ));
  }
  Ok(user)
}

pub fn is_windows_binary<P: AsRef<Path>>(app_bin: P) -> bool {
  app_bin
    .as_ref()
    .extension()
    .map_or(false, |ext| ext.eq_ignore_ascii_case("exe"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

  #[test]
  fn test_get_wine_user_rejects_invalid_fallback_user_paths() {
    let prefix = tempdir().unwrap();
    fs::create_dir_all(prefix.path().join("drive_c/users")).unwrap();
    for user in ["../player", "/player", ".", ""] {
      let error = get_wine_user(prefix.path(), user).err().unwrap();
      assert_eq!(error.kind(), io::ErrorKind::InvalidData);
      assert_eq!(error.to_string(), "wine user must be a single path component");
    }
  }

  #[test]
  fn test_exe_extensions() {
    assert!(is_windows_binary("program.exe"));
    assert!(is_windows_binary("program.EXE"));
    assert!(is_windows_binary("program.ExE"));
    assert!(is_windows_binary("program.eXe"));
  }

  #[test]
  fn test_non_exe_extensions() {
    assert!(!is_windows_binary("program.txt"));
    assert!(!is_windows_binary("program.bin"));
  }

  #[test]
  fn test_files_with_no_extension() {
    assert!(!is_windows_binary("program"));
    assert!(!is_windows_binary(".hiddenfile"));
    assert!(!is_windows_binary(Path::new("/usr/bin/bash")));
  }

  #[test]
  fn test_weird_extensions() {
    assert!(!is_windows_binary("file.exe.backup"));
    assert!(is_windows_binary("file.with.many.dots.exe"));
  }
}

/// Paths and user information for a Wine prefix.
pub struct WinePrefixInfo {
  /// Path to the Wine prefix.
  pub path: PathBuf,
  /// Windows user detected in the prefix.
  pub user: String,
  /// Home directory of the detected Windows user.
  pub home: PathBuf,
}

impl WinePrefixInfo {
  pub fn new(path: PathBuf, fallback_user: &str) -> io::Result<Self> {
    let user = get_wine_user(&path, fallback_user)?;
    let home = path.join("drive_c").join("users").join(&user);
    Ok(Self { path, user, home })
  }
}
