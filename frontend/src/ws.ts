/**
 * Live LED-frame WebSocket client.
 *
 * Opens a same-origin `/ws` connection and hands each decoded frame (32 `[r,g,b]` triples) to the
 * caller. If the socket closes for any reason it reconnects after ~1 s, so an unplugged device or
 * a service restart transparently resumes the preview (R2.3). Returns a disposer that stops
 * reconnecting and closes the socket.
 */

export type Pixel = [number, number, number];

/**
 * Stream LED frames to `onFrame` until the returned disposer is called.
 * @param onFrame invoked with the panel's 32 pixels for every frame received.
 * @returns a function that permanently closes the stream (no further reconnects).
 */
export function connectFrames(onFrame: (pixels: Pixel[]) => void): () => void {
  let socket: WebSocket | null = null;
  let closed = false;
  let retry: ReturnType<typeof setTimeout> | undefined;

  const open = () => {
    if (closed) return;
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    socket = new WebSocket(`${proto}://${location.host}/ws`);
    socket.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data as string) as { pixels?: Pixel[] };
        if (payload.pixels) onFrame(payload.pixels);
      } catch {
        // ignore malformed frames; the next frame will arrive shortly
      }
    };
    socket.onclose = () => {
      if (!closed) retry = setTimeout(open, 1000);
    };
    socket.onerror = () => socket?.close();
  };

  open();
  return () => {
    closed = true;
    if (retry) clearTimeout(retry);
    socket?.close();
  };
}
