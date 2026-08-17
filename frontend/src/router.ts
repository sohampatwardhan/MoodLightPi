/**
 * Minimal History-API client router.
 *
 * The UI is a single page whose visible view is a pure function of the URL path, so routing is
 * just "read the current path" (`useRoute`) plus "change it" (`navigate`). Hand-rolled rather than
 * pulling in a routing library, to keep the dependency surface to `preact` alone (R15 footprint).
 * `navigate` emits a `popstate` so `useRoute` subscribers re-render on both programmatic
 * navigation and the browser back/forward buttons (R10.3).
 */
import { useEffect, useState } from 'preact/hooks';

/** Subscribe to the current pathname; re-renders on `navigate`, back, and forward. */
export function useRoute(): { path: string } {
  const [path, setPath] = useState(() => location.pathname);
  useEffect(() => {
    const onChange = () => setPath(location.pathname);
    window.addEventListener('popstate', onChange);
    return () => window.removeEventListener('popstate', onChange);
  }, []);
  return { path };
}

/** Navigate to `path` via `pushState` and notify `useRoute` subscribers. */
export function navigate(path: string): void {
  if (path === location.pathname) return;
  history.pushState(null, '', path);
  window.dispatchEvent(new PopStateEvent('popstate'));
}
