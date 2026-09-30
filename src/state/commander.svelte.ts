import { api, errorMessage } from '../filesystem/api';
import type { Column, FileEntry, PanelState, Root } from '../filesystem/types';
import { globToRegExp } from '../utils/glob';
import {
  archiveLocation,
  locationKey,
  parseLocation,
  providerFor,
  isLocalPath,
  searchSessions,
  searchSession,
  cancelSearch,
} from '../filesystem/providers';
export type Side = 'left' | 'right';
export function createPanel(): PanelState {
  const state: PanelState = $state({
    path: '',
    entries: [],
    cursor: 0,
    selected: new Set(),
    sort: { column: 'name', direction: 'asc' },
    showHidden: false,
    loading: false,
    revision: 0,
    get visibleRows(): Row[] {
      return visible;
    },
  });
  const visible: Row[] = $derived(buildRows(state));
  return state;
}
export const commander = $state({
  activePanel: 'left' as Side,
  left: createPanel(),
  right: createPanel(),
  roots: [] as Root[],
  quickFind: null as { side: Side; query: string; matched: boolean } | null,
});
export interface Row extends FileEntry {
  parentEntry?: boolean;
}
const collator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: 'base',
});
export function rows(panel: PanelState): Row[] {
  return panel.visibleRows ?? buildRows(panel);
}

function buildRows(panel: PanelState): Row[] {
  const items: Row[] = panel.entries
    .filter((e) => panel.showHidden || !e.hidden)
    .sort((a, b) => {
      const directory =
        Number(b.type === 'directory') - Number(a.type === 'directory');
      if (directory) return directory;
      const column = panel.sort.column;
      const cmp =
        column === 'size' || column === 'modified'
          ? (a[column] ?? 0) - (b[column] ?? 0)
          : collator.compare(a[column], b[column]);
      return (
        (cmp || collator.compare(a.name, b.name)) *
        (panel.sort.direction === 'asc' ? 1 : -1)
      );
    });
  if (panel.parent)
    items.unshift({
      name: '..',
      path: panel.parent,
      type: 'directory',
      extension: '',
      size: 0,
      hidden: false,
      readonly: false,
      directoryTarget: true,
      parentEntry: true,
    });
  return items;
}
export async function load(
  panel: PanelState,
  path = panel.path || '~',
  focusPath?: string,
) {
  void cancelSearch(panel.path).catch(() => {});
  const revision = ++panel.revision;
  const oldCursor = rows(panel)[panel.cursor]?.path;
  panel.loading = true;
  panel.error = undefined;
  try {
    const location = parseLocation(path);
    if (
      location.kind === 'archive' &&
      location.directory &&
      !location.directory.endsWith('/')
    )
      location.directory += '/';
    if (location.kind === 'search') {
      panel.path = path;
      panel.parent = searchSession(path)?.returnPath;
      panel.entries = [];
      panel.selected = new Set();
    }
    const result = await providerFor(location).list(location, (entries) => {
      if (revision === panel.revision)
        panel.entries = [...panel.entries, ...entries];
    });
    if (revision !== panel.revision) return;
    const same = panel.path === result.path;
    panel.path = result.path;
    panel.parent = result.parent;
    panel.entries = result.entries;
    panel.skippedEntries = result.skippedEntries ?? 0;
    panel.warnings = result.warnings ?? [];
    const existingPaths = new Set(result.entries.map((entry) => entry.path));
    panel.selected = same
      ? new Set([...panel.selected].filter((p) => existingPaths.has(p)))
      : new Set();
    const index = rows(panel).findIndex(
      (e) => e.path === (focusPath ?? (same ? oldCursor : undefined)),
    );
    panel.cursor = index >= 0 ? index : 0;
  } catch (error) {
    if (revision === panel.revision) panel.error = errorMessage(error);
  } finally {
    if (revision === panel.revision) panel.loading = false;
  }
}
export function toggle(panel: PanelState, row?: Row) {
  if (!row || row.parentEntry) return;
  const next = new Set(panel.selected);
  if (next.has(row.path)) next.delete(row.path);
  else next.add(row.path);
  panel.selected = next;
}

export type SelectionMode = 'extend' | 'shrink';

export function invertSelection(panel: PanelState) {
  const visiblePaths = rows(panel)
    .filter((row) => !row.parentEntry)
    .map((row) => row.path);
  panel.selected = new Set(
    visiblePaths.filter((path) => !panel.selected.has(path)),
  );
}

export function selectByGlob(
  panel: PanelState,
  pattern: string,
  mode: SelectionMode,
) {
  const matcher = globToRegExp(pattern);
  const matchingPaths = rows(panel)
    .filter((row) => !row.parentEntry && matcher.test(row.name))
    .map((row) => row.path);
  const next = new Set(panel.selected);
  for (const path of matchingPaths) {
    if (mode === 'extend') next.add(path);
    else next.delete(path);
  }
  panel.selected = next;
}

export const dirSizing = $state({ paths: [] as string[], cancelling: false });
const sizingInFlight = new Map<
  string,
  { promise: Promise<number>; id: string }
>();
const measuredDirectories = new WeakSet<FileEntry>();

export async function cancelDirectorySizing() {
  if (!dirSizing.paths.length || dirSizing.cancelling) return;
  dirSizing.cancelling = true;
  try {
    const ids = [...sizingInFlight.values()].map((request) => request.id);
    for (let offset = 0; offset < ids.length; offset += 64)
      await api.cancelDirectorySizing(ids.slice(offset, offset + 64));
  } finally {
    dirSizing.cancelling = false;
  }
}

