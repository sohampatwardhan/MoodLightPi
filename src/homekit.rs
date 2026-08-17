use crate::color::Rgb;
use crate::engine::{Command, EngineHandle, Source};
use crate::settings::{generate_pairing_code, HomeKitSettings, SettingsStore};
use hap::accessory::{lightbulb::LightbulbAccessory, AccessoryCategory, AccessoryInformation};
use hap::characteristic::{AsyncCharacteristicCallbacks, HapCharacteristic};
use hap::futures::FutureExt;
use hap::server::{IpServer, Server};
use hap::storage::{FileStorage, Storage};
use hap::{Config, Pin};
use serde::Serialize;
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

const HAP_PORT: u16 = 32000;
const RESTART_DELAY: Duration = Duration::from_secs(5);
const MIN_MIRED: i32 = 140;
const NEUTRAL_MIRED: i32 = 250;
const MAX_MIRED: i32 = 500;

#[derive(Clone, Copy, Debug)]
struct HomeKitColorState {
    hue: f32,
    saturation: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct HomeKitRuntimeSnapshot {
    pub status: String,
    pub paired: bool,
    pub ready_to_pair: bool,
    pub port: Option<u16>,
}

impl Default for HomeKitRuntimeSnapshot {
    fn default() -> Self {
        Self {
            status: "Disabled".into(),
            paired: false,
            ready_to_pair: false,
            port: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct HomeKitStatus {
    inner: Arc<RwLock<HomeKitRuntimeSnapshot>>,
}

impl HomeKitStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> HomeKitRuntimeSnapshot {
        self.inner
            .read()
            .expect("homekit status lock poisoned")
            .clone()
    }

    fn set(&self, status: impl Into<String>, paired: bool, ready_to_pair: bool, port: Option<u16>) {
        *self.inner.write().expect("homekit status lock poisoned") = HomeKitRuntimeSnapshot {
            status: status.into(),
            paired,
            ready_to_pair,
            port,
        };
    }
}

pub async fn run(
    engine: EngineHandle,
    settings: SettingsStore,
    storage_path: PathBuf,
    status: HomeKitStatus,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    status.set("Disabled", false, false, None);

    loop {
        if *shutdown.borrow() {
            return;
        }

        let homekit = settings.load().homekit;
        if homekit.enabled {
            let result = run_enabled(
                engine.clone(),
                settings.clone(),
                storage_path.clone(),
                status.clone(),
                homekit,
                shutdown.clone(),
            )
            .await;

            if *shutdown.borrow() {
                return;
            }

            match result {
                Ok(()) => {
                    tracing::warn!("homekit accessory server stopped unexpectedly; restarting");
                    status.set("Restarting", false, false, Some(HAP_PORT));
                }
                Err(err) => {
                    tracing::error!("homekit accessory server stopped: {err:?}");
                    status.set(format!("Restarting after error: {err}"), false, false, None);
                }
            }

            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return;
                    }
                }
                _ = tokio::time::sleep(RESTART_DELAY) => {}
            }
            continue;
        }

        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(3)) => {}
        }
    }
}

