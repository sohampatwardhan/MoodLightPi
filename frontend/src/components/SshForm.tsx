/** SSH authorized-keys editor: view, validate, and save keys (R8). Validation and save errors from
 * the server (invalid key material) surface to the user. */
import { useEffect, useState } from 'preact/hooks';
import { getSshKeys, saveSshKeys, validateSshKeys } from '../api';
import { useStore } from '../store';

const keyCount = (text: string) =>
  text.split('\n').filter((line) => line.trim() && !line.trim().startsWith('#')).length;

export function SshForm() {
  const { pushToast } = useStore();
  const [keys, setKeys] = useState<string | null>(null);

  useEffect(() => {
    getSshKeys().then(setKeys).catch((err) => pushToast((err as Error).message, 'error'));
  }, []);

  if (keys === null) return <section class="card"><p class="hint">Loading…</p></section>;

  const validate = async () => {
    try {
      await validateSshKeys(keys);
      pushToast('Keys are valid');
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  const save = async () => {
    try {
      await saveSshKeys(keys);
      setKeys(await getSshKeys());
      pushToast('Authorized keys saved');
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  return (
    <section class="card">
      <h2>SSH authorized keys</h2>
      <p class="hint">One key per line. {keyCount(keys)} key(s).</p>
      <textarea value={keys} spellcheck={false} onInput={(e) => setKeys((e.currentTarget as HTMLTextAreaElement).value)} />
      <div class="actions">
        <button class="button" onClick={validate}>Validate</button>
        <button class="button primary" onClick={save}>Save keys</button>
      </div>
    </section>
  );
}
