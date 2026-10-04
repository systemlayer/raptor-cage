# Repository Guidelines

## Code formatting

- Functions shorter than 30 lines must not contain blank lines.
- Do not put blank lines between `use` imports.

## Error handling

- For fallible backend work, prefer `anyhow::Result<T>` and add useful context with `.context(...)`. Always qualify the result type as `anyhow::Result`.
- Handle recoverable runtime errors explicitly. Reserve `unwrap` and `expect` for app bootstrap invariants, unrecoverable failures, unreachable states, and programming bugs.
- Write `expect` messages in lowercase without trailing punctuation. State the invariant that makes success certain, preferably with "should" (for example, `expect("output directory should exist after setup")`); do not merely restate the failed operation (such as `expect("failed to open output directory")`).

## User-facing output

- Send program results and output intended for scripts to stdout (`println!` or `print!`). Send diagnostics and errors to stderr (`eprintln!` or `eprint!`).
- Treat internal state, timing, fallbacks, retries, process IDs, cleanup, and troubleshooting details as diagnostics unless they are part of the program's expected output.
- Use normal sentence capitalization and punctuation for user-facing prose. Keep error messages and `.context(...)` text lowercase without trailing punctuation so chained errors read naturally.
- Write numbers directly next to abbreviated units (for example, `2.34s` and `100ms`).

## Code organization

- Prefer declaring constants at the top level of a file.
- Give top-level declarations 1-3 lines of triple-slash documentation explaining their purpose.
- Prefer placing declarations before their first use.
- Keep related code close together in the same file.

## Git safety

- Never execute `git push` commands under any circumstances.
- Never perform Git write operations. Do not stage or commit changes, modify branches, tags, refs, the index, or repository configuration, or run any command that mutates Git state.
