import { describe, it, expect } from 'vitest';
import { hexToRgb, rgbToHex, brightnessToPercent, percentToBrightness } from '../lib';
import { decodeFrame } from '../ws';

describe('colour helpers', () => {
  it('round-trips hex ↔ rgb', () => {
    expect(hexToRgb('#38bdf8')).toEqual({ r: 0x38, g: 0xbd, b: 0xf8 });
    expect(rgbToHex({ r: 0x38, g: 0xbd, b: 0xf8 })).toBe('#38bdf8');
    expect(rgbToHex(hexToRgb('#ffffff'))).toBe('#ffffff');
  });
  it('treats malformed hex as black', () => {
    expect(hexToRgb('nope')).toEqual({ r: 0, g: 0, b: 0 });
  });
});

describe('brightness mapping', () => {
  it('maps percent ↔ 0-255 with the device rounding', () => {
    expect(percentToBrightness(0)).toBe(0);
    expect(percentToBrightness(100)).toBe(255);
    expect(percentToBrightness(50)).toBe(128);
    expect(brightnessToPercent(255)).toBe(100);
    expect(brightnessToPercent(0)).toBe(0);
  });
  it('clamps out-of-range input', () => {
    expect(percentToBrightness(150)).toBe(255);
    expect(percentToBrightness(-10)).toBe(0);
  });
});

describe('frame decoding', () => {
  it('decodes a 32-pixel frame', () => {
    const pixels = Array.from({ length: 32 }, (_, i) => [i, i, i]);
    const frame = decodeFrame(JSON.stringify({ pixels }));
    expect(frame).toHaveLength(32);
    expect(frame[0]).toEqual([0, 0, 0]);
    expect(frame[31]).toEqual([31, 31, 31]);
  });
  it('returns [] for malformed input', () => {
    expect(decodeFrame('not json')).toEqual([]);
    expect(decodeFrame('{}')).toEqual([]);
  });
});
