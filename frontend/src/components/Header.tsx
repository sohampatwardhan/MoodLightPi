/** Top header: brand (home), reachability pill, settings link, and a system-power button that
 * opens the power dialog (restart/reboot/poweroff) — not a light toggle, matching the prior UI. */
import { navigate } from '../router';
import { StatusPill } from './StatusPill';

export function Header({ onOpenSystem }: { onOpenSystem: () => void }) {
  return (
    <header class="top-pane">
      <a
        class="brand-home"
        href="/"
        onClick={(e) => {
          e.preventDefault();
          navigate('/');
        }}
      >
        <span class="brand-dot" />
        MoodLightPi
      </a>
      <span class="top-spacer" />
      <StatusPill />
      <button
        class="icon-button"
        onClick={() => navigate('/identity')}
        aria-label="Settings"
      >
        Settings
      </button>
      <button class="icon-button" onClick={onOpenSystem} aria-label="System power">
        Power
      </button>
    </header>
  );
}
