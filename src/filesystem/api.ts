import { invoke } from '@tauri-apps/api/core';
import type { Listing, Root } from './types';
import type { FileOperation, Resolution } from '../operations/types';
export const api = {
  list: (path: string) => invoke<Listing>('list_directory', { path }),
  roots: () => invoke<Root[]>('list_roots'),
  open: (path: string) => invoke<void>('open_file', { path }),
  start: (operation: FileOperation) =>
    invoke<string>('start_operation', { operation }),
  cancel: (operationId: string) =>
    invoke<void>('cancel_operation', { operationId }),
  resolve: (operationId: string, resolution: Resolution) =>
    invoke<void>('resolve_conflict', { operationId, resolution }),
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
