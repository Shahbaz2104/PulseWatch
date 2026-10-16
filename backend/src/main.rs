//! PulseWatch backend entrypoint: configuration, database, HTTP server and
//! graceful shutdown.

mod config;
mod db;
mod error;
mod models;
mod openapi;
mod routes;
mod state;

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = Arc::new(Config::from_env().map_err(|err| {
        tracing::error!(error = %err, "invalid configuration");
        err
    })?);
    tracing::info!(
        bind = %config.bind_addr(),
        max_concurrent_checks = config.max_concurrent_checks,
        "starting PulseWatch backend"
    );

    let pool = db::connect(&config.database_url).await?;
    tracing::info!("database ready (migrations applied)");

    let http_client = reqwest::Client::builder()
        .user_agent(concat!("PulseWatch/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let state = AppState {
        pool,
        config: config.clone(),
        http_client: Arc::new(http_client),
        semaphore: Arc::new(tokio::sync::Semaphore::new(config.max_concurrent_checks)),
        in_flight: Arc::new(state::InFlightSet::default()),
        shutdown: CancellationToken::new(),
    };

    let app = routes::router(state.clone());
    let listener = tokio::net::TcpListener::bind(config.bind_addr()).await?;
    tracing::info!(addr = %listener.local_addr()?, "HTTP server listening");

    let shutdown_token = state.shutdown.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            wait_for_shutdown_signal().await;
            tracing::info!("shutdown signal received");
            shutdown_token.cancel();
        })
        .await?;

    // Allow in-flight work (scheduler tasks, running checks) to observe the
    // cancellation token and finish cleanly before the process exits.
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    tracing::info!("PulseWatch backend stopped");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,pulsewatch_backend=info,tower_http=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
