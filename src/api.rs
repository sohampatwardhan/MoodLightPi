use crate::color::Rgb;
use crate::effects::{effect_names, is_valid_effect};
use crate::engine::{Command, EngineHandle, Source};
use crate::homekit::HomeKitStatus;
use crate::settings::{
    generate_pairing_code, read_authorized_keys, read_wifi_settings, save_authorized_keys,
    save_wifi_settings, scan_wifi_networks, validate_mqtt_settings, HomeKitSettings,
    IdentitySettings, MqttSettings, SettingsStore, SystemPaths, WifiSave,
};
use axum::extract::State as AxState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Command as ProcessCommand;
use std::time::Duration;

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
    pub settings: SettingsStore,
    pub system_paths: SystemPaths,
    pub homekit_status: HomeKitStatus,
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
    engine
        .commands
        .try_send((cmd, Source::Rest))
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

fn json_error(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(json!({ "error": message.into() })))
}

pub async fn healthz(AxState(st): AxState<AppState>) -> impl IntoResponse {
    // `backend` is "hardware" when the pHAT driver initialised, else "mock"
    // (on the Pi, "mock" signals a hardware init failure — see main.rs).
    Json(json!({ "alive": true, "backend": st.backend }))
}

async fn get_state(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    let snap = st.engine.snapshots.borrow().clone();
    Ok(Json(json!({ "state": snap.state, "seq": snap.seq })))
}

async fn get_effects(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(json!({ "effects": effect_names() })))
}

#[derive(Deserialize)]
struct PowerBody {
    on: bool,
}
#[derive(Deserialize)]
struct ColorBody {
    r: u8,
    g: u8,
    b: u8,
}
#[derive(Deserialize)]
struct BrightnessBody {
    value: u8,
}
#[derive(Deserialize)]
struct EffectBody {
    name: String,
    speed: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SystemAction {
    RestartService,
    Reboot,
    Poweroff,
}

impl SystemAction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "restart_service" => Some(Self::RestartService),
            "reboot" => Some(Self::Reboot),
            "poweroff" => Some(Self::Poweroff),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::RestartService => "restart_service",
            Self::Reboot => "reboot",
            Self::Poweroff => "poweroff",
        }
    }

    fn systemctl_args(self) -> &'static [&'static str] {
        match self {
            Self::RestartService => &["--no-block", "restart", "moodlightpi.service"],
            Self::Reboot => &["--no-block", "reboot"],
            Self::Poweroff => &["--no-block", "poweroff"],
        }
    }
}

#[derive(Deserialize)]
struct SystemActionBody {
    action: String,
}

async fn post_power(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(b): Json<PowerBody>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetPower(b.on)).await?;
    Ok(StatusCode::OK)
}

async fn post_color(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(b): Json<ColorBody>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(
        &st.engine,
        Command::SetColor(Rgb {
            r: b.r,
            g: b.g,
            b: b.b,
        }),
    )
    .await?;
    Ok(StatusCode::OK)
}

async fn post_brightness(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(b): Json<BrightnessBody>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetBrightness(b.value)).await?;
    Ok(StatusCode::OK)
}

async fn post_effect(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(b): Json<EffectBody>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    if !is_valid_effect(&b.name) {
        return Err(StatusCode::BAD_REQUEST);
    }
    send(
        &st.engine,
        Command::SetEffect {
            name: b.name,
            speed: b.speed,
        },
    )
    .await?;
    Ok(StatusCode::OK)
}

async fn post_system_action(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(body): Json<SystemActionBody>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    let action = SystemAction::parse(&body.action)
        .ok_or_else(|| json_error(StatusCode::BAD_REQUEST, "unknown system action"))?;
    queue_system_action(action).map_err(|e| {
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not queue system action: {e}"),
        )
    })?;
    Ok(Json(json!({ "queued": true, "action": action.id() })))
}

fn queue_system_action(action: SystemAction) -> anyhow::Result<()> {
    std::thread::Builder::new()
        .name(format!("system-action-{}", action.id()))
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            if let Err(e) = ProcessCommand::new("systemctl")
                .args(action.systemctl_args())
                .spawn()
            {
                tracing::error!("failed to run system action {}: {e}", action.id());
            }
        })?;
    Ok(())
}

