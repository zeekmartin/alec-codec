// ALEC Exporter - Prometheus exporter for ALEC metrics
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! # ALEC Exporter
//!
//! Prometheus exporter for ALEC metrics with dataset replay support.
//!
//! ## Usage
//!
//! ```bash
//! # Run with a CSV dataset
//! alec-exporter --csv dataset.csv --speed 10.0
//!
//! # Run on custom port
//! alec-exporter --csv dataset.csv --port 9090
//! ```

mod metrics;

#[cfg(feature = "replay")]
mod replay;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use clap::Parser;
use metrics::encode_metrics;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

#[cfg(feature = "replay")]
use replay::{DatasetInfo, ReplayConfig, ReplayEngine, ReplayState};

/// ALEC Prometheus Exporter
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "9100")]
    port: u16,

    /// CSV file to replay
    #[arg(short, long)]
    csv: Option<String>,

    /// Replay speed multiplier (1.0 = real-time)
    #[arg(short, long, default_value = "1.0")]
    speed: f64,

    /// Loop the replay when it reaches the end
    #[arg(short, long, default_value = "true")]
    loop_replay: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

/// Application state shared across handlers.
struct AppState {
    #[cfg(feature = "replay")]
    replay_state: Option<Arc<ReplayState>>,
    #[cfg(feature = "replay")]
    dataset_info: Option<DatasetInfo>,
    #[allow(dead_code)]
    start_time: std::time::Instant,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize tracing
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match args.log_level.to_lowercase().as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" => Level::WARN,
            "error" => Level::ERROR,
            _ => Level::INFO,
        };
        EnvFilter::from_default_env().add_directive(level.into())
    });

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("ALEC Exporter v{}", env!("CARGO_PKG_VERSION"));

    // Initialize replay engine if CSV provided
    #[cfg(feature = "replay")]
    let (replay_state, dataset_info) = if let Some(csv_path) = args.csv.clone() {
        let config = ReplayConfig {
            csv_path,
            speed: args.speed,
            loop_replay: args.loop_replay,
            default_sample_interval_ms: 60_000,
        };

        match ReplayEngine::from_csv(config) {
            Ok(engine) => {
                let state = engine.state();
                let info = engine.dataset_info();

                info!(
                    "Dataset loaded: {} sensors, {} samples",
                    info.sensor_count, info.sample_count
                );

                // Start replay in background
                tokio::spawn(async move {
                    engine.run().await;
                });

                (Some(state), Some(info))
            }
            Err(e) => {
                tracing::error!("Failed to load dataset: {}", e);
                (None, None)
            }
        }
    } else {
        info!("No dataset specified, running in static mode");
        (None, None)
    };

    #[cfg(not(feature = "replay"))]
    if args.csv.is_some() {
        tracing::warn!("Replay feature not enabled, ignoring --csv argument");
    }

    // Create app state
    let state = Arc::new(AppState {
        #[cfg(feature = "replay")]
        replay_state,
        #[cfg(feature = "replay")]
        dataset_info,
        start_time: std::time::Instant::now(),
    });

    // Build router
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/status", get(status_handler))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    info!("Starting server on http://{}", addr);
    info!("Metrics endpoint: http://{}/metrics", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Root handler - shows a simple HTML page.
async fn root_handler() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>ALEC Exporter</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 800px; margin: 50px auto; padding: 20px; }
        h1 { color: #2c3e50; }
        a { color: #3498db; text-decoration: none; }
        a:hover { text-decoration: underline; }
        .endpoints { background: #f8f9fa; padding: 20px; border-radius: 8px; margin: 20px 0; }
        .endpoint { margin: 10px 0; }
        code { background: #e9ecef; padding: 2px 6px; border-radius: 4px; }
    </style>
</head>
<body>
    <h1>ALEC Exporter</h1>
    <p>Prometheus exporter for ALEC (Adaptive Lossless Entropy Codec) metrics.</p>

    <div class="endpoints">
        <h2>Endpoints</h2>
        <div class="endpoint"><a href="/metrics">/metrics</a> - Prometheus metrics</div>
        <div class="endpoint"><a href="/health">/health</a> - Health check</div>
        <div class="endpoint"><a href="/ready">/ready</a> - Readiness check</div>
        <div class="endpoint"><a href="/status">/status</a> - Status information (JSON)</div>
    </div>

    <h2>Metrics</h2>
    <ul>
        <li><code>alec_resilience_index</code> - Resilience Index (R)</li>
        <li><code>alec_resilience_zone</code> - Zone (0=Healthy, 1=Warning, 2=Critical)</li>
        <li><code>alec_total_correlation_bits</code> - Total Correlation</li>
        <li><code>alec_joint_entropy_bits</code> - Joint Entropy</li>
        <li><code>alec_payload_entropy_bits</code> - Payload Entropy</li>
        <li><code>alec_channel_entropy_bits</code> - Per-channel entropy</li>
        <li><code>alec_baseline_progress</code> - Baseline learning progress</li>
        <li><code>alec_zscore_*</code> - Z-scores for deviation detection</li>
        <li><code>alec_anomaly_events_total</code> - Anomaly event counter</li>
    </ul>

    <p>See <a href="https://github.com/zeekmartin/alec-codec">alec-codec</a> for more information.</p>
</body>
</html>"#,
    )
}

/// Metrics handler - returns Prometheus text format.
async fn metrics_handler() -> impl IntoResponse {
    let metrics = encode_metrics();
    (
        StatusCode::OK,
        [("Content-Type", "text/plain; charset=utf-8")],
        metrics,
    )
}

/// Health check handler.
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Readiness check handler.
async fn ready_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    #[cfg(feature = "replay")]
    {
        if let Some(ref replay_state) = state.replay_state {
            if replay_state
                .running
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return (StatusCode::OK, "Ready");
            }
        }
    }
    let _ = state; // Silence unused warning
    (StatusCode::OK, "Ready")
}

/// Status information response.
#[derive(Serialize)]
struct StatusResponse {
    version: String,
    uptime_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay: Option<ReplayStatus>,
}

/// Replay status information.
#[derive(Serialize)]
struct ReplayStatus {
    running: bool,
    paused: bool,
    position: usize,
    total_samples: usize,
    progress_percent: f64,
    sensor_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
}

/// Status handler - returns JSON status information.
async fn status_handler(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    #[cfg(feature = "replay")]
    let replay = if let Some(ref replay_state) = state.replay_state {
        let position = replay_state
            .position
            .load(std::sync::atomic::Ordering::SeqCst);
        let total = replay_state
            .total_samples
            .load(std::sync::atomic::Ordering::SeqCst);
        let progress = if total > 0 {
            (position as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Some(ReplayStatus {
            running: replay_state
                .running
                .load(std::sync::atomic::Ordering::SeqCst),
            paused: replay_state
                .paused
                .load(std::sync::atomic::Ordering::SeqCst),
            position,
            total_samples: total,
            progress_percent: progress,
            sensor_count: state
                .dataset_info
                .as_ref()
                .map(|i| i.sensor_count)
                .unwrap_or(0),
            duration_ms: state.dataset_info.as_ref().map(|i| i.duration_ms),
        })
    } else {
        None
    };

    #[cfg(not(feature = "replay"))]
    let replay: Option<ReplayStatus> = None;

    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: state.start_time.elapsed().as_secs(),
        replay,
    })
}
