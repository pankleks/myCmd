import { api } from '../filesystem/api';
import {
  buildSavePayload,
  DEFAULT_FILE_FONT_SIZE,
  normalizeConfig,
} from '../utils/config';
import { displayPath } from '../utils/paths';
import { commander } from './commander.svelte';

export interface SavedPaths {
  leftPath: string | null;
  rightPath: string | null;
}

export const preferences = $state({
  fileFontSize: DEFAULT_FILE_FONT_SIZE,
  columnWidths: null as number[] | null,
});

export async function loadPreferences(): Promise<SavedPaths> {
  let raw: unknown = null;
  try {
    raw = await api.loadConfig();
  } catch {
    raw = null;
  }
  const normalized = normalizeConfig(raw);
  preferences.fileFontSize = normalized.fileFontSize;
  preferences.columnWidths = normalized.columns;
  return { leftPath: normalized.leftPath, rightPath: normalized.rightPath };
}

export async function persistPreferences(): Promise<void> {
  const payload = buildSavePayload({
    leftPath: commander.left.path ? displayPath(commander.left.path) : null,
    rightPath: commander.right.path ? displayPath(commander.right.path) : null,
    columns: preferences.columnWidths,
    fileFontSize: preferences.fileFontSize,
  });
  try {
    await api.saveConfig(payload);
  } catch {
    // Configuration saving is best-effort and must never break browsing.
  }
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

export function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = undefined;
    void persistPreferences();
  }, 250);
}
