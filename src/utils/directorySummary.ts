import type { Listing } from '../filesystem/types';

export interface DirectorySummaryResult {
  files: number;
  directories: number;
  size: number;
  incomplete: boolean;
}

export async function summarizeDirectory(
  root: string,
  list: (path: string) => Promise<Listing>,
  isCancelled: () => boolean = () => false,
): Promise<DirectorySummaryResult> {
  const result: DirectorySummaryResult = {
    files: 0,
    directories: 0,
    size: 0,
    incomplete: false,
  };
  const stack = [root];
  let first = true;
  while (stack.length) {
    if (isCancelled()) return result;
    const current = stack.pop()!;
    let listing: Listing;
    try {
      listing = await list(current);
    } catch (cause) {
      // A failed root means nothing could be counted; report it instead of
      // showing empty totals. Nested failures only mark partial results.
      if (first) throw cause;
      result.incomplete = true;
      continue;
    } finally {
      first = false;
    }
    if (isCancelled()) return result;
    if (listing.skippedEntries) result.incomplete = true;
    for (const entry of listing.entries) {
      if (entry.type === 'directory') {
        result.directories += 1;
        stack.push(entry.path);
      } else {
        // Symlinks are leaves: counted as files but never descended into,
        // so linked trees cannot cause infinite walks.
        result.files += 1;
        if (entry.type === 'file') result.size += entry.size;
      }
    }
  }
  return result;
}
