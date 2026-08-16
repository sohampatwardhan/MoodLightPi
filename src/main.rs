mod api;
mod color;
mod display;
mod effects;
mod engine;
mod geometry;
#[cfg(feature = "hardware")]
mod hardware;
mod homekit;
mod mqtt;
mod persist;
mod settings;
mod state;
mod web;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Returns the live display and a label ("hardware" | "mock") for `/healthz`.
fn select_display() -> (Box<dyn display::Display>, &'static str) {
    #[cfg(feature = "hardware")]
    {
        match hardware::Ws281xDisplay::new(10) {
            Ok(d) => {
                tracing::info!("hardware display initialised");
                return (Box::new(d), "hardware");
            }
            Err(e) => {
                tracing::error!("hardware init failed ({e}); running with mock display");
            }
        }
    }
    (Box::new(display::MockDisplay::new()), "mock")
}

/// Resolve on SIGINT (ctrl-c) or SIGTERM (systemctl stop).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut s) = signal(SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
    tracing::info!("shutdown signal received");
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // 1. Load state (or safe default).
    let state_path = PathBuf::from(persist::DEFAULT_PATH);
    let initial = persist::load(&state_path);

    // 2. Init hardware (mock fallback) and 3. start engine seeded with state.
    let (display, backend) = select_display();
    let (handle, engine) = engine::Engine::new_boxed(display, initial.clone());
    let engine_task = tokio::spawn(engine.run());

    // Persist snapshots (rate-limited, atomic). An interval flush lands any
    // rate-limited pending write even without further changes; a final flush
    // runs when the snapshot channel closes on shutdown.
    let persist_task = {
        let mut rx = handle.snapshots.clone();
        let path = state_path.clone();
        tokio::spawn(async move {
            let mut persister = persist::Persister::new(path, Duration::from_millis(1000), initial);
            let mut flush_tick = tokio::time::interval(Duration::from_millis(1000));
            loop {
                tokio::select! {
                    changed = rx.changed() => {
                        if changed.is_err() {
                            break; // engine gone
                        }
                        let snap = rx.borrow().clone();
                        persister.record(&snap.state);
                    }
                    _ = flush_tick.tick() => {
                        persister.flush();
                    }
                }
            }
            persister.flush();
        })
    };

    // Shutdown flag observed by WS loops so upgraded connections close promptly
    // (otherwise an open browser tab keeps hyper's graceful shutdown pending and
    // the engine never gets to clear the panel).
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // 4. Bind HTTP only after the engine is live.
    // Optional extra host allowlist (comma-separated); LAN hosts (private IPs,
    // *.local, bare hostnames, localhost) are accepted automatically.
    let extra_hosts = std::env::var("MLP_HOST")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let system_paths = settings::SystemPaths::from_env();
    let settings_store = settings::SettingsStore::new(system_paths.settings.clone());
    let homekit_status = homekit::HomeKitStatus::new();
    let mqtt_task = tokio::spawn(mqtt::run(
        handle.clone(),
        settings_store.clone(),
        shutdown_rx.clone(),
    ));
    let homekit_task = tokio::spawn(homekit::run(
        handle.clone(),
        settings_store.clone(),
        system_paths.homekit_storage.clone(),
        homekit_status.clone(),
        shutdown_rx.clone(),
    ));
    let app = api::router(api::AppState {
        engine: handle,
        security: api::SecurityConfig { extra_hosts },
        shutdown: shutdown_rx,
        backend,
        settings: settings_store,
        system_paths,
        homekit_status,
    });
    // Bind all interfaces by default (the Pi may be multi-homed); the Host/Origin
    // check — not the bind address — is what enforces the LAN-only policy.
    let bind = std::env::var("MLP_BIND").unwrap_or_else(|_| "0.0.0.0:80".into());
    let addr: SocketAddr = bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr} (backend: {backend})");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            // Signal WS loops to close BEFORE hyper waits on connection tasks.
            let _ = shutdown_tx.send(true);
        })
        .await?;

    // `app` (holding the only command senders) is dropped when serve returns, so
    // the engine loop ends and clears the panel; the persister then sees the
    // snapshot channel close and flushes.
    let _ = mqtt_task.await;
    let _ = homekit_task.await;
    let _ = engine_task.await;
    let _ = persist_task.await;
    tracing::info!("stopped cleanly");
    Ok(())
}
