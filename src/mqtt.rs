use crate::color::Rgb;
use crate::effects::{effect_names, is_valid_effect};
use crate::engine::{Command, EngineHandle, Source};
use crate::settings::{IdentitySettings, MqttSettings, SettingsStore};
use rumqttc::{AsyncClient, Event, Incoming, LastWill, MqttOptions, QoS};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::watch;

/// Payload published to the availability topic when the device is reachable / gone.
const AVAILABILITY_ONLINE: &str = "online";
const AVAILABILITY_OFFLINE: &str = "offline";

const SETTINGS_POLL: Duration = Duration::from_secs(5);
const RETRY_DELAY: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokerEndpoint {
    host: String,
    port: u16,
}

pub async fn run(
    engine: EngineHandle,
    settings_store: SettingsStore,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            return;
        }

        let settings = settings_store.load().mqtt;
        if !settings.enabled {
            wait_for_settings_or_shutdown(&mut shutdown).await;
            continue;
        }

        match run_connection(&engine, &settings_store, settings, &mut shutdown).await {
            Ok(()) => return,
            Err(err) => {
                tracing::warn!("MQTT connection stopped: {err}");
                wait_retry_or_shutdown(&mut shutdown).await;
            }
        }
    }
}

async fn run_connection(
    engine: &EngineHandle,
    settings_store: &SettingsStore,
    settings: MqttSettings,
    shutdown: &mut watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let endpoint = parse_broker_url(&settings.broker_url)?;
    let mut options = MqttOptions::new(
        settings.client_id.trim(),
        endpoint.host.as_str(),
        endpoint.port,
    );
    options.set_keep_alive(Duration::from_secs(30));
    if !settings.username.trim().is_empty() {
        options.set_credentials(
            settings.username.trim(),
            settings.password.as_deref().unwrap_or_default(),
        );
    }
    // Register the availability Last-Will BEFORE connecting so the broker publishes `offline`
    // (retained) if this client drops unexpectedly — Home Assistant then marks the light
    // unavailable without any action from us (R17.1).
    if let Some(will) = availability_will(&settings) {
        options.set_last_will(will);
    }

    let (client, mut eventloop) = AsyncClient::new(options, 10);
    if !settings.subscribe_topic.trim().is_empty() {
        client
            .subscribe(settings.subscribe_topic.trim(), QoS::AtMostOnce)
            .await?;
    }
    let initial_snapshot = { engine.snapshots.borrow().clone() };
    publish_state(&client, &settings, initial_snapshot).await;

    // Announce availability (`online`, retained) and, when enabled, publish the retained Home
    // Assistant discovery config so the light auto-registers (R17.2, R17.3, R18.1).
    let identity = settings_store.load().identity;
    let effects = effect_names();
    publish_availability(&client, &settings, true).await;
    if settings.discovery_enabled {
        publish_discovery(&client, &settings, &identity, &effects).await;
    }

    let mut snapshots = engine.snapshots.clone();
    let mut settings_tick = tokio::time::interval(SETTINGS_POLL);
    settings_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Break out with an outcome instead of returning directly, so the cleanup below always runs.
    let outcome: anyhow::Result<()> = loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break Ok(());
                }
            }
            changed = snapshots.changed() => {
                if changed.is_err() {
                    break Ok(());
                }
                let snapshot = { snapshots.borrow().clone() };
                publish_state(&client, &settings, snapshot).await;
            }
            _ = settings_tick.tick() => {
                let current = settings_store.load().mqtt;
                if current != settings {
                    break Err(anyhow::anyhow!("MQTT settings changed"));
                }
            }
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Incoming::Publish(message))) => {
                        match commands_from_payload(&message.payload) {
                            Ok(commands) => {
                                for command in commands {
                                    // A send failure means the engine is shutting down; the
                                    // shutdown/snapshots arms will end the loop shortly.
                                    let _ = engine.commands.send((command, Source::Mqtt)).await;
                                }
                            }
                            Err(err) => tracing::warn!("ignoring invalid MQTT command payload: {err}"),
                        }
                    }
                    Ok(_) => {}
                    Err(err) => break Err(err.into()),
                }
            }
        }
    };

    // Graceful cleanup while the connection is usually still alive. Always announce `offline`;
    // additionally clear the retained discovery config when MQTT or discovery has just been
    // turned off, so Home Assistant does not keep a stale entity (R17.4, R18.4). A normal restart
    // or shutdown keeps the retained discovery config in place. Errors are best-effort — on an
    // unexpected disconnect the broker's Last-Will already published `offline`.
    let current = settings_store.load().mqtt;
    publish_availability(&client, &settings, false).await;
    if settings.discovery_enabled && (!current.enabled || !current.discovery_enabled) {
        clear_discovery(&client, &settings).await;
    }
    let _ = client.disconnect().await;
    outcome
}