async fn get_identity(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(st.settings.load().identity))
}

async fn post_identity(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(body): Json<IdentitySettings>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    if body.device_name.trim().is_empty() || body.light_label.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "device name and light label are required",
        ));
    }
    let settings = st
        .settings
        .update(|settings| {
            settings.identity = IdentitySettings {
                device_name: body.device_name.trim().to_string(),
                room: body.room.trim().to_string(),
                light_label: body.light_label.trim().to_string(),
            };
        })
        .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(settings.identity))
}

#[derive(Clone, Debug, Serialize)]
struct HomeKitSettingsResponse {
    accessory_name: String,
    enabled: bool,
    pairing_code: Option<String>,
    status: String,
    paired: bool,
    ready_to_pair: bool,
    port: Option<u16>,
}

fn homekit_response(settings: HomeKitSettings, status: HomeKitStatus) -> HomeKitSettingsResponse {
    let runtime = status.snapshot();
    let pairing_code = if runtime.ready_to_pair && !runtime.paired {
        settings.pairing_code
    } else {
        None
    };
    HomeKitSettingsResponse {
        accessory_name: settings.accessory_name,
        enabled: settings.enabled,
        pairing_code,
        status: runtime.status,
        paired: runtime.paired,
        ready_to_pair: runtime.ready_to_pair,
        port: runtime.port,
    }
}

async fn get_homekit(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(homekit_response(
        st.settings.load().homekit,
        st.homekit_status,
    )))
}

async fn post_homekit(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(body): Json<HomeKitSettings>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    if body.accessory_name.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "accessory name is required",
        ));
    }
    let previous_code = st.settings.load().homekit.pairing_code;
    let pairing_code = if body.enabled {
        body.pairing_code
            .or(previous_code)
            .or_else(|| generate_pairing_code().ok())
    } else {
        body.pairing_code.or(previous_code)
    };
    let settings = st
        .settings
        .update(|settings| {
            settings.homekit.accessory_name = body.accessory_name.trim().to_string();
            settings.homekit.enabled = body.enabled;
            settings.homekit.pairing_code = pairing_code;
        })
        .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(homekit_response(settings.homekit, st.homekit_status)))
}

async fn post_homekit_pairing_code(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    let code = generate_pairing_code()
        .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let settings = st
        .settings
        .update(|settings| {
            settings.homekit.pairing_code = Some(code);
        })
        .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(homekit_response(settings.homekit, st.homekit_status)))
}

#[derive(Clone, Debug, serde::Serialize)]
struct MqttSettingsResponse {
    enabled: bool,
    broker_url: String,
    client_id: String,
    username: String,
    password_set: bool,
    subscribe_topic: String,
    publish_topic: String,
    availability_topic: String,
    discovery_enabled: bool,
    discovery_prefix: String,
    status: &'static str,
}

impl From<MqttSettings> for MqttSettingsResponse {
    fn from(settings: MqttSettings) -> Self {
        let status = if settings.enabled {
            "Reconnect pending"
        } else {
            "Disabled"
        };
        let password_set = settings.password_set();
        Self {
            enabled: settings.enabled,
            broker_url: settings.broker_url,
            client_id: settings.client_id,
            username: settings.username,
            password_set,
            subscribe_topic: settings.subscribe_topic,
            publish_topic: settings.publish_topic,
            availability_topic: settings.availability_topic,
            discovery_enabled: settings.discovery_enabled,
            discovery_prefix: settings.discovery_prefix,
            status,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct MqttSettingsSave {
    enabled: bool,
    broker_url: String,
    client_id: String,
    username: String,
    password: String,
    clear_password: bool,
    subscribe_topic: String,
    publish_topic: String,
    availability_topic: String,
    discovery_enabled: bool,
    discovery_prefix: String,
}

async fn get_mqtt(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(MqttSettingsResponse::from(st.settings.load().mqtt)))
}

async fn post_mqtt(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(body): Json<MqttSettingsSave>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    let existing = st.settings.load().mqtt;
    let password = if body.clear_password || body.username.trim().is_empty() {
        None
    } else if body.password.is_empty() {
        existing.password.clone()
    } else {
        Some(body.password)
    };
    let next = MqttSettings {
        enabled: body.enabled,
        broker_url: body.broker_url.trim().to_string(),
        client_id: body.client_id.trim().to_string(),
        username: body.username.trim().to_string(),
        password,
        subscribe_topic: body.subscribe_topic.trim().to_string(),
        publish_topic: body.publish_topic.trim().to_string(),
        availability_topic: body.availability_topic.trim().to_string(),
        discovery_enabled: body.discovery_enabled,
        discovery_prefix: body.discovery_prefix.trim().to_string(),
    };
    validate_mqtt_settings(&next)
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, e.to_string()))?;
    let settings = st
        .settings
        .update(|settings| {
            settings.mqtt = next;
        })
        .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(MqttSettingsResponse::from(settings.mqtt)))
}

