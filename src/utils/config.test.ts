import { describe, expect, it } from 'vitest';
import {
  buildSavePayload,
  DEFAULT_FILE_FONT_SIZE,
  normalizeColumnWeights,
  normalizeConfig,
  normalizeFontSize,
} from './config';

describe('normalizeFontSize', () => {
  it('clamps to the supported range', () => {
    expect(normalizeFontSize(20)).toBe(20);
    expect(normalizeFontSize(99)).toBe(24);
    expect(normalizeFontSize(1)).toBe(12);
  });

  it('falls back to the default for missing values', () => {
    expect(normalizeFontSize(undefined)).toBe(DEFAULT_FILE_FONT_SIZE);
    expect(normalizeFontSize(null)).toBe(DEFAULT_FILE_FONT_SIZE);
    expect(normalizeFontSize('large')).toBe(DEFAULT_FILE_FONT_SIZE);
    expect(normalizeFontSize(NaN)).toBe(DEFAULT_FILE_FONT_SIZE);
  });
});

describe('normalizeColumnWeights', () => {
  it('accepts four finite weights', () => {
    expect(normalizeColumnWeights([175, 78.55, 113, 218])).toEqual([
      175, 78.6, 113, 218,
    ]);
  });

  it('rejects wrong shapes and out-of-range values', () => {
    expect(normalizeColumnWeights([100, 100, 100])).toBeNull();
    expect(normalizeColumnWeights([10, 78, 113, 218])).toBeNull();
    expect(normalizeColumnWeights([175, NaN, 113, 218])).toBeNull();
    expect(normalizeColumnWeights('wide')).toBeNull();
    expect(normalizeColumnWeights(undefined)).toBeNull();
  });
});

describe('normalizeConfig', () => {
  it('merges stored values over defaults', () => {
    expect(
      normalizeConfig({
        leftPath: '  C:\\Users\\root  ',
        rightPath: '',
        columnWeights: { left: [175, 78, 113, 218], right: [1, 2, 3] },
        fileFontSize: 20,
      }),
    ).toEqual({
      leftPath: 'C:\\Users\\root',
      rightPath: null,
      columns: [175, 78, 113, 218],
      fileFontSize: 20,
      showHidden: { left: false, right: false },
    });
  });

  it('normalizes the hidden-files visibility', () => {
    expect(
      normalizeConfig({ showHidden: { left: true, right: 'yes' } }).showHidden,
    ).toEqual({ left: true, right: false });
  });

  it('migrates split panel widths into one shared value', () => {
    expect(
      normalizeConfig({
        columnWeights: { left: null, right: [200, 80, 100, 220] },
      }).columns,
    ).toEqual([200, 80, 100, 220]);
    expect(
      normalizeConfig({ columnWeights: [100, 100, 100, 100] }).columns,
    ).toEqual([100, 100, 100, 100]);
  });

  it('returns defaults for corrupt payloads', () => {
    expect(normalizeConfig(null)).toEqual({
      leftPath: null,
      rightPath: null,
      columns: null,
      fileFontSize: DEFAULT_FILE_FONT_SIZE,
      showHidden: { left: false, right: false },
    });
    expect(normalizeConfig('oops')).toEqual(normalizeConfig(null));
  });

  it('round-trips through the save payload', () => {
    const normalized = normalizeConfig({
      version: 1,
      leftPath: '/home/user',
      rightPath: '/tmp',
      columnWeights: { left: null, right: null },
      fileFontSize: 16,
      showHidden: { left: true, right: false },
    });
    expect(buildSavePayload(normalized)).toEqual({
      version: 1,
      leftPath: '/home/user',
      rightPath: '/tmp',
      columnWeights: { left: null, right: null },
      fileFontSize: 16,
      showHidden: { left: true, right: false },
    });
  });
});
