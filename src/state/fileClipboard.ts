import { api } from '../filesystem/api';
import { isLocalPath } from '../filesystem/providers';
import type { FileOperation } from '../operations/types';
import {
  commander,
  sources,
  quickFindClose,
  type Side,
} from './commander.svelte';

export function createFileClipboard(dependencies: {
  available: () => boolean;
  start: (operation: FileOperation, side: Side) => Promise<void>;
  failed: (error: unknown) => void;
}) {
  let pending = false;

  async function run(key: string) {
    pending = true;
    const side = commander.activePanel;
    const panel = commander[side];
    const destination = panel.path;
    try {
      if (key === 'c') {
        const paths = sources(panel).map((entry) => entry.path);
        if (!paths.length) return;
        if (!paths.every(isLocalPath))
          throw new Error(
            'Only local files and folders can be copied to the clipboard.',
          );
        await api.copyFilesToClipboard(paths);
      } else {
        if (!destination || !isLocalPath(destination))
          throw new Error('Paste is only available in a local folder.');
        const paths = await api.readFilesFromClipboard();
        if (!paths.length || !dependencies.available()) return;
        await dependencies.start(
          { type: 'copy', sources: paths, destination },
          side,
        );
      }
    } catch (error) {
      dependencies.failed(error);
    } finally {
      pending = false;
    }
  }

  return {
    handle(event: KeyboardEvent): boolean {
      const key = event.key.toLowerCase();
      if (
        !(event.ctrlKey || event.metaKey) ||
        event.altKey ||
        event.shiftKey ||
        !['c', 'v'].includes(key) ||
        event.defaultPrevented ||
        event.isComposing ||
        (event.target instanceof Element &&
          event.target.closest(
            'input, textarea, select, dialog, [contenteditable]:not([contenteditable="false"]), .monaco-editor',
          ))
      )
        return false;
      event.preventDefault();
      if (event.repeat || pending || !dependencies.available()) return true;
      if (commander[commander.activePanel].loading) return true;
      quickFindClose();
      void run(key);
      return true;
    },
  };
}
