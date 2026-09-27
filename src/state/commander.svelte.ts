import { api, errorMessage } from '../filesystem/api';
import type { Column, FileEntry, PanelState, Root } from '../filesystem/types';
export type Side = 'left' | 'right';
function panel(): PanelState {
  return { path: '', entries: [], cursor: 0, selected: new Set(), sort: { column: 'name', direction: 'asc' }, showHidden: false, loading: false, revision: 0 };
}
export const commander = $state({ activePanel: 'left' as Side, left: panel(), right: panel(), roots: [] as Root[] });
export interface Row extends FileEntry { parentEntry?: boolean }
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
export function rows(panel: PanelState): Row[] {
  const items: Row[] = panel.entries.filter(e => panel.showHidden || !e.hidden).sort((a, b) => {
    const directory = Number(b.type === 'directory') - Number(a.type === 'directory');
    if (directory) return directory;
    const column = panel.sort.column;
    const cmp = column === 'size' || column === 'modified' ? (a[column] ?? 0) - (b[column] ?? 0) : collator.compare(a[column], b[column]);
    return (cmp || collator.compare(a.name, b.name)) * (panel.sort.direction === 'asc' ? 1 : -1);
  });
  if (panel.parent) items.unshift({ name: '..', path: panel.parent, type: 'directory', extension: '', size: 0, hidden: false, readonly: false, directoryTarget: true, parentEntry: true });
  return items;
}
export async function load(panel: PanelState, path = panel.path || '~', focusPath?: string) {
  const revision = ++panel.revision;
  const oldCursor = rows(panel)[panel.cursor]?.path;
  panel.loading = true; panel.error = undefined;
  try {
    const result = await api.list(path);
    if (revision !== panel.revision) return;
    const same = panel.path === result.path;
    panel.path = result.path; panel.parent = result.parent; panel.entries = result.entries;
    panel.selected = same ? new Set([...panel.selected].filter(p => result.entries.some(e => e.path === p))) : new Set();
    const index = rows(panel).findIndex(e => e.path === (focusPath ?? (same ? oldCursor : undefined)));
    panel.cursor = index >= 0 ? index : 0;
  } catch (error) { if (revision === panel.revision) panel.error = errorMessage(error); }
  finally { if (revision === panel.revision) panel.loading = false; }
}
export function toggle(panel: PanelState, row?: Row) {
  if (!row || row.parentEntry) return;
  const next = new Set(panel.selected); if (next.has(row.path)) next.delete(row.path); else next.add(row.path); panel.selected = next;
}
export function sources(panel: PanelState): Row[] {
  const visible = rows(panel);
  return panel.selected.size ? visible.filter(e => panel.selected.has(e.path) && !e.parentEntry) : visible.filter((e, i) => i === panel.cursor && !e.parentEntry);
}
export async function open(panel: PanelState, row?: Row) {
  if (!row || panel.loading) return;
  if (row.directoryTarget) await load(panel, row.path, row.parentEntry ? panel.path : undefined);
  else try { await api.open(row.path); } catch (error) { panel.error = errorMessage(error); }
}
export function sort(panel: PanelState, column: Column) {
  const current = rows(panel)[panel.cursor]?.path;
  panel.sort = { column, direction: panel.sort.column === column && panel.sort.direction === 'asc' ? 'desc' : 'asc' };
  panel.cursor = Math.max(0, rows(panel).findIndex(e => e.path === current));
}
export function hidden(panel: PanelState) {
  panel.showHidden = !panel.showHidden;
  const visible = rows(panel); panel.selected = new Set([...panel.selected].filter(p => visible.some(e => e.path === p))); panel.cursor = Math.min(panel.cursor, Math.max(0, visible.length - 1));
}
