mod api;
mod color;
mod display;
mod effects;
mod engine;
mod geometry;
mod persist;
mod state;
mod web;
mod ws;
#[cfg(feature = "hardware")]
mod hardware;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

fn select_display() -> Box<dyn display::Display> {
    #[cfg(feature = "hardware")]
    {
        match hardware::Ws281xDisplay::new(10) {
            Ok(d) => {
                tracing::info!("hardware display initialised");
                return Box::new(d);
            }
            Err(e) => {
                tracing::error!("hardware init failed ({e}); running with mock display");
            }
        }
    }
    Box::new(display::MockDisplay::new())
}

/// Resolve on SIGINT (ctrl-c) or SIGTERM (systemctl stop).
async fn shutdown_signal() {
    let ctrl_c = async { let _ = tokio::signal::ctrl_c().await; };
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
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // 1. Load state (or safe default).
    let state_path = PathBuf::from(persist::DEFAULT_PATH);
    let initial = persist::load(&state_path);

    // 2. Init hardware (mock fallback) and 3. start engine seeded with state.
    let display = select_display();
    let (handle, engine) = engine::Engine::new_boxed(display, initial);
    let engine_task = tokio::spawn(engine.run());

    // Persist snapshots (rate-limited, atomic); flush when the channel closes.
    let persist_task = {
        let mut rx = handle.snapshots.clone();
        let path = state_path.clone();
        tokio::spawn(async move {
            let mut persister = persist::Persister::new(path, Duration::from_millis(1000));
            while rx.changed().await.is_ok() {
                let snap = rx.borrow().clone();
                persister.record(&snap.state);
            }
            persister.flush();
        })
    };

    // 4. Bind HTTP only after the engine is live.
    let allowed_host = std::env::var("MLP_HOST").unwrap_or_else(|_| "192.168.1.230".into());
    let app = api::router(api::AppState {
        engine: handle,
        security: api::SecurityConfig { allowed_host },
    });
    let bind = std::env::var("MLP_BIND").unwrap_or_else(|_| "192.168.1.230:80".into());
    let addr: SocketAddr = bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Graceful shutdown: `app` (holding the only command senders) is dropped
    // here, so the engine loop ends and clears the panel; the persister then
    // sees the watch channel close and flushes pending state.
    let _ = engine_task.await;
    let _ = persist_task.await;
    tracing::info!("stopped cleanly");
    Ok(())
}
