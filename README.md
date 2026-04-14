# peeprs

Claude Code session logs dashboard — Axum web server that scans agent log directories and serves a high-density summary UI.

## Install

```bash
cargo install --path .
```

## Usage

```
peeprs [OPTIONS]
```

### Options

```
    --root <PATH>           Log root directory (default: ~/logs/agents)
    --host <HOST>           Bind address (default: 127.0.0.1)
    --port <PORT>           Listen port (default: 8765)
    --refresh-seconds <N>   Dashboard auto-refresh interval (default: 10)
    --cache-seconds <N>     In-memory summary cache TTL (default: 5)
    --cache-db <PATH>       SQLite scan cache path (default: <root>/.peeprs.db)
```

### Endpoints

| Route | Description |
|-------|-------------|
| `GET /` | HTML dashboard |
| `GET /api/summary` | JSON summary of all scanned logs |
| `GET /healthz` | Health check |

### What it displays

- Totals: files, events, bytes
- Events by type and day
- Top sessions by event count
- Recent events with preview text
- Per-agent stats
- Token usage (input, output, cache read/write)
- Tool usage frequency
- Turn duration distribution
- Session timeline
- Hook performance
- Cache efficiency and error rates

Logs are expected in a structured directory layout: `<root>/<agent>/<day>/<session>/<channel>.*`. Files may be plain JSONL or gzip-compressed.

### Examples

```bash
# Start with defaults
peeprs

# Custom log root and port
peeprs --root ~/dev/logs --port 9000

# Faster refresh, persistent scan cache
peeprs --refresh-seconds 5 --cache-db ~/.peeprs/cache.db
```

## License

MIT
