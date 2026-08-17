/**
 * Pure, side-effect-free helpers shared across components (and unit-tested in 3.3):
 * colour hex↔rgb conversion and brightness 0-255 ↔ 0-100 % mapping, mirroring the device's
 * own rounding so the UI and device agree.
 */
import type { Rgb } from './types';

const clampByte = (n: number): number => Math.max(0, Math.min(255, Math.round(n)));

/** Parse `#rrggbb` (or `rrggbb`) into an {r,g,b}; invalid input yields black. */
export function hexToRgb(hex: string): Rgb {
  const h = hex.trim().replace(/^#/, '');
  if (h.length !== 6 || /[^0-9a-fA-F]/.test(h)) return { r: 0, g: 0, b: 0 };
  return {
    r: parseInt(h.slice(0, 2), 16),
    g: parseInt(h.slice(2, 4), 16),
    b: parseInt(h.slice(4, 6), 16),
  };
}

/** Format an {r,g,b} as `#rrggbb`. */
export function rgbToHex({ r, g, b }: Rgb): string {
  const hex = (n: number) => clampByte(n).toString(16).padStart(2, '0');
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}

/** Map a 0-100 % brightness to the device's 0-255 scale. */
export function percentToBrightness(percent: number): number {
  return clampByte((Math.max(0, Math.min(100, percent)) * 255) / 100);
}

/** Map a device 0-255 brightness to a 0-100 % display value. */
export function brightnessToPercent(value: number): number {
  return Math.round((clampByte(value) * 100) / 255);
}

/** The 5 preset colour swatches offered on the dashboard (parity with the previous UI). */
export const PRESET_SWATCHES: string[] = ['#ffffff', '#38bdf8', '#f472b6', '#f59e0b', '#34d399'];