async fn get_wifi(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(read_wifi_settings(&st.system_paths.wpa_supplicant)))
}

async fn post_wifi(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(body): Json<WifiSave>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    let result = save_wifi_settings(&st.system_paths.wpa_supplicant, &body)
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(result))
}

async fn get_wifi_scan(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    let result = scan_wifi_networks()
        .map_err(|e| json_error(StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct SshKeysBody {
    keys: String,
}

async fn get_ssh_keys(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(
        json!({ "keys": read_authorized_keys(&st.system_paths.authorized_keys) }),
    ))
}

async fn post_ssh_keys(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(body): Json<SshKeysBody>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    save_authorized_keys(&st.system_paths.authorized_keys, &body.keys)
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(json!({ "saved": true })))
}

async fn post_ssh_keys_validate(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
    Json(body): Json<SshKeysBody>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check(&headers, &st.security).map_err(|s| json_error(s, "request rejected"))?;
    crate::settings::validate_authorized_keys(&body.keys)
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(json!({ "valid": true })))
}

/// Aggregate first-load payload: current light state + seq, the effect list, and every settings
/// section, in one Host/Origin-gated response. Each field mirrors its individual endpoint exactly
/// (same sources: engine snapshot, `effect_names`, settings store, wpa/authorized_keys readers),
/// so the SPA can seed the whole UI in one round-trip instead of eight (R11.1–R11.3). Gated
/// because it exposes settings (MQTT username, SSH keys) just as the per-section endpoints do
/// (R14.2).
async fn get_bootstrap(
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    let snap = st.engine.snapshots.borrow().clone();
    let settings = st.settings.load();
    let mqtt = MqttSettingsResponse::from(settings.mqtt.clone());
    let homekit = homekit_response(settings.homekit.clone(), st.homekit_status.clone());
    let wifi = read_wifi_settings(&st.system_paths.wpa_supplicant);
    let ssh_keys = read_authorized_keys(&st.system_paths.authorized_keys);
    Ok(Json(json!({
        "state": snap.state,
        "seq": snap.seq,
        "effects": effect_names(),
        "settings": {
            "identity": settings.identity,
            "mqtt": mqtt,
            "homekit": homekit,
            "wifi": wifi,
            "ssh_keys": ssh_keys,
        },
    })))
}

