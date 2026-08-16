const WIDTH = 8;
const HEIGHT = 4;

const canvas = document.getElementById('preview');
const ctx = canvas.getContext('2d');
const cellW = canvas.width / WIDTH;
const cellH = canvas.height / HEIGHT;

const els = {
  brightness: document.getElementById('brightness'),
  brightnessValue: document.getElementById('brightnessValue'),
  color: document.getElementById('color'),
  colorValue: document.getElementById('colorValue'),
  dashboard: document.getElementById('dashboard'),
  effect: document.getElementById('effect'),
  effectSpeedControl: document.getElementById('effectSpeedControl'),
  homekitAccessoryName: document.getElementById('homekitAccessoryName'),
  homekitEnabled: document.getElementById('homekitEnabled'),
  homekitPairingBox: document.getElementById('homekitPairingBox'),
  homekitPairingCode: document.getElementById('homekitPairingCode'),
  homekitState: document.getElementById('homekitState'),
  identityDeviceName: document.getElementById('identityDeviceName'),
  identityLightLabel: document.getElementById('identityLightLabel'),
  identityRoom: document.getElementById('identityRoom'),
  modeLabel: document.getElementById('modeLabel'),
  mqttBrokerUrl: document.getElementById('mqttBrokerUrl'),
  mqttClientId: document.getElementById('mqttClientId'),
  mqttEnabled: document.getElementById('mqttEnabled'),
  mqttPassword: document.getElementById('mqttPassword'),
  mqttPublishTopic: document.getElementById('mqttPublishTopic'),
  mqttState: document.getElementById('mqttState'),
  mqttSubscribeTopic: document.getElementById('mqttSubscribeTopic'),
  mqttUsername: document.getElementById('mqttUsername'),
  power: document.getElementById('power'),
  sequenceLabel: document.getElementById('sequenceLabel'),
  settingsButton: document.getElementById('settingsButton'),
  settingsPanel: document.getElementById('settingsPanel'),
  sshKeys: document.getElementById('sshKeys'),
  sshState: document.getElementById('sshState'),
  speed: document.getElementById('speed'),
  speedValue: document.getElementById('speedValue'),
  statusPill: document.getElementById('statusPill'),
  statusText: document.getElementById('statusText'),
  stateSummary: document.getElementById('stateSummary'),
  systemDialog: document.getElementById('systemDialog'),
  systemDialogCancel: document.getElementById('systemDialogCancel'),
  systemDialogClose: document.getElementById('systemDialogClose'),
  toast: document.getElementById('toast'),
  wifiAutoconnect: document.getElementById('wifiAutoconnect'),
  wifiNetworks: document.getElementById('wifiNetworks'),
  wifiPassword: document.getElementById('wifiPassword'),
  wifiSecurity: document.getElementById('wifiSecurity'),
  wifiSsid: document.getElementById('wifiSsid'),
  wifiState: document.getElementById('wifiState'),
};

let latestState = null;
let toastTimer = 0;
let brightnessTimer = 0;
let speedTimer = 0;

const systemActionLabels = {
  restart_service: 'Restarting MoodLightPi service...',
  reboot: 'Restarting Raspberry Pi...',
  poweroff: 'Shutting down Raspberry Pi...',
};

const settingsSections = new Set(['identity', 'wifi', 'mqtt', 'homekit', 'ssh', 'device']);
const settingsSectionAliases = new Map([
  ['ssh-keys', 'ssh'],
]);

