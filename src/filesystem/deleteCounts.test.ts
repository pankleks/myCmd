import { describe, it, expect, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { countDeleteEntries } from './deleteCounts';
import { archiveProvider, locationKey } from './providers';
import type { FileEntry } from './types';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('delete confirmation counts', () => {
  it('cancels the local backend count once the caller is cancelled', async () => {
    vi.useFakeTimers();
    try {
      let reject!: (error: Error) => void;
      const pending = new Promise((_, no) => (reject = no));
      vi.mocked(invoke).mockImplementation((command) =>
        command === 'count_delete_entries'
          ? pending
          : Promise.resolve(undefined),
      );
      let cancelled = false;
      const counting = countDeleteEntries(
        [{ name: 'a', type: 'directory', path: '/a' } as FileEntry],
        () => cancelled,
      );
      const { requestId } = vi.mocked(invoke).mock.calls[0][1] as {
        requestId: string;
      };
      vi.advanceTimersByTime(500);
      expect(invoke).toHaveBeenCalledTimes(1);
      cancelled = true;
      vi.advanceTimersByTime(250);
      expect(invoke).toHaveBeenLastCalledWith('cancel_directory_sizing', {
        requestIds: [requestId],
      });
      reject(new Error('cancelled'));
      await expect(counting).rejects.toThrow('cancelled');
    } finally {
      vi.useRealTimers();
    }
  });

  it('counts archive directories and children but treats nested archives as files', async () => {
    const resource = {
      kind: 'archive' as const,
      archivePath: 'test.zip',
      directory: 'docs/',
    };
    const folder = {
      name: 'docs',
      type: 'directory',
      path: locationKey(resource),
    } as FileEntry;
    const nestedArchive = {
      name: 'nested.7z',
      type: 'file',
      path: locationKey({ ...resource, directory: 'docs/nested.7z' }),
    } as FileEntry;
    const listing = vi
      .spyOn(archiveProvider, 'list')
      .mockResolvedValue({ path: folder.path, entries: [nestedArchive] });
    expect(await countDeleteEntries([folder, folder], () => false)).toEqual({
      folders: 1,
      files: 1,
      skipped: 0,
    });
    expect(listing).toHaveBeenCalledTimes(1);
    listing.mockRestore();
  });
});