export async function measureDirectory(panel: PanelState, row?: Row) {
  if (dirSizing.cancelling) return;
  if (!row || row.parentEntry || row.type !== 'directory') return;
  if (!isLocalPath(row.path)) return;
  if (measuredDirectories.has(row)) return;
  const revision = panel.revision;
  let pending = sizingInFlight.get(row.path);
  try {
    if (!pending) {
      const id = crypto.randomUUID();
      pending = { id, promise: api.measureDirectory(row.path, id) };
      sizingInFlight.set(row.path, pending);
      dirSizing.paths = [...dirSizing.paths, row.path];
    }
    const size = await pending.promise;
    if (panel.revision !== revision) return;
    const entry = panel.entries.find((e) => e.path === row.path);
    if (entry) {
      const current = rows(panel)[panel.cursor]?.path;
      entry.size = size;
      measuredDirectories.add(entry);
      panel.cursor = Math.max(
        0,
        rows(panel).findIndex((item) => item.path === current),
      );
    }
  } catch {
    // Keep showing <DIR> when the size cannot be computed.
  } finally {
    if (pending && sizingInFlight.get(row.path) === pending) {
      sizingInFlight.delete(row.path);
      dirSizing.paths = dirSizing.paths.filter((p) => p !== row.path);
    }
  }
}
export function sources(panel: PanelState): Row[] {
  const visible = rows(panel);
  return panel.selected.size
    ? visible.filter((e) => panel.selected.has(e.path) && !e.parentEntry)
    : visible.filter((e, i) => i === panel.cursor && !e.parentEntry);
}
export async function open(panel: PanelState, row?: Row) {
  if (!row || panel.loading) return;
  const session = searchSession(panel.path);
  if (session) {
    if (row.parentEntry) {
      await load(panel, session.returnPath, session.focusPath);
      return;
    }
    let parent = row.path.replace(/[\\/][^\\/]+[\\/]?$/, '') || '/';
    if (/^[a-z]:$/i.test(parent)) parent += '\\';
    await load(panel, parent, row.path);
    return;
  }
  const archive = archiveLocation(row);
  if (archive) {
    await load(panel, locationKey(archive));
    return;
  }
  if (row.directoryTarget) {
    const current = parseLocation(panel.path);
    const focus =
      row.parentEntry && current.kind === 'archive' && !current.directory
        ? current.archivePath
        : row.parentEntry
          ? panel.path
          : undefined;
    await load(panel, row.path, focus);
  } else
    try {
      if (!isLocalPath(row.path))
        throw new Error('Opening archived files is not supported yet.');
      await api.open(row.path);
    } catch (error) {
      panel.error = errorMessage(error);
    }
}
export async function startSearch(
  panel: PanelState,
  root: string,
  pattern: string,
) {
  const id = crypto.randomUUID();
  const previous = searchSession(panel.path);
  searchSessions.set(id, {
    root,
    pattern,
    returnPath: previous?.returnPath ?? panel.path,
    focusPath: previous?.focusPath ?? rows(panel)[panel.cursor]?.path,
  });
  await load(panel, locationKey({ kind: 'search', sessionId: id }));
}
export async function leaveSearch(panel: PanelState) {
  const session = searchSession(panel.path);
  if (session) await load(panel, session.returnPath, session.focusPath);
}
export function sort(panel: PanelState, column: Column) {
  const current = rows(panel)[panel.cursor]?.path;
  panel.sort = {
    column,
    direction:
      panel.sort.column === column && panel.sort.direction === 'asc'
        ? 'desc'
        : 'asc',
  };
  panel.cursor = Math.max(
    0,
    rows(panel).findIndex((e) => e.path === current),
  );
}

const FOLD_EXTRA: Record<string, string> = {
  ł: 'l',
  ø: 'o',
  æ: 'ae',
  œ: 'oe',
  ß: 'ss',
  đ: 'd',
};

function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[łøæœßđ]/g, (char) => FOLD_EXTRA[char] ?? char);
}

export function matchQuickFind(panel: PanelState, query: string): number {
  const prefix = fold(query);
  const list = rows(panel);
  let directoryFallback = -1;
  for (let i = 0; i < list.length; i++) {
    const entry = list[i];
    if (entry.parentEntry) continue;
    if (!fold(entry.name).startsWith(prefix)) continue;
    if (entry.type === 'directory') {
      if (directoryFallback < 0) directoryFallback = i;
    } else {
      return i;
    }
  }
  return directoryFallback;
}

export function quickFindAppend(side: Side, char: string) {
  const panel = commander[side];
  const current =
    commander.quickFind?.side === side ? commander.quickFind.query : '';
  const query = (current + char).slice(0, 64);
  const index = matchQuickFind(panel, query);
  if (index >= 0) {
    panel.cursor = index;
    commander.quickFind = { side, query, matched: true };
  } else {
    commander.quickFind = { side, query, matched: false };
  }
}

export function quickFindBackspace() {
  const active = commander.quickFind;
  if (!active) return;
  const query = active.query.slice(0, -1);
  if (!query) {
    commander.quickFind = null;
    return;
  }
  const panel = commander[active.side];
  const index = matchQuickFind(panel, query);
  if (index >= 0) {
    panel.cursor = index;
    commander.quickFind = { side: active.side, query, matched: true };
  } else {
    commander.quickFind = { side: active.side, query, matched: false };
  }
}

export function quickFindClose() {
  commander.quickFind = null;
}
