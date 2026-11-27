import type { FileEntry } from '../filesystem/types';
import { isImageFile } from './viewer';

export type GalleryImage = Pick<
  FileEntry,
  'path' | 'name' | 'extension' | 'size' | 'modified'
>;

/** Images among panel rows, in the order the panel shows them. */
export function galleryImages(
  rows: readonly (FileEntry & { parentEntry?: boolean })[],
): GalleryImage[] {
  return rows
    .filter(
      (row) =>
        !row.parentEntry &&
        row.type !== 'directory' &&
        !row.directoryTarget &&
        isImageFile(row.extension),
    )
    .map(({ path, name, extension, size, modified }) => ({
      path,
      name,
      extension,
      size,
      modified,
    }));
}

function clamp(index: number, count: number) {
  return Math.min(Math.max(index, 0), count - 1);
}

/**
 * Next image index for a key in the single-image view, or undefined when the
 * key does not navigate. The ends do not wrap around.
 */
export function viewerStep(
  key: string,
  index: number,
  count: number,
): number | undefined {
  if (count < 1) return;
  switch (key) {
    case 'ArrowUp':
    case 'ArrowLeft':
    case 'PageUp':
      return clamp(index - 1, count);
    case 'ArrowDown':
    case 'ArrowRight':
    case 'PageDown':
      return clamp(index + 1, count);
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
  }
}

/** Next selected index for a key in a grid with `columns` tiles per row. */
export function gridStep(
  key: string,
  index: number,
  count: number,
  columns: number,
  rowsPerPage = 1,
): number | undefined {
  if (count < 1) return;
  const width = Math.max(1, Math.floor(columns));
  const page = width * Math.max(1, Math.floor(rowsPerPage));
  switch (key) {
    case 'ArrowLeft':
      return clamp(index - 1, count);
    case 'ArrowRight':
      return clamp(index + 1, count);
    case 'ArrowUp':
      return index - width >= 0 ? index - width : index;
    case 'ArrowDown':
      // From the row above a shorter last row, go to the last image.
      return Math.floor(index / width) < Math.floor((count - 1) / width)
        ? Math.min(index + width, count - 1)
        : index;
    case 'PageUp':
      return clamp(index - page, count);
    case 'PageDown':
      return clamp(index + page, count);
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
  }
}
