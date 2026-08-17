// Vite build config for the MoodLightPi frontend.
//
// Embed contract: the Rust binary serves the UI via `rust-embed` pointed at the
// repo-level `web-dist/` directory (see src/web.rs). We therefore build OUT OF the
// `frontend/` project INTO `../web-dist` and set `emptyOutDir: true` so each build
// fully replaces the previous committed bundle (Vite otherwise refuses to clean an
// outDir outside the project root). `base: '/'` because the SPA is served from the
// site root on the device — no sub-path rewriting. The committed `web-dist/` is what
// the Pi (which has no Node toolchain) compiles into the binary.
import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';

export default defineConfig({
  plugins: [preact()],
  base: '/',
  build: {
    outDir: '../web-dist',
    emptyOutDir: true,
  },
});
