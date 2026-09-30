import { beforeEach, describe, expect, it, vi } from 'vitest';
import { commander, createPanel, load, rows } from './commander.svelte';
import { preferences } from './preferences.svelte';
import { handleControlShortcut, swapPanels } from './controlShortcuts';
import {
  DEFAULT_FILE_FONT_SIZE,
  MAX_FILE_FONT_SIZE,
  MIN_FILE_FONT_SIZE,
} from '../utils/config';
import type { FileEntry } from '../filesystem/types';

vi.mock('./commander.svelte', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./commander.svelte')>()),
  load: vi.fn().mockResolvedValue(undefined),
}));
function file(name: string, hidden = false): FileEntry {
  return {
    name,
    path: `/files/${name}`,
    type: 'file',
    extension: '',
    size: 0,
    hidden,
    readonly: false,
    directoryTarget: false,
  };
}
function press(key: string) {
  const event = { key, preventDefault: vi.fn() };
  const focusPath = vi.fn();
  return { handled: handleControlShortcut(event, focusPath), event, focusPath };
}
beforeEach(() => {
  vi.clearAllMocks();
  commander.left = createPanel();
  commander.right = createPanel();
  commander.activePanel = 'left';
  preferences.showHidden = false;
  preferences.fileFontSize = DEFAULT_FILE_FONT_SIZE;
});
describe('panel control shortcuts', () => {
  it.each(['left', 'right'] as const)(
    'swaps complete panel state while retaining the active directory from %s',
    (side) => {
      commander.activePanel = side;
      commander.left.path = '/left';
      commander.right.path = '/right';
      commander.left.selected = new Set(['/left/file']);
      commander.left.cursor = 3;
      const left = commander.left;
      const right = commander.right;
      const active = commander[side];
      swapPanels();
      expect(commander.left).toBe(right);
      expect(commander.right).toBe(left);
      expect(commander[commander.activePanel]).toBe(active);
      expect(commander.right.selected.has('/left/file')).toBe(true);
      expect(commander.right.cursor).toBe(3);
      swapPanels();
      expect(commander.left).toBe(left);
      expect(commander.activePanel).toBe(side);
    },
  );
  it('selects visible entries in the active panel without selecting the parent', () => {
    commander.left.path = '/files';
    commander.left.parent = '/';
    commander.left.entries = [file('a'), file('secret', true)];
    const { event, handled } = press('A');
    expect(handled).toBe(true);
    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect([...commander.left.selected]).toEqual(['/files/a']);
    expect(commander.right.selected.size).toBe(0);
  });
  it('toggles hidden entries while retaining cursor identity', () => {
    commander.left.entries = [file('.hidden', true), file('a')];
    press('h');
    expect(preferences.showHidden).toBe(true);
    expect(commander.right.showHidden).toBe(true);
    expect(rows(commander.left)[commander.left.cursor].name).toBe('a');
    press('h');
    expect(preferences.showHidden).toBe(false);
  });
  it.each(['left', 'right'] as const)(
    'refreshes and mirrors from the %s panel',
    (side) => {
      commander.activePanel = side;
      const active = commander[side];
      const other = commander[side === 'left' ? 'right' : 'left'];
      active.path = '/files';
      active.entries = [file('a'), file('b')];
      active.cursor = 1;
      press('r');
      expect(load).toHaveBeenCalledWith(active);
      press('i');
      expect(load).toHaveBeenCalledWith(other, '/files', '/files/b');
    },
  );
  it('does not mirror an unavailable path', () => {
    press('i');
    expect(load).not.toHaveBeenCalled();
  });
  it('delegates path-bar focus to the component', () => {
    const { focusPath } = press('l');
    expect(focusPath).toHaveBeenCalledOnce();
  });
  it.each(['+', '='])(
    'increases font size with %s without exceeding its limit',
    (key) => {
      press(key);
      expect(preferences.fileFontSize).toBe(DEFAULT_FILE_FONT_SIZE + 1);
      preferences.fileFontSize = MAX_FILE_FONT_SIZE;
      press(key);
      expect(preferences.fileFontSize).toBe(MAX_FILE_FONT_SIZE);
    },
  );
  it.each(['-', '_'])(
    'decreases font size with %s without exceeding its limit',
    (key) => {
      preferences.fileFontSize = MAX_FILE_FONT_SIZE;
      press(key);
      expect(preferences.fileFontSize).toBe(MAX_FILE_FONT_SIZE - 1);
      preferences.fileFontSize = MIN_FILE_FONT_SIZE;
      press(key);
      expect(preferences.fileFontSize).toBe(MIN_FILE_FONT_SIZE);
    },
  );
  it('resets font size and leaves unrecognized shortcuts untouched', () => {
    preferences.fileFontSize = MAX_FILE_FONT_SIZE;
    press('0');
    expect(preferences.fileFontSize).toBe(DEFAULT_FILE_FONT_SIZE);
    const { event, handled, focusPath } = press('unknown');
    expect(handled).toBe(false);
    expect(event.preventDefault).not.toHaveBeenCalled();
    expect(focusPath).not.toHaveBeenCalled();
  });
});
