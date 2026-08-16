use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

pub const DEFAULT_SETTINGS_PATH: &str = "/var/lib/moodlightpi/settings.json";
pub const DEFAULT_HOMEKIT_STORAGE_PATH: &str = "/var/lib/moodlightpi/homekit";
pub const DEFAULT_WPA_SUPPLICANT_PATH: &str = "/etc/wpa_supplicant/wpa_supplicant.conf";
pub const DEFAULT_AUTHORIZED_KEYS_PATH: &str = "/root/.ssh/authorized_keys";

#[derive(Clone, Debug)]
pub struct SystemPaths {
    pub settings: PathBuf,
    pub homekit_storage: PathBuf,
    pub wpa_supplicant: PathBuf,
    pub authorized_keys: PathBuf,
}

impl SystemPaths {
    pub fn from_env() -> Self {
        Self {
            settings: std::env::var("MLP_SETTINGS_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_SETTINGS_PATH)),
            homekit_storage: std::env::var("MLP_HOMEKIT_STORAGE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_HOMEKIT_STORAGE_PATH)),
            wpa_supplicant: std::env::var("MLP_WPA_SUPPLICANT_CONF")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_WPA_SUPPLICANT_PATH)),
            authorized_keys: std::env::var("MLP_AUTHORIZED_KEYS")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_AUTHORIZED_KEYS_PATH)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySettings {
    pub device_name: String,
    pub room: String,
    pub light_label: String,
}

impl Default for IdentitySettings {
    fn default() -> Self {
        Self {
            device_name: "MoodLightPi".into(),
            room: "Living Room".into(),
            light_label: "Mood Light".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeKitSettings {
    pub accessory_name: String,
    pub enabled: bool,
    pub pairing_code: Option<String>,
}

impl Default for HomeKitSettings {
    fn default() -> Self {
        Self {
            accessory_name: "Mood Light".into(),
            enabled: false,
            pairing_code: None,
        }
    }
}

/// Availability topic the device publishes `online`/`offline` to (Home Assistant LWT).
fn default_availability_topic() -> String {
    "moodlightpi/availability".into()
}
/// Home Assistant MQTT discovery is on by default so the light auto-registers in HA.
fn default_discovery_enabled() -> bool {
    true
}
/// Home Assistant's default MQTT discovery topic prefix.
fn default_discovery_prefix() -> String {
    "homeassistant".into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttSettings {
    pub enabled: bool,
    pub broker_url: String,
    pub client_id: String,
    pub username: String,
    pub password: Option<String>,
    pub subscribe_topic: String,
    pub publish_topic: String,
    /// Topic the device publishes its Home Assistant availability (`online`/`offline`) to.
    /// `#[serde(default = ...)]` keeps pre-existing `settings.json` (written before these
    /// fields existed) loadable, filling the HA default rather than an empty string.
    #[serde(default = "default_availability_topic")]
    pub availability_topic: String,
    /// Whether to publish a Home Assistant MQTT discovery config so HA auto-creates the entity.
    #[serde(default = "default_discovery_enabled")]
    pub discovery_enabled: bool,
    /// Home Assistant discovery topic prefix (`<prefix>/light/<object_id>/config`).
    #[serde(default = "default_discovery_prefix")]
    pub discovery_prefix: String,
}

impl Default for MqttSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            broker_url: "mqtt://localhost:1883".into(),
            client_id: "moodlightpi".into(),
            username: String::new(),
            password: None,
            subscribe_topic: "moodlightpi/set".into(),
            publish_topic: "moodlightpi/state".into(),
            availability_topic: default_availability_topic(),
            discovery_enabled: default_discovery_enabled(),
            discovery_prefix: default_discovery_prefix(),
        }
    }
}

impl MqttSettings {
    pub fn password_set(&self) -> bool {
        self.password
            .as_deref()
            .is_some_and(|password| !password.is_empty())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub identity: IdentitySettings,
    #[serde(default)]
    pub homekit: HomeKitSettings,
    #[serde(default)]
    pub mqtt: MqttSettings,
}

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
    gate: Arc<Mutex<()>>,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn load(&self) -> Settings {
        load_settings(&self.path)
    }

    pub fn update<F>(&self, f: F) -> anyhow::Result<Settings>
    where
        F: FnOnce(&mut Settings),
    {
        let _guard = self.gate.lock().expect("settings lock poisoned");
        let mut settings = load_settings(&self.path);
        f(&mut settings);
        save_settings_atomic(&self.path, &settings)?;
        Ok(settings)
    }
}

pub fn load_settings(path: &Path) -> Settings {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!("corrupt settings file, using defaults: {e}");
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub fn save_settings_atomic(path: &Path, settings: &Settings) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(settings)?;
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WifiSettings {
    pub ssid: String,
    pub security: String,
    pub autoconnect: bool,
    pub password_set: bool,
    pub connected_ssid: Option<String>,
    pub address: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WifiSave {
    pub ssid: String,
    pub password: String,
    pub security: String,
    pub autoconnect: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WifiSaveResult {
    pub saved: bool,
    pub reconfigured: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WifiScanResult {
    pub networks: Vec<String>,
}

pub fn read_wifi_settings(path: &Path) -> WifiSettings {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let block = first_network_block(&text).unwrap_or_default();
    let ssid = parse_wpa_value(&block, "ssid").unwrap_or_default();
    let key_mgmt = parse_wpa_value(&block, "key_mgmt").unwrap_or_else(|| "WPA-PSK".into());
    let psk = parse_wpa_value(&block, "psk");
    let disabled = parse_wpa_value(&block, "disabled").as_deref() == Some("1");
    WifiSettings {
        ssid,
        security: if key_mgmt == "NONE" {
            "Open network".into()
        } else {
            "WPA/WPA2 Personal".into()
        },
        autoconnect: !disabled,
        password_set: psk.map(|p| !p.is_empty()).unwrap_or(false),
        connected_ssid: current_wifi_ssid(),
        address: interface_ipv4("wlan0"),
    }
}

pub fn save_wifi_settings(path: &Path, body: &WifiSave) -> anyhow::Result<WifiSaveResult> {
    validate_ssid(&body.ssid)?;
    let open = body.security == "Open network";
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let existing_block = first_network_block(&existing).unwrap_or_default();
    let existing_ssid = parse_wpa_value(&existing_block, "ssid").unwrap_or_default();
    let existing_psk = parse_wpa_value(&existing_block, "psk");
    let psk_line = if open {
        None
    } else if body.password.is_empty() && existing_ssid == body.ssid {
        Some(
            existing_psk
                .filter(|p| !p.is_empty())
                .ok_or_else(|| anyhow::anyhow!("password is required for a WPA/WPA2 network"))?,
        )
    } else {
        validate_wpa_password(&body.password)?;
        Some(hash_wpa_passphrase(&body.ssid, &body.password)?)
    };
    write_wpa_supplicant(path, &existing, body, psk_line.as_deref())?;
    let reconfigured = reconfigure_wifi();
    Ok(WifiSaveResult {
        saved: true,
        reconfigured,
        message: if reconfigured {
            "Wi-Fi saved and reconfigured".into()
        } else {
            "Wi-Fi saved; reconnect may require a reboot or service restart".into()
        },
    })
}

pub fn scan_wifi_networks() -> anyhow::Result<WifiScanResult> {
    let output = Command::new("iw").args(["dev", "wlan0", "scan"]).output()?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("Wi-Fi scan failed"));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut networks = Vec::new();
    for line in text.lines() {
        let s = line.trim();
        if let Some(name) = s.strip_prefix("SSID: ") {
            let name = name.trim();
            if !name.is_empty() && !networks.iter().any(|n| n == name) {
                networks.push(name.to_string());
            }
        }
    }
    networks.sort();
    Ok(WifiScanResult { networks })
}

pub fn read_authorized_keys(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn save_authorized_keys(path: &Path, keys: &str) -> anyhow::Result<()> {
    validate_authorized_keys(keys)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        set_dir_mode(parent, 0o700)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(keys.trim_end().as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    set_file_mode(&tmp, 0o600)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn validate_authorized_keys(keys: &str) -> anyhow::Result<()> {
    for (idx, line) in keys.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let key_type = parts.next().unwrap_or_default();
        let key_body = parts.next().unwrap_or_default();
        if !is_allowed_key_type(key_type) {
            return Err(anyhow::anyhow!(
                "line {} has an unsupported key type",
                idx + 1
            ));
        }
        if key_body.len() < 32 || !key_body.bytes().all(is_base64ish) {
            return Err(anyhow::anyhow!(
                "line {} has an invalid public key body",
                idx + 1
            ));
        }
    }
    Ok(())
}

pub fn validate_mqtt_settings(settings: &MqttSettings) -> anyhow::Result<()> {
    if settings.enabled {
        if settings.broker_url.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "broker URL is required when MQTT is enabled"
            ));
        }
        if settings.client_id.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "client ID is required when MQTT is enabled"
            ));
        }
        if settings.subscribe_topic.trim().is_empty() && settings.publish_topic.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "at least one MQTT publish or subscribe topic is required"
            ));
        }
        if settings.availability_topic.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "availability topic is required when MQTT is enabled"
            ));
        }
        if settings.discovery_enabled && settings.discovery_prefix.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "discovery prefix is required when Home Assistant discovery is enabled"
            ));
        }
    }
    validate_mqtt_text("broker URL", &settings.broker_url)?;
    validate_mqtt_text("client ID", &settings.client_id)?;
    validate_mqtt_text("username", &settings.username)?;
    validate_mqtt_topic("subscribe topic", &settings.subscribe_topic)?;
    validate_mqtt_topic("publish topic", &settings.publish_topic)?;
    validate_mqtt_topic("availability topic", &settings.availability_topic)?;
    validate_mqtt_topic("discovery prefix", &settings.discovery_prefix)?;
    if let Some(password) = &settings.password {
        validate_mqtt_text("password", password)?;
    }
    Ok(())
}

