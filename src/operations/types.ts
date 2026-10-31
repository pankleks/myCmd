import type { FileEntry } from '../filesystem/types';
export type FileOperation =
  | {
      type: 'sftp';
      connectionId: string;
      operation:
        | {
            type: 'copy';
            sources: string[];
            destination: string;
            upload: boolean;
          }
        | { type: 'rename'; path: string; name: string }
        | { type: 'createDirectory'; parent: string; name: string }
        | { type: 'delete'; sources: string[] };
    }
  | {
      type: 'compress';
      sources: string[];
      destination: string;
      name: string;
      compressionLevel?: number;
    }
  | {
      type: 'deleteArchive';
      archivePath: string;
      directory: string;
      members: string[];
    }
  | {
      type: 'extract';
      archivePath: string;
      directory: string;
      members: string[];
      destination: string;
    }
  | { type: 'copy' | 'move'; sources: string[]; destination: string }
  | { type: 'delete'; sources: string[]; permanent: boolean }
  | { type: 'rename'; path: string; name: string }
  | { type: 'createDirectory' | 'createFile'; parent: string; name: string };
export interface Progress {
  operationId: string;
  state: 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
  currentItem?: string;
  processedBytes: number;
  totalBytes: number;
  processedItems: number;
  totalItems: number;
  error?: { code: string; message: string; path?: string };
  resultPath?: string;
}
export interface Conflict {
  operationId: string;
  source: FileEntry;
  destination: FileEntry;
}
export interface Resolution {
  action: 'skip' | 'rename' | 'overwrite' | 'cancel';
  name?: string;
  applyToAll: boolean;
}
