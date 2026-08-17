/** Wi-Fi settings: shows current SSID/security/autoconnect + whether a password is stored and the
 * connected network; scan lists nearby SSIDs; save reconfigures (R5). A failed scan surfaces an
 * error without discarding the entered SSID (R5.4). */
import { useEffect, useState } from 'preact/hooks';
import { getWifi, saveWifi, scanWifi } from '../api';
import { useStore } from '../store';
import type { WifiSettings } from '../types';

const SECURITY_OPTIONS = ['WPA/WPA2 Personal', 'Open'];

export function WifiForm() {
  const { pushToast } = useStore();
  const [info, setInfo] = useState<WifiSettings | null>(null);
  const [ssid, setSsid] = useState('');
  const [password, setPassword] = useState('');
  const [security, setSecurity] = useState(SECURITY_OPTIONS[0]);
  const [autoconnect, setAutoconnect] = useState(true);
  const [networks, setNetworks] = useState<string[]>([]);

  useEffect(() => {
    getWifi()
      .then((w) => {
        setInfo(w);
        setSsid(w.ssid);
        setSecurity(w.security && SECURITY_OPTIONS.includes(w.security) ? w.security : SECURITY_OPTIONS[0]);
        setAutoconnect(w.autoconnect);
      })
      .catch((err) => pushToast((err as Error).message, 'error'));
  }, []);

  const scan = async () => {
    try {
      const result = await scanWifi();
      setNetworks(result.networks);
      pushToast(`Found ${result.networks.length} network(s)`);
    } catch (err) {
      pushToast((err as Error).message, 'error'); // entered SSID is preserved (state untouched)
    }
  };

  const save = async () => {
    try {
      const result = await saveWifi({ ssid, password, security, autoconnect });
      pushToast(result.message || 'Wi-Fi saved');
      setPassword('');
      setInfo(await getWifi());
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  return (
    <section class="card">
      <h2>Wi-Fi</h2>
      <label class="field"><span>Network (SSID)</span>
        <input type="text" list="wifi-networks" value={ssid} onInput={(e) => setSsid((e.currentTarget as HTMLInputElement).value)} />
        <datalist id="wifi-networks">{networks.map((n) => <option key={n} value={n} />)}</datalist>
      </label>
      <label class="field"><span>Password {info?.password_set ? '(a password is stored)' : ''}</span>
        <input type="password" value={password} placeholder={info?.password_set ? '••••••••' : ''} onInput={(e) => setPassword((e.currentTarget as HTMLInputElement).value)} /></label>
      <div class="field-row">
        <label class="field"><span>Security</span>
          <select value={security} onChange={(e) => setSecurity((e.currentTarget as HTMLSelectElement).value)}>
            {SECURITY_OPTIONS.map((s) => <option key={s} value={s}>{s}</option>)}
          </select></label>
        <label class="switch" style={{ alignSelf: 'end', marginBottom: '14px' }}>
          <input type="checkbox" checked={autoconnect} onChange={(e) => setAutoconnect((e.currentTarget as HTMLInputElement).checked)} />
          <span class="track" /> Autoconnect
        </label>
      </div>
      {info?.connected_ssid && <p class="status-line good">Connected to {info.connected_ssid}{info.address ? ` (${info.address})` : ''}</p>}
      <div class="actions">
        <button class="button" onClick={scan}>Scan</button>
        <button class="button primary" onClick={save}>Save Wi-Fi</button>
      </div>
    </section>
  );
}