async fn run_enabled(
    engine: EngineHandle,
    settings: SettingsStore,
    storage_path: PathBuf,
    status: HomeKitStatus,
    mut homekit: HomeKitSettings,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    status.set("Starting", false, false, Some(HAP_PORT));

    let pairing_code = ensure_pairing_code(&settings, &mut homekit)?;
    let pin = pin_from_pairing_code(&pairing_code)?;
    let mut storage = FileStorage::new(&storage_path).await?;

    let pairings = storage.count_pairings().await.unwrap_or(0);
    let mut config = match storage.load_config().await {
        Ok(config) => config,
        Err(_) => Config {
            pin: pin.clone(),
            name: homekit.accessory_name.clone(),
            category: AccessoryCategory::Lightbulb,
            port: HAP_PORT,
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            ..Default::default()
        },
    };
    config.pin = pin;
    config.name = homekit.accessory_name.clone();
    config.category = AccessoryCategory::Lightbulb;
    config.protocol_version = "1.1".into();
    config.configuration_number = config.configuration_number.saturating_add(1);
    config.port = HAP_PORT;
    config.host = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    storage.save_config(&config).await?;

    let device_id = config.device_id.to_hex_string();
    let mut accessory = LightbulbAccessory::new(
        1,
        AccessoryInformation {
            name: homekit.accessory_name.clone(),
            manufacturer: "MoodLightPi".into(),
            model: "Pi Zero W Mood Light".into(),
            serial_number: device_id,
            firmware_revision: Some(env!("CARGO_PKG_VERSION").into()),
            ..Default::default()
        },
    )?;
    let (hue, saturation, _) = rgb_to_hsv(engine.current().rgb);
    let color_state = Arc::new(Mutex::new(HomeKitColorState { hue, saturation }));
    let color_temperature_state = Arc::new(Mutex::new(NEUTRAL_MIRED));
    configure_homekit_light_accessory(&mut accessory, &engine, color_temperature_state.clone())
        .await?;
    wire_lightbulb_callbacks(&mut accessory, engine, color_state, color_temperature_state);

    let server = IpServer::new(config, storage).await?;
    server.add_accessory(accessory).await?;
    let storage_pointer = server.storage_pointer();

    update_status_from_pairings(&status, pairings, Some(HAP_PORT));
    let monitor_status = status.clone();
    let mut monitor_shutdown = shutdown.clone();
    let monitor = tokio::spawn(async move {
        loop {
            let count = storage_pointer
                .lock()
                .await
                .count_pairings()
                .await
                .unwrap_or(0);
            update_status_from_pairings(&monitor_status, count, Some(HAP_PORT));
            tokio::select! {
                changed = monitor_shutdown.changed() => {
                    if changed.is_err() || *monitor_shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            }
        }
    });

    tracing::info!("homekit light accessory listening on tcp/{HAP_PORT}");
    let handle = server.run_handle();
    tokio::select! {
        result = handle => {
            monitor.abort();
            result?;
        }
        changed = shutdown.changed() => {
            if changed.is_err() || *shutdown.borrow() {
                monitor.abort();
            }
        }
    }
    Ok(())
}

async fn configure_homekit_light_accessory(
    accessory: &mut LightbulbAccessory,
    engine: &EngineHandle,
    color_temperature_state: Arc<Mutex<i32>>,
) -> anyhow::Result<()> {
    accessory
        .lightbulb
        .characteristic_value_active_transition_count = None;
    accessory.lightbulb.characteristic_value_transition_control = None;
    accessory.lightbulb.name = None;
    accessory
        .lightbulb
        .supported_characteristic_value_transition_configuration = None;

    let state = engine.current();
    let (hue, saturation, _) = rgb_to_hsv(state.rgb);
    accessory
        .lightbulb
        .power_state
        .set_value(json!(state.power))
        .await?;
    if let Some(brightness) = &mut accessory.lightbulb.brightness {
        brightness
            .set_value(json!(raw_brightness_to_percent(state.brightness)))
            .await?;
    }
    if let Some(hue_characteristic) = &mut accessory.lightbulb.hue {
        hue_characteristic.set_value(json!(hue)).await?;
    }
    if let Some(color_temperature) = &mut accessory.lightbulb.color_temperature {
        let color_temperature_value = *color_temperature_state
            .lock()
            .expect("homekit color temperature lock poisoned");
        color_temperature
            .set_value(json!(color_temperature_value))
            .await?;
    }
    if let Some(saturation_characteristic) = &mut accessory.lightbulb.saturation {
        saturation_characteristic
            .set_value(json!(saturation))
            .await?;
    }

    Ok(())
}

fn wire_lightbulb_callbacks(
    accessory: &mut LightbulbAccessory,
    engine: EngineHandle,
    color_state: Arc<Mutex<HomeKitColorState>>,
    color_temperature_state: Arc<Mutex<i32>>,
) {
    let read_engine = engine.clone();
    accessory.lightbulb.power_state.on_read_async(Some(move || {
        let engine = read_engine.clone();
        async move { Ok(Some(engine.current().power)) }.boxed()
    }));

    let write_engine = engine.clone();
    accessory
        .lightbulb
        .power_state
        .on_update_async(Some(move |_current, new| {
            let engine = write_engine.clone();
            async move {
                let _ = engine
                    .commands
                    .try_send((Command::SetPower(new), Source::HomeKit));
                Ok(())
            }
            .boxed()
        }));

    if let Some(brightness) = &mut accessory.lightbulb.brightness {
        let read_engine = engine.clone();
        brightness.on_read_async(Some(move || {
            let engine = read_engine.clone();
            async move { Ok(Some(raw_brightness_to_percent(engine.current().brightness))) }.boxed()
        }));

        let write_engine = engine.clone();
        brightness.on_update_async(Some(move |_current, new| {
            let engine = write_engine.clone();
            async move {
                let _ = engine.commands.try_send((
                    Command::SetBrightness(percent_to_raw_brightness(new)),
                    Source::HomeKit,
                ));
                Ok(())
            }
            .boxed()
        }));
    }

    if let Some(hue) = &mut accessory.lightbulb.hue {
        let read_color = color_state.clone();
        hue.on_read_async(Some(move || {
            let hue = read_color.lock().expect("homekit color lock poisoned").hue;
            async move { Ok(Some(hue)) }.boxed()
        }));

        let write_engine = engine.clone();
        let write_color = color_state.clone();
        hue.on_update_async(Some(move |_current, new| {
            let engine = write_engine.clone();
            let color = write_color.clone();
            async move {
                let rgb = {
                    let mut color = color.lock().expect("homekit color lock poisoned");
                    color.hue = new;
                    hsv_to_rgb(color.hue, color.saturation)
                };
                let _ = engine
                    .commands
                    .try_send((Command::SetColor(rgb), Source::HomeKit));
                Ok(())
            }
            .boxed()
        }));
    }

    if let Some(saturation) = &mut accessory.lightbulb.saturation {
        let read_color = color_state.clone();
        saturation.on_read_async(Some(move || {
            let saturation = read_color
                .lock()
                .expect("homekit color lock poisoned")
                .saturation;
            async move { Ok(Some(saturation)) }.boxed()
        }));

        let write_engine = engine.clone();
        let write_color = color_state.clone();
        saturation.on_update_async(Some(move |_current, new| {
            let engine = write_engine.clone();
            let color = write_color.clone();
            async move {
                let rgb = {
                    let mut color = color.lock().expect("homekit color lock poisoned");
                    color.saturation = new;
                    hsv_to_rgb(color.hue, color.saturation)
                };
                let _ = engine
                    .commands
                    .try_send((Command::SetColor(rgb), Source::HomeKit));
                Ok(())
            }
            .boxed()
        }));
    }

    if let Some(color_temperature) = &mut accessory.lightbulb.color_temperature {
        let read_color_temperature = color_temperature_state.clone();
        color_temperature.on_read_async(Some(move || {
            let color_temperature = *read_color_temperature
                .lock()
                .expect("homekit color temperature lock poisoned");
            async move { Ok(Some(color_temperature)) }.boxed()
        }));

        let write_engine = engine.clone();
        let write_color = color_state;
        let write_color_temperature = color_temperature_state;
        color_temperature.on_update_async(Some(move |_current, new: i32| {
            let engine = write_engine.clone();
            let color = write_color.clone();
            let color_temperature = write_color_temperature.clone();
            async move {
                let mired = new.clamp(MIN_MIRED, MAX_MIRED);
                *color_temperature
                    .lock()
                    .expect("homekit color temperature lock poisoned") = mired;
                let rgb = mired_to_rgb(mired);
                let (hue, saturation, _) = rgb_to_hsv(rgb);
                *color.lock().expect("homekit color lock poisoned") =
                    HomeKitColorState { hue, saturation };
                let _ = engine
                    .commands
                    .try_send((Command::SetColor(rgb), Source::HomeKit));
                Ok(())
            }
            .boxed()
        }));
    }
}

fn update_status_from_pairings(status: &HomeKitStatus, pairings: usize, port: Option<u16>) {
    if pairings > 0 {
        status.set("Paired", true, false, port);
    } else {
        status.set("Ready to pair", false, true, port);
    }
}

fn ensure_pairing_code(
    settings: &SettingsStore,
    homekit: &mut HomeKitSettings,
) -> anyhow::Result<String> {
    if let Some(code) = homekit
        .pairing_code
        .as_deref()
        .filter(|code| pin_from_pairing_code(code).is_ok())
    {
        return Ok(code.to_string());
    }

    for _ in 0..32 {
        let code = generate_pairing_code()?;
        if pin_from_pairing_code(&code).is_ok() {
            homekit.pairing_code = Some(code.clone());
            settings.update(|settings| {
                settings.homekit.pairing_code = Some(code.clone());
            })?;
            return Ok(code);
        }
    }
    Err(anyhow::anyhow!("could not generate a HomeKit pairing code"))
}

fn pin_from_pairing_code(code: &str) -> anyhow::Result<Pin> {
    let digits: Vec<u8> = code
        .bytes()
        .filter(|byte| byte.is_ascii_digit())
        .map(|byte| byte - b'0')
        .collect();
    let digits: [u8; 8] = digits
        .try_into()
        .map_err(|_| anyhow::anyhow!("HomeKit pairing code must contain 8 digits"))?;
    Ok(Pin::new(digits)?)
}

fn raw_brightness_to_percent(raw: u8) -> i32 {
    ((raw as u16 * 100 + 127) / 255) as i32
}

fn percent_to_raw_brightness(percent: i32) -> u8 {
    ((percent.clamp(0, 100) as u16 * 255 + 50) / 100) as u8
}

fn rgb_to_hsv(rgb: Rgb) -> (f32, f32, f32) {
    let r = rgb.r as f32 / 255.0;
    let g = rgb.g as f32 / 255.0;
    let b = rgb.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max * 100.0 };
    (hue, saturation, max * 100.0)
}

