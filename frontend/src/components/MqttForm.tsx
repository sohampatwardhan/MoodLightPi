/** MQTT settings incl. Home Assistant availability topic + discovery enable/prefix (R6). The
 * stored password is never shown (only whether one is set); leaving it blank keeps it, and the
 * clear-password option removes it. Invalid settings are rejected by the server and surfaced. */
import { useEffect, useState } from 'preact/hooks';
import { getMqtt, saveMqtt } from '../api';
import { useStore } from '../store';
import type { MqttSettings } from '../types';

export function MqttForm() {
  const { pushToast } = useStore();
  const [info, setInfo] = useState<MqttSettings | null>(null);
  const [password, setPassword] = useState('');
  const [clearPassword, setClearPassword] = useState(false);

  useEffect(() => {
    getMqtt().then(setInfo).catch((err) => pushToast((err as Error).message, 'error'));
  }, []);

  if (!info) return <section class="card"><p class="hint">Loading…</p></section>;

  const set = <K extends keyof MqttSettings>(key: K, value: MqttSettings[K]) => setInfo({ ...info, [key]: value });

  const save = async () => {
    try {
      const result = await saveMqtt({
        enabled: info.enabled,
        broker_url: info.broker_url,
        client_id: info.client_id,
        username: info.username,
        password,
        clear_password: clearPassword,
        subscribe_topic: info.subscribe_topic,
        publish_topic: info.publish_topic,
        availability_topic: info.availability_topic,
        discovery_enabled: info.discovery_enabled,
        discovery_prefix: info.discovery_prefix,
      });
      setInfo(result);
      setPassword('');
      setClearPassword(false);
      pushToast('MQTT settings saved');
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  const text = (key: keyof MqttSettings) => (e: Event) => set(key, (e.currentTarget as HTMLInputElement).value as never);

  return (
    <section class="card">
      <h2>MQTT</h2>
      <label class="switch">
        <input type="checkbox" checked={info.enabled} onChange={(e) => set('enabled', (e.currentTarget as HTMLInputElement).checked)} />
        <span class="track" /> Enable MQTT
      </label>
      <p class="status-line">{info.status}</p>
      <label class="field"><span>Broker URL</span><input type="text" value={info.broker_url} onInput={text('broker_url')} /></label>
      <label class="field"><span>Client ID</span><input type="text" value={info.client_id} onInput={text('client_id')} /></label>
      <label class="field"><span>Username</span><input type="text" value={info.username} onInput={text('username')} /></label>
      <label class="field"><span>Password {info.password_set ? '(stored)' : ''}</span>
        <input type="password" value={password} placeholder={info.password_set ? '••••••••' : ''} onInput={(e) => setPassword((e.currentTarget as HTMLInputElement).value)} /></label>
      <label class="switch"><input type="checkbox" checked={clearPassword} onChange={(e) => setClearPassword((e.currentTarget as HTMLInputElement).checked)} /><span class="track" /> Clear stored password</label>
      <div class="field-row">
        <label class="field"><span>Subscribe topic</span><input type="text" value={info.subscribe_topic} onInput={text('subscribe_topic')} /></label>
        <label class="field"><span>Publish topic</span><input type="text" value={info.publish_topic} onInput={text('publish_topic')} /></label>
      </div>
      <label class="field"><span>Availability topic</span><input type="text" value={info.availability_topic} onInput={text('availability_topic')} /></label>
      <div class="field-row">
        <label class="switch" style={{ alignSelf: 'end', marginBottom: '14px' }}>
          <input type="checkbox" checked={info.discovery_enabled} onChange={(e) => set('discovery_enabled', (e.currentTarget as HTMLInputElement).checked)} /><span class="track" /> Home Assistant discovery
        </label>
        <label class="field"><span>Discovery prefix</span><input type="text" value={info.discovery_prefix} onInput={text('discovery_prefix')} /></label>
      </div>
      <div class="actions"><button class="button primary" onClick={save}>Save MQTT</button></div>
    </section>
  );
}
