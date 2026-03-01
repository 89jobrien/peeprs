use std::collections::HashMap;
use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::Row;

use crate::models::{
    FileScanStats, HookAccum, RecentEvent, RecentEventAccum, SessionLineStats, TokenUsage,
};

pub struct ScanCache {
    pool: SqlitePool,
}

impl ScanCache {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS file_cache (
                path TEXT PRIMARY KEY,
                mtime_ms INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                event_count INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_cache (
                file_path TEXT NOT NULL,
                session_id TEXT NOT NULL,
                events INTEGER NOT NULL,
                bytes INTEGER NOT NULL,
                last_seen_ms INTEGER,
                agent TEXT,
                PRIMARY KEY (file_path, session_id)
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS recent_cache (
                file_path TEXT NOT NULL,
                sort_key_ms INTEGER NOT NULL,
                timestamp_display TEXT NOT NULL,
                session_id TEXT NOT NULL,
                agent TEXT NOT NULL,
                event_type TEXT NOT NULL,
                preview TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_recent_sort ON recent_cache(sort_key_ms DESC)",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS token_cache (
                file_path TEXT PRIMARY KEY,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                cache_read_tokens INTEGER NOT NULL,
                cache_creation_tokens INTEGER NOT NULL,
                api_error_count INTEGER NOT NULL,
                tool_error_count INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tool_cache (
                file_path TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                count INTEGER NOT NULL,
                PRIMARY KEY (file_path, tool_name)
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS duration_cache (
                file_path TEXT NOT NULL,
                duration_ms INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_duration_file ON duration_cache(file_path)",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS hook_cache (
                file_path TEXT NOT NULL,
                command TEXT NOT NULL,
                count INTEGER NOT NULL,
                total_ms INTEGER NOT NULL,
                PRIMARY KEY (file_path, command)
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS model_cache (
                file_path TEXT NOT NULL,
                model TEXT NOT NULL,
                count INTEGER NOT NULL,
                PRIMARY KEY (file_path, model)
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_hourly_cache (
                file_path TEXT NOT NULL,
                session_id TEXT NOT NULL,
                hour INTEGER NOT NULL,
                count INTEGER NOT NULL,
                PRIMARY KEY (file_path, session_id, hour)
            )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn lookup(
        &self,
        path: &str,
        mtime_ms: i64,
        size_bytes: u64,
    ) -> Option<FileScanStats> {
        let size_i64 = size_bytes as i64;

        let row = sqlx::query(
            "SELECT event_count FROM file_cache
             WHERE path = ? AND mtime_ms = ? AND size_bytes = ?",
        )
        .bind(path)
        .bind(mtime_ms)
        .bind(size_i64)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        let row = row?;
        let event_count: i64 = row.get("event_count");

        let session_rows = sqlx::query(
            "SELECT session_id, events, bytes, last_seen_ms, agent
             FROM session_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .ok()?;

        let mut session_stats = HashMap::new();
        for row in session_rows {
            let session_id: String = row.get("session_id");
            session_stats.insert(
                session_id,
                SessionLineStats {
                    events: row.get::<i64, _>("events") as u64,
                    bytes: row.get::<i64, _>("bytes") as u64,
                    last_seen_ms: row.get("last_seen_ms"),
                    agent: row.get("agent"),
                    hourly_events: [0u64; 24],
                },
            );
        }

        let recent_rows = sqlx::query(
            "SELECT sort_key_ms, timestamp_display, session_id, agent, event_type, preview
             FROM recent_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .ok()?;

        let recent_events = recent_rows
            .into_iter()
            .map(|row| RecentEventAccum {
                sort_key_ms: row.get("sort_key_ms"),
                event: RecentEvent {
                    timestamp: row.get("timestamp_display"),
                    session: row.get("session_id"),
                    agent: row.get("agent"),
                    event_type: row.get("event_type"),
                    preview: row.get("preview"),
                },
            })
            .collect();

        // Read token cache
        let token_row = sqlx::query(
            "SELECT input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                    api_error_count, tool_error_count
             FROM token_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();

        let (token_usage, api_error_count, tool_error_count) = match token_row {
            Some(tr) => (
                TokenUsage {
                    input_tokens: tr.get("input_tokens"),
                    output_tokens: tr.get("output_tokens"),
                    cache_read_input_tokens: tr.get("cache_read_tokens"),
                    cache_creation_input_tokens: tr.get("cache_creation_tokens"),
                },
                tr.get::<i64, _>("api_error_count") as u64,
                tr.get::<i64, _>("tool_error_count") as u64,
            ),
            None => (TokenUsage::default(), 0, 0),
        };

        // Read tool cache
        let tool_rows = sqlx::query(
            "SELECT tool_name, count FROM tool_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .ok()
        .unwrap_or_default();

        let mut tool_counts = HashMap::new();
        for row in tool_rows {
            let name: String = row.get("tool_name");
            let count: i64 = row.get("count");
            tool_counts.insert(name, count as u64);
        }

        // Read duration cache
        let dur_rows = sqlx::query(
            "SELECT duration_ms FROM duration_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .ok()
        .unwrap_or_default();

        let turn_durations: Vec<i64> = dur_rows.iter().map(|r| r.get("duration_ms")).collect();

        // Read hook cache
        let hook_rows = sqlx::query(
            "SELECT command, count, total_ms FROM hook_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .ok()
        .unwrap_or_default();

        let mut hook_stats = HashMap::new();
        for row in hook_rows {
            let command: String = row.get("command");
            hook_stats.insert(
                command,
                HookAccum {
                    count: row.get::<i64, _>("count") as u64,
                    total_ms: row.get("total_ms"),
                },
            );
        }

        // Read model cache
        let model_rows = sqlx::query(
            "SELECT model, count FROM model_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .ok()
        .unwrap_or_default();

        let mut model_counts = HashMap::new();
        for row in model_rows {
            let model: String = row.get("model");
            let count: i64 = row.get("count");
            model_counts.insert(model, count as u64);
        }

        // Read session hourly cache
        let hourly_rows = sqlx::query(
            "SELECT session_id, hour, count FROM session_hourly_cache WHERE file_path = ?",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .ok()
        .unwrap_or_default();

        for row in hourly_rows {
            let session_id: String = row.get("session_id");
            let hour: i64 = row.get("hour");
            let count: i64 = row.get("count");
            if let Some(ss) = session_stats.get_mut(&session_id) {
                if (hour as usize) < 24 {
                    ss.hourly_events[hour as usize] = count as u64;
                }
            }
        }

        Some(FileScanStats {
            event_count: event_count as u64,
            session_stats,
            recent_events,
            token_usage,
            tool_counts,
            turn_durations,
            hook_stats,
            api_error_count,
            tool_error_count,
            model_counts,
        })
    }

    pub async fn store(&self, path: &str, mtime_ms: i64, size_bytes: u64, stats: &FileScanStats) {
        let size_i64 = size_bytes as i64;
        let event_count = stats.event_count as i64;

        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(_) => return,
        };

        for table in [
            "recent_cache",
            "session_cache",
            "token_cache",
            "tool_cache",
            "duration_cache",
            "hook_cache",
            "model_cache",
            "session_hourly_cache",
        ] {
            let sql = format!("DELETE FROM {table} WHERE file_path = ?");
            let _ = sqlx::query(&sql).bind(path).execute(&mut *tx).await;
        }

        let _ = sqlx::query(
            "INSERT OR REPLACE INTO file_cache (path, mtime_ms, size_bytes, event_count)
             VALUES (?, ?, ?, ?)",
        )
        .bind(path)
        .bind(mtime_ms)
        .bind(size_i64)
        .bind(event_count)
        .execute(&mut *tx)
        .await;

        for (session_id, ss) in &stats.session_stats {
            let _ = sqlx::query(
                "INSERT INTO session_cache (file_path, session_id, events, bytes, last_seen_ms, agent)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(path)
            .bind(session_id)
            .bind(ss.events as i64)
            .bind(ss.bytes as i64)
            .bind(ss.last_seen_ms)
            .bind(&ss.agent)
            .execute(&mut *tx)
            .await;
        }

        for re in &stats.recent_events {
            let _ = sqlx::query(
                "INSERT INTO recent_cache (file_path, sort_key_ms, timestamp_display, session_id, agent, event_type, preview)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(path)
            .bind(re.sort_key_ms)
            .bind(&re.event.timestamp)
            .bind(&re.event.session)
            .bind(&re.event.agent)
            .bind(&re.event.event_type)
            .bind(&re.event.preview)
            .execute(&mut *tx)
            .await;
        }

        // Token cache
        let _ = sqlx::query(
            "INSERT INTO token_cache (file_path, input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, api_error_count, tool_error_count)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(path)
        .bind(stats.token_usage.input_tokens)
        .bind(stats.token_usage.output_tokens)
        .bind(stats.token_usage.cache_read_input_tokens)
        .bind(stats.token_usage.cache_creation_input_tokens)
        .bind(stats.api_error_count as i64)
        .bind(stats.tool_error_count as i64)
        .execute(&mut *tx)
        .await;

        // Tool cache
        for (tool_name, count) in &stats.tool_counts {
            let _ = sqlx::query(
                "INSERT INTO tool_cache (file_path, tool_name, count) VALUES (?, ?, ?)",
            )
            .bind(path)
            .bind(tool_name)
            .bind(*count as i64)
            .execute(&mut *tx)
            .await;
        }

        // Duration cache
        for dur in &stats.turn_durations {
            let _ = sqlx::query(
                "INSERT INTO duration_cache (file_path, duration_ms) VALUES (?, ?)",
            )
            .bind(path)
            .bind(*dur)
            .execute(&mut *tx)
            .await;
        }

        // Hook cache
        for (command, accum) in &stats.hook_stats {
            let _ = sqlx::query(
                "INSERT INTO hook_cache (file_path, command, count, total_ms) VALUES (?, ?, ?, ?)",
            )
            .bind(path)
            .bind(command)
            .bind(accum.count as i64)
            .bind(accum.total_ms)
            .execute(&mut *tx)
            .await;
        }

        // Model cache
        for (model, count) in &stats.model_counts {
            let _ = sqlx::query(
                "INSERT INTO model_cache (file_path, model, count) VALUES (?, ?, ?)",
            )
            .bind(path)
            .bind(model)
            .bind(*count as i64)
            .execute(&mut *tx)
            .await;
        }

        // Session hourly cache
        for (session_id, ss) in &stats.session_stats {
            for (hour, count) in ss.hourly_events.iter().enumerate() {
                if *count > 0 {
                    let _ = sqlx::query(
                        "INSERT INTO session_hourly_cache (file_path, session_id, hour, count) VALUES (?, ?, ?, ?)",
                    )
                    .bind(path)
                    .bind(session_id)
                    .bind(hour as i64)
                    .bind(*count as i64)
                    .execute(&mut *tx)
                    .await;
                }
            }
        }

        let _ = tx.commit().await;
    }

    pub async fn remove_stale(&self, valid_paths: &[String]) {
        let cached_rows = match sqlx::query("SELECT path FROM file_cache")
            .fetch_all(&self.pool)
            .await
        {
            Ok(rows) => rows,
            Err(_) => return,
        };

        let valid_set: std::collections::HashSet<&str> =
            valid_paths.iter().map(|s| s.as_str()).collect();

        for row in cached_rows {
            let path: String = row.get("path");
            if !valid_set.contains(path.as_str()) {
                let _ = sqlx::query("DELETE FROM file_cache WHERE path = ?")
                    .bind(&path)
                    .execute(&self.pool)
                    .await;
                for table in [
                    "session_cache",
                    "recent_cache",
                    "token_cache",
                    "tool_cache",
                    "duration_cache",
                    "hook_cache",
                    "model_cache",
                    "session_hourly_cache",
                ] {
                    let sql = format!("DELETE FROM {table} WHERE file_path = ?");
                    let _ = sqlx::query(&sql).bind(&path).execute(&self.pool).await;
                }
            }
        }
    }
}
