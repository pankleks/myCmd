import { api, errorMessage } from '../filesystem/api';
import type { Column, FileEntry, PanelState, Root } from '../filesystem/types';
export type Side = 'left' | 'right';
function panel(): PanelState {
  return {
    path: '',
    entries: [],
    cursor: 0,
    selected: new Set(),
    sort: { column: 'name', direction: 'asc' },
    showHidden: false,
    loading: false,
    revision: 0,
  };
}
export const commander = $state({
  activePanel: 'left' as Side,
  left: panel(),
  right: panel(),
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
  const revision = ++panel.revision;
  const oldCursor = rows(panel)[panel.cursor]?.path;
  panel.loading = true;
  panel.error = undefined;
  try {
    const result = await api.list(path);
    if (revision !== panel.revision) return;
    const same = panel.path === result.path;
    panel.path = result.path;
    panel.parent = result.parent;
    panel.entries = result.entries;
    panel.selected = same
      ? new Set(
          [...panel.selected].filter((p) =>
            result.entries.some((e) => e.path === p),
          ),
        )
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

export const dirSizing = $state({ paths: [] as string[] });
const sizingInFlight = new Set<string>();
const measuredDirectories = new WeakSet<FileEntry>();

export async function measureDirectory(panel: PanelState, row?: Row) {
  if (!row || row.parentEntry || row.type !== 'directory') return;
  if (measuredDirectories.has(row)) return;
  if (sizingInFlight.has(row.path)) return;
  const revision = panel.revision;
  sizingInFlight.add(row.path);
  if (!dirSizing.paths.includes(row.path))
    dirSizing.paths = [...dirSizing.paths, row.path];
  try {
    const size = await api.measureDirectory(row.path);
    if (panel.revision !== revision) return;
    const entry = panel.entries.find((e) => e.path === row.path);
    if (entry) {
      entry.size = size;
      measuredDirectories.add(entry);
    }
  } catch {
    // Keep showing <DIR> when the size cannot be computed.
  } finally {
    sizingInFlight.delete(row.path);
    dirSizing.paths = dirSizing.paths.filter((p) => p !== row.path);
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
  if (row.directoryTarget)
    await load(panel, row.path, row.parentEntry ? panel.path : undefined);
  else
    try {
      await api.open(row.path);
    } catch (error) {
      panel.error = errorMessage(error);
    }
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
export function hidden(panel: PanelState) {
  panel.showHidden = !panel.showHidden;
  const visible = rows(panel);
  panel.selected = new Set(
    [...panel.selected].filter((p) => visible.some((e) => e.path === p)),
  );
  panel.cursor = Math.min(panel.cursor, Math.max(0, visible.length - 1));
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
