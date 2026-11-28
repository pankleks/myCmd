import type { Listing } from '../filesystem/types';

export interface DirectorySummaryResult {
  files: number;
  directories: number;
  size: number;
  incomplete: boolean;
}

/**
 * Totals of a local folder from two backend walks, without sending every
 * listing to the frontend. Undefined when the size walk fails (it stops at
 * the first unreadable folder): the caller then walks the listings instead,
 * which reports partial results and errors for the folder itself.
 */
export async function summarizeLocalDirectory(
  count: () => Promise<{ folders: number; files: number; skipped: number }>,
  measure: () => Promise<number>,
): Promise<DirectorySummaryResult | undefined> {
  const measuring = measure().catch(() => undefined);
  const counts = await count();
  const size = await measuring;
  if (size === undefined) return;
  return {
    files: counts.files,
    // The backend counts the folder itself too.
    directories: Math.max(0, counts.folders - 1),
    size,
    incomplete: counts.skipped > 0,
  };
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
