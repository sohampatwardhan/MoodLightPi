/**
 * Root application: the header, the routed view (dashboard at `/`, settings elsewhere), and the
 * global system dialog + toast stack, all inside the store provider.
 *
 * Initial load is a single `GET /api/bootstrap` that seeds light state + the effect list (R11.3);
 * the dashboard then opens the frame WebSocket (in <Preview/>). Health is polled every 15 s and
 * light state at least every 5 s (R3.3) to keep the UI consistent with the device.
 */
import { useEffect, useState } from 'preact/hooks';
import { StoreProvider, useStore } from './store';
import { useRoute } from './router';
import { getBootstrap, getHealth, getState } from './api';
import { Header } from './components/Header';
import { Dashboard } from './components/Dashboard';
import { SettingsLayout } from './components/SettingsLayout';
import { SystemDialog } from './components/SystemDialog';
import { Toast } from './components/Toast';

function Shell() {
  const { setLight, setEffects, setOnline } = useStore();
  const { path } = useRoute();
  const [systemOpen, setSystemOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    getBootstrap()
      .then((boot) => {
        if (!alive) return;
        setLight(boot.state);
        setEffects(boot.effects);
        setOnline(true);
      })
      .catch(() => alive && setOnline(false));

    const health = setInterval(() => {
      getHealth().then(() => setOnline(true)).catch(() => setOnline(false));
    }, 15000);
    const state = setInterval(() => {
      getState().then((r) => { setLight(r.state); setOnline(true); }).catch(() => setOnline(false));
    }, 5000);

    return () => {
      alive = false;
      clearInterval(health);
      clearInterval(state);
    };
  }, []);

  return (
    <div class="app-shell">
      <Header onOpenSystem={() => setSystemOpen(true)} />
      {path === '/' ? <Dashboard /> : <SettingsLayout />}
      <SystemDialog open={systemOpen} onClose={() => setSystemOpen(false)} />
      <Toast />
    </div>
  );
}

/** App root — wires the store provider around the shell. */
export function App() {
  return (
    <StoreProvider>
      <Shell />
    </StoreProvider>
  );
}
