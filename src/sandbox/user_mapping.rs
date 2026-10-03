use rand::RngExt;
use std::fmt;
use std::str::FromStr;

// Minimum id to use when generating a random UID/GID; usually system accounts use from 0 to 999,
// normal users start from 1000 and so on, and other applications like Docker use ranges up to
// 296608 (231072 + 65536), so a safe bet is to use values >= 300_000.
const MIN_RANDOM_ID: u32 = 300_000;
// Minimum UID/GID allowed by the Linux kernel.
const MIN_ID: u32 = 0;
// On many Linux systems, even though the maximum UID/GID is an u32, we are only allowed to use ids
// up to 2147483647 (the max value of i32), otherwise namespace creation may fail.
// This cast simply performs a conversion without checking for overflow, and for this particular
// scenario is safe.
const MAX_ID: u32 = i32::MAX as u32;

#[derive(Debug)]
pub enum UserMappingError {
  InvalidFormat(String),
  InvalidId(String),
  OutOfRangeId(u32),
}

// Yeah, yeah, thiserror could be used, it just doesn't feel good to add an extra crate to avoid
// writing ~8 lines; thiserror can be used if we start implementing lots of custom errors.
impl fmt::Display for UserMappingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      UserMappingError::InvalidFormat(s) => write!(f, "expected UID:GID format, but got: {}", s),
      UserMappingError::InvalidId(s) => write!(f, "input is not a valid ID: {}", s),
      UserMappingError::OutOfRangeId(id) => {
        write!(f, "value must be between {} and {}, but got {}", MIN_ID, MAX_ID, id)
      }
    }
  }
}

impl std::error::Error for UserMappingError {}

fn parse_id(id: &str) -> Result<u32, UserMappingError> {
  let value = id
    .parse::<u32>()
    .map_err(|_| UserMappingError::InvalidId(id.to_string()))?;
  if value < MIN_ID || value > MAX_ID {
    return Err(UserMappingError::OutOfRangeId(value));
  }
  Ok(value)
}

/// Represents a user mapping configuration.
#[derive(Debug, Clone, Copy)]
pub enum UserMapping {
  /// Do not map user and group ids.
  None,
  /// Use random user and group ids.
  Random,
  /// Explicitly specify user and group ids.
  Custom(u32, u32),
}

impl UserMapping {
  pub fn get_uid_gid(&self) -> Option<(u32, u32)> {
    match self {
      UserMapping::None => None,
      UserMapping::Random => {
        let mut rng = rand::rng();
        let random_uid = rng.random_range(MIN_RANDOM_ID..=MAX_ID);
        let random_gid = rng.random_range(MIN_RANDOM_ID..=MAX_ID);
        Some((random_uid, random_gid))
      }
      UserMapping::Custom(uid, gid) => Some((*uid, *gid)),
    }
  }
  pub fn get_uid_gid_string(&self) -> Option<(String, String)> {
    let Some((uid, gid)) = self.get_uid_gid() else {
      return None;
    };
    Some((uid.to_string(), gid.to_string()))
  }
}

impl FromStr for UserMapping {
  type Err = UserMappingError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.eq_ignore_ascii_case("none") {
      return Ok(UserMapping::None);
    }
    if s.eq_ignore_ascii_case("random") {
      return Ok(UserMapping::Random);
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
      return Err(UserMappingError::InvalidFormat(s.to_string()));
    }
    let uid = parse_id(parts[0])?;
    let gid = parse_id(parts[1])?;
    Ok(UserMapping::Custom(uid, gid))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_random_user_mapping() {
    let mapping = UserMapping::Random;
    let result = mapping.get_uid_gid();
    assert!(result.is_some());
    let (uid, gid) = result.unwrap();
    assert!(uid >= MIN_RANDOM_ID && uid <= MAX_ID);
    assert!(gid >= MIN_RANDOM_ID && gid <= MAX_ID);
  }

  #[test]
  fn test_custom_user_mapping() {
    let mapping = UserMapping::Custom(500_000, 600_000);
    let result = mapping.get_uid_gid();
    assert!(result.is_some());
    let (uid, gid) = result.unwrap();
    assert_eq!(uid, 500_000);
    assert_eq!(gid, 600_000);
  }

  #[test]
  fn test_user_mapping_from_str_none() {
    let mapping = UserMapping::from_str("none").unwrap();
    assert!(matches!(mapping, UserMapping::None));
  }

  #[test]
  fn test_user_mapping_from_str_random() {
    let mapping = UserMapping::from_str("random").unwrap();
    assert!(matches!(mapping, UserMapping::Random));
  }

  #[test]
  fn test_user_mapping_from_str_custom() {
    let mapping = UserMapping::from_str("500000:600000").unwrap();
    if let UserMapping::Custom(uid, gid) = mapping {
      assert_eq!(uid, 500_000);
      assert_eq!(gid, 600_000);
    } else {
      panic!("expected custom mapping");
    }
  }
}
