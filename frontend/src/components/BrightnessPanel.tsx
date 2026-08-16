/** Brightness slider shown as 0-100 %, mapped to the device's 0-255 scale. Continuous dragging is
 * debounced to at most one send per 80 ms of inactivity so the device isn't flooded (R1.2, R1.5). */
import { useEffect, useRef, useState } from 'preact/hooks';
import { setBrightness, getState } from '../api';
import { useStore } from '../store';
import { brightnessToPercent, percentToBrightness } from '../lib';

export function BrightnessPanel() {
  const { light, setLight, pushToast } = useStore();
  const devicePercent = light ? brightnessToPercent(light.brightness) : 0;
  const [percent, setPercent] = useState(devicePercent);
  const dragging = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout>>();

  // Track the device value except while the user is actively dragging.
  useEffect(() => {
    if (!dragging.current) setPercent(devicePercent);
  }, [devicePercent]);

  const onInput = (value: number) => {
    dragging.current = true;
    setPercent(value);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      try {
        await setBrightness(percentToBrightness(value));
        const next = await getState();
        setLight(next.state);
      } catch (err) {
        pushToast((err as Error).message, 'error');
      } finally {
        dragging.current = false;
      }
    }, 80);
  };

  return (
    <section class="card">
      <h2>Brightness</h2>
      <div class="slider-row">
        <input
          type="range"
          min={0}
          max={100}
          value={percent}
          aria-label="Brightness"
          onInput={(e) => onInput(Number((e.currentTarget as HTMLInputElement).value))}
        />
        <output>{percent}%</output>
      </div>
    </section>
  );
}
