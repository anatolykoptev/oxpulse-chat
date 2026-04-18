use std::net::SocketAddr;

use axum::http::HeaderName;
use tokio::signal;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use oxpulse_chat::config::Config;
use oxpulse_chat::router::{build_router, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .json()
        .init();

    let config = Config::from_env();
    oxpulse_chat::branding::init();

    let rooms = oxpulse_signaling::Rooms::new();
    rooms.start_cleanup_task();

    let pool = if let Some(ref db_url) = config.database_url {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .connect(db_url)
            .await
            .expect("failed to connect to database");
        oxpulse_chat::migrate::run(&pool).await;
        Some(pool)
    } else {
        tracing::warn!("DATABASE_URL not set — analytics disabled");
        None
    };

    let turn_pool = oxpulse_chat::turn_pool::TurnPool::new(config.turn_servers.clone());
    let _probe_handle = turn_pool.start_probe_task(
        std::time::Duration::from_secs(config.turn_probe_interval_secs),
        config.turn_unhealthy_after_fails,
    );

    let state = AppState {
        rooms,
        turn_secret: config.turn_secret,
        turn_urls: config.turn_urls,
        stun_urls: config.stun_urls,
        pool,
        turn_pool,
        metrics: std::sync::Arc::new(oxpulse_chat::metrics::Metrics::new()),
        metrics_token: config.metrics_token,
    };

    let cors = build_cors(&config.cors_origins);

    let app = build_router(state, &config.room_assets_dir)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr = SocketAddr::new(
        config.bind_address.parse().expect("invalid BIND_ADDRESS"),
        config.port,
    );
    tracing::info!(%addr, "starting oxpulse-chat");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

fn build_cors(origins: &[String]) -> CorsLayer {
    if origins.len() == 1 && origins[0] == "*" {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }
    let origins: Vec<_> = origins.iter().filter_map(|o| o.parse().ok()).collect();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(Any)
        .allow_headers(vec![
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
        ])
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.expect("failed to listen for ctrl+c") };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("received SIGINT"),
        () = terminate => tracing::info!("received SIGTERM"),
    }
}
