use globset::{Glob, GlobMatcher};
use procfs::process::all_processes;
use std::collections::HashSet;
use std::{collections::HashMap, thread::sleep, time::Instant};
use std::{
  process::{Command, Stdio},
  time::Duration,
};

/// How often to scan for processes.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Max time a process must be unseen to be considered exited.
const GRACE_PERIOD_LG: Duration = Duration::from_secs(5);
/// Min time a process must be unseen to be considered exited.
const GRACE_PERIOD_SM: Duration = Duration::from_secs(2);

/// Tracks a process-name pattern and when it was last observed.
struct ProcessState {
  last_seen: Instant,
  name_glob: GlobMatcher,
}

impl ProcessState {
  /// Compiles the same name matcher used by process polling.
  fn new(name: &str) -> Self {
    Self {
      last_seen: Instant::now(),
      name_glob: Glob::new(name)
        .expect("process name should be a valid glob expression")
        .compile_matcher(),
    }
  }
}

/// Waits until all target process names have not been seen for at least `GRACE_PERIOD_{SM,LG}`.
pub fn wait_for_processes_to_exit(target_names: Vec<String>) -> anyhow::Result<()> {
  // How long to allow a process to not exist to consider it dead.
  let mut grace_period = GRACE_PERIOD_LG;
  // Contains the "last seen" times for each process name.
  let mut process_states: HashMap<String, ProcessState> = target_names
    .into_iter()
    .map(|name| {
      let state = ProcessState::new(&name);
      (name, state)
    })
    .collect();
  loop {
    let now = Instant::now();
    // Keeps track of the seen processes, so we avoid needless cmdline() calls down the line.
    let mut seen_processes: HashSet<String> = HashSet::new();
    // It's important to be able to list processes. If this fails, it may indicate a more
    // significant issue, such as insufficient permissions or denied capabilities. In any case,
    // polling for processes can not continue.
    let processes = all_processes()?;
    for process in processes {
      // Failing to get stat/cmdline a single process is not big issue here, it can happen sometimes
      // when trying to stat a process from a different user or due to mandatory access control.
      if let Ok(process) = process {
        // Avoid `stat()?.comm` because it contains the truncated program name.
        if let Ok(cmdline) = process.cmdline() {
          if let Some(program_name) = cmdline.first() {
            let entry = process_states
              .iter_mut()
              .find(|(_, state)| state.name_glob.is_match(program_name));
            if let Some((name, state)) = entry {
              state.last_seen = now;
              seen_processes.insert(name.clone());
            }
          }
        }
      }
      // If we got enough processes, there is no need to keep looping even if we find process with
      // a matching name glob, because last_seen would be the same.
      // Also, since we already know that all processes are running, grace_period can be shorter.
      if seen_processes.len() == process_states.len() {
        grace_period = GRACE_PERIOD_SM;
        break;
      }
    }
    let all_gone = process_states
      .iter()
      .all(|(_, state)| now.duration_since(state.last_seen) >= grace_period);
    if all_gone {
      break;
    }
    // TODO: bail if total time is greater than timeout (current behavior is to wait indefinitely).
    sleep(POLL_INTERVAL);
  }
  Ok(())
}

/// Checks process discovery, matching, and launch errors.
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn process_patterns_match_command_lines() -> anyhow::Result<()> {
    let pattern = ProcessState::new("*game*.exe");
    assert!(pattern.name_glob.is_match("/games/mygame.exe"));
    assert!(!pattern.name_glob.is_match("/apps/other.exe"));
    let process = all_processes()?
      .filter_map(Result::ok)
      .find(|process| process.pid as u32 == std::process::id())
      .expect("the test process should appear in procfs");
    let cmdline = process.cmdline()?;
    assert_eq!(cmdline.first(), std::env::args().next().as_ref());
    assert!(ProcessState::new("*").name_glob.is_match(&cmdline[0]));
    Ok(())
  }

  #[test]
  fn missing_executable_returns_an_error() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let program = directory
      .path()
      .join("missing")
      .to_string_lossy()
      .into_owned();
    let error = run(Vec::new(), program.clone(), None).unwrap_err();
    assert!(error.to_string().contains(&program));
    assert!(error.to_string().contains("could not spawn"));
    Ok(())
  }
}

pub fn run(
  process_names: Vec<String>,
  program: String,
  args: Option<Vec<String>>,
) -> anyhow::Result<()> {
  Command::new(&program)
    .args(args.unwrap_or_default())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()
    .map_err(|e| anyhow::anyhow!("could not spawn {}: {}", &program, e))?;
  // Unlike bwrap::run, there is no need to use cmd.wait() because we want to
  // wait for other processes not the one we just executed.
  eprintln!(
    "Waiting for the following process(es) to terminate:\n{}",
    process_names
      .iter()
      .map(|name| format!("  {}", name))
      .collect::<Vec<String>>()
      .join("\n")
  );
  let num_processes = process_names.len();
  wait_for_processes_to_exit(process_names)?;
  eprintln!("Finished waiting for {} process(es)", num_processes);
  Ok(())
}
