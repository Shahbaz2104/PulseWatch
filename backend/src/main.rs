//! PulseWatch backend entrypoint: configuration, database, HTTP server and
//! graceful shutdown.

use std::sync::Arc;

use pulsewatch_backend::{config::Config, db, routes, state, validation};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

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

    let http_client = validation::build_safe_http_client(&config).map_err(|err| {
        tracing::error!(error = %err, "failed to build HTTP client");
        err
    })?;
    tracing::info!("outbound HTTP client ready (SSRF-filtered DNS, bounded redirects)");

    let state = state::AppState {
        pool,
        config: config.clone(),
        http_client: Arc::new(http_client),
        semaphore: Arc::new(tokio::sync::Semaphore::new(config.max_concurrent_checks)),
        in_flight: Arc::new(state::InFlightSet::default()),
        shutdown: CancellationToken::new(),
    };

    if config.demo_mode {
        match pulsewatch_backend::services::demo_service::seed_if_empty(&state).await {
            Ok(outcome) if outcome.seeded() => tracing::info!(
                monitors = outcome.monitors,
                checks = outcome.checks,
                incidents = outcome.incidents,
                "seeded demo fixtures"
            ),
            Ok(_) => tracing::info!("demo data already present; skipping seed"),
            Err(err) => tracing::warn!(error = %err, "demo seeding failed"),
        }
    }

    let app = routes::router(state.clone());
    let _scheduler = pulsewatch_backend::services::scheduler::spawn(state.clone());
    let _alerts = pulsewatch_backend::services::alert_service::spawn(state.clone());
    let _retention = pulsewatch_backend::services::retention_service::spawn(state.clone());
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
