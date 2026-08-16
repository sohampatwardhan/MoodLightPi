/**
 * TypeScript mirrors of the device's HTTP/JSON contracts.
 *
 * Each type mirrors a specific server shape (see src/api.rs / src/settings.rs). Keeping these in
 * one file gives the whole SPA a single typed source of truth for what the device returns and
 * accepts; drift shows up as a compile error rather than a runtime surprise.
 */

/** An 8-bit RGB colour, matching the device `Rgb` struct. */
export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** Current light state (`GET /api/state` `.state`). `mode` is `"solid"` or `"effect"`. */
export interface LightState {
  power: boolean;
  mode: string;
  rgb: Rgb;
  brightness: number; // 0-255
  effect_name: string;
  speed: number; // 0-255
}

/** `GET /api/state` response. `seq` increments on every state change. */
export interface StateResponse {
  state: LightState;
  seq: number;
}

/** `GET /healthz` response; `backend` is `"hardware"` or `"mock"`. */
export interface Health {
  alive: boolean;
  backend: string;
}

/** Identity settings (`GET/POST /api/settings/identity`). */
export interface IdentitySettings {
  device_name: string;
  room: string;
  light_label: string;
}

/** Wi-Fi status (`GET /api/settings/wifi`). */
export interface WifiSettings {
  ssid: string;
  security: string;
  autoconnect: boolean;
  password_set: boolean;
  connected_ssid: string | null;
  address: string | null;
}

/** Wi-Fi save body (`POST /api/settings/wifi`). */
export interface WifiSave {
  ssid: string;
  password: string;
  security: string;
  autoconnect: boolean;
}

/** Wi-Fi save result. */
export interface WifiSaveResult {
  saved: boolean;
  reconfigured: boolean;
  message: string;
}

/** MQTT settings as returned (`GET /api/settings/mqtt`); the password is never sent to the client. */
export interface MqttSettings {
  enabled: boolean;
  broker_url: string;
  client_id: string;
  username: string;
  password_set: boolean;
  subscribe_topic: string;
  publish_topic: string;
  availability_topic: string;
  discovery_enabled: boolean;
  discovery_prefix: string;
  status: string;
}

/** MQTT save body (`POST /api/settings/mqtt`); `password` empty keeps the stored one unless `clear_password`. */
export interface MqttSave {
  enabled: boolean;
  broker_url: string;
  client_id: string;
  username: string;
  password: string;
  clear_password: boolean;
  subscribe_topic: string;
  publish_topic: string;
  availability_topic: string;
  discovery_enabled: boolean;
  discovery_prefix: string;
}

/** HomeKit settings + live pairing status (`GET/POST /api/settings/homekit`). */
export interface HomeKitSettings {
  accessory_name: string;
  enabled: boolean;
  pairing_code: string | null;
  status: string;
  paired: boolean;
  ready_to_pair: boolean;
  port: number | null;
}

/** HomeKit save body. */
export interface HomeKitSave {
  accessory_name: string;
  enabled: boolean;
  pairing_code: string | null;
}

/** Aggregated first-load payload (`GET /api/bootstrap`) — one round-trip for the whole UI. */
export interface BootstrapResponse {
  state: LightState;
  seq: number;
  effects: string[];
  settings: {
    identity: IdentitySettings;
    mqtt: MqttSettings;
    homekit: HomeKitSettings;
    wifi: WifiSettings;
    ssh_keys: string;
  };
}

/** System actions accepted by `POST /api/system/action`. */
export type SystemActionName = 'restart_service' | 'reboot' | 'poweroff';
