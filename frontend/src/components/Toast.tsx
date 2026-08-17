/** Transient notification stack (success/failure feedback, R16). Tap to dismiss; each also
 * auto-dismisses (see the store). */
import { useStore } from '../store';

export function Toast() {
  const { toasts, dismissToast } = useStore();
  if (toasts.length === 0) return null;
  return (
    <div class="toast-stack" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} class={`toast ${toast.kind}`} onClick={() => dismissToast(toast.id)}>
          {toast.message}
        </div>
      ))}
    </div>
  );
}