fn hsv_to_rgb(hue: f32, saturation: f32) -> Rgb {
    let h = hue.rem_euclid(360.0);
    let s = (saturation.clamp(0.0, 100.0)) / 100.0;
    let c = s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = 1.0 - c;
    let (rp, gp, bp) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Rgb {
        r: ((rp + m) * 255.0).round() as u8,
        g: ((gp + m) * 255.0).round() as u8,
        b: ((bp + m) * 255.0).round() as u8,
    }
}

fn mired_to_rgb(mired: i32) -> Rgb {
    let mired = mired.clamp(MIN_MIRED, MAX_MIRED);
    if mired <= NEUTRAL_MIRED {
        let ratio = (NEUTRAL_MIRED - mired) as f32 / (NEUTRAL_MIRED - MIN_MIRED) as f32;
        Rgb {
            r: channel_between(255.0, 178.0, ratio),
            g: channel_between(255.0, 222.0, ratio),
            b: 255,
        }
    } else {
        let ratio = (mired - NEUTRAL_MIRED) as f32 / (MAX_MIRED - NEUTRAL_MIRED) as f32;
        Rgb {
            r: 255,
            g: channel_between(255.0, 137.0, ratio),
            b: channel_between(255.0, 14.0, ratio),
        }
    }
}

fn channel_between(from: f32, to: f32, ratio: f32) -> u8 {
    clamp_color_channel(from + (to - from) * ratio)
}

