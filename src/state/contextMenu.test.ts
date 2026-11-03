import { describe, expect, it, vi } from 'vitest';
import { locationKey } from '../filesystem/providers';
import { contextMenuItems, type ContextMenuState } from './contextMenu';
import type { Row } from './commander.svelte';

const remote = (path: string) =>
  locationKey({ kind: 'sftp', connectionId: 'c1', path });
const file = (path: string): Row =>
  ({ name: path.split('/').pop(), path, type: 'file' }) as Row;
const actions = () => ({
  request: vi.fn(),
  connections: vi.fn(),
  refresh: vi.fn(),
  open: vi.fn(),
  openTerminal: vi.fn(),
  paste: vi.fn(),
  copy: vi.fn(),
  cut: vi.fn(),
  view: vi.fn(),
  edit: vi.fn(),
  copyPath: vi.fn(),
  properties: vi.fn(),
  systemActions: vi.fn(),
});
function state(overrides: Partial<ContextMenuState>): ContextMenuState {
  return {
    panelPath: '/home',
    oppositePath: '/tmp',
    oppositeRoot: '/tmp',
    item: true,
    paste: false,
    entry: file('/home/a.txt'),
    entries: [file('/home/a.txt')],
    systemActionsRunning: false,
    ...overrides,
  };
}
const labels = (items: { label: string }[]) => items.map((i) => i.label);

describe('contextMenuItems', () => {
  it('offers folder actions on empty space in a local panel', () => {
    const a = actions();
    const items = contextMenuItems(state({ item: false, paste: true }), a);
    expect(labels(items)).toEqual([
      'New folder…',
      'New file…',
      'Paste',
      'Open terminal here',
      'Refresh',
    ]);
    expect(items[2].disabled).toBe(false);
    items[0].run();
    expect(a.request).toHaveBeenCalledWith('createDirectory');
  });

  it('offers only Refresh on empty space in archives', () => {
    const items = contextMenuItems(
      state({ item: false, panelPath: 'archive:/a.zip!/' }),
      actions(),
    );
    expect(labels(items)).toEqual(['Refresh']);
  });

  it('offers remote actions in an SFTP panel and deletes permanently', () => {
    const a = actions();
    const entry = file(remote('/data/a.txt'));
    const items = contextMenuItems(
      state({ panelPath: remote('/data'), entry, entries: [entry] }),
      a,
    );
    expect(labels(items)).toEqual([
      'Open folder',
      'Download to opposite panel…',
      'New folder…',
      'Rename…',
      'Delete permanently…',
      'Connections…',
    ]);
    expect(items[0].disabled).toBe(true);
    items[4].run();
    expect(a.request).toHaveBeenCalledWith('delete', true);
    expect(
      labels(
        contextMenuItems(state({ panelPath: remote('/'), item: false }), a),
      ),
    ).toEqual(['New folder…', 'Connections…', 'Refresh']);
  });

  it('labels F5 as Upload when the opposite panel is remote', () => {
    const items = contextMenuItems(
      state({ oppositePath: remote('/'), oppositeRoot: remote('/') }),
      actions(),
    );
    const upload = items.find((i) => i.shortcut === 'F5')!;
    expect(upload.label).toBe('Upload to opposite panel…');
    expect(upload.disabled).toBe(false);
    expect(items.find((i) => i.shortcut === 'F6')!.disabled).toBe(true);
  });

  it('disables single-item actions for multiple selections', () => {
    const entries = [file('/home/a.txt'), file('/home/b.txt')];
    const items = contextMenuItems(state({ entries }), actions());
    for (const label of ['Open', 'Rename…', 'Copy path', 'Properties…'])
      expect(items.find((i) => i.label === label)!.disabled).toBe(true);
  });

  it('shows a running state for system actions', () => {
    const items = contextMenuItems(
      state({ systemActionsRunning: true }),
      actions(),
    );
    const item = items.find((i) => i.label.startsWith('System actions'))!;
    expect(item.label).toBe('System actions running…');
    expect(item.disabled).toBe(true);
  });
});
