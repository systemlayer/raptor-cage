use std::{fs, os::unix::fs::FileTypeExt, path::Path};

/// Finds NVIDIA character device paths in the top level of `/dev`.
///
/// A device matches when its filename starts with the case-sensitive prefix
/// `nvidia` and its resolved metadata identifies a character device. This includes
/// numbered GPU nodes such as `/dev/nvidia0` and supporting nodes such as
/// `/dev/nvidiactl` and `/dev/nvidia-uvm`. Symbolic links to character devices also
/// qualify; the returned path names the link, not its target.
pub fn find_nvidia_devices() -> anyhow::Result<Vec<String>> {
  find_nvidia_devices_in(Path::new("/dev"))
}

/// Scans a supplied directory using the production device filtering rules.
fn find_nvidia_devices_in(directory: &Path) -> anyhow::Result<Vec<String>> {
  let mut nvidia_devices = Vec::new();
  let entries = fs::read_dir(directory)?;
  for entry in entries.flatten() {
    let path = entry.path();
    let metadata = path.metadata()?;
    if metadata.file_type().is_char_device() {
      let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
      if file_name.starts_with("nvidia") {
        if let Some(path_str) = path.to_str() {
          nvidia_devices.push(path_str.to_string());
        }
      }
    }
  }
  Ok(nvidia_devices)
}

/// Checks device filtering and filesystem error propagation.
#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::fs::symlink;

  #[test]
  fn includes_only_nvidia_character_devices() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let device = directory.path().join("nvidia0");
    symlink("/dev/null", &device)?;
    symlink("/dev/null", directory.path().join("other"))?;
    fs::write(directory.path().join("nvidia-regular"), "")?;
    assert_eq!(find_nvidia_devices_in(directory.path())?, vec![device.to_string_lossy()]);
    Ok(())
  }

  #[test]
  fn missing_device_directory_returns_io_error() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let error = find_nvidia_devices_in(&directory.path().join("missing")).unwrap_err();
    assert_eq!(
      error.downcast_ref::<std::io::Error>().unwrap().kind(),
      std::io::ErrorKind::NotFound
    );
    Ok(())
  }
}
