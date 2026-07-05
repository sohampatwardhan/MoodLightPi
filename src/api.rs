use crate::effects::{effect_names, is_valid_effect};
use crate::engine::{Command, EngineHandle, Source};
use crate::color::Rgb;
use axum::extract::State as AxState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone)]
pub struct SecurityConfig {
    /// Extra explicit host allowlist (from `MLP_HOST`, comma-separated). LAN
    /// addresses / hostnames are accepted automatically regardless of this.
    pub extra_hosts: Vec<String>,
}

/// Strip an optional `:port` (IPv4 / hostname; IPv6 literals aren't handled,
/// which is fine for LAN use).
fn host_label(h: &str) -> &str {
    h.split(':').next().unwrap_or(h)
}

/// True for hosts that can only be a device on the local network — private/
/// loopback/link-local IPv4 literals, `*.local`, bare hostnames, `localhost`.
/// A public FQDN (e.g. `evil.com`) returns false, which is what blocks
/// DNS-rebinding: the browser sends the *attacker's* domain as `Host`.
fn is_lan_hostish(h: &str) -> bool {
    let h = host_label(h);
    if h.is_empty() {
        return false;
    }
    if h == "localhost" || h.ends_with(".local") {
        return true;
    }
    match h.parse::<std::net::Ipv4Addr>() {
        Ok(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        Err(_) => !h.contains('.'), // bare hostname = LAN; dotted FQDN = reject
    }
}

impl SecurityConfig {
    fn allowed(&self, h: &str) -> bool {
        is_lan_hostish(h) || self.extra_hosts.iter().any(|e| e == host_label(h))
    }
    pub fn host_ok(&self, host_header: Option<&str>) -> bool {
        host_header.map(|h| self.allowed(h)).unwrap_or(false)
    }
    /// Origin is only sent by browsers. Absent = non-browser client (curl) = allow.
    /// Present = must resolve to a LAN host (defeats DNS-rebinding/CSRF).
    pub fn origin_ok(&self, origin_header: Option<&str>) -> bool {
        match origin_header {
            None => true,
            Some(o) => o
                .strip_prefix("http://")
                .or_else(|| o.strip_prefix("https://"))
                .map(|rest| self.allowed(rest))
                .unwrap_or(false),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub engine: EngineHandle,
    pub security: SecurityConfig,
    /// Flips to true when the process is shutting down; WS loops observe this so
    /// upgraded connections close promptly and don't keep graceful shutdown
    /// pending (which would block the engine from clearing the panel).
    pub shutdown: tokio::sync::watch::Receiver<bool>,
    /// Which Display backend is live: "hardware" or "mock".
    pub backend: &'static str,
}

fn check(headers: &HeaderMap, sec: &SecurityConfig) -> Result<(), StatusCode> {
    let host = headers.get("host").and_then(|v| v.to_str().ok());
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    if !sec.host_ok(host) || !sec.origin_ok(origin) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

async fn send(engine: &EngineHandle, cmd: Command) -> Result<(), StatusCode> {
    engine.commands.try_send((cmd, Source::Rest))
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

pub async fn healthz(AxState(st): AxState<AppState>) -> impl IntoResponse {
    // `backend` is "hardware" when the pHAT driver initialised, else "mock"
    // (on the Pi, "mock" signals a hardware init failure — see main.rs).
    Json(json!({ "alive": true, "backend": st.backend }))
}

async fn get_state(headers: HeaderMap, AxState(st): AxState<AppState>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    let snap = st.engine.snapshots.borrow().clone();
    Ok(Json(json!({ "state": snap.state, "seq": snap.seq })))
}

async fn get_effects(headers: HeaderMap, AxState(st): AxState<AppState>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(json!({ "effects": effect_names() })))
}

#[derive(Deserialize)] struct PowerBody { on: bool }
#[derive(Deserialize)] struct ColorBody { r: u8, g: u8, b: u8 }
#[derive(Deserialize)] struct BrightnessBody { value: u8 }
#[derive(Deserialize)] struct EffectBody { name: String, speed: Option<u8> }

async fn post_power(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<PowerBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetPower(b.on)).await?;
    Ok(StatusCode::OK)
}

async fn post_color(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<ColorBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetColor(Rgb { r: b.r, g: b.g, b: b.b })).await?;
    Ok(StatusCode::OK)
}

async fn post_brightness(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<BrightnessBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetBrightness(b.value)).await?;
    Ok(StatusCode::OK)
}

async fn post_effect(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<EffectBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    if !is_valid_effect(&b.name) {
        return Err(StatusCode::BAD_REQUEST);
    }
    send(&st.engine, Command::SetEffect { name: b.name, speed: b.speed }).await?;
    Ok(StatusCode::OK)
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(crate::web::serve_index))
        .route("/style.css", get(crate::web::serve_asset))
        .route("/app.js", get(crate::web::serve_asset))
        .route("/healthz", get(healthz))
        .route("/api/state", get(get_state))
        .route("/api/effects", get(get_effects))
        .route("/api/power", post(post_power))
        .route("/api/color", post(post_color))
        .route("/api/brightness", post(post_brightness))
        .route("/api/effect", post(post_effect))
        .route("/ws", get(crate::ws::ws_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_expected_lan_host() {
        let cfg = SecurityConfig { extra_hosts: vec![] };
        assert!(cfg.host_ok(Some("192.168.1.230")));
        assert!(cfg.host_ok(Some("192.168.1.230:80")));
    }

    #[test]
    fn rejects_foreign_host_and_missing() {
        let cfg = SecurityConfig { extra_hosts: vec![] };
        assert!(!cfg.host_ok(Some("evil.example.com")));
        assert!(!cfg.host_ok(None));
    }

    #[test]
    fn origin_ok_only_for_expected_or_absent_nonbrowser() {
        let cfg = SecurityConfig { extra_hosts: vec![] };
        assert!(cfg.origin_ok(Some("http://192.168.1.230")));
        assert!(cfg.origin_ok(None));
        assert!(!cfg.origin_ok(Some("http://evil.example.com")));
    }

    #[test]
    fn accepts_any_lan_host_and_rejects_public() {
        let cfg = SecurityConfig { extra_hosts: vec![] };
        // multi-homed Pi: any private IP, plus .local / bare hostname / localhost
        assert!(cfg.host_ok(Some("192.168.1.230")));
        assert!(cfg.host_ok(Some("192.168.2.161:80")));
        assert!(cfg.host_ok(Some("10.0.0.5")));
        assert!(cfg.host_ok(Some("moodlightpi.local")));
        assert!(cfg.host_ok(Some("moodlightpi")));
        assert!(cfg.host_ok(Some("localhost:80")));
        // public FQDN rejected (DNS-rebinding guard)
        assert!(!cfg.host_ok(Some("evil.example.com")));
        assert!(!cfg.origin_ok(Some("http://evil.example.com")));
    }

    use crate::display::MockDisplay;
    use crate::engine::Engine;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // oneshot

    async fn test_app() -> axum::Router {
        let (handle, engine) = Engine::new(MockDisplay::new(), crate::state::State::default());
        tokio::spawn(engine.run());
        // Shutdown never fires in these REST tests and no handler here observes
        // it, so dropping the sender is fine.
        let (_sd_tx, sd_rx) = tokio::sync::watch::channel(false);
        router(AppState {
            engine: handle,
            security: SecurityConfig { extra_hosts: vec![] },
            shutdown: sd_rx,
            backend: "mock",
        })
    }

    #[tokio::test]
    async fn healthz_ok() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().uri("/healthz").header("host", "testhost")
                .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn effects_lists_registry() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().uri("/api/effects").header("host", "testhost")
                .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_effect_is_400() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().method("POST").uri("/api/effect")
                .header("host", "testhost").header("content-type", "application/json")
                .body(Body::from(r#"{"name":"bogus"}"#)).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn foreign_host_is_403() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().uri("/api/state").header("host", "evil.com")
                .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}