fn validate_mqtt_text(label: &str, value: &str) -> anyhow::Result<()> {
    if value.chars().any(|c| c.is_control()) {
        return Err(anyhow::anyhow!("{label} cannot contain control characters"));
    }
    Ok(())
}

fn validate_mqtt_topic(label: &str, value: &str) -> anyhow::Result<()> {
    validate_mqtt_text(label, value)?;
    if value.len() > 256 {
        return Err(anyhow::anyhow!("{label} must be 256 characters or fewer"));
    }
    Ok(())
}

pub fn generate_pairing_code() -> anyhow::Result<String> {
    let mut bytes = [0u8; 8];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let digits: String = bytes.iter().map(|b| char::from(b'0' + (b % 10))).collect();
    Ok(format!(
        "{}-{}-{}",
        &digits[0..3],
        &digits[3..5],
        &digits[5..8]
    ))
}

fn first_network_block(text: &str) -> Option<String> {
    let start = text.find("network={")?;
    let rest = &text[start..];
    let end = rest.find("\n}")?;
    Some(rest[..end + 2].to_string())
}

fn parse_wpa_value(block: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    for line in block.lines() {
        let s = line.trim();
        if let Some(raw) = s.strip_prefix(&prefix) {
            return Some(unquote_wpa_value(raw.trim()));
        }
    }
    None
}

