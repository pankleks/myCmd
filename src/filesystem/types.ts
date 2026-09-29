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
  skippedEntries?: number;
  warnings?: ListingWarning[];
}
export interface ListingWarning {
  code: string;
  message: string;
  path?: string;
}
export interface Root {
  name: string;
  path: string;
  type: 'drive' | 'mount' | 'home' | 'root';
}
export type Column = 'name' | 'extension' | 'size' | 'modified';
export interface PanelState {
  readonly visibleRows?: (FileEntry & { parentEntry?: boolean })[];
  path: string;
  parent?: string;
  entries: FileEntry[];
  cursor: number;
  selected: Set<string>;
  sort: { column: Column; direction: 'asc' | 'desc' };
  showHidden: boolean;
  loading: boolean;
  error?: string;
  skippedEntries?: number;
  warnings?: ListingWarning[];
  revision: number;
}
