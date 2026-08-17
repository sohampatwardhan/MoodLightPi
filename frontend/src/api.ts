/**
 * Typed, same-origin HTTP client for the device API.
 *
 * Every call targets a relative URL, so requests stay same-origin — that is what keeps the
 * device's Host/Origin guard (src/api.rs) satisfied and the LAN-only policy intact (R14.1). A
 * non-2xx response rejects with the server's `error` message when present, so callers can surface
 * a meaningful reason in a toast (R16.2) rather than a bare status code.
 */
import type {
  BootstrapResponse,
  Health,
  HomeKitSave,
  HomeKitSettings,
  IdentitySettings,
  MqttSave,
  MqttSettings,
  Rgb,
  StateResponse,
  SystemActionName,
  WifiSave,
  WifiSaveResult,
  WifiSettings,
} from './types';

/** Parse a JSON response, throwing `Error(payload.error ?? "<status> <statusText>")` on non-2xx. */
async function parse<T>(res: Response): Promise<T> {
  const text = await res.text();
  const body = text ? (JSON.parse(text) as unknown) : undefined;
  if (!res.ok) {
    const message =
      body && typeof body === 'object' && 'error' in body && typeof body.error === 'string'
        ? body.error
        : `${res.status} ${res.statusText}`.trim();
    throw new Error(message);
  }
  return body as T;
}

/** GET a JSON resource. */
async function getJson<T>(path: string): Promise<T> {
  return parse<T>(await fetch(path, { headers: { accept: 'application/json' } }));
}

/** POST a JSON body and parse the JSON response (or `undefined` for empty 2xx bodies). */
async function postJson<T>(path: string, body: unknown): Promise<T> {
  return parse<T>(
    await fetch(path, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    }),
  );
}

// --- light control -------------------------------------------------------------------------

export const getState = (): Promise<StateResponse> => getJson('/api/state');
export const getEffects = (): Promise<string[]> =>
  getJson<{ effects: string[] }>('/api/effects').then((r) => r.effects);
export const getHealth = (): Promise<Health> => getJson('/healthz');

export const setColor = (rgb: Rgb): Promise<void> => postJson('/api/color', rgb).then(() => undefined);
export const setBrightness = (value: number): Promise<void> =>
  postJson('/api/brightness', { value }).then(() => undefined);
export const setEffect = (name: string, speed?: number): Promise<void> =>
  postJson('/api/effect', speed === undefined ? { name } : { name, speed }).then(() => undefined);
export const setPower = (on: boolean): Promise<void> =>
  postJson('/api/power', { on }).then(() => undefined);
export const systemAction = (action: SystemActionName): Promise<{ queued: boolean; action: string }> =>
  postJson('/api/system/action', { action });

// --- bootstrap -----------------------------------------------------------------------------

/** One round-trip that seeds the whole UI (R11.3). */
export const getBootstrap = (): Promise<BootstrapResponse> => getJson('/api/bootstrap');

// --- settings ------------------------------------------------------------------------------

export const getIdentity = (): Promise<IdentitySettings> => getJson('/api/settings/identity');
export const saveIdentity = (body: IdentitySettings): Promise<IdentitySettings> =>
  postJson('/api/settings/identity', body);

export const getWifi = (): Promise<WifiSettings> => getJson('/api/settings/wifi');
export const saveWifi = (body: WifiSave): Promise<WifiSaveResult> => postJson('/api/settings/wifi', body);
export const scanWifi = (): Promise<{ networks: string[] }> => getJson('/api/settings/wifi/scan');

export const getMqtt = (): Promise<MqttSettings> => getJson('/api/settings/mqtt');
export const saveMqtt = (body: MqttSave): Promise<MqttSettings> => postJson('/api/settings/mqtt', body);

export const getHomeKit = (): Promise<HomeKitSettings> => getJson('/api/settings/homekit');
export const saveHomeKit = (body: HomeKitSave): Promise<HomeKitSettings> =>
  postJson('/api/settings/homekit', body);
export const regenHomeKitPairingCode = (): Promise<HomeKitSettings> =>
  postJson('/api/settings/homekit/pairing-code', {});

export const getSshKeys = (): Promise<string> =>
  getJson<{ keys: string }>('/api/settings/ssh-keys').then((r) => r.keys);
export const saveSshKeys = (keys: string): Promise<void> =>
  postJson('/api/settings/ssh-keys', { keys }).then(() => undefined);
export const validateSshKeys = (keys: string): Promise<void> =>
  postJson('/api/settings/ssh-keys/validate', { keys }).then(() => undefined);