fn unquote_wpa_value(raw: &str) -> String {
    let raw = raw.trim();
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        raw[1..raw.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        raw.to_string()
    }
}

fn quote_wpa_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn validate_ssid(ssid: &str) -> anyhow::Result<()> {
    if ssid.trim().is_empty() {
        return Err(anyhow::anyhow!("network name is required"));
    }
    if ssid.as_bytes().len() > 32 {
        return Err(anyhow::anyhow!("network name must be 32 bytes or fewer"));
    }
    if ssid.chars().any(|c| c.is_control()) {
        return Err(anyhow::anyhow!(
            "network name cannot contain control characters"
        ));
    }
    Ok(())
}

fn validate_wpa_password(password: &str) -> anyhow::Result<()> {
    if !(8..=63).contains(&password.len()) {
        return Err(anyhow::anyhow!("WPA password must be 8 to 63 characters"));
    }
    if password.chars().any(|c| c.is_control()) {
        return Err(anyhow::anyhow!(
            "WPA password cannot contain control characters"
        ));
    }
    Ok(())
}

fn hash_wpa_passphrase(ssid: &str, password: &str) -> anyhow::Result<String> {
    let mut child = Command::new("wpa_passphrase")
        .arg(ssid)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("missing stdin"))?;
        stdin.write_all(password.as_bytes())?;
        stdin.write_all(b"\n")?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("wpa_passphrase failed"));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let s = line.trim();
        if let Some(psk) = s.strip_prefix("psk=") {
            if psk.len() == 64 && psk.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Ok(psk.to_string());
            }
        }
    }
    Err(anyhow::anyhow!(
        "wpa_passphrase did not return a hashed psk"
    ))
}

fn write_wpa_supplicant(
    path: &Path,
    existing: &str,
    body: &WifiSave,
    psk: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let country = parse_global_value(existing, "country").unwrap_or_else(|| "US".into());
    let mut out = String::new();
    out.push_str("# WiFi country code, set here in case the access point does send one\n");
    out.push_str(&format!("country={country}\n"));
    out.push_str("# Grant all members of group \"netdev\" permissions to configure WiFi, e.g. via wpa_cli or wpa_gui\n");
    out.push_str("ctrl_interface=DIR=/run/wpa_supplicant GROUP=netdev\n");
    out.push_str("# Allow wpa_cli/wpa_gui to overwrite this config file\n");
    out.push_str("update_config=1\n");
    out.push_str("network={\n");
    out.push_str(&format!("\tssid={}\n", quote_wpa_value(&body.ssid)));
    out.push_str("\tscan_ssid=1\n");
    if body.security == "Open network" {
        out.push_str("\tkey_mgmt=NONE\n");
    } else {
        out.push_str("\tkey_mgmt=WPA-PSK\n");
        out.push_str(&format!("\tpsk={}\n", psk.unwrap_or_default()));
    }
    if !body.autoconnect {
        out.push_str("\tdisabled=1\n");
    }
    out.push_str("}\n");
    let tmp = path.with_extension("conf.tmp");
    std::fs::write(&tmp, out)?;
    set_file_mode(&tmp, 0o600)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn parse_global_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    text.lines().map(str::trim).find_map(|line| {
        line.strip_prefix(&prefix)
            .map(|value| value.trim().to_string())
    })
}

