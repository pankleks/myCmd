import fixture from '../test/fixtures/heic.base64?raw';
import { expect, it } from 'vitest';
import { decodeHeic, validateHeicDimensions } from './heicDecode';

it('decodes a real HEVC-compressed HEIC and applies its crop', async () => {
  const pixels = await decodeHeic(
    Uint8Array.from(atob(fixture.trim()), (byte) => byte.charCodeAt(0)),
  );
  expect(pixels.width).toBe(1);
  expect(pixels.height).toBe(1);
  expect([...pixels.data]).toEqual([21, 21, 21, 255]);
});

it('rejects corrupt data and oversized inputs', async () => {
  await expect(decodeHeic(new Uint8Array([1, 2, 3]))).rejects.toThrow(
    'could not be decoded',
  );
  await expect(decodeHeic(new Uint8Array(4 * 1024 * 1024 + 1))).rejects.toThrow(
    '4 MiB',
  );
});

it('checks decoded dimensions before allocating RGBA pixels', () => {
  validateHeicDimensions(4000, 4000);
  for (const [width, height] of [
    [0, 1],
    [1, -1],
    [NaN, 1],
    [1.5, 1],
    [16_385, 1],
    [4001, 4000],
  ])
    expect(() => validateHeicDimensions(width, height)).toThrow();
});
