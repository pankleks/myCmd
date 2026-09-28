import { describe, expect, it } from 'vitest';
import { isQuickFindTrigger } from './keyboard';

describe('isQuickFindTrigger', () => {
  it('accepts the left Alt with a letter', () => {
    expect(
      isQuickFindTrigger({
        key: 'd',
        altKey: true,
        metaKey: false,
        shiftKey: false,
        altGraph: false,
      }),
    ).toBe(true);
  });

  it('accepts AltGraph without the altKey flag', () => {
    expect(
      isQuickFindTrigger({
        key: 'd',
        altKey: false,
        metaKey: false,
        shiftKey: false,
        altGraph: true,
      }),
    ).toBe(true);
  });

  it('accepts diacritics and shifted AltGraph letters', () => {
    expect(
      isQuickFindTrigger({
        key: 'ą',
        altKey: false,
        metaKey: false,
        shiftKey: false,
        altGraph: true,
      }),
    ).toBe(true);
    expect(
      isQuickFindTrigger({
        key: 'Ź',
        altKey: false,
        metaKey: false,
        shiftKey: true,
        altGraph: true,
      }),
    ).toBe(true);
  });

  it('rejects plain typing and control keys', () => {
    const base = {
      altKey: false,
      metaKey: false,
      shiftKey: false,
      altGraph: false,
    };
    expect(isQuickFindTrigger({ ...base, key: 'd' })).toBe(false);
    expect(isQuickFindTrigger({ ...base, key: 'Escape', altKey: true })).toBe(
      false,
    );
    expect(isQuickFindTrigger({ ...base, key: ' ', altKey: true })).toBe(false);
    expect(
      isQuickFindTrigger({ ...base, key: 'd', altKey: true, metaKey: true }),
    ).toBe(false);
    expect(
      isQuickFindTrigger({ ...base, key: 'd', altKey: true, shiftKey: true }),
    ).toBe(false);
  });
});
