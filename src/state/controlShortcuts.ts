import { commander, load, rows } from './commander.svelte';
import { preferences, setShowHidden } from './preferences.svelte';
import {
  DEFAULT_FILE_FONT_SIZE,
  MAX_FILE_FONT_SIZE,
  MIN_FILE_FONT_SIZE,
} from '../utils/config';

/** The caller owns modifier/typing guards; this module owns panel-state actions. */
export function handleControlShortcut(
  event: Pick<KeyboardEvent, 'key' | 'preventDefault'>,
  focusPath: () => void,
): boolean {
  const key = event.key.toLowerCase();
  if (!['a', 'h', 'r', 'i', 'l', '+', '=', '-', '_', '0'].includes(key))
    return false;
  event.preventDefault();
  const active = commander[commander.activePanel];
  switch (key) {
    case 'a':
      active.selected = new Set(
        rows(active)
          .filter((entry) => !entry.parentEntry)
          .map((entry) => entry.path),
      );
      break;
    case 'h':
      setShowHidden(!preferences.showHidden);
      break;
    case 'r':
      void load(active);
      break;
    case 'i':
      if (active.path) {
        const other =
          commander[commander.activePanel === 'left' ? 'right' : 'left'];
        void load(other, active.path, rows(active)[active.cursor]?.path);
      }
      break;
    case 'l':
      focusPath();
      break;
    case '+':
    case '=':
      preferences.fileFontSize = Math.min(
        MAX_FILE_FONT_SIZE,
        preferences.fileFontSize + 1,
      );
      break;
    case '-':
    case '_':
      preferences.fileFontSize = Math.max(
        MIN_FILE_FONT_SIZE,
        preferences.fileFontSize - 1,
      );
      break;
    case '0':
      preferences.fileFontSize = DEFAULT_FILE_FONT_SIZE;
      break;
  }
  return true;
}
