/** Identity settings: device name, room, and light label (R4). A save with a non-empty device
 * name and light label persists; the server rejects empty required fields and the error surfaces. */
import { useEffect, useState } from 'preact/hooks';
import { getIdentity, saveIdentity } from '../api';
import { useStore } from '../store';
import type { IdentitySettings } from '../types';

export function IdentityForm() {
  const { pushToast } = useStore();
  const [form, setForm] = useState<IdentitySettings | null>(null);

  useEffect(() => {
    getIdentity()
      .then(setForm)
      .catch((err) => pushToast((err as Error).message, 'error'));
  }, []);

  if (!form) return <section class="card"><p class="hint">Loading…</p></section>;

  const field = (key: keyof IdentitySettings) => (e: Event) =>
    setForm({ ...form, [key]: (e.currentTarget as HTMLInputElement).value });

  const save = async () => {
    try {
      setForm(await saveIdentity(form));
      pushToast('Identity saved');
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  return (
    <section class="card">
      <h2>Identity</h2>
      <label class="field"><span>Device name</span>
        <input type="text" value={form.device_name} onInput={field('device_name')} /></label>
      <label class="field"><span>Room</span>
        <input type="text" value={form.room} onInput={field('room')} /></label>
      <label class="field"><span>Light label</span>
        <input type="text" value={form.light_label} onInput={field('light_label')} /></label>
      <div class="actions"><button class="button primary" onClick={save}>Save identity</button></div>
    </section>
  );
}