pub fn router(state: AppState) -> Router {
    // The enumerated `serve_index` client routes and the `/style.css`,`/app.js` asset routes are
    // gone: a single `.fallback(serve_spa)` now serves the SPA entry document for client routes
    // and the hashed Vite assets by path. Explicit `/api/*` and `/ws` routes still match first, so
    // the fallback never shadows them (R12.3).
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/state", get(get_state))
        .route("/api/bootstrap", get(get_bootstrap))
        .route("/api/effects", get(get_effects))
        .route("/api/power", post(post_power))
        .route("/api/system/action", post(post_system_action))
        .route("/api/color", post(post_color))
        .route("/api/brightness", post(post_brightness))
        .route("/api/effect", post(post_effect))
        .route(
            "/api/settings/identity",
            get(get_identity).post(post_identity),
        )
        .route("/api/settings/homekit", get(get_homekit).post(post_homekit))
        .route(
            "/api/settings/homekit/pairing-code",
            post(post_homekit_pairing_code),
        )
        .route("/api/settings/wifi", get(get_wifi).post(post_wifi))
        .route("/api/settings/wifi/scan", get(get_wifi_scan))
        .route("/api/settings/mqtt", get(get_mqtt).post(post_mqtt))
        .route(
            "/api/settings/ssh-keys",
            get(get_ssh_keys).post(post_ssh_keys),
        )
        .route(
            "/api/settings/ssh-keys/validate",
            post(post_ssh_keys_validate),
        )
        .route("/ws", get(crate::ws::ws_handler))
        .fallback(crate::web::serve_spa)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_expected_lan_host() {
        let cfg = SecurityConfig {
            extra_hosts: vec![],
        };
        assert!(cfg.host_ok(Some("192.168.1.230")));
        assert!(cfg.host_ok(Some("192.168.1.230:80")));
    }

    #[test]
    fn rejects_foreign_host_and_missing() {
        let cfg = SecurityConfig {
            extra_hosts: vec![],
        };
        assert!(!cfg.host_ok(Some("evil.example.com")));
        assert!(!cfg.host_ok(None));
    }

    #[test]
    fn origin_ok_only_for_expected_or_absent_nonbrowser() {
        let cfg = SecurityConfig {
            extra_hosts: vec![],
        };
        assert!(cfg.origin_ok(Some("http://192.168.1.230")));
        assert!(cfg.origin_ok(None));
        assert!(!cfg.origin_ok(Some("http://evil.example.com")));
    }

    #[test]
    fn accepts_any_lan_host_and_rejects_public() {
        let cfg = SecurityConfig {
            extra_hosts: vec![],
        };
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

    #[test]
    fn parses_known_system_actions() {
        assert_eq!(
            SystemAction::parse("restart_service"),
            Some(SystemAction::RestartService)
        );
        assert_eq!(SystemAction::parse("reboot"), Some(SystemAction::Reboot));
        assert_eq!(
            SystemAction::parse("poweroff"),
            Some(SystemAction::Poweroff)
        );
        assert_eq!(SystemAction::parse("lights_out"), None);
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
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let base =
            std::env::temp_dir().join(format!("mlp-api-test-{}-{unique}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        router(AppState {
            engine: handle,
            security: SecurityConfig {
                extra_hosts: vec![],
            },
            shutdown: sd_rx,
            backend: "mock",
            settings: crate::settings::SettingsStore::new(base.join("settings.json")),
            system_paths: crate::settings::SystemPaths {
                settings: base.join("settings.json"),
                homekit_storage: base.join("homekit"),
                wpa_supplicant: base.join("wpa_supplicant.conf"),
                authorized_keys: base.join("authorized_keys"),
            },
            homekit_status: HomeKitStatus::new(),
        })
    }

    #[tokio::test]
    async fn healthz_ok() {
        let app = test_app().await;
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .header("host", "testhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn effects_lists_registry() {
        let app = test_app().await;
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/effects")
                    .header("host", "testhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_effect_is_400() {
        let app = test_app().await;
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/effect")
                    .header("host", "testhost")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"bogus"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn foreign_host_is_403() {
        let app = test_app().await;
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/state")
                    .header("host", "evil.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn identity_settings_roundtrip() {
        let app = test_app().await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/settings/identity")
                    .header("host", "testhost")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"device_name":"Desk","room":"Office","light_label":"Mood"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/settings/identity")
                    .header("host", "testhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn settings_section_routes_serve_the_app() {
        let app = test_app().await;
        for route in ["/homekit", "/settings/homekit"] {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(route)
                        .header("host", "testhost")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "{route}");
        }
    }

    #[tokio::test]
    async fn mqtt_settings_hide_password() {
        let app = test_app().await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/settings/mqtt")
                    .header("host", "testhost")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"enabled":true,"broker_url":"mqtt://broker.local:1883","client_id":"moodlightpi-test","username":"user","password":"secret","clear_password":false,"subscribe_topic":"moodlightpi/set","publish_topic":"moodlightpi/state","availability_topic":"moodlightpi/availability","discovery_enabled":true,"discovery_prefix":"homeassistant"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/settings/mqtt")
                    .header("host", "testhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn invalid_ssh_key_is_400() {
        let app = test_app().await;
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/settings/ssh-keys/validate")
                    .header("host", "testhost")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"keys":"not-a-key nope"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
