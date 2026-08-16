/** Effect selector + speed slider. The effect list comes from the device (R1.6); choosing an
 * effect posts it with the current speed (R1.3). The speed control is hidden while the effect is
 * `solid` (R1.4) and its dragging is debounced to ≤1 send / 80 ms idle (R1.5). */
import { useEffect, useRef, useState } from 'preact/hooks';
import { setEffect, getState } from '../api';
import { useStore } from '../store';

export function EffectPanel() {
  const { light, effects, setLight, pushToast } = useStore();
  const current = light?.effect_name ?? 'solid';
  const deviceSpeed = light?.speed ?? 128;
  const [speed, setSpeed] = useState(deviceSpeed);
  const dragging = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    if (!dragging.current) setSpeed(deviceSpeed);
  }, [deviceSpeed]);

  const applyEffect = async (name: string, nextSpeed?: number) => {
    try {
      await setEffect(name, nextSpeed);
      const next = await getState();
      setLight(next.state);
    } catch (err) {
      pushToast((err as Error).message, 'error');
    }
  };

  const onSpeedInput = (value: number) => {
    dragging.current = true;
    setSpeed(value);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      await applyEffect(current, value);
      dragging.current = false;
    }, 80);
  };

  return (
    <section class="card">
      <h2>Effect</h2>
      <label class="field">
        <span>Pattern</span>
        <select
          value={current}
          onChange={(e) => applyEffect((e.currentTarget as HTMLSelectElement).value, speed)}
        >
          {effects.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      </label>
      {current !== 'solid' && (
        <div class="slider-row">
          <input
            type="range"
            min={0}
            max={255}
            value={speed}
            aria-label="Speed"
            onInput={(e) => onSpeedInput(Number((e.currentTarget as HTMLInputElement).value))}
          />
          <output>{speed}</output>
        </div>
      )}
    </section>
  );
}
