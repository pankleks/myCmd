import { invoke } from '@tauri-apps/api/core';
import { api } from './api';
import { listen } from '@tauri-apps/api/event';
import type { FileEntry, Listing } from './types';

export type PanelLocation =
  | { kind: 'local'; path: string }
  | { kind: 'archive'; archivePath: string; directory: string }
  | { kind: 'ftp' | 'sftp' | 'smb' | 'nfs'; connectionId: string; path: string }
  | { kind: 'search'; sessionId: string };

export interface ContentProvider {
  readonly kind: PanelLocation['kind'];
  readonly capabilities: {
    mutate: boolean;
    preview: boolean;
    command: boolean;
    extract: boolean;
  };
  list(
    location: PanelLocation,
    onBatch?: (entries: FileEntry[]) => void,
  ): Promise<Listing>;
}

export interface SearchSession {
  root: string;
  pattern: string;
  text?: string;
  returnPath: string;
  focusPath?: string;
  requestId?: string;
}
export const searchSessions = new Map<string, SearchSession>();
export function searchSession(path: string): SearchSession | undefined {
  const location = parseLocation(path);
  return location.kind === 'search'
    ? searchSessions.get(location.sessionId)
    : undefined;
}
export async function cancelSearch(path: string) {
  const requestId = searchSession(path)?.requestId;
  if (requestId) await invoke('cancel_search', { requestId });
}
export const searchProvider: ContentProvider = {
  kind: 'search',
  capabilities: {
    mutate: false,
    preview: true,
    command: false,
    extract: false,
  },
  async list(location, onBatch) {
    if (location.kind !== 'search')
      throw new Error('Expected search location.');
    const session = searchSessions.get(location.sessionId);
    if (!session) throw new Error('Search session no longer exists.');
    const requestId = crypto.randomUUID();
    session.requestId = requestId;
    const resourceEntries = (entries: FileEntry[]) =>
      entries.map((entry) => ({
        ...entry,
        resource: { kind: 'local' as const, path: entry.path },
      }));
    const unlisten = await listen<{ requestId: string; entries: FileEntry[] }>(
      'search-results',
      ({ payload }) => {
        if (payload.requestId === requestId)
          onBatch?.(resourceEntries(payload.entries));
      },
    );
    try {
      const result = await invoke<{
        root: string;
        entries: FileEntry[];
        skipped: number;
        limited: boolean;
        cancelled: boolean;
      }>('search_files', {
        root: session.root,
        pattern: session.pattern,
        text: session.text,
        requestId,
      });
      session.root = result.root;
      return {
        path: locationKey(location),
        parent: session.returnPath,
        entries: resourceEntries(result.entries),
        skippedEntries: result.skipped,
        warnings: [
          ...(result.skipped
            ? [
                {
                  code: 'search_skipped',
                  message: `${result.skipped} locations could not be read.`,
                },
              ]
            : []),
          ...(result.limited
            ? [
                {
                  code: 'search_limit',
                  message: 'Results limited to 50,000 entries.',
                },
              ]
            : []),
          ...(result.cancelled
            ? [
                {
                  code: 'search_cancelled',
                  message: 'Search cancelled; results are incomplete.',
                },
              ]
            : []),
        ],
      };
    } finally {
      unlisten();
      if (session.requestId === requestId) session.requestId = undefined;
    }
  },
};

const archivePrefix = 'archive:';
export function locationKey(location: PanelLocation): string {
  if (location.kind === 'search') return `search:${location.sessionId}`;
  if (location.kind === 'local') return location.path;
  if (location.kind === 'archive')
    return `${archivePrefix}${encodeURIComponent(location.archivePath)}!/${encodeURIComponent(location.directory)}`;
  throw new Error(`Provider ${location.kind} is not implemented yet.`);
}
export function parseLocation(path: string): PanelLocation {
  if (path.startsWith('search:'))
    return { kind: 'search', sessionId: path.slice(7) };
  if (!path.startsWith(archivePrefix)) return { kind: 'local', path };
  const separator = path.indexOf('!/', archivePrefix.length);
  if (separator < 0) throw new Error('Invalid archive location.');
  return {
    kind: 'archive',
    archivePath: decodeURIComponent(
      path.slice(archivePrefix.length, separator),
    ),
    directory: decodeURIComponent(path.slice(separator + 2)),
  };
}
export function isLocalPath(path: string): boolean {
  return !path.startsWith(archivePrefix) && !path.startsWith('search:');
}
export function locationLabel(path: string): string {
  const location = parseLocation(path);
  if (location.kind === 'search') {
    const session = searchSessions.get(location.sessionId);
    return session
      ? `Search: ${session.pattern}${session.text ? ` containing "${session.text}"` : ''} — ${session.root}`
      : 'Search results';
  }
  return location.kind === 'archive'
    ? `${location.archivePath}!/${location.directory}`
    : path;
}
export function parentFocus(path: string): string {
  const session = searchSession(path);
  if (session) return session.focusPath ?? session.returnPath;
  const location = parseLocation(path);
  return location.kind === 'archive' && !location.directory
    ? location.archivePath
    : path;
}
export const localProvider: ContentProvider = {
  kind: 'local',
  capabilities: { mutate: true, preview: true, command: true, extract: false },
  async list(location) {
    if (location.kind !== 'local') throw new Error('Expected local location.');
    const result = await api.list(location.path);
    result.entries = result.entries.map((entry) => ({
      ...entry,
      resource: { kind: 'local', path: entry.path },
    }));
    return result;
  },
};
export const archiveProvider: ContentProvider = {
  kind: 'archive',
  capabilities: {
    mutate: false,
    preview: false,
    command: false,
    extract: true,
  },
  async list(location) {
    if (location.kind !== 'archive')
      throw new Error('Expected archive location.');
    const result = await invoke<Listing>('list_archive', {
      archivePath: location.archivePath,
      directory: location.directory,
    });
    result.entries = result.entries.map((entry) => {
      const directory =
        location.directory +
        entry.name +
        (entry.type === 'directory' ? '/' : '');
      const resource: PanelLocation = { ...location, directory };
      return { ...entry, path: locationKey(resource), resource };
    });
    result.path = locationKey(location);
    result.parent = location.directory
      ? locationKey({
          ...location,
          directory: location.directory.replace(/[^/]+\/$/, ''),
        })
      : result.parent;
    return result;
  },
};
export function providerFor(location: PanelLocation): ContentProvider {
  if (location.kind === 'search') return searchProvider;
  if (location.kind === 'local') return localProvider;
  if (location.kind === 'archive') return archiveProvider;
  throw new Error(`Provider ${location.kind} is not implemented yet.`);
}
export function archiveLocation(entry: FileEntry): PanelLocation | undefined {
  if (!isLocalPath(entry.path) || entry.type !== 'file') return;
  if (!/\.(zip|7z)$/i.test(entry.name)) return;
  return { kind: 'archive', archivePath: entry.path, directory: '' };
}
