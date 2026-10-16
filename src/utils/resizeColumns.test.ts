import { describe, expect, it } from 'vitest';
import { resizeColumn } from './resizeColumns';

describe('resizeColumn', () => {
  it('grows a single column so truncated text can be revealed', () => {
    expect(resizeColumn([200, 80, 100, 220], [80, 46, 72, 170], 0, 32)).toEqual(
      [232, 80, 100, 220],
    );
  });

  it('does not shrink a column below its minimum', () => {
    expect(resizeColumn([90, 50, 100], [80, 46, 72], 0, -100)).toEqual([
      80, 50, 100,
    ]);
  });

  it('leaves other columns unchanged and rejects an unknown column', () => {
    expect(resizeColumn([100, 100, 100], [50, 50, 50], 1, 10)).toEqual([
      100, 110, 100,
    ]);
    expect(() => resizeColumn([100], [50], 1, 10)).toThrow(RangeError);
  });
});
