import { describe, it, expect, vi } from 'vitest';
import { countDeleteEntries } from './deleteCounts';
import { archiveProvider, locationKey } from './providers';
import type { FileEntry } from './types';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('delete confirmation counts', () => {
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
