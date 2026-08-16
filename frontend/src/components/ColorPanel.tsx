/** Colour control: a colour picker plus 5 preset swatches. Each change posts the RGB to the
 * device and refreshes state so the UI reflects what was applied (R1.1). */
import { setColor, getState } from '../api';
import { useStore } from '../store';
import { hexToRgb, rgbToHex, PRESET_SWATCHES } from '../lib';
import type { Rgb } from '../types';

export function ColorPanel() {
  const { light, setLight, pushToast } = useStore();
  const value = light ? rgbToHex(light.rgb) : '#ffffff';

  const apply = async (rgb: Rgb) => {
    try {
      await setColor(rgb);
      const next = await getState();
      setLight(next.state);
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  return (
    <section class="card">
      <h2>Colour</h2>
      <p class="hint">Pick a colour or tap a preset.</p>
      <div class="color-row">
        <input
          type="color"
          value={value}
          aria-label="Colour"
          onInput={(e) => apply(hexToRgb((e.currentTarget as HTMLInputElement).value))}
        />
        <span class="color-value">{value}</span>
      </div>
      <div class="swatches">
        {PRESET_SWATCHES.map((swatch) => (
          <button
            key={swatch}
            class="swatch"
            style={{ background: swatch }}
            aria-label={`Preset ${swatch}`}
            onClick={() => apply(hexToRgb(swatch))}
          />
        ))}
      </div>
    </section>
  );
}
