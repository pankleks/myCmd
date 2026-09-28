import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileEntry, Listing, PanelState } from '../filesystem/types';
import { api } from '../filesystem/api';
import {
  commander,
  hidden,
  load,
  matchQuickFind,
  open,
  quickFindAppend,
  quickFindBackspace,
  quickFindClose,
  rows,
  sort,
  sources,
  toggle,
} from './commander.svelte';

vi.mock('../filesystem/api', () => ({
  api: { list: vi.fn(), open: vi.fn() },
  errorMessage: (error: { message: string }) => error.message,
}));

function entry(name: string, options: Partial<FileEntry> = {}): FileEntry {
  return {
    name,
    path: `/home/${name}`,
    type: 'file',
    extension: 'txt',
    size: 0,
    hidden: false,
    readonly: false,
    directoryTarget: false,
    ...options,
  };
}
function panel(): PanelState {
  return {
    path: '/home',
    parent: '/',
    entries: [],
    cursor: 0,
    selected: new Set(),
    sort: { column: 'name', direction: 'asc' },
    showHidden: false,
    loading: false,
    revision: 0,
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
beforeEach(() => {
  vi.resetAllMocks();
  commander.left = panel();
  commander.right = panel();
});

describe('sorting and selection', () => {
  it.each(['name', 'extension', 'size', 'modified'] as const)(
    'keeps parent and directories first when sorting %s in both directions',
    (column) => {
      const p = panel();
      p.entries = [
        entry('file10', { size: 10, extension: 'z', modified: 10 }),
        entry('file2', { size: 2, extension: 'a', modified: 2 }),
        entry('z-dir', { type: 'directory' }),
      ];
      for (const direction of ['asc', 'desc'] as const) {
        p.sort = { column, direction };
        expect(rows(p).map((e) => e.name)).toEqual([
          '..',
          'z-dir',
          ...(direction === 'asc' ? ['file2', 'file10'] : ['file10', 'file2']),
        ]);
      }
    },
  );
  it('preserves cursor identity while changing sort order', () => {
    const p = panel();
    p.entries = [entry('a', { size: 3 }), entry('b', { size: 1 })];
    p.cursor = 1;
    sort(p, 'size');
    expect(rows(p)[p.cursor].name).toBe('a');
    sort(p, 'size');
    expect(p.sort.direction).toBe('desc');
    expect(rows(p)[p.cursor].name).toBe('a');
  });
  it('uses selection before cursor and never operates on the synthetic parent', () => {
    const p = panel();
    p.entries = [entry('a'), entry('b')];
    toggle(p, rows(p)[0]);
    toggle(p, undefined);
    expect(sources(p)).toEqual([]);
    p.cursor = 1;
    expect(sources(p).map((e) => e.name)).toEqual(['a']);
    toggle(p, rows(p)[2]);
    expect(sources(p).map((e) => e.name)).toEqual(['b']);
    toggle(p, rows(p)[2]);
    expect(sources(p).map((e) => e.name)).toEqual(['a']);
  });
  it('removes hidden items from the selection and clamps the cursor', () => {
    const p = panel();
    p.parent = undefined;
    p.entries = [entry('.secret', { hidden: true })];
    expect(rows(p)).toEqual([]);
    hidden(p);
    toggle(p, rows(p)[0]);
    expect(p.selected.size).toBe(1);
    hidden(p);
    expect(p.selected.size).toBe(0);
    expect(p.cursor).toBe(0);
    expect(sources(p)).toEqual([]);
  });
  it('keeps panel state independent', () => {
    commander.left.entries = [entry('a')];
    commander.right.entries = [entry('a')];
    toggle(commander.left, rows(commander.left)[1]);
    hidden(commander.left);
    sort(commander.left, 'size');
    expect(commander.right.selected.size).toBe(0);
    expect(commander.right.showHidden).toBe(false);
    expect(commander.right.sort.column).toBe('name');
  });
});

describe('quick find', () => {
  function setup() {
    commander.left.entries = [
      entry('packdir', { type: 'directory' }),
      entry('package.txt'),
      entry('other.txt'),
    ];
    commander.left.cursor = 0;
    commander.quickFind = null;
  }
  function rowIndex(name: string) {
    return rows(commander.left).findIndex((e) => e.name === name);
  }
  it('jumps to the best matching file before any directory', () => {
    setup();
    for (const char of 'pack') quickFindAppend('left', char);
    expect(commander.quickFind).toEqual({
      side: 'left',
      query: 'pack',
      matched: true,
    });
    expect(commander.left.cursor).toBe(rowIndex('package.txt'));
  });
  it('falls back to a directory when no file matches', () => {
    setup();
    for (const char of 'packd') quickFindAppend('left', char);
    expect(commander.quickFind?.matched).toBe(true);
    expect(commander.left.cursor).toBe(rowIndex('packdir'));
  });
  it('keeps the query open without moving on no match', () => {
    setup();
    for (const char of 'zzz') quickFindAppend('left', char);
    expect(commander.quickFind).toEqual({
      side: 'left',
      query: 'zzz',
      matched: false,
    });
    expect(commander.left.cursor).toBe(0);
  });
  it('skips the synthetic parent entry', () => {
    const p = panel();
    p.entries = [entry('.gitignore')];
    expect(matchQuickFind(p, '.')).toBe(
      rows(p).findIndex((e) => e.name === '.gitignore'),
    );
  });
  it('ignores Polish diacritics when matching', () => {
    const p = panel();
    p.entries = [entry('łódka.txt')];
    expect(matchQuickFind(p, 'lod')).toBe(
      rows(p).findIndex((e) => e.name === 'łódka.txt'),
    );
  });
  it('backspace shrinks the query and closes it when empty', () => {
    setup();
    quickFindAppend('left', 'p');
    quickFindAppend('left', 'a');
    expect(commander.quickFind?.query).toBe('pa');
    quickFindBackspace();
    expect(commander.quickFind?.query).toBe('p');
    quickFindBackspace();
    expect(commander.quickFind).toBeNull();
    quickFindBackspace();
    expect(commander.quickFind).toBeNull();
  });
  it('closes explicitly', () => {
    setup();
    quickFindAppend('left', 'p');
    quickFindClose();
    expect(commander.quickFind).toBeNull();
  });
});

describe('asynchronous navigation', () => {
  it('ignores an older successful response when navigation finishes out of order', async () => {
    const first = deferred<Listing>();
    const second = deferred<Listing>();
    vi.mocked(api.list)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const p = panel();
    const a = load(p, '/first');
    const b = load(p, '/second');
    second.resolve({ path: '/second', parent: '/', entries: [entry('new')] });
    await b;
    first.resolve({ path: '/first', entries: [entry('old')] });
    await a;
    expect(p.path).toBe('/second');
    expect(p.entries[0].name).toBe('new');
    expect(p.loading).toBe(false);
  });
  it('ignores an older error without clearing the current loading state', async () => {
    const first = deferred<Listing>();
    const second = deferred<Listing>();
    vi.mocked(api.list)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const p = panel();
    const a = load(p, '/first');
    const b = load(p, '/second');
    first.reject({ message: 'old error' });
    await a;
    expect(p.error).toBeUndefined();
    expect(p.loading).toBe(true);
    second.resolve({ path: '/second', entries: [] });
    await b;
    expect(p.loading).toBe(false);
  });
  it('keeps the current directory on error and permits recovery', async () => {
    const p = panel();
    p.entries = [entry('keep')];
    vi.mocked(api.list).mockRejectedValueOnce({ message: 'Permission denied' });
    await load(p, '/denied');
    expect(p.path).toBe('/home');
    expect(p.entries[0].name).toBe('keep');
    expect(p.error).toBe('Permission denied');
    vi.mocked(api.list).mockResolvedValueOnce({ path: '/ok', entries: [] });
    await load(p, '/ok');
    expect(p.error).toBeUndefined();
  });
  it('preserves surviving selections on refresh and focuses an operation result', async () => {
    const p = panel();
    p.entries = [entry('a'), entry('gone')];
    p.cursor = 1;
    p.selected = new Set(['/home/a', '/home/gone']);
    vi.mocked(api.list).mockResolvedValue({
      path: '/home',
      parent: '/',
      entries: [entry('a'), entry('new')],
    });
    await load(p);
    expect(p.selected).toEqual(new Set(['/home/a']));
    expect(rows(p)[p.cursor].name).toBe('a');
    await load(p, undefined, '/home/new');
    expect(rows(p)[p.cursor].name).toBe('new');
  });
  it('loads home initially and clears selections when changing directory', async () => {
    const p = panel();
    p.path = '';
    p.selected.add('/home/a');
    vi.mocked(api.list).mockResolvedValue({ path: '/other', entries: [] });
    await load(p);
    expect(api.list).toHaveBeenCalledWith('~');
    expect(p.selected.size).toBe(0);
    expect(p.cursor).toBe(0);
  });
  it('follows directory links and restores the child cursor on parent navigation', async () => {
    const p = panel();
    p.entries = [entry('link', { type: 'symlink', directoryTarget: true })];
    vi.mocked(api.list).mockResolvedValueOnce({
      path: '/linked',
      parent: '/',
      entries: [],
    });
    await open(p, rows(p)[1]);
    expect(api.list).toHaveBeenCalledWith('/home/link');
    expect(api.open).not.toHaveBeenCalled();
    vi.mocked(api.list).mockResolvedValueOnce({
      path: '/',
      entries: [
        entry('linked', {
          path: '/linked',
          type: 'directory',
          directoryTarget: true,
        }),
      ],
    });
    await open(p, rows(p)[0]);
    expect(rows(p)[p.cursor].path).toBe('/linked');
  });
  it('delegates file opening and displays errors without losing the listing', async () => {
    const p = panel();
    const file = entry('file');
    p.entries = [file];
    await open(p, undefined);
    p.loading = true;
    await open(p, file);
    expect(api.open).not.toHaveBeenCalled();
    p.loading = false;
    vi.mocked(api.open).mockRejectedValue({ message: 'No association' });
    await open(p, file);
    expect(api.open).toHaveBeenCalledWith('/home/file');
    expect(p.error).toBe('No association');
    expect(p.entries).toHaveLength(1);
  });
});
