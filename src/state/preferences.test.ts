import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from '../filesystem/api';
import { DEFAULT_FILE_FONT_SIZE } from '../utils/config';
import { commander } from './commander.svelte';
import {
  loadPreferences,
  persistPreferences,
  preferences,
  scheduleSave,
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
  preferences.columns = { left: null, right: null };
  commander.left.path = '';
  commander.right.path = '';
});

describe('loadPreferences', () => {
  it('applies stored paths, columns and font size', async () => {
    loadConfig.mockResolvedValue({
      version: 1,
      leftPath: 'C:\\Users\\root',
      rightPath: '/tmp',
      columnWeights: { left: [175, 78, 113, 218], right: null },
      fileFontSize: 20,
    });
    const paths = await loadPreferences();
    expect(paths).toEqual({
      leftPath: 'C:\\Users\\root',
      rightPath: '/tmp',
    });
    expect(preferences.fileFontSize).toBe(20);
    expect(preferences.columns.left).toEqual([175, 78, 113, 218]);
  });

  it('falls back to defaults when the backend is unavailable', async () => {
    loadConfig.mockRejectedValue(new Error('no backend'));
    const paths = await loadPreferences();
    expect(paths).toEqual({ leftPath: null, rightPath: null });
    expect(preferences.fileFontSize).toBe(DEFAULT_FILE_FONT_SIZE);
    expect(preferences.columns).toEqual({ left: null, right: null });
  });
});

describe('persistPreferences', () => {
  it('saves paths, columns and font size', async () => {
    commander.left.path = 'C:\\Users\\root';
    commander.right.path = 'D:\\Backup';
    preferences.fileFontSize = 16;
    preferences.columns = { left: [200, 80, 100, 220], right: null };
    await persistPreferences();
    expect(saveConfig).toHaveBeenCalledWith({
      version: 1,
      leftPath: 'C:\\Users\\root',
      rightPath: 'D:\\Backup',
      columnWeights: { left: [200, 80, 100, 220], right: null },
      fileFontSize: 16,
    });
  });

  it('ignores save failures', async () => {
    saveConfig.mockRejectedValue(new Error('disk full'));
    await expect(persistPreferences()).resolves.toBeUndefined();
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