/// The availability Last-Will: publishes `offline` (retained) to the availability topic when the
/// client disconnects unexpectedly. `None` when MQTT is disabled or no availability topic is set.
fn availability_will(settings: &MqttSettings) -> Option<LastWill> {
    let topic = settings.availability_topic.trim();
    if !settings.enabled || topic.is_empty() {
        return None;
    }
    Some(LastWill::new(topic, AVAILABILITY_OFFLINE, QoS::AtLeastOnce, true))
}

/// Publish the current availability (`online`/`offline`) to the availability topic, retained so a
/// subscriber (Home Assistant) sees the correct status immediately on connect (R17.3).
async fn publish_availability(client: &AsyncClient, settings: &MqttSettings, online: bool) {
    let topic = settings.availability_topic.trim();
    if topic.is_empty() {
        return;
    }
    let payload = if online { AVAILABILITY_ONLINE } else { AVAILABILITY_OFFLINE };
    if let Err(err) = client.publish(topic, QoS::AtLeastOnce, true, payload).await {
        tracing::warn!("failed to publish MQTT availability: {err}");
    }
}

/// Home Assistant discovery config topic: `<prefix>/light/<object_id>/config`.
fn discovery_topic(settings: &MqttSettings) -> String {
    format!(
        "{}/light/{}/config",
        settings.discovery_prefix.trim().trim_end_matches('/'),
        discovery_object_id(settings)
    )
}

/// A stable, sanitized object/unique id derived from the client id (`[a-z0-9_-]`).
fn discovery_object_id(settings: &MqttSettings) -> String {
    let id: String = settings
        .client_id
        .trim()
        .chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if id.is_empty() {
        "moodlightpi".to_string()
    } else {
        id
    }
}

/// Build the Home Assistant JSON-schema light discovery payload (R18.2, R18.3).
fn build_discovery_config(
    settings: &MqttSettings,
    identity: &IdentitySettings,
    effects: &[&str],
) -> Value {
    let object_id = discovery_object_id(settings);
    let name = if identity.light_label.trim().is_empty() {
        "Mood Light".to_string()
    } else {
        identity.light_label.trim().to_string()
    };
    let device_name = if identity.device_name.trim().is_empty() {
        "MoodLightPi".to_string()
    } else {
        identity.device_name.trim().to_string()
    };
    json!({
        "schema": "json",
        "name": name,
        "unique_id": object_id,
        "command_topic": settings.subscribe_topic.trim(),
        "state_topic": settings.publish_topic.trim(),
        "availability_topic": settings.availability_topic.trim(),
        "payload_available": AVAILABILITY_ONLINE,
        "payload_not_available": AVAILABILITY_OFFLINE,
        "brightness": true,
        "brightness_scale": 255,
        "color_mode": true,
        "supported_color_modes": ["rgb"],
        "effect": true,
        "effect_list": effects,
        "device": {
            "identifiers": [object_id],
            "name": device_name,
            "manufacturer": "MoodLightPi",
            "model": "WS281x LED pHAT",
            "sw_version": env!("CARGO_PKG_VERSION"),
        },
        "origin": { "name": "moodlightpi" },
    })
}

