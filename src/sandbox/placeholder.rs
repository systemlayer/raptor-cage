use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

static PLACEHOLDER_RE: OnceLock<Regex> = OnceLock::new();

// Compile regex expression once and return static reference.
fn get_placeholder_re() -> &'static Regex {
  PLACEHOLDER_RE.get_or_init(|| Regex::new(r"\{\{([A-Za-z0-9_]+)\}\}").unwrap())
}

/// Replaces placeholders surrounded by double braces (e.g., `{{PLACEHOLDER}}`)
/// in the input string. The placeholders are replaced by corresponding values
/// from the provided `HashMap`.
pub fn replace_placeholders(
  input: &str,
  values: &HashMap<String, String>,
) -> anyhow::Result<String> {
  let mut output = String::with_capacity(input.len());
  let mut last_end = 0;
  let re = get_placeholder_re();
  for caps in re.captures_iter(input) {
    let full = caps.get(0).unwrap();
    let key = caps.get(1).unwrap().as_str();
    if input[..full.start()].ends_with('{') || input[full.end()..].starts_with('}') {
      anyhow::bail!("Input contains malformed placeholder")
    }
    // Append text before the placeholder.
    output.push_str(&input[last_end..full.start()]);
    let value = values
      .get(key)
      .ok_or_else(|| anyhow::anyhow!("Unknown placeholder: {{{{{}}}}}", key))?;
    // Append value and update last_end.
    output.push_str(value);
    last_end = full.end();
  }
  // Append text after last match.
  output.push_str(&input[last_end..]);
  // Any remaining delimiter will be considered a malformed placeholder.
  if output.contains("{{") || output.contains("}}") {
    anyhow::bail!("Input contains malformed placeholder")
  }
  Ok(output)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect()
  }

  #[test]
  fn test_basic_replacement() {
    let input = "/media/{{name}}/files/{{date}}";
    let values = map(&[("name", "test"), ("date", "2025-01-01")]);
    assert_eq!(replace_placeholders(input, &values).unwrap(), "/media/test/files/2025-01-01");
  }

  #[test]
  fn test_single_placeholder() {
    let input = "{{a}}";
    let values = map(&[("a", "ok")]);
    assert_eq!(replace_placeholders(input, &values).unwrap(), "ok");
  }

  #[test]
  fn test_underscore_key() {
    let input = "{{USER_name}}";
    let values = map(&[("USER_name", "test")]);
    assert_eq!(replace_placeholders(input, &values).unwrap(), "test");
  }

  #[test]
  fn test_missing_key_error() {
    let input = "hello {{name}}";
    let values = HashMap::new();
    assert!(replace_placeholders(input, &values).is_err());
  }

  #[test]
  fn test_malformed_missing_end() {
    let input = "hello {{name";
    let values = map(&[("name", "test")]);
    assert!(replace_placeholders(input, &values).is_err());
  }

  #[test]
  fn test_malformed_missing_start() {
    let input = "hello name}}";
    let values = map(&[("name", "test")]);
    assert!(replace_placeholders(input, &values).is_err());
  }

  #[test]
  fn test_extra_braces() {
    let input = "{{{name}}}";
    let values = map(&[("name", "test")]);
    assert!(replace_placeholders(input, &values).is_err());
  }

  #[test]
  fn test_invalid_characters_in_key() {
    let input = "Hello {{na-me}}";
    let values = map(&[("na-me", "wrong")]);
    assert!(replace_placeholders(input, &values).is_err());
  }

  #[test]
  fn test_adjacent_placeholders() {
    let input = "{{a}}{{b}}";
    let values = map(&[("a", "1"), ("b", "2")]);
    assert_eq!(replace_placeholders(input, &values).unwrap(), "12");
  }

  #[test]
  fn test_no_placeholders() {
    let input = "no placeholders";
    let values = HashMap::new();
    assert_eq!(replace_placeholders(input, &values).unwrap(), "no placeholders");
  }

  #[test]
  fn test_single_braces_are_literal() {
    let input = "brace expansion {a,b}";
    let values = HashMap::new();
    assert_eq!(replace_placeholders(input, &values).unwrap(), input);
  }
}
