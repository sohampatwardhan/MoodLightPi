import { describe, it, expect, vi, afterEach } from 'vitest';
import { getState, setColor } from '../api';

afterEach(() => vi.unstubAllGlobals());

function stubFetch(status: number, body: unknown) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () =>
      new Response(body === undefined ? '' : JSON.stringify(body), {
        status,
        headers: { 'content-type': 'application/json' },
      }),
    ),
  );
}

describe('api client', () => {
  it('rejects with the server error message on non-2xx', async () => {
    stubFetch(400, { error: 'availability topic is required' });
    await expect(setColor({ r: 1, g: 2, b: 3 })).rejects.toThrow('availability topic is required');
  });

  it('falls back to status text when no error field', async () => {
    stubFetch(503, undefined);
    await expect(getState()).rejects.toThrow(/503/);
  });

  it('parses a 2xx JSON body', async () => {
    stubFetch(200, { state: { power: true }, seq: 7 });
    await expect(getState()).resolves.toEqual({ state: { power: true }, seq: 7 });
  });
});
