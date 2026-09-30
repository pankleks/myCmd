import { api } from '../filesystem/api';
import { searchSession } from '../filesystem/providers';
import {
  buildSavePayload,
  DEFAULT_FILE_FONT_SIZE,
  normalizeConfig,
} from '../utils/config';
import { displayPath } from '../utils/paths';
import { commander, rows } from './commander.svelte';

export interface SavedPaths {
  leftPath: string | null;
  rightPath: string | null;
}

export const preferences = $state({
  fileFontSize: DEFAULT_FILE_FONT_SIZE,
  columnWidths: null as number[] | null,
  showHidden: false,
  showFunctionBar: true,
});

export function setShowHidden(value: boolean) {
  preferences.showHidden = value;
  for (const panel of [commander.left, commander.right]) {
    const current = rows(panel)[panel.cursor]?.path;
    panel.showHidden = value;
    const visible = rows(panel);
    const visiblePaths = new Set(visible.map((entry) => entry.path));
    panel.selected = new Set(
      [...panel.selected].filter((p) => visiblePaths.has(p)),
    );
    const index = visible.findIndex((entry) => entry.path === current);
    panel.cursor =
      index >= 0
        ? index
        : Math.min(panel.cursor, Math.max(0, visible.length - 1));
  }
}

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
  preferences.showFunctionBar = normalized.showFunctionBar;
  setShowHidden(normalized.showHidden);
  return { leftPath: normalized.leftPath, rightPath: normalized.rightPath };
}

export async function persistPreferences(): Promise<void> {
  const payload = buildSavePayload({
    leftPath: commander.left.path
      ? displayPath(
          searchSession(commander.left.path)?.returnPath ?? commander.left.path,
        )
      : null,
    rightPath: commander.right.path
      ? displayPath(
          searchSession(commander.right.path)?.returnPath ??
            commander.right.path,
        )
      : null,
    columns: preferences.columnWidths,
    fileFontSize: preferences.fileFontSize,
    showHidden: preferences.showHidden,
    showFunctionBar: preferences.showFunctionBar,
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
