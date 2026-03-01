use std::io::Cursor;
use std::path::PathBuf;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use tempfile::TempDir;

use peeprs::models::{EventFile, RecentEvent, RecentEventAccum};
use peeprs::parse::{compact_preview, parse_event_line};
use peeprs::scan::{build_summary, insert_recent_event, scan_event_lines};
use peeprs::template::render_html;

const VALID_JSON_LINE: &str = r#"{"timestamp":"2025-06-01T12:00:00.000Z","session":"sess-abc123","type":"assistant","message":{"content":[{"type":"text","text":"Hello world, this is a test event with some content for benchmarking purposes."},{"type":"tool_use","name":"Read","input":{}}],"usage":{"input_tokens":500,"output_tokens":200,"cache_read_input_tokens":1000,"cache_creation_input_tokens":50},"model":"claude-3-opus-20240229"}}"#;

const INVALID_LINE: &str = "this is not valid json at all {{{";

fn make_event_file() -> EventFile {
    EventFile {
        day: "2025-06-01".to_string(),
        session: "sess-fallback".to_string(),
        channel: "json".to_string(),
        file_type: "json".to_string(),
        path: PathBuf::from("/tmp/fake/events.jsonl"),
    }
}

fn bench_parse_event_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_event_line");
    let ef = make_event_file();

    group.bench_function("valid_json", |b| {
        b.iter(|| parse_event_line(black_box(VALID_JSON_LINE), black_box(&ef)))
    });

    group.bench_function("invalid_line", |b| {
        b.iter(|| parse_event_line(black_box(INVALID_LINE), black_box(&ef)))
    });

    group.finish();
}

fn bench_scan_event_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan_event_lines");
    let ef = make_event_file();

    for n in [10, 100, 1000] {
        let data = std::iter::repeat_n(VALID_JSON_LINE, n)
            .collect::<Vec<_>>()
            .join("\n");

        group.bench_with_input(BenchmarkId::from_parameter(n), &data, |b, data| {
            b.iter(|| {
                let cursor = Cursor::new(data.as_bytes());
                let reader = std::io::BufReader::new(cursor);
                scan_event_lines(reader, black_box(&ef)).unwrap()
            })
        });
    }

    group.finish();
}

fn bench_build_summary(c: &mut Criterion) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create realistic structure: 5 days x 3 sessions x 1 channel
    for day_idx in 1..=5 {
        let day = format!("2025-06-{day_idx:02}");
        for sess_idx in 1..=3 {
            let session = format!("sess-{sess_idx:03}");
            let dir = root.join(&day).join(&session).join("json");
            std::fs::create_dir_all(&dir).unwrap();

            let mut content = String::new();
            for _ in 0..20 {
                content.push_str(VALID_JSON_LINE);
                content.push('\n');
            }
            std::fs::write(dir.join("events.jsonl"), &content).unwrap();
        }
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("build_summary", |b| {
        b.iter(|| rt.block_on(build_summary(black_box(root), None)).unwrap())
    });
}

fn bench_render_html(c: &mut Criterion) {
    c.bench_function("render_html", |b| {
        b.iter(|| render_html(black_box(5000)))
    });
}

fn bench_insert_recent_event(c: &mut Criterion) {
    c.bench_function("insert_recent_event", |b| {
        b.iter_batched(
            || {
                // Setup: pool of 5 events
                (0..5)
                    .map(|i| RecentEventAccum {
                        sort_key_ms: 1_000_000 + i * 1000,
                        event: RecentEvent {
                            timestamp: format!("2025-06-01T12:00:0{i}.000Z"),
                            session: "sess-bench".to_string(),
                            agent: "unknown".to_string(),
                            event_type: "assistant".to_string(),
                            preview: "bench event".to_string(),
                        },
                    })
                    .collect::<Vec<_>>()
            },
            |mut pool| {
                let new_event = RecentEventAccum {
                    sort_key_ms: 1_003_500,
                    event: RecentEvent {
                        timestamp: "2025-06-01T12:00:03.500Z".to_string(),
                        session: "sess-new".to_string(),
                        agent: "unknown".to_string(),
                        event_type: "user".to_string(),
                        preview: "new event".to_string(),
                    },
                };
                insert_recent_event(black_box(&mut pool), new_event);
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_compact_preview(c: &mut Criterion) {
    let mut group = c.benchmark_group("compact_preview");

    let short = "This is a short preview string for benchmarking.";
    let long = "A ".repeat(1000);

    group.bench_function("short_50chars", |b| {
        b.iter(|| compact_preview(black_box(short), black_box(110)))
    });

    group.bench_function("long_2000chars", |b| {
        b.iter(|| compact_preview(black_box(&long), black_box(110)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_event_line,
    bench_scan_event_lines,
    bench_build_summary,
    bench_render_html,
    bench_insert_recent_event,
    bench_compact_preview,
);
criterion_main!(benches);
