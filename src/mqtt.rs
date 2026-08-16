use crate::color::Rgb;
use crate::effects::is_valid_effect;
use crate::engine::{Command, EngineHandle, Source};
use crate::settings::{MqttSettings, SettingsStore};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::watch;

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

    let (client, mut eventloop) = AsyncClient::new(options, 10);
    if !settings.subscribe_topic.trim().is_empty() {
        client
            .subscribe(settings.subscribe_topic.trim(), QoS::AtMostOnce)
            .await?;
    }
    let initial_snapshot = { engine.snapshots.borrow().clone() };
    publish_state(&client, &settings, initial_snapshot).await;

    let mut snapshots = engine.snapshots.clone();
    let mut settings_tick = tokio::time::interval(SETTINGS_POLL);
    settings_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
            changed = snapshots.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
                let snapshot = { snapshots.borrow().clone() };
                publish_state(&client, &settings, snapshot).await;
            }
            _ = settings_tick.tick() => {
                let current = settings_store.load().mqtt;
                if current != settings {
                    return Err(anyhow::anyhow!("MQTT settings changed"));
                }
            }
            event = eventloop.poll() => {
                match event? {
                    Event::Incoming(Incoming::Publish(message)) => {
                        for command in commands_from_payload(&message.payload)? {
                            engine.commands.send((command, Source::Mqtt)).await?;
                        }
                    }
                    _ => {}
                }
            }
        }
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
    let payload = json!({
        "seq": snapshot.seq,
        "source": format!("{:?}", snapshot.source).to_lowercase(),
        "power": state.power,
        "mode": state.mode,
        "effect": state.effect_name,
        "speed": state.speed,
        "brightness": state.brightness,
        "brightness_percent": brightness_to_percent(state.brightness),
        "rgb": state.rgb,
        "color": format!("#{:02x}{:02x}{:02x}", state.rgb.r, state.rgb.g, state.rgb.b),
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
    if let Some(power) = value.get("power").and_then(Value::as_bool) {
        commands.push(Command::SetPower(power));
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
    if let Some(raw) = value.get("color").and_then(Value::as_str) {
        return Ok(Some(parse_hex_color(raw)?));
    }
    if let Some(rgb) = value.get("rgb") {
        let channel = |name: &str| {
            rgb.get(name)
                .and_then(Value::as_u64)
                .and_then(|v| u8::try_from(v).ok())
                .ok_or_else(|| anyhow::anyhow!("rgb.{name} must be 0..255"))
        };
        return Ok(Some(Rgb {
            r: channel("r")?,
            g: channel("g")?,
            b: channel("b")?,
        }));
    }
    Ok(None)
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
