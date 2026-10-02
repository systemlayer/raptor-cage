use anyhow::Context;
use zbus::{Connection, zvariant::OwnedFd};

pub struct InhibitHandle {
  _fd: OwnedFd,
}

/// Prevents the system from entering idle mode, which may include screen dimming or suspend,
/// depending on the desktop environment's configuration.
///
/// The inhibition remains active as long as the returned handle is held. Dropping the handle
/// releases the inhibition automatically.
///
/// You can view active inhibitions using the `systemd-inhibit --list` command.
pub async fn inhibit_idle() -> anyhow::Result<InhibitHandle> {
  let connection = Connection::system()
    .await
    .context("failed to connect to D-Bus")?;
  let proxy = zbus::Proxy::new(
    &connection,
    "org.freedesktop.login1",         // Destination
    "/org/freedesktop/login1",        // Path
    "org.freedesktop.login1.Manager", // Interface
  )
  .await
  .context("failed to create D-Bus proxy")?;
  let fd: OwnedFd = proxy
    .call(
      "Inhibit",
      &(
        "idle",                 // What
        env!("CARGO_PKG_NAME"), // Who
        "Running Game",         // Why
        "block",                // Mode
      ),
    )
    .await
    .context("method call to D-Bus failed")?;
  Ok(InhibitHandle { _fd: fd })
}
