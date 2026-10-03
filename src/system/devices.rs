use std::{fs, os::unix::fs::FileTypeExt};

/// Finds NVIDIA character device paths in the top level of `/dev`.
///
/// A device matches when its filename starts with the case-sensitive prefix
/// `nvidia` and its resolved metadata identifies a character device. This includes
/// numbered GPU nodes such as `/dev/nvidia0` and supporting nodes such as
/// `/dev/nvidiactl` and `/dev/nvidia-uvm`. Symbolic links to character devices also
/// qualify; the returned path names the link, not its target.
pub fn find_nvidia_devices() -> anyhow::Result<Vec<String>> {
  let mut nvidia_devices = Vec::new();
  let entries = fs::read_dir("/dev")?;
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