fn clamp_color_channel(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::MockDisplay;
    use crate::engine::Engine;
    use crate::state::State;
    use hap::service::HapService;
    use serde_json::Value;

    #[test]
    fn parses_pairing_pin() {
        assert!(pin_from_pairing_code("078-46-454").is_ok());
        assert!(pin_from_pairing_code("111-11-111").is_err());
        assert!(pin_from_pairing_code("1234").is_err());
    }

    #[test]
    fn maps_brightness_between_homekit_and_engine() {
        assert_eq!(raw_brightness_to_percent(0), 0);
        assert_eq!(raw_brightness_to_percent(255), 100);
        assert_eq!(percent_to_raw_brightness(0), 0);
        assert_eq!(percent_to_raw_brightness(100), 255);
    }

    #[test]
    fn maps_hsv_to_rgb_primary_colors() {
        assert_eq!(hsv_to_rgb(0.0, 100.0), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(hsv_to_rgb(120.0, 100.0), Rgb { r: 0, g: 255, b: 0 });
        assert_eq!(hsv_to_rgb(240.0, 100.0), Rgb { r: 0, g: 0, b: 255 });
    }

    #[tokio::test]
    async fn homekit_accessory_is_rgb_light_with_cct() {
        let (engine, _) = Engine::new(MockDisplay::new(), State::default());
        let mut accessory = LightbulbAccessory::new(
            1,
            AccessoryInformation {
                name: "Mood Light".into(),
                manufacturer: "MoodLightPi".into(),
                model: "Pi Zero W Mood Light".into(),
                serial_number: "04:6b:a6:a1:1f:10".into(),
                firmware_revision: Some(env!("CARGO_PKG_VERSION").into()),
                ..Default::default()
            },
        )
        .unwrap();

        configure_homekit_light_accessory(&mut accessory, &engine, Arc::new(Mutex::new(250)))
            .await
            .unwrap();

        let light_characteristics = accessory
            .lightbulb
            .get_characteristics()
            .into_iter()
            .map(|characteristic| characteristic.get_type())
            .collect::<Vec<_>>();

        assert_eq!(
            light_characteristics,
            vec![
                hap::HapType::PowerState,
                hap::HapType::Brightness,
                hap::HapType::ColorTemperature,
                hap::HapType::Hue,
                hap::HapType::Saturation,
            ]
        );

        let serialized = serde_json::to_value(&accessory).unwrap();
        let mut iids = serialized["services"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|service| service["characteristics"].as_array().unwrap())
            .map(|characteristic| characteristic["iid"].as_u64().unwrap())
            .collect::<Vec<_>>();
        iids.sort_unstable();
        iids.dedup();
        let characteristic_count = serialized["services"]
            .as_array()
            .unwrap()
            .iter()
            .map(|service| service["characteristics"].as_array().unwrap().len())
            .sum::<usize>();
        assert_eq!(iids.len(), characteristic_count);

        let light_service = serialized["services"]
            .as_array()
            .unwrap()
            .iter()
            .find(|service| service["type"] == Value::String("43".into()))
            .unwrap();
        let serialized_types = light_service["characteristics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|characteristic| characteristic["type"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();

        assert_eq!(serialized_types, vec!["25", "8", "CE", "13", "2F"]);
    }

    #[test]
    fn maps_homekit_cct_to_rgb() {
        assert_eq!(
            mired_to_rgb(250),
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
        assert_eq!(
            mired_to_rgb(500),
            Rgb {
                r: 255,
                g: 137,
                b: 14
            }
        );
        assert_eq!(
            mired_to_rgb(140),
            Rgb {
                r: 178,
                g: 222,
                b: 255
            }
        );
    }
}
