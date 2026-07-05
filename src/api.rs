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
    /// Host (no scheme/port) the service is reached at on the LAN.
    pub allowed_host: String,
}

impl SecurityConfig {
    pub fn host_ok(&self, host_header: Option<&str>) -> bool {
        match host_header {
            Some(h) => h.split(':').next() == Some(self.allowed_host.as_str()),
            None => false,
        }
    }
    /// Origin is only sent by browsers. Absent = non-browser client (curl) = allow.
    /// Present = must match the expected host (defeats DNS-rebinding/CSRF).
    pub fn origin_ok(&self, origin_header: Option<&str>) -> bool {
        match origin_header {
            None => true,
            Some(o) => o
                .strip_prefix("http://")
                .or_else(|| o.strip_prefix("https://"))
                .map(|rest| rest.split(':').next() == Some(self.allowed_host.as_str()))
                .unwrap_or(false),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub engine: EngineHandle,
    pub security: SecurityConfig,
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

pub async fn healthz() -> impl IntoResponse {
    Json(json!({ "alive": true, "hardware": "ok" }))
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
        .route("/healthz", get(healthz))
        .route("/api/state", get(get_state))
        .route("/api/effects", get(get_effects))
        .route("/api/power", post(post_power))
        .route("/api/color", post(post_color))
        .route("/api/brightness", post(post_brightness))
        .route("/api/effect", post(post_effect))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_expected_lan_host() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(cfg.host_ok(Some("192.168.1.230")));
        assert!(cfg.host_ok(Some("192.168.1.230:80")));
    }

    #[test]
    fn rejects_foreign_host_and_missing() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(!cfg.host_ok(Some("evil.example.com")));
        assert!(!cfg.host_ok(None));
    }

    #[test]
    fn origin_ok_only_for_expected_or_absent_nonbrowser() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(cfg.origin_ok(Some("http://192.168.1.230")));
        assert!(cfg.origin_ok(None));
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
        router(AppState {
            engine: handle,
            security: SecurityConfig { allowed_host: "testhost".into() },
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