/// Publish the retained discovery config so Home Assistant creates the light entity (R18.1).
async fn publish_discovery(
    client: &AsyncClient,
    settings: &MqttSettings,
    identity: &IdentitySettings,
    effects: &[&str],
) {
    if settings.publish_topic.trim().is_empty() || settings.subscribe_topic.trim().is_empty() {
        // HA needs both a state and command topic; without them the entity would be useless.
        return;
    }
    let config = build_discovery_config(settings, identity, effects);
    if let Err(err) = client
        .publish(discovery_topic(settings), QoS::AtLeastOnce, true, config.to_string())
        .await
    {
        tracing::warn!("failed to publish MQTT discovery config: {err}");
    }
}

/// Remove a previously published discovery config by publishing an empty retained payload to its
/// topic — Home Assistant treats an empty retained config as "delete this entity" (R18.4).
async fn clear_discovery(client: &AsyncClient, settings: &MqttSettings) {
    if let Err(err) = client
        .publish(discovery_topic(settings), QoS::AtLeastOnce, true, Vec::new())
        .await
    {
        tracing::warn!("failed to clear MQTT discovery config: {err}");
    }
}

async fn publish_state(
    client: &AsyncClient,
    settings: &MqttSettings,
    snapshot: crate::engine::StateSnapshot,
) {
    let topic = settings.publish_topic.trim();
    if topic.is_empty() {
        return;
    }
    let state = snapshot.state;
    // Home Assistant JSON-schema light fields (`state`, `color` object, `color_mode`) alongside
    // the device-native fields. Per AUDIT-1 the legacy hex-string `color` is dropped; `color` is
    // now the `{r,g,b}` object HA expects, and `rgb` remains the native object for other consumers.
    let payload = json!({
        "seq": snapshot.seq,
        "source": format!("{:?}", snapshot.source).to_lowercase(),
        "state": if state.power { "ON" } else { "OFF" },
        "power": state.power,
        "mode": state.mode,
        "effect": state.effect_name,
        "speed": state.speed,
        "brightness": state.brightness,
        "brightness_percent": brightness_to_percent(state.brightness),
        "color_mode": "rgb",
        "color": { "r": state.rgb.r, "g": state.rgb.g, "b": state.rgb.b },
        "rgb": state.rgb,
    });
    if let Err(err) = client
        .publish(topic, QoS::AtLeastOnce, false, payload.to_string())
        .await
    {
        tracing::warn!("failed to publish MQTT state: {err}");
    }
}

async fn wait_for_settings_or_shutdown(shutdown: &mut watch::Receiver<bool>) {
    tokio::select! {
        _ = tokio::time::sleep(SETTINGS_POLL) => {}
        _ = shutdown.changed() => {}
    }
}

async fn wait_retry_or_shutdown(shutdown: &mut watch::Receiver<bool>) {
    tokio::select! {
        _ = tokio::time::sleep(RETRY_DELAY) => {}
        _ = shutdown.changed() => {}
    }
}

fn parse_broker_url(value: &str) -> anyhow::Result<BrokerEndpoint> {
    let value = value.trim();
    if value.is_empty() {
        return Err(anyhow::anyhow!("MQTT broker URL is required"));
    }
    if value.starts_with("mqtts://") {
        return Err(anyhow::anyhow!(
            "mqtts:// brokers are not enabled yet; use mqtt:// with username/password"
        ));
    }
    let rest = value.strip_prefix("mqtt://").unwrap_or(value);
    if rest.contains("://") {
        return Err(anyhow::anyhow!("MQTT broker URL must use mqtt://"));
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit_once('@')
        .map(|(_, authority)| authority)
        .unwrap_or_else(|| rest.split(['/', '?', '#']).next().unwrap_or_default());
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, raw_port)) if !raw_port.is_empty() => {
            let port = raw_port.parse::<u16>()?;
            (host, port)
        }
        _ => (authority, 1883),
    };
    if host.trim().is_empty() {
        return Err(anyhow::anyhow!("MQTT broker host is required"));
    }
    Ok(BrokerEndpoint {
        host: host.trim().to_string(),
        port,
    })
}

