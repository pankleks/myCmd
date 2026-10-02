import { describe, expect, it, vi } from 'vitest';
import type { Listing } from '../filesystem/types';
import { summarizeDirectory } from './directorySummary';

function entry(
  name: string,
  path: string,
  type: 'file' | 'directory' | 'symlink',
  size = 0,
) {
  return {
    name,
    path,
    type,
    extension: '',
    size,
    hidden: false,
    readonly: false,
    directoryTarget: false,
  };
}

function listing(path: string, entries: ReturnType<typeof entry>[]): Listing {
  return { path, entries };
}

describe('summarizeDirectory', () => {
  it('totals nested files, subdirectories and sizes', async () => {
    const data: Record<string, Listing> = {
      '/root': listing('/root', [
        entry('a.txt', '/root/a.txt', 'file', 100),
        entry('sub', '/root/sub', 'directory'),
      ]),
      '/root/sub': listing('/root/sub', [
        entry('b.txt', '/root/sub/b.txt', 'file', 50),
        entry('deep', '/root/sub/deep', 'directory'),
      ]),
      '/root/sub/deep': listing('/root/sub/deep', [
        entry('c.txt', '/root/sub/deep/c.txt', 'file', 25),
      ]),
    };
    const result = await summarizeDirectory(
      '/root',
      async (path) => data[path],
    );
    expect(result).toEqual({
      files: 3,
      directories: 2,
      size: 175,
      incomplete: false,
    });
  });

  it('counts symlinks as files without descending into them', async () => {
    const list = vi.fn(async (path: string) =>
      listing(path, [entry('link', `${path}/link`, 'symlink', 7)]),
    );
    const result = await summarizeDirectory('/root', list);
    expect(result.files).toBe(1);
    expect(result.directories).toBe(0);
    expect(result.size).toBe(0);
    expect(list).toHaveBeenCalledTimes(1);
  });

  it('marks partial results when folders are unreadable or skipped', async () => {
    const result = await summarizeDirectory('/root', async (path) => {
      if (path === '/root')
        return {
          path,
          entries: [
            entry('ok.txt', '/root/ok.txt', 'file', 10),
            entry('locked', '/root/locked', 'directory'),
          ],
          skippedEntries: 2,
        };
      throw new Error('denied');
    });
    expect(result).toEqual({
      files: 1,
      directories: 1,
      size: 10,
      incomplete: true,
    });
  });

  it('throws when the root folder cannot be read', async () => {
    await expect(
      summarizeDirectory('/root', async () => {
        throw new Error('denied');
      }),
    ).rejects.toThrow('denied');
  });

  it('stops early when cancelled', async () => {
    const list = vi.fn(async (path: string) => listing(path, []));
    let cancelled = false;
    const result = await summarizeDirectory('/root', list, () => cancelled);
    expect(result.files).toBe(0);
    cancelled = true;
    await summarizeDirectory('/root', list, () => cancelled);
    expect(list).toHaveBeenCalledTimes(1);
  });
});
