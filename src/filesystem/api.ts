import { invoke } from '@tauri-apps/api/core';
import { parseLocation } from './providers';
import type {
  FileProperties,
  Listing,
  Root,
  SftpConnectRequest,
  SftpConnectResult,
} from './types';
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
  outputReadFailed: boolean;
}
export interface PreviewImage {
  dataUrl: string;
  width: number;
  height: number;
  frames: number;
}
/** How to update this installation (see src-tauri/src/updates.rs). */
export interface UpdateInstruction {
  target: string;
  label: string;
  note: string;
  command: string;
}
export interface AvailableUpdate {
  version: string;
  name: string;
  publishedAt: string;
  notes: string;
  notesTruncated: boolean;
  instructions: UpdateInstruction[];
}
export interface UpdateCheck {
  current: string;
  /** Null when this is the newest release. */
  update: AvailableUpdate | null;
}
export interface ClipboardFiles {
  paths: string[];
  cut: boolean;
}
function previewArgs(path: string) {
  const location = parseLocation(path);
  return location.kind === 'sftp'
    ? { path: location.path, connectionId: location.connectionId }
    : { path };
}
export const api = {
  connectSftp: (request: SftpConnectRequest) =>
    invoke<SftpConnectResult>('connect_sftp', { request }),
  disconnectSftp: (id: string) => invoke<void>('disconnect_sftp', { id }),
  listSftp: (id: string, path: string) =>
    invoke<Listing>('list_sftp', { id, path }),
  systemActions: (path: string) =>
    invoke<void>('show_system_actions', { path }),
  properties: (path: string) =>
    invoke<FileProperties>('get_file_properties', { path }),
  setPermissions: (
    path: string,
    permissions: { mode?: number; readOnly?: boolean },
  ) => invoke<FileProperties>('set_file_permissions', { path, permissions }),
  copyText: (text: string) => invoke<void>('copy_text_to_clipboard', { text }),
  copyFilesToClipboard: (paths: string[], cut = false) =>
    invoke<void>('copy_files_to_clipboard', { paths, cut }),
  readFilesFromClipboard: () =>
    invoke<ClipboardFiles>('read_files_from_clipboard'),
  list: (path: string) => invoke<Listing>('list_directory', { path }),
  readTextPreview: (path: string) =>
    invoke<string>('read_text_preview', previewArgs(path)),
  readPdfPreview: (path: string) =>
    invoke<ArrayBuffer>('read_pdf_preview', previewArgs(path)),
  readImagePreview: (path: string) =>
    invoke<string>('read_image_preview', previewArgs(path)),
  readImageThumbnail: (path: string, maxSide: number) =>
    invoke<string>('read_image_thumbnail', { ...previewArgs(path), maxSide }),
  readMarkdownImage: (markdownPath: string, source: string) => {
    const { path, ...connection } = previewArgs(markdownPath);
    return invoke<PreviewImage>('read_markdown_image', {
      markdownPath: path,
      ...connection,
      source,
    });
  },
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
  checkForUpdate: (force: boolean) =>
    invoke<UpdateCheck>('check_for_update', { force }),
  /** Opens the release found by the last check; the URL never leaves Rust. */
  openReleasePage: () => invoke<void>('open_release_page'),
  loadConfig: () => invoke<AppConfig>('load_config'),
  takeConfigNotice: () => invoke<string | null>('take_config_notice'),
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
