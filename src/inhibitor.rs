use anyhow::Context;
use zbus::{Connection, zvariant::OwnedFd};

/// Requests idle inhibition for this application until its descriptor is closed.
const INHIBIT_ARGS: (&str, &str, &str, &str) = (
  "idle",                 // What
  env!("CARGO_PKG_NAME"), // Who
  "Running Game",         // Why
  "block",                // Mode
);

/// Holds the descriptor that keeps idle inhibition active.
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
    .call("Inhibit", &INHIBIT_ARGS)
    .await
    .context("method call to D-Bus failed")?;
  Ok(InhibitHandle { _fd: fd })
}

/// Checks D-Bus payload encoding and inhibition descriptor ownership.
#[cfg(test)]
mod tests {
  use super::*;
  use std::{
    io::{ErrorKind, Read},
    os::{fd::OwnedFd as StdOwnedFd, unix::net::UnixStream},
  };

  #[test]
  fn inhibition_request_round_trips_through_zbus() -> anyhow::Result<()> {
    let message = zbus::Message::method_call("/org/freedesktop/login1", "Inhibit")?
      .interface("org.freedesktop.login1.Manager")?
      .destination("org.freedesktop.login1")?
      .build(&INHIBIT_ARGS)?;
    let args: (String, String, String, String) = message.body().deserialize()?;
    assert_eq!(
      args,
      ("idle".into(), env!("CARGO_PKG_NAME").into(), "Running Game".into(), "block".into())
    );
    Ok(())
  }

  #[test]
  fn dropping_inhibit_handle_closes_its_descriptor() -> anyhow::Result<()> {
    let (socket, mut peer) = UnixStream::pair()?;
    peer.set_nonblocking(true)?;
    let handle = InhibitHandle {
      _fd: OwnedFd::from(StdOwnedFd::from(socket)),
    };
    let mut buffer = [0];
    assert_eq!(peer.read(&mut buffer).unwrap_err().kind(), ErrorKind::WouldBlock);
    drop(handle);
    assert_eq!(peer.read(&mut buffer)?, 0);
    Ok(())
  }
}
