# Repository Guidelines

## Code formatting

- Functions shorter than 30 lines must not contain blank lines.
- Do not put blank lines between `use` imports.

## Error handling

- For fallible backend work, prefer `anyhow::Result<T>` and add useful context with `.context(...)`. Always qualify the result type as `anyhow::Result`.
- Handle recoverable runtime errors explicitly. Reserve `unwrap` and `expect` for app bootstrap invariants, unrecoverable failures, unreachable states, and programming bugs.
- Write `expect` messages in lowercase without trailing punctuation. State the invariant that makes success certain, preferably with "should" (for example, `expect("output directory should exist after setup")`); do not merely restate the failed operation (such as `expect("failed to open output directory")`).

## User-facing output

- Capitalize user-facing `println!` and `eprintln!` messages as normal sentences, with appropriate punctuation. Write error messages and `.context(...)` text in lowercase without trailing punctuation so chained errors read naturally.
- In user-facing output, write numeric values directly next to abbreviated units (for example, `2.34s` and `100ms`).

## Code organization

- Prefer declaring constants at the top level of a file.
- Give top-level declarations 1-3 lines of triple-slash documentation explaining their purpose.
- Prefer placing declarations before their first use.
- Keep related code close together in the same file.

## Git safety

- Never perform Git write operations. Do not stage or commit changes, modify branches, tags, refs, the index, or repository configuration, or run any command that mutates Git state.
