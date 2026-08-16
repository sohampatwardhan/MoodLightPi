/** Device reachability indicator: green "Active" when health checks pass, red "Offline" otherwise
 * (R3.1, R3.2). Status is conveyed by text, not colour alone, for accessibility. */
import { useStore } from '../store';

export function StatusPill() {
  const { online } = useStore();
  return (
    <span class={`status-pill ${online ? 'online' : 'offline'}`} role="status" aria-live="polite">
      <span class="dot" />
      <span>{online ? 'Active' : 'Offline'}</span>
    </span>
  );
}
