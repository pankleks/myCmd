import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from '../filesystem/api';
import type { FileEntry } from '../filesystem/types';
import { DEFAULT_FILE_FONT_SIZE } from '../utils/config';
import { commander, createPanel, rows } from './commander.svelte';
import {
  loadPreferences,
  persistPreferences,
  preferences,
  scheduleSave,
  setShowHidden,
} from './preferences.svelte';

vi.mock('../filesystem/api', () => ({
  api: { loadConfig: vi.fn(), saveConfig: vi.fn() },
  errorMessage: (error: { message: string }) => error.message,
}));

const loadConfig = vi.mocked(api.loadConfig);
const saveConfig = vi.mocked(api.saveConfig);

beforeEach(() => {
  vi.resetAllMocks();
  vi.useRealTimers();
  preferences.fileFontSize = DEFAULT_FILE_FONT_SIZE;
  preferences.columnWidths = null;
  preferences.showHidden = false;
  preferences.showFunctionBar = true;
  preferences.editor = null;
  commander.left.path = '';
  commander.right.path = '';
  commander.left.showHidden = false;
  commander.right.showHidden = false;
  commander.left = createPanel();
  commander.right = createPanel();
});

describe('loadPreferences', () => {
  it('loads the configured editor and persists it', async () => {
    loadConfig.mockResolvedValue({
      version: 1,
      editor: '/opt/My Editor/editor',
    });
    await loadPreferences();
    expect(preferences.editor).toBe('/opt/My Editor/editor');
    await persistPreferences();
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        editor: '/opt/My Editor/editor',
      }),
    );
  });
  it('applies stored paths, columns and font size', async () => {
    loadConfig.mockResolvedValue({
      version: 1,
      leftPath: 'C:\\Users\\root',
      rightPath: '/tmp',
      columnWeights: { left: [175, 78, 113, 218], right: null },
      fileFontSize: 20,
      showHidden: true,
      showFunctionBar: false,
    });
    const paths = await loadPreferences();
    expect(paths).toEqual({
      leftPath: 'C:\\Users\\root',
      rightPath: '/tmp',
    });
    expect(preferences.fileFontSize).toBe(20);
    expect(preferences.columnWidths).toEqual([175, 78, 113, 218]);
    expect(preferences.showHidden).toBe(true);
    expect(preferences.showFunctionBar).toBe(false);
    expect(commander.left.showHidden).toBe(true);
    expect(commander.right.showHidden).toBe(true);
  });

  it('ignores legacy per-panel hidden flags', async () => {
    loadConfig.mockResolvedValue({
      version: 1,
      showHidden: { left: true, right: false },
    });
    await loadPreferences();
    expect(preferences.showHidden).toBe(false);
    expect(commander.left.showHidden).toBe(false);
    expect(commander.right.showHidden).toBe(false);
  });

  it('prefers the left panel when migrating split widths', async () => {
    loadConfig.mockResolvedValue({
      version: 1,
      leftPath: null,
      rightPath: null,
      columnWeights: { left: null, right: [200, 80, 100, 220] },
      fileFontSize: null,
    });
    await loadPreferences();
    expect(preferences.columnWidths).toEqual([200, 80, 100, 220]);
  });

  it('falls back to defaults when the backend is unavailable', async () => {
    loadConfig.mockRejectedValue(new Error('no backend'));
    const paths = await loadPreferences();
    expect(paths).toEqual({ leftPath: null, rightPath: null });
    expect(preferences.fileFontSize).toBe(DEFAULT_FILE_FONT_SIZE);
    expect(preferences.columnWidths).toBeNull();
  });
});

describe('persistPreferences', () => {
  it('saves paths, columns and font size', async () => {
    commander.left.path = 'C:\\Users\\root';
    commander.right.path = 'D:\\Backup';
    preferences.showHidden = true;
    preferences.showFunctionBar = false;
    preferences.fileFontSize = 16;
    preferences.columnWidths = [200, 80, 100, 220];
    await persistPreferences();
    expect(saveConfig).toHaveBeenCalledWith({
      pinnedDirectories: [],
      editor: null,
      version: 1,
      leftPath: 'C:\\Users\\root',
      rightPath: 'D:\\Backup',
      columnWeights: { left: [200, 80, 100, 220], right: [200, 80, 100, 220] },
      fileFontSize: 16,
      showHidden: true,
      showFunctionBar: false,
    });
  });

  it('ignores save failures', async () => {
    saveConfig.mockRejectedValue(new Error('disk full'));
    await expect(persistPreferences()).resolves.toBeUndefined();
  });
});

describe('setShowHidden', () => {
  function file(name: string, hidden = false): FileEntry {
    return {
      name,
      path: `/files/${name}`,
      type: 'file',
      extension: 'txt',
      size: 0,
      hidden,
      readonly: false,
      directoryTarget: false,
    };
  }

  it('preserves cursor identity as hidden rows are inserted and removed', () => {
    for (const panel of [commander.left, commander.right]) {
      panel.entries = [file('.secret', true), file('a'), file('b')];
      panel.cursor = 1;
    }
    setShowHidden(true);
    for (const panel of [commander.left, commander.right]) {
      expect(rows(panel)[panel.cursor].name).toBe('b');
      expect(panel.cursor).toBe(2);
    }
    setShowHidden(false);
    for (const panel of [commander.left, commander.right]) {
      expect(rows(panel)[panel.cursor].name).toBe('b');
      expect(panel.cursor).toBe(1);
    }
  });

  it('chooses a valid fallback when the current file becomes hidden', () => {
    commander.left.entries = [file('.secret', true), file('a')];
    setShowHidden(true);
    commander.left.cursor = 0;
    setShowHidden(false);
    expect(rows(commander.left)[commander.left.cursor].name).toBe('a');
  });
  it('syncs both panels and prunes the selection', async () => {
    const file = (name: string, path: string, hidden = false): FileEntry => ({
      name,
      path,
      type: 'file',
      extension: 'txt',
      size: 0,
      hidden,
      readonly: false,
      directoryTarget: false,
    });
    commander.left.entries = [
      file('a', '/left/a'),
      file('.secret', '/left/.secret', true),
    ];
    commander.left.selected = new Set(['/left/a', '/left/.secret']);
    commander.left.cursor = 5;

    setShowHidden(false);
    expect(preferences.showHidden).toBe(false);
    expect(commander.left.showHidden).toBe(false);
    expect(commander.right.showHidden).toBe(false);
    expect(commander.left.selected).toEqual(new Set(['/left/a']));
    expect(commander.left.cursor).toBeLessThanOrEqual(1);

    setShowHidden(true);
    expect(preferences.showHidden).toBe(true);
    expect(commander.left.showHidden).toBe(true);
    expect(commander.right.showHidden).toBe(true);
  });
});

describe('scheduleSave', () => {
  it('debounces rapid changes into a single write', async () => {
    vi.useFakeTimers();
    scheduleSave();
    scheduleSave();
    expect(saveConfig).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(300);
    expect(saveConfig).toHaveBeenCalledTimes(1);
  });
});
