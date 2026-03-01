use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use peeprs::cache::ScanCache;
use peeprs::models::DashboardSummary;
use peeprs::parse::{expand_tilde, now_iso};
use peeprs::scan::build_summary;
use peeprs::template::render_html;
use tokio::sync::Mutex;

#[derive(Parser, Debug, Clone)]
#[command(name = "peeprs")]
#[command(about = "AI agent centralized logs dashboard")]
struct Args {
    #[arg(long, default_value = "~/logs/agents")]
    root: String,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 8765)]
    port: u16,

    #[arg(long, default_value_t = 10)]
    refresh_seconds: u64,

    #[arg(long, default_value_t = 5)]
    cache_seconds: u64,

    #[arg(long)]
    cache_db: Option<String>,
}

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    refresh_ms: u64,
    cache_seconds: u64,
    cache: Arc<Mutex<SummaryCache>>,
    scan_cache: Option<Arc<ScanCache>>,
}

#[derive(Debug, Default)]
struct SummaryCache {
    at: Option<Instant>,
    summary: Option<DashboardSummary>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let root = expand_tilde(&args.root);
    let host = args.host.clone();
    let port = args.port;

    let cache_db_path = args
        .cache_db
        .map(|p| expand_tilde(&p))
        .unwrap_or_else(|| root.join(".peeprs.db"));

    let scan_cache = match ScanCache::open(&cache_db_path).await {
        Ok(c) => {
            println!("SQLite cache: {}", cache_db_path.display());
            Some(Arc::new(c))
        }
        Err(e) => {
            eprintln!("Warning: could not open cache DB: {e}");
            None
        }
    };

    let state = AppState {
        root,
        refresh_ms: args.refresh_seconds.max(1) * 1000,
        cache_seconds: args.cache_seconds.max(1),
        cache: Arc::new(Mutex::new(SummaryCache::default())),
        scan_cache,
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .route("/api/summary", get(summary_handler))
        .route("/healthz", get(health_handler))
        .with_state(state.clone());

    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    println!("Dashboard listening on http://{host}:{port}");
    println!("Log root: {}", state.root.display());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler(State(state): State<AppState>) -> Html<String> {
    Html(render_html(state.refresh_ms))
}

async fn summary_handler(
    State(state): State<AppState>,
) -> Result<Json<DashboardSummary>, (axum::http::StatusCode, String)> {
    let mut cache = state.cache.lock().await;
    let is_fresh = cache
        .at
        .map(|at| at.elapsed().as_secs() < state.cache_seconds)
        .unwrap_or(false);

    if is_fresh {
        if let Some(summary) = &cache.summary {
            return Ok(Json(summary.clone()));
        }
    }

    let scan_cache_ref = state.scan_cache.as_deref();
    let summary = build_summary(&state.root, scan_cache_ref)
        .await
        .map_err(|err| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to build summary: {err}"),
            )
        })?;

    cache.at = Some(Instant::now());
    cache.summary = Some(summary.clone());
    Ok(Json(summary))
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true, "time": now_iso() }))
}
