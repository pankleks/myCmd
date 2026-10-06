import { invoke } from '@tauri-apps/api/core';
import type { Listing, Root } from './types';
import type { FileOperation, Resolution } from '../operations/types';
import type { AppConfig } from '../utils/config';
export interface CommandResult {
  exitCode: number | null;
  success: boolean;
  stdout: string;
  stderr: string;
  cancelled: boolean;
  timedOut: boolean;
  outputTruncated: boolean;
  outputIncomplete: boolean;
}
export interface PreviewImage {
  dataUrl: string;
  width: number;
  height: number;
  frames: number;
}
export const api = {
  list: (path: string) => invoke<Listing>('list_directory', { path }),
  readTextPreview: (path: string) =>
    invoke<string>('read_text_preview', { path }),
  readImagePreview: (path: string) =>
    invoke<string>('read_image_preview', { path }),
  readMarkdownImage: (markdownPath: string, source: string) =>
    invoke<PreviewImage>('read_markdown_image', { markdownPath, source }),
  roots: () => invoke<Root[]>('list_roots'),
  open: (path: string) => invoke<void>('open_file', { path }),
  edit: (path: string, editor: string | null) =>
    invoke<void>('edit_file', { path, editor }),
  openTerminal: (path: string) => invoke<void>('open_terminal', { path }),
  measureDirectory: (path: string, requestId: string) =>
    invoke<number>('measure_directory', { path, requestId }),
  cancelDirectorySizing: (requestIds: string[]) =>
    invoke<void>('cancel_directory_sizing', { requestIds }),
  start: (operation: FileOperation) =>
    invoke<string>('start_operation', { operation }),
  cancel: (operationId: string) =>
    invoke<void>('cancel_operation', { operationId }),
  resolve: (operationId: string, resolution: Resolution) =>
    invoke<void>('resolve_conflict', { operationId, resolution }),
  watch: (paths: string[]) => invoke<void>('watch_directories', { paths }),
  runCommand: (command: string, cwd: string, commandId: string) =>
    invoke<CommandResult>('run_system_command', { command, cwd, commandId }),
  cancelCommand: (commandId: string) =>
    invoke<void>('cancel_system_command', { commandId }),
  loadConfig: () => invoke<AppConfig>('load_config'),
  saveConfig: (config: AppConfig) => invoke<void>('save_config', { config }),
};
export function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error && 'message' in error) {
    return (
      String(error.message) +
      ('path' in error && error.path ? ` — ${error.path}` : '')
    );
  }
  return String(error);
}
