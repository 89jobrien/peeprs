# AGENTS.md

Guidance for coding agents working in this repository.

## Project Snapshot
- Name: `peeprs`
- Language: Rust (`edition = "2024"`)
- App shape: Axum web server + embedded dashboard HTML/CSS/JS
- Entrypoint: `src/main.rs`
- Log root default: `~/logs/claude`
- Log layout: `~/logs/claude/<YYYY-MM-DD>/<session>/<json|text>/events.jsonl(.gz)`
- Current layout: single binary crate (no `src/lib.rs`)

## Tech Stack
- HTTP/server: `axum`, `tokio`
- CLI: `clap`
- Serialization: `serde`, `serde_json`
- Time: `chrono`
- Compression: `flate2`

## Build, Run, Lint, Test
Run all commands from repo root: `/Users/rentamac/dev/peeprs`.

### Build and Run
- `cargo build` - debug build
- `cargo build --release` - optimized build
- `cargo run` - run with defaults
- `cargo run -- --root ~/logs/claude --host 127.0.0.1 --port 8765` - explicit args
- `cargo run -- --help` - CLI sanity check

### Format and Lint
- `cargo fmt` - apply formatting
- `cargo fmt -- --check` - formatting check only
- `cargo clippy --all-targets -- -D warnings` - strict linting
- `cargo check` - fast compile validation

### Test Commands (including single test)
No committed tests yet, but use these when adding tests.

- `cargo test` - run all tests
- `cargo test test_name` - run tests matching substring
- `cargo test module::submodule::test_name -- --exact` - exact unit test path
- `cargo test test_name -- --nocapture` - run one test with output
- `cargo test --test api_summary` - one integration test file
- `cargo test --test api_summary test_name -- --exact` - exact integration test

### Recommended Validation Before PR
1. `cargo fmt -- --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
4. `cargo run -- --help`

## Architecture Notes
- `Args` controls runtime config: `root`, `host`, `port`, refresh/cache seconds
- `AppState` stores expanded root path + shared cache
- Cache type: `tokio::sync::Mutex<SummaryCache>`
- Routes:
  - `/` and `/index.html` -> dashboard page
  - `/api/summary` -> aggregated summary payload
  - `/healthz` -> health payload
- Aggregation flow:
  - `iter_event_files` finds `events.jsonl` / `events.jsonl.gz`
  - `count_lines` counts events for plain and gzip files
  - `build_summary` computes totals, day rows, type split, top sessions
- UI is emitted from `render_html(...)` in Rust source

## Code Style Guidelines
Use these unless a task explicitly asks otherwise.

### Formatting and Structure
- Run `cargo fmt` after edits
- Prefer small focused functions and clear data flow
- Prefer early returns over deep nesting
- Keep control flow easy to scan

### Imports
- Group imports in order: `std`, third-party crates, local modules
- Keep imports explicit; avoid wildcard imports
- Remove unused imports before finalizing

### Types and Data Modeling
- Prefer concrete types over trait objects unless abstraction is needed
- Use structs for payloads/aggregates; keep fields explicit
- Derive only what is required (`Debug`, `Clone`, `Serialize`, `Default`, etc.)
- Prefer `u64` for counters and byte sizes
- Keep API structs additive and stable when possible

### Naming
- Types/traits/enums: `PascalCase`
- Functions/variables/modules: `snake_case`
- Constants/statics: `SCREAMING_SNAKE_CASE`
- Use descriptive names over abbreviations

### Error Handling
- Do not use `unwrap()`/`expect()` in production paths
- Return `Result` with useful context
- Map handler failures to explicit status + readable message
- Degrade gracefully for best-effort metadata (for example, file mtime)

### Async and Concurrency
- Keep mutex lock scopes short
- Avoid expensive filesystem work while holding a lock
- Reuse cached summaries within cache TTL

### Filesystem and Parsing
- Treat logs as untrusted, mutable external input
- Validate day shard names before descending directories
- Support `.jsonl` and `.jsonl.gz` consistently
- Preserve deterministic ordering and truncation behavior

### API and Serialization
- Keep `/api/summary` schema backward compatible where feasible
- If schema changes, update server and dashboard together
- Prefer additive fields over renames/removals

### Frontend Template in Rust
- Keep template IDs/class names stable unless all references are updated
- Preserve mobile responsiveness when changing CSS/layout
- Keep dashboard compact and operationally readable
- Minimize risky string interpolation in HTML/JS

## Testing Priorities
When tests are added, prioritize:

- `looks_like_day` edge cases and malformed shard names
- summary totals (events/files/bytes/sessions/days)
- ordering/truncation behavior of `top_sessions`
- line count parity for plain vs gzipped files
- `/api/summary` and `/healthz` behavior for empty/missing roots

## Repository Policies
- Do not commit `target/` artifacts
- Keep default CLI values practical for local usage
- Keep dependencies lightweight unless clearly justified

## Cursor and Copilot Rules
Checked in this repository:

- `.cursor/rules/`: not present
- `.cursorrules`: not present
- `.github/copilot-instructions.md`: not present

If these files are added later, merge their guidance into this file and treat them as higher-priority instructions.
