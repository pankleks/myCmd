import { describe, expect, it } from 'vitest';
import { resizeAdjacentColumns, resizeColumn } from './resizeColumns';

describe('resizeAdjacentColumns', () => {
  it('resizes the adjacent pair while preserving the panel width', () => {
    expect(
      resizeAdjacentColumns([200, 80, 100, 220], [80, 46, 72, 170], 1, 24),
    ).toEqual([200, 104, 76, 220]);
    expect(
      resizeAdjacentColumns(
        [200, 80, 100, 220],
        [80, 46, 72, 170],
        1,
        24,
      ).reduce((a, b) => a + b),
    ).toBe(600);
  });

  it('does not shrink either adjacent column below its minimum', () => {
    expect(resizeAdjacentColumns([90, 50, 100], [80, 46, 72], 0, 100)).toEqual([
      94, 46, 100,
    ]);
    expect(resizeAdjacentColumns([90, 50, 100], [80, 46, 72], 0, -100)).toEqual(
      [80, 60, 100],
    );
  });

  it('leaves other columns unchanged and rejects a divider without a neighbor', () => {
    expect(resizeAdjacentColumns([100, 100, 100], [50, 50, 50], 1, 10)).toEqual(
      [100, 110, 90],
    );
    expect(() => resizeAdjacentColumns([100], [50], 0, 10)).toThrow(RangeError);
  });
});

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
