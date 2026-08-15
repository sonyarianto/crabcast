//! Criterion micro-benchmarks for the API hot paths the Phase 9 SLOs
//! target: password hashing/verification (auth), payload serialization,
//! and the listener-series query that powers the analytics chart. Run with
//! `cargo bench --manifest-path server/Cargo.toml`; record results in
//! ROADMAP.md §7 whenever numbers move materially.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use serde_json::json;

use crabcast_server::auth::{hash_password, verify_password};
use crabcast_server::db::analytics;
use crabcast_server::db::stations::Station;

fn fake_stations(count: usize) -> Vec<Station> {
    (0..count)
        .map(|i| Station {
            id: format!("station-{i}"),
            name: format!("Test Radio {i}"),
            description: "A benchmark station".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            sample_rate: 44100,
            channels: 2,
            frames_per_buffer: 4096,
            crossfade_seconds: 3.0,
            fade_curve: 1.0,
            duck_seconds: 1.5,
            playlist_dir: "/media/test".into(),
            jingles_dir: "/media/jingles".into(),
            harbor_port: 8005 + i as i64,
            harbor_mount: "/live".into(),
            harbor_password: "dj".into(),
            control_port: 1234 + i as i64,
            control_http_port: 9234 + i as i64,
            icecast_host: "localhost".into(),
            icecast_port: 8000,
            icecast_mount: "/radio".into(),
            icecast_format: "mp3".into(),
            icecast_bitrate: 128000,
            icecast_source_user: "source".into(),
            icecast_source_password: "hackme".into(),
            hls_enabled: false,
            hls_dir: String::new(),
            hls_segment_seconds: 5.0,
            hls_retention: 12,
            website: String::new(),
            facebook: String::new(),
            twitter: String::new(),
            instagram: String::new(),
        })
        .collect()
}

fn bench_auth(c: &mut Criterion) {
    let password = "correct horse battery staple";
    let hash = hash_password(password).expect("hash");
    let mut group = c.benchmark_group("auth");
    group.bench_function("hash_password", |b| {
        b.iter(|| hash_password(black_box(password)).expect("hash"))
    });
    group.bench_function("verify_password", |b| {
        b.iter(|| assert!(verify_password(black_box(password), &hash)))
    });
    group.finish();
}

fn bench_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize");
    for count in [10, 100, 1000] {
        let stations = fake_stations(count);
        group.bench_with_input(
            BenchmarkId::new("stations_json", count),
            &stations,
            |b, s| b.iter(|| serde_json::to_string(black_box(s)).expect("serialize")),
        );
    }
    // A typical public now-playing payload (nested JSON built by the API).
    let payload = json!({
        "id": "s1",
        "name": "Test Radio",
        "description": "desc",
        "stream_url": "/api/stations/s1/stream",
        "now": { "title": "Artist - Song", "started_at": "2026-01-01T00:00:00Z" },
        "history": [
            { "title": "Song A", "started_at": "2026-01-01T00:00:00Z" },
            { "title": "Song B", "started_at": "2026-01-01T00:00:00Z" },
        ]
    });
    group.bench_function("now_playing_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&payload)).expect("serialize"))
    });
    group.finish();
}

/// The listener-series query against a seeded in-memory SQLite database
/// (one station, 7 days of per-minute samples = 10 080 rows).
fn bench_listener_series(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let pool = rt.block_on(async {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        sqlx::raw_sql("CREATE TABLE stations (id TEXT PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("stations table");
        sqlx::query("INSERT INTO stations (id, name) VALUES ('s1', 'Bench FM')")
            .execute(&pool)
            .await
            .expect("station row");
        let mig = std::fs::read_to_string("migrations/0009_analytics.sql").expect("migration");
        sqlx::raw_sql(&mig).execute(&pool).await.expect("migrate");
        // Seed 7 days of per-minute samples with a 60-second bucket stride.
        let mut tx = pool.begin().await.expect("tx");
        let mut ts = "2026-07-01T00:00:00.000Z".to_string();
        for i in 0..(7 * 24 * 60) {
            sqlx::query(
                "INSERT INTO listener_samples (station_id, ts, listeners, listener_connections, reachable) \
VALUES ('s1', ?, ?, ?, 1)",
            )
            .bind(&ts)
            .bind((i % 25) as i64)
            .bind((i % 97) as i64)
            .execute(&mut *tx)
            .await
            .expect("insert");
            ts = format!("2026-07-01T00:00:{:02}.000Z", (i % 60) + 1);
            if i % 60 == 59 {
                ts = format!("2026-07-0{}T00:00:00.000Z", 1 + (i / 60 / 60) % 7);
            }
        }
        tx.commit().await.expect("commit");
        pool
    });

    let mut group = c.benchmark_group("db");
    group.bench_function("listener_series_7d_bucket_60m", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(
                    analytics::listener_series(
                        &pool,
                        "s1",
                        "2026-07-01T00:00:00Z",
                        "2026-07-08T00:00:00Z",
                        60,
                    )
                    .await
                    .expect("series"),
                )
            })
        })
    });
    group.finish();
}

criterion_group!(benches, bench_auth, bench_serialize, bench_listener_series);
criterion_main!(benches);
