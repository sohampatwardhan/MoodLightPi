/** HomeKit settings: accessory name + enable, with live pairing status. The pairing code is shown
 * only while HomeKit is enabled, ready to pair, and not yet paired (R7.2). Empty accessory name is
 * rejected by the server (R7.4). */
import { useEffect, useState } from 'preact/hooks';
import { getHomeKit, saveHomeKit } from '../api';
import { useStore } from '../store';
import type { HomeKitSettings } from '../types';

export function HomeKitForm() {
  const { pushToast } = useStore();
  const [info, setInfo] = useState<HomeKitSettings | null>(null);

  useEffect(() => {
    getHomeKit().then(setInfo).catch((err) => pushToast((err as Error).message, 'error'));
  }, []);

  if (!info) return <section class="card"><p class="hint">Loading…</p></section>;

  const save = async () => {
    try {
      const result = await saveHomeKit({
        accessory_name: info.accessory_name,
        enabled: info.enabled,
        pairing_code: null,
      });
      setInfo(result);
      pushToast('HomeKit settings saved');
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  const showCode = info.enabled && info.ready_to_pair && !info.paired && info.pairing_code;

  return (
    <section class="card">
      <h2>HomeKit</h2>
      <label class="switch">
        <input type="checkbox" checked={info.enabled} onChange={(e) => setInfo({ ...info, enabled: (e.currentTarget as HTMLInputElement).checked })} />
        <span class="track" /> Enable HomeKit
      </label>
      <p class="status-line">{info.status}{info.paired ? ' · paired' : ''}</p>
      <label class="field"><span>Accessory name</span>
        <input type="text" value={info.accessory_name} onInput={(e) => setInfo({ ...info, accessory_name: (e.currentTarget as HTMLInputElement).value })} /></label>
      {showCode && <div class="pairing-code">{info.pairing_code}</div>}
      <div class="actions"><button class="button primary" onClick={save}>Save HomeKit</button></div>
    </section>
  );
}
