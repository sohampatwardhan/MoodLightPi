/** Modal system-power dialog: restart-service / reboot / poweroff (R9.1). Cancelling or pressing
 * Escape closes it without issuing any action (R9.3). Confirming requests the action and reports
 * whether it was queued. */
import { useEffect } from 'preact/hooks';
import { systemAction } from '../api';
import { useStore } from '../store';
import type { SystemActionName } from '../types';

export function SystemDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { pushToast } = useStore();

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;

  const run = async (action: SystemActionName) => {
    try {
      const result = await systemAction(action);
      pushToast(result.queued ? `Queued: ${action.replace('_', ' ')}` : `Requested: ${action}`);
    } catch (err) {
      pushToast((err as Error).message, 'error');
    } finally {
      onClose();
    }
  };

  return (
    <div class="dialog-backdrop" onClick={onClose}>
      <div class="dialog" role="dialog" aria-modal="true" aria-label="System power" onClick={(e) => e.stopPropagation()}>
        <h2>System power</h2>
        <p class="hint">These affect the device itself, not the light.</p>
        <div class="dialog-actions">
          <button class="button" onClick={() => run('restart_service')}>Restart service</button>
          <button class="button" onClick={() => run('reboot')}>Reboot device</button>
          <button class="button danger" onClick={() => run('poweroff')}>Power off</button>
          <button class="button" onClick={onClose}>Cancel</button>
        </div>
      </div>
    </div>
  );
}
