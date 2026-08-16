/** Dashboard view: the live preview plus the colour, brightness, and effect controls. */
import { Preview } from './Preview';
import { ColorPanel } from './ColorPanel';
import { BrightnessPanel } from './BrightnessPanel';
import { EffectPanel } from './EffectPanel';

export function Dashboard() {
  return (
    <main class="dashboard">
      <Preview />
      <ColorPanel />
      <BrightnessPanel />
      <EffectPanel />
    </main>
  );
}
