export interface FileEntry {
  name: string;
  path: string;
  type: 'file' | 'directory' | 'symlink';
  extension: string;
  size: number;
  modified?: number;
  created?: number;
  hidden: boolean;
  readonly: boolean;
  directoryTarget: boolean;
}
export interface Listing {
  path: string;
  parent?: string;
  entries: FileEntry[];
}
export interface Root {
  name: string;
  path: string;
  type: 'drive' | 'mount' | 'home' | 'root';
}
export type Column = 'name' | 'extension' | 'size' | 'modified';
export interface PanelState {
  path: string;
  parent?: string;
  entries: FileEntry[];
  cursor: number;
  selected: Set<string>;
  sort: { column: Column; direction: 'asc' | 'desc' };
  showHidden: boolean;
  loading: boolean;
  error?: string;
  revision: number;
}