fn reconfigure_wifi() -> bool {
    Command::new("wpa_cli")
        .args(["-i", "wlan0", "reconfigure"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn current_wifi_ssid() -> Option<String> {
    let output = Command::new("iw")
        .args(["dev", "wlan0", "link"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("SSID: ").map(|s| s.to_string()))
}

fn interface_ipv4(interface: &str) -> Option<String> {
    let output = Command::new("ip")
        .args(["-4", "-brief", "addr", "show", interface])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .find(|part| part.contains('/') && part.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|s| s.to_string())
}

fn is_allowed_key_type(key_type: &str) -> bool {
    matches!(
        key_type,
        "ssh-ed25519"
            | "ssh-rsa"
            | "ecdsa-sha2-nistp256"
            | "ecdsa-sha2-nistp384"
            | "ecdsa-sha2-nistp521"
            | "sk-ssh-ed25519@openssh.com"
            | "sk-ecdsa-sha2-nistp256@openssh.com"
    )
}

fn is_base64ish(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')
}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_dir_mode(path: &Path, mode: u32) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &Path, _mode: u32) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mlp-settings-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn settings_roundtrip() {
        let p = tmp_path("roundtrip");
        let store = SettingsStore::new(p.clone());
        let saved = store
            .update(|settings| {
                settings.identity.device_name = "Desk Lamp".into();
                settings.homekit.enabled = true;
            })
            .unwrap();
        assert_eq!(saved.identity.device_name, "Desk Lamp");
        assert!(store.load().homekit.enabled);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_mqtt_settings_loads_default() {
        let p = tmp_path("legacy-settings");
        std::fs::write(
            &p,
            r#"{"identity":{"device_name":"Desk","room":"Office","light_label":"Lamp"},"homekit":{"accessory_name":"Lamp","enabled":false,"pairing_code":null}}"#,
        )
        .unwrap();
        let settings = load_settings(&p);
        assert_eq!(settings.mqtt.subscribe_topic, "moodlightpi/set");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn validates_mqtt_settings() {
        let mut settings = MqttSettings {
            enabled: true,
            ..Default::default()
        };
        assert!(validate_mqtt_settings(&settings).is_ok());
        settings.client_id.clear();
        assert!(validate_mqtt_settings(&settings).is_err());
    }

    #[test]
    fn mqtt_availability_and_discovery_defaults() {
        let d = MqttSettings::default();
        assert_eq!(d.availability_topic, "moodlightpi/availability");
        assert!(d.discovery_enabled);
        assert_eq!(d.discovery_prefix, "homeassistant");
    }

    #[test]
    fn old_mqtt_settings_json_loads_with_ha_defaults() {
        // A settings.json written before the HA fields existed must still deserialize,
        // filling the Home Assistant defaults rather than empty strings.
        let json = r#"{"enabled":true,"broker_url":"mqtt://b:1883","client_id":"x",
            "username":"","password":null,"subscribe_topic":"moodlightpi/set",
            "publish_topic":"moodlightpi/state"}"#;
        let mqtt: MqttSettings = serde_json::from_str(json).unwrap();
        assert_eq!(mqtt.availability_topic, "moodlightpi/availability");
        assert!(mqtt.discovery_enabled);
        assert_eq!(mqtt.discovery_prefix, "homeassistant");
    }

    #[test]
    fn rejects_empty_availability_and_discovery_prefix_when_used() {
        let mut s = MqttSettings {
            enabled: true,
            ..Default::default()
        };
        s.availability_topic.clear();
        assert!(validate_mqtt_settings(&s).is_err());
        s.availability_topic = "moodlightpi/availability".into();
        s.discovery_enabled = true;
        s.discovery_prefix.clear();
        assert!(validate_mqtt_settings(&s).is_err());
    }

    #[test]
    fn parses_wpa_settings() {
        let p = tmp_path("wifi");
        std::fs::write(
            &p,
            "country=US\nnetwork={\n\tssid=\"Home\"\n\tkey_mgmt=WPA-PSK\n\tpsk=abc123\n}\n",
        )
        .unwrap();
        let wifi = read_wifi_settings(&p);
        assert_eq!(wifi.ssid, "Home");
        assert_eq!(wifi.security, "WPA/WPA2 Personal");
        assert!(wifi.password_set);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn validates_authorized_keys() {
        assert!(validate_authorized_keys("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIKaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa user\n").is_ok());
        assert!(validate_authorized_keys(
            "nope AAAAC3NzaC1lZDI1NTE5AAAAIKaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa user\n"
        )
        .is_err());
    }
}
