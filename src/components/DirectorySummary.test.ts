// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import DirectorySummary from './DirectorySummary.svelte';
import type { Listing } from '../filesystem/types';

const mocks = vi.hoisted(() => ({ list: vi.fn() }));
vi.mock('../filesystem/api', () => ({
  api: { list: mocks.list },
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
    readonly: false,
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
    readonly: false,
    directoryTarget: false,
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  target = document.createElement('div');
  document.body.append(target);
});
afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  target.remove();
});

describe('DirectorySummary', () => {
  it('shows file, subdirectory and size totals', async () => {
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
