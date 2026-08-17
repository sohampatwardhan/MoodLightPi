/**
 * App-wide reactive state: live light state, the effect list, device reachability, and a toast
 * queue.
 *
 * A single Preact context owns everything the dashboard and header need to share, so components
 * read/update device state without prop-drilling. The store is intentionally small (plain hooks,
 * no external state library) to hold the R15 footprint budget. Toasts are transient user feedback
 * (R16): `pushToast` appends one and auto-dismisses it after a short delay.
 */
import { createContext, type ComponentChildren } from 'preact';
import { useCallback, useContext, useMemo, useState } from 'preact/hooks';
import type { LightState } from './types';

export type ToastKind = 'success' | 'error';
export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

export interface Store {
  /** Latest known light state, or null before the first load. */
  light: LightState | null;
  /** Effect names advertised by the device. */
  effects: string[];
  /** Whether the device is currently reachable (health checks succeeding). */
  online: boolean;
  /** Active transient notifications. */
  toasts: Toast[];
  setLight(light: LightState): void;
  setEffects(effects: string[]): void;
  setOnline(online: boolean): void;
  /** Show a transient message; auto-dismisses after ~2.6 s. */
  pushToast(message: string, kind?: ToastKind): void;
  dismissToast(id: number): void;
}

const StoreContext = createContext<Store | null>(null);

let nextToastId = 1;

export function StoreProvider({ children }: { children: ComponentChildren }) {
  const [light, setLight] = useState<LightState | null>(null);
  const [effects, setEffects] = useState<string[]>([]);
  const [online, setOnline] = useState(false);
  const [toasts, setToasts] = useState<Toast[]>([]);

  const dismissToast = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const pushToast = useCallback(
    (message: string, kind: ToastKind = 'success') => {
      const id = nextToastId++;
      setToasts((current) => [...current, { id, message, kind }]);
      setTimeout(() => dismissToast(id), 2600);
    },
    [dismissToast],
  );

  const store = useMemo<Store>(
    () => ({
      light,
      effects,
      online,
      toasts,
      setLight,
      setEffects,
      setOnline,
      pushToast,
      dismissToast,
    }),
    [light, effects, online, toasts, pushToast, dismissToast],
  );

  return <StoreContext.Provider value={store}>{children}</StoreContext.Provider>;
}

/** Access the app store; must be used within a {@link StoreProvider}. */
export function useStore(): Store {
  const store = useContext(StoreContext);
  if (!store) throw new Error('useStore must be used within a StoreProvider');
  return store;
}
