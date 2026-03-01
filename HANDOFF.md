# Handoff: Rust Claude Logs Dashboard

## What was built

- Replaced the placeholder `Hello, world!` app with a real Rust web dashboard server.
- Implemented file-system aggregation for centralized Claude logs at:
  - `~/logs/claude/<YYYY-MM-DD>/<session>/<type>/events.jsonl`
  - `~/logs/claude/<YYYY-MM-DD>/<session>/<type>/events.jsonl.gz`
- Added API endpoints:
  - `GET /` and `GET /index.html`: dashboard UI
  - `GET /api/summary`: JSON summary payload
  - `GET /healthz`: health check
- Implemented summary caching (`--cache-seconds`, default `5`) to reduce repeated disk scans.
- Ported the compact, high-density modern dashboard layout with additional KPIs and visuals:
  - KPI strip (10 metrics)
  - 21-shard daily trend bars
  - type split donut + legend
  - session concentration bars
  - daily + top sessions tables

## Key files

- `Cargo.toml`
  - Added runtime/web/data dependencies (`axum`, `tokio`, `clap`, `serde`, `serde_json`, `chrono`, `flate2`).
- `src/main.rs`
  - Full server, scanner, caching, API, and embedded dashboard HTML/JS.

## Run

```bash
cd ~/dev/peeprs
cargo run -- --root ~/logs/claude --host 127.0.0.1 --port 8765
```

Open: `http://127.0.0.1:8765`

## Flags

- `--root` (default `~/logs/claude`)
- `--host` (default `127.0.0.1`)
- `--port` (default `8765`)
- `--refresh-seconds` (default `10`)
- `--cache-seconds` (default `5`)

## Notes / tradeoffs

- Line counting for `.gz` files currently decompresses and counts lines in-memory.
  - This is simple and correct for typical file sizes, but can be optimized later for very large archives.
- HTML is embedded directly in `src/main.rs`.
  - This keeps deployment simple but can be split into static assets later.

## Suggested next steps

1. Add request timing and scan-duration metrics in `/api/summary` for observability.
2. Move dashboard assets to `assets/` and use compile-time embedding (`include_str!`).
3. Add optional filters to `/api/summary` (`days`, `type`, `session_prefix`).
4. Add a launchd/systemd unit wrapper for always-on local dashboard service.
