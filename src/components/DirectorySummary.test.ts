// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import DirectorySummary from './DirectorySummary.svelte';
import type { Listing } from '../filesystem/types';
import { sftpProvider } from '../filesystem/providers';

const mocks = vi.hoisted(() => ({
  list: vi.fn(),
  measure: vi.fn(),
  cancel: vi.fn(),
  invoke: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }));
vi.mock('../filesystem/api', () => ({
  api: {
    list: mocks.list,
    measureDirectory: mocks.measure,
    cancelDirectorySizing: mocks.cancel,
  },
  errorMessage: (cause: unknown) =>
    cause instanceof Error ? cause.message : String(cause),
}));

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;

function summary(path = '/root', name = 'root') {
  component = mount(DirectorySummary, {
    target,
    props: { path, name },
  });
  flushSync();
}

async function settle(check: () => void) {
  await vi.waitFor(() => {
    flushSync();
    check();
  });
}

function listing(path: string, entries: Listing['entries']): Listing {
  return { path, entries };
}

function file(name: string, path: string, size: number) {
  return {
    name,
    path,
    type: 'file' as const,
    extension: 'txt',
    size,
    hidden: false,
    directoryTarget: false,
  };
}

function dir(name: string, path: string) {
  return {
    name,
    path,
    type: 'directory' as const,
    extension: '',
    size: 0,
    hidden: false,
    directoryTarget: false,
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  // By default the backend size walk fails, so the listing walk runs.
  mocks.invoke.mockResolvedValue({ folders: 1, files: 0, skipped: 1 });
  mocks.measure.mockRejectedValue(new Error('denied'));
  mocks.cancel.mockResolvedValue(undefined);
  target = document.createElement('div');
  document.body.append(target);
});
afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  target.remove();
});

function values() {
  return [...target.querySelectorAll('dd')].map((value) => value.textContent);
}

describe('DirectorySummary', () => {
  it('totals a local folder with backend walks instead of listings', async () => {
    mocks.invoke.mockResolvedValue({ folders: 3, files: 5, skipped: 0 });
    mocks.measure.mockResolvedValue(2048);
    summary();
    await settle(() => expect(values()).toEqual(['5', '2', '2.0 KB']));
    expect(target.textContent).not.toContain('Partial result');
    expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith(
      'count_delete_entries',
      { paths: ['/root'], requestId: expect.any(String) },
    );
    expect(mocks.measure).toHaveBeenCalledExactlyOnceWith(
      '/root',
      expect.any(String),
    );
    expect(mocks.list).not.toHaveBeenCalled();
  });

  it('flags partial backend totals when entries were skipped', async () => {
    mocks.invoke.mockResolvedValue({ folders: 1, files: 2, skipped: 1 });
    mocks.measure.mockResolvedValue(10);
    summary();
    await settle(() => expect(target.textContent).toContain('Partial result'));
    expect(values()).toEqual(['2', '0', '10 B']);
  });

  it('cancels both backend walks when it closes', async () => {
    mocks.invoke.mockReturnValue(new Promise(() => {}));
    mocks.measure.mockReturnValue(new Promise(() => {}));
    summary();
    await unmount(component!);
    component = undefined;
    expect(mocks.cancel).toHaveBeenCalledExactlyOnceWith([
      mocks.invoke.mock.calls[0][1].requestId,
      mocks.measure.mock.calls[0][1],
    ]);
  });

  it('walks SFTP folders through listings', async () => {
    const list = vi
      .spyOn(sftpProvider, 'list')
      .mockResolvedValue(
        listing('sftp:s!/%2Fhome', [file('a', 'sftp:s!/%2Fa', 3)]),
      );
    try {
      summary('sftp:s!/%2Fhome');
      await settle(() => expect(values()).toEqual(['1', '0', '3 B']));
      expect(mocks.invoke).not.toHaveBeenCalled();
      expect(mocks.measure).not.toHaveBeenCalled();
    } finally {
      list.mockRestore();
    }
  });

  it('walks listings when the backend size walk fails', async () => {
    mocks.list.mockImplementation(async (path: string) => {
      if (path === '/root')
        return listing(path, [
          file('a.txt', '/root/a.txt', 1536),
          dir('sub', '/root/sub'),
        ]);
      return listing(path, [file('b.txt', '/root/sub/b.txt', 512)]);
    });
    summary();
    await settle(() => expect(target.textContent).toContain('Total size'));
    expect(target.textContent).toContain('2');
    expect(target.textContent).toContain('1');
    expect(target.textContent).toContain('2.0 KB');
    expect(mocks.list).toHaveBeenCalledWith('/root');
  });

  it('flags partial results when a folder cannot be read', async () => {
    mocks.list.mockImplementation(async (path: string) => {
      if (path === '/root')
        return listing(path, [dir('locked', '/root/locked')]);
      throw new Error('denied');
    });
    summary();
    await settle(() => expect(target.textContent).toContain('Partial result'));
  });

  it('reports a hard error when the root folder cannot be read', async () => {
    mocks.list.mockRejectedValue(new Error('Cannot preview'));
    summary();
    await settle(() => expect(target.textContent).toContain('Cannot preview'));
  });
});
