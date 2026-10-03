use std::{fs, os::unix::fs::FileTypeExt};

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
