import type { FileEntry } from '../filesystem/types';
export type FileOperation =
  | { type: 'copy' | 'move'; sources: string[]; destination: string }
  | { type: 'delete'; sources: string[] }
  | { type: 'rename'; path: string; name: string }
  | { type: 'createDirectory'; parent: string; name: string };
export interface Progress {
  operationId: string; state: 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
  currentItem?: string; processedBytes: number; totalBytes: number; processedItems: number; totalItems: number;
  error?: { code: string; message: string; path?: string }; resultPath?: string;
}
export interface Conflict { operationId: string; source: FileEntry; destination: FileEntry }
export interface Resolution { action: 'skip' | 'rename' | 'overwrite' | 'cancel'; name?: string; applyToAll: boolean }