function colorToHex({ r, g, b }) {
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

function hexToRgb(value) {
  return {
    r: parseInt(value.slice(1, 3), 16),
    g: parseInt(value.slice(3, 5), 16),
    b: parseInt(value.slice(5, 7), 16),
  };
}

function brightnessToPercent(value) {
  return Math.round((value / 255) * 100);
}

function percentToBrightness(value) {
  return Math.round((Math.max(0, Math.min(100, value)) / 100) * 255);
}

function formatEffect(name) {
  if (!name) return '-';
  return name.replace(/[-_]/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

function isSolidEffect(name) {
  return String(name || '').toLowerCase() === 'solid';
}

function updateSpeedVisibility(effectName) {
  els.effectSpeedControl.hidden = isSolidEffect(effectName);
}

function showToast(message) {
  clearTimeout(toastTimer);
  els.toast.textContent = message;
  els.toast.classList.add('visible');
  toastTimer = setTimeout(() => els.toast.classList.remove('visible'), 2600);
}

function draw(pixels) {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = '#050605';
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  const gap = 6;
  for (let y = 0; y < HEIGHT; y++) {
    for (let x = 0; x < WIDTH; x++) {
      const [r, g, b] = pixels[y * WIDTH + x];
      const drawX = WIDTH - 1 - x;
      const drawY = HEIGHT - 1 - y;
      const px = drawX * cellW + gap;
      const py = drawY * cellH + gap;
      const w = cellW - gap * 2;
      const h = cellH - gap * 2;

      ctx.fillStyle = `rgb(${r},${g},${b})`;
      ctx.beginPath();
      ctx.roundRect(px, py, w, h, 12);
      ctx.fill();
    }
  }
}

async function post(path, body) {
  const res = await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${path} returned ${res.status}`);
}

async function postJson(path, body = {}) {
  const res = await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const text = await res.text();
  const payload = text ? JSON.parse(text) : {};
  if (!res.ok) throw new Error(payload.error || `${path} returned ${res.status}`);
  return payload;
}

async function getJson(path) {
  const res = await fetch(path);
  if (!res.ok) throw new Error(`${path} returned ${res.status}`);
  return res.json();
}

function applyState(payload) {
  const state = payload.state;
  latestState = state;

  els.power.setAttribute('aria-label', 'Open system power options');
  els.power.title = 'System power options';

  const hex = colorToHex(state.rgb);
  els.color.value = hex;
  els.colorValue.textContent = hex;

  const brightnessPercent = brightnessToPercent(state.brightness);
  els.brightness.value = brightnessPercent;
  els.brightnessValue.textContent = `${brightnessPercent}%`;
  els.speed.value = state.speed;
  els.speedValue.textContent = state.speed;

  if (state.effect_name && els.effect.value !== state.effect_name) {
    els.effect.value = state.effect_name;
  }
  updateSpeedVisibility(state.effect_name || state.mode);

  els.modeLabel.textContent = `Effect ${formatEffect(state.effect_name || state.mode)}`;
  els.sequenceLabel.textContent = `Seq ${payload.seq}`;
  els.stateSummary.textContent = `${state.power ? 'On' : 'Off'} · ${formatEffect(state.effect_name)} · brightness ${brightnessPercent}%`;
}

async function refreshState() {
  const payload = await getJson('/api/state');
  applyState(payload);
}

async function refreshHealth() {
  try {
    await getJson('/healthz');
    els.statusText.textContent = 'active';
    els.statusPill.classList.remove('offline');
  } catch {
    els.statusText.textContent = 'offline';
    els.statusPill.classList.add('offline');
  }
}

async function loadEffects() {
  const { effects } = await getJson('/api/effects');
  els.effect.replaceChildren();
  for (const name of effects) {
    const opt = document.createElement('option');
    opt.value = name;
    opt.textContent = formatEffect(name);
    els.effect.appendChild(opt);
  }
}

function applyIdentity(identity) {
  els.identityDeviceName.value = identity.device_name || 'MoodLightPi';
  els.identityRoom.value = identity.room || '';
  els.identityLightLabel.value = identity.light_label || 'Mood Light';
}

function applyWifi(wifi) {
  els.wifiSsid.value = wifi.ssid || '';
  els.wifiPassword.value = '';
  els.wifiSecurity.value = wifi.security || 'WPA/WPA2 Personal';
  els.wifiAutoconnect.checked = wifi.autoconnect !== false;
  const connected = wifi.connected_ssid ? `Connected: ${wifi.connected_ssid}` : 'Configured';
  els.wifiState.textContent = wifi.ssid ? connected : 'Not configured';
}

function applyMqtt(mqtt) {
  els.mqttEnabled.checked = Boolean(mqtt.enabled);
  els.mqttBrokerUrl.value = mqtt.broker_url || 'mqtt://localhost:1883';
  els.mqttClientId.value = mqtt.client_id || 'moodlightpi';
  els.mqttUsername.value = mqtt.username || '';
  els.mqttPassword.value = '';
  els.mqttPassword.placeholder = mqtt.password_set ? 'Password saved' : 'Optional password';
  els.mqttSubscribeTopic.value = mqtt.subscribe_topic || 'moodlightpi/set';
  els.mqttPublishTopic.value = mqtt.publish_topic || 'moodlightpi/state';
  els.mqttState.textContent = mqtt.status || (mqtt.enabled ? 'Reconnect pending' : 'Disabled');
}

function homeKitStatus(homekit) {
  if (homekit.status) return homekit.status;
  if (homekit.paired) return 'Paired';
  if (homekit.enabled) return 'Accessory server pending';
  return 'Disabled';
}

function homeKitCanPair(homekit) {
  const status = homeKitStatus(homekit).toLowerCase();
  return Boolean(
    homekit.enabled
      && !homekit.paired
      && homekit.pairing_code
      && (homekit.ready_to_pair || status.includes('ready to pair')),
  );
}

function applyHomeKit(homekit) {
  els.homekitAccessoryName.value = homekit.accessory_name || 'Mood Light';
  els.homekitEnabled.checked = Boolean(homekit.enabled);
  const canPair = homeKitCanPair(homekit);
  els.homekitPairingBox.hidden = !canPair;
  els.homekitPairingCode.textContent = canPair ? homekit.pairing_code : '--- -- ---';
  els.homekitState.textContent = homeKitStatus(homekit);
}

function applySshKeys(payload) {
  els.sshKeys.value = payload.keys || '';
  const count = (payload.keys || '')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#')).length;
  els.sshState.textContent = `${count} key${count === 1 ? '' : 's'}`;
}

async function loadSettings() {
  const [identity, wifi, mqtt, homekit, ssh] = await Promise.all([
    getJson('/api/settings/identity'),
    getJson('/api/settings/wifi'),
    getJson('/api/settings/mqtt'),
    getJson('/api/settings/homekit'),
    getJson('/api/settings/ssh-keys'),
  ]);
  applyIdentity(identity);
  applyWifi(wifi);
  applyMqtt(mqtt);
  applyHomeKit(homekit);
  applySshKeys(ssh);
}

async function saveIdentity() {
  const identity = await postJson('/api/settings/identity', {
    device_name: els.identityDeviceName.value,
    room: els.identityRoom.value,
    light_label: els.identityLightLabel.value,
  });
  applyIdentity(identity);
  showToast('Identity saved.');
}

async function scanWifi() {
  const { networks } = await getJson('/api/settings/wifi/scan');
  els.wifiNetworks.replaceChildren(...networks.map((network) => {
    const option = document.createElement('option');
    option.value = network;
    return option;
  }));
  showToast(networks.length ? `Found ${networks.length} Wi-Fi networks.` : 'No Wi-Fi networks found.');
}

async function saveWifi() {
  const result = await postJson('/api/settings/wifi', {
    ssid: els.wifiSsid.value,
    password: els.wifiPassword.value,
    security: els.wifiSecurity.value,
    autoconnect: els.wifiAutoconnect.checked,
  });
  applyWifi(await getJson('/api/settings/wifi'));
  showToast(result.message || 'Wi-Fi saved.');
}

async function saveMqtt() {
  const mqtt = await postJson('/api/settings/mqtt', {
    enabled: els.mqttEnabled.checked,
    broker_url: els.mqttBrokerUrl.value,
    client_id: els.mqttClientId.value,
    username: els.mqttUsername.value,
    password: els.mqttPassword.value,
    clear_password: false,
    subscribe_topic: els.mqttSubscribeTopic.value,
    publish_topic: els.mqttPublishTopic.value,
  });
  applyMqtt(mqtt);
  showToast(mqtt.enabled ? 'MQTT saved. Connection will refresh shortly.' : 'MQTT disabled.');
}

async function saveHomeKit() {
  const homekit = await postJson('/api/settings/homekit', {
    accessory_name: els.homekitAccessoryName.value,
    enabled: els.homekitEnabled.checked,
    pairing_code: null,
  });
  applyHomeKit(homekit);
  showToast('HomeKit settings saved.');
}

async function validateSshKeys() {
  await postJson('/api/settings/ssh-keys/validate', { keys: els.sshKeys.value });
  showToast('SSH keys look valid.');
}

async function saveSshKeys() {
  await postJson('/api/settings/ssh-keys', { keys: els.sshKeys.value });
  applySshKeys(await getJson('/api/settings/ssh-keys'));
  showToast('SSH keys saved.');
}

async function runSettingsAction(button) {
  const action = button.dataset.action;
  const oldText = button.textContent;
  button.disabled = true;
  button.textContent = 'Working...';
  try {
    if (action === 'save-identity') await saveIdentity();
    if (action === 'scan-wifi') await scanWifi();
    if (action === 'save-wifi') await saveWifi();
    if (action === 'save-mqtt') await saveMqtt();
    if (action === 'save-homekit') await saveHomeKit();
    if (action === 'validate-ssh') await validateSshKeys();
    if (action === 'save-ssh') await saveSshKeys();
  } catch (err) {
    showToast(err.message);
  } finally {
    button.disabled = false;
    button.textContent = oldText;
  }
}

function connect() {
  const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
  const ws = new WebSocket(`${scheme}://${location.host}/ws`);

  ws.onmessage = (e) => {
    const payload = JSON.parse(e.data);
    draw(payload.pixels);
  };

  ws.onclose = () => {
    setTimeout(connect, 1000);
  };
}

function settingsSectionFromPath() {
  if (location.pathname === '/settings') return 'identity';
  const match = location.pathname.match(/^\/(?:settings\/)?([^/]+)$/);
  if (!match) return null;
  const section = settingsSectionAliases.get(match[1]) || match[1];
  return settingsSections.has(section) ? section : 'identity';
}

function renderRoute() {
  const section = settingsSectionFromPath();
  const isSettings = section !== null;
  els.dashboard.hidden = isSettings;
  els.settingsPanel.hidden = !isSettings;
  els.settingsPanel.setAttribute('aria-hidden', String(!isSettings));
  els.settingsButton.setAttribute('aria-current', isSettings ? 'page' : 'false');
  if (isSettings) {
    if (location.pathname === '/settings' || location.pathname.startsWith('/settings/')) {
      history.replaceState({}, '', `/${section}`);
    }
    selectSettingsSection(section, false);
  }
}

function selectSettingsSection(name, updateUrl = true) {
  document.querySelectorAll('.nav-item').forEach((button) => {
    button.classList.toggle('active', button.dataset.section === name);
    button.setAttribute('aria-current', button.dataset.section === name ? 'page' : 'false');
  });
  document.querySelectorAll('.settings-section').forEach((section) => {
    section.classList.toggle('active', section.id === `settings-${name}`);
  });
  if (updateUrl && settingsSectionFromPath()) {
    history.pushState({}, '', `/${name}`);
  }
}

function schedule(fn, delay, key) {
  clearTimeout(key === 'brightness' ? brightnessTimer : speedTimer);
  const timer = setTimeout(fn, delay);
  if (key === 'brightness') brightnessTimer = timer;
  if (key === 'speed') speedTimer = timer;
}

function openSystemDialog() {
  els.systemDialog.hidden = false;
  els.systemDialog.querySelector('[data-system-action]')?.focus();
}

function closeSystemDialog() {
  els.systemDialog.hidden = true;
  els.power.focus();
}

function setSystemActionButtonsDisabled(disabled) {
  els.systemDialog.querySelectorAll('button').forEach((button) => {
    button.disabled = disabled;
  });
}

async function runSystemAction(action) {
  setSystemActionButtonsDisabled(true);
  try {
    await postJson('/api/system/action', { action });
    showToast(systemActionLabels[action] || 'System action queued.');
    closeSystemDialog();
  } catch (err) {
    showToast(err.message);
  } finally {
    setSystemActionButtonsDisabled(false);
  }
}

function bindControls() {
  els.power.onclick = openSystemDialog;

  els.color.oninput = (e) => {
    const value = e.target.value;
    els.colorValue.textContent = value;
    post('/api/color', hexToRgb(value)).then(refreshState).catch((err) => showToast(err.message));
  };

  document.getElementById('swatches').onclick = (e) => {
    const button = e.target.closest('button[data-color]');
    if (!button) return;
    els.color.value = button.dataset.color;
    els.color.dispatchEvent(new Event('input', { bubbles: true }));
  };

  els.brightness.oninput = (e) => {
    const percent = parseInt(e.target.value, 10);
    els.brightnessValue.textContent = `${percent}%`;
    schedule(() => {
      const value = percentToBrightness(percent);
      post('/api/brightness', { value }).then(refreshState).catch((err) => showToast(err.message));
    }, 80, 'brightness');
  };

  els.speed.oninput = (e) => {
    const speed = parseInt(e.target.value, 10);
    els.speedValue.textContent = speed;
    schedule(() => {
      post('/api/effect', { name: els.effect.value, speed }).then(refreshState).catch((err) => showToast(err.message));
    }, 80, 'speed');
  };

  els.effect.onchange = () => {
    updateSpeedVisibility(els.effect.value);
    post('/api/effect', { name: els.effect.value }).then(refreshState).catch((err) => showToast(err.message));
  };

  document.querySelectorAll('.nav-item').forEach((button) => {
    button.addEventListener('click', (e) => {
      e.preventDefault();
      selectSettingsSection(button.dataset.section);
    });
  });

  document.querySelectorAll('[data-action]').forEach((button) => {
    button.addEventListener('click', () => runSettingsAction(button));
  });

  document.querySelectorAll('[data-pending]').forEach((button) => {
    button.addEventListener('click', () => {
      showToast(`${button.dataset.pending} needs the settings API backend.`);
    });
  });

  els.systemDialog.addEventListener('click', (e) => {
    if (e.target === els.systemDialog) closeSystemDialog();
    const button = e.target.closest('[data-system-action]');
    if (button) runSystemAction(button.dataset.systemAction);
  });
  els.systemDialogClose.addEventListener('click', closeSystemDialog);
  els.systemDialogCancel.addEventListener('click', closeSystemDialog);

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !els.systemDialog.hidden) {
      closeSystemDialog();
      return;
    }
    if (e.key === 'Escape' && settingsSectionFromPath()) location.assign('/');
  });

  window.addEventListener('popstate', renderRoute);
}

async function init() {
  renderRoute();
  draw(Array.from({ length: WIDTH * HEIGHT }, () => [0, 0, 0]));
  bindControls();

  try {
    await Promise.all([loadEffects(), refreshHealth(), loadSettings()]);
    await refreshState();
  } catch (err) {
    showToast(err.message);
  }

  connect();
  setInterval(refreshHealth, 15000);
  setInterval(() => refreshState().catch(() => {}), 5000);
}

init();
