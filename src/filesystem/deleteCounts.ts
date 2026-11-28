import { invoke } from '@tauri-apps/api/core';
import type { FileEntry } from './types';
import { parseLocation, locationKey, providerFor } from './providers';

export interface DeleteCounts {
  folders: number;
  files: number;
  skipped: number;
}
export async function countDeleteEntries(
  entries: FileEntry[],
  cancelled: () => boolean,
): Promise<DeleteCounts> {
  const first = entries[0];
  if (!first) return { folders: 0, files: 0, skipped: 0 };
  const location = parseLocation(first.path);
  if (location.kind === 'local') {
    // Stop the backend walk once the dialog closes; it can take minutes on a big tree.
    const requestId = crypto.randomUUID();
    const poll = setInterval(() => {
      if (!cancelled()) return;
      clearInterval(poll);
      void invoke('cancel_directory_sizing', {
        requestIds: [requestId],
      }).catch(() => {});
    }, 250);
    try {
      return await invoke<DeleteCounts>('count_delete_entries', {
        paths: entries.map((entry) => entry.path),
        requestId,
      });
    } finally {
      clearInterval(poll);
    }
  }
  const counts: DeleteCounts = { folders: 0, files: 0, skipped: 0 };
  const stack = [...entries];
  const visited = new Set<string>();
  while (stack.length && !cancelled()) {
    const entry = stack.pop()!;
    if (visited.has(entry.path)) continue;
    visited.add(entry.path);
    if (entry.type !== 'directory') {
      counts.files++;
      continue;
    }
    counts.folders++;
    try {
      const resource = parseLocation(entry.path);
      if (resource.kind === 'archive' && !resource.directory.endsWith('/'))
        resource.directory += '/';
      const listing = await providerFor(resource).list(
        parseLocation(locationKey(resource)),
      );
      counts.skipped += listing.skippedEntries ?? 0;
      stack.push(...listing.entries);
    } catch {
      counts.skipped++;
    }
  }
  return counts;
}
