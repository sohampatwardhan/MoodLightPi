/** Live LED preview: subscribes to the frame WebSocket and paints the panel's 32 pixels onto an
 * 8×4 canvas, reversed on both axes to match the physical panel orientation (R2.1, R2.2). The
 * subscription auto-reconnects (R2.3, handled in ws.ts). */
import { useEffect, useRef } from 'preact/hooks';
import { connectFrames, type Pixel } from '../ws';
import { useStore } from '../store';

const COLS = 8;
const ROWS = 4;

export function Preview() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const { light } = useStore();

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const draw = (pixels: Pixel[]) => {
      const cw = canvas.width / COLS;
      const ch = canvas.height / ROWS;
      for (let i = 0; i < COLS * ROWS && i < pixels.length; i++) {
        const [r, g, b] = pixels[i];
        // Reverse both axes so the preview matches the physical panel.
        const x = COLS - 1 - (i % COLS);
        const y = ROWS - 1 - Math.floor(i / COLS);
        ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;
        ctx.fillRect(x * cw, y * ch, cw, ch);
      }
    };

    return connectFrames(draw);
  }, []);

  return (
    <section class="card preview-card">
      <canvas ref={canvasRef} class="preview-canvas" width={640} height={320} aria-label="LED preview" />
      <div class="preview-labels">
        <span>
          Mode <strong>{light?.mode ?? '—'}</strong>
        </span>
        <span>
          Effect <strong>{light?.effect_name ?? '—'}</strong>
        </span>
      </div>
    </section>
  );
}
