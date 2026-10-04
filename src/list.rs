use crate::sandbox::bottles;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Category {
  All,
  PrefixesOnly,
  RunnersOnly,
}

impl FromStr for Category {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "all" | "a" => Ok(Category::All),
      "prefixes" | "p" => Ok(Category::PrefixesOnly),
      "runners" | "r" => Ok(Category::RunnersOnly),
      _ => Err(format!("invalid category: {}", s)),
    }
  }
}

pub fn list(category: Category) -> anyhow::Result<()> {
  let data_root = bottles::get_data_root()?;
  match category {
    Category::All => {
      let prefixes = bottles::list_prefixes(&data_root)?;
      println!(
        "Prefixes:\n{}",
        prefixes
          .iter()
          .map(|name| format!("  {}", name))
          .collect::<Vec<String>>()
          .join("\n")
      );
      let runners = bottles::list_runners(&data_root)?;
      println!(
        "Runners:\n{}",
        runners
          .iter()
          .map(|name| format!("  {}", name))
          .collect::<Vec<String>>()
          .join("\n")
      );
    }
    Category::PrefixesOnly => {
      let prefixes = bottles::list_prefixes(&data_root)?;
      println!("{}", prefixes.join("\n"));
    }
    Category::RunnersOnly => {
      let runners = bottles::list_runners(&data_root)?;
      println!("{}", runners.join("\n"));
    }
  }
  Ok(())
}