fn commands_from_payload(payload: &[u8]) -> anyhow::Result<Vec<Command>> {
    let value: Value = serde_json::from_slice(payload)?;
    let mut commands = Vec::new();
    // Accept the device-native `power` bool and Home Assistant's `state` string ("ON"/"OFF").
    if let Some(power) = value.get("power").and_then(Value::as_bool) {
        commands.push(Command::SetPower(power));
    } else if let Some(state) = value.get("state").and_then(Value::as_str) {
        match state.trim().to_ascii_uppercase().as_str() {
            "ON" => commands.push(Command::SetPower(true)),
            "OFF" => commands.push(Command::SetPower(false)),
            other => return Err(anyhow::anyhow!("unknown state {other}")),
        }
    }
    if let Some(color) = parse_color(&value)? {
        commands.push(Command::SetColor(color));
    }
    if let Some(brightness) = value
        .get("brightness")
        .and_then(Value::as_u64)
        .and_then(|v| u8::try_from(v).ok())
    {
        commands.push(Command::SetBrightness(brightness));
    } else if let Some(percent) = value.get("brightness_percent").and_then(Value::as_u64) {
        commands.push(Command::SetBrightness(percent_to_brightness(percent)));
    }
    if let Some(effect) = value.get("effect").and_then(Value::as_str) {
        if !is_valid_effect(effect) {
            return Err(anyhow::anyhow!("unknown effect {effect}"));
        }
        let speed = value
            .get("speed")
            .and_then(Value::as_u64)
            .and_then(|v| u8::try_from(v).ok());
        commands.push(Command::SetEffect {
            name: effect.to_string(),
            speed,
        });
    }
    Ok(commands)
}

fn parse_color(value: &Value) -> anyhow::Result<Option<Rgb>> {
    // `color` may be a hex string (device-native) or an `{r,g,b}` object (Home Assistant JSON
    // schema); `rgb` is the device-native object form.
    if let Some(color) = value.get("color") {
        if let Some(raw) = color.as_str() {
            return Ok(Some(parse_hex_color(raw)?));
        }
        if color.is_object() {
            return Ok(Some(parse_rgb_object(color)?));
        }
    }
    if let Some(rgb) = value.get("rgb") {
        return Ok(Some(parse_rgb_object(rgb)?));
    }
    Ok(None)
}

/// Parse an `{r,g,b}` JSON object with each channel in 0..=255.
fn parse_rgb_object(value: &Value) -> anyhow::Result<Rgb> {
    let channel = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|v| u8::try_from(v).ok())
            .ok_or_else(|| anyhow::anyhow!("color.{name} must be 0..255"))
    };
    Ok(Rgb {
        r: channel("r")?,
        g: channel("g")?,
        b: channel("b")?,
    })
}

fn parse_hex_color(value: &str) -> anyhow::Result<Rgb> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow::anyhow!("color must be #rrggbb"));
    }
    Ok(Rgb {
        r: u8::from_str_radix(&hex[0..2], 16)?,
        g: u8::from_str_radix(&hex[2..4], 16)?,
        b: u8::from_str_radix(&hex[4..6], 16)?,
    })
}

fn brightness_to_percent(value: u8) -> u8 {
    ((value as u16 * 100 + 127) / 255) as u8
}

fn percent_to_brightness(value: u64) -> u8 {
    ((value.min(100) * 255 + 50) / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_broker_url_with_defaults() {
        assert_eq!(
            parse_broker_url("mqtt://broker.local").unwrap(),
            BrokerEndpoint {
                host: "broker.local".into(),
                port: 1883,
            }
        );
        assert_eq!(
            parse_broker_url("192.168.2.10:1884").unwrap(),
            BrokerEndpoint {
                host: "192.168.2.10".into(),
                port: 1884,
            }
        );
    }

    #[test]
    fn parses_mqtt_commands() {
        let commands = commands_from_payload(
            br##"{"power":true,"color":"#38bdf8","brightness_percent":50,"effect":"rainbow","speed":46}"##,
        )
        .unwrap();
        assert_eq!(commands.len(), 4);
    }

    #[test]
    fn rejects_unknown_effects() {
        assert!(commands_from_payload(br#"{"effect":"sparkleblast"}"#).is_err());
    }
}
