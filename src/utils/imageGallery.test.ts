import { describe, expect, it } from 'vitest';
import type { FileEntry } from '../filesystem/types';
import {
  galleryImages,
  gridStep,
  gridWindow,
  viewerStep,
} from './imageGallery';

function entry(name: string, extra: Partial<FileEntry> = {}) {
  const dot = name.lastIndexOf('.');
  return {
    name,
    path: `/photos/${name}`,
    type: 'file' as const,
    extension: dot > 0 ? name.slice(dot + 1) : '',
    size: 1,
    modified: 2,
    hidden: false,
    directoryTarget: false,
    ...extra,
  };
}

describe('galleryImages', () => {
  it('keeps image files in panel order and skips folders, links to folders and other files', () => {
    const rows = [
      { ...entry('..', { type: 'directory' }), parentEntry: true },
      entry('b.JPG'),
      entry('notes.txt'),
      entry('album.png', { type: 'directory', directoryTarget: true }),
      entry('link.png', { type: 'symlink', directoryTarget: true }),
      entry('photo.heic', { type: 'symlink' }),
      entry('a.svg'),
    ];
    expect(galleryImages(rows).map((image) => image.name)).toEqual([
      'b.JPG',
      'photo.heic',
      'a.svg',
    ]);
    expect(galleryImages(rows)[0]).toEqual({
      path: '/photos/b.JPG',
      name: 'b.JPG',
      extension: 'JPG',
      size: 1,
      modified: 2,
    });
  });
});

describe('viewerStep', () => {
  it('moves to the previous and next image without wrapping', () => {
    expect(viewerStep('ArrowDown', 0, 3)).toBe(1);
    expect(viewerStep('ArrowRight', 1, 3)).toBe(2);
    expect(viewerStep('PageDown', 2, 3)).toBe(2);
    expect(viewerStep('ArrowUp', 2, 3)).toBe(1);
    expect(viewerStep('ArrowLeft', 0, 3)).toBe(0);
    expect(viewerStep('PageUp', 1, 3)).toBe(0);
    expect(viewerStep('Home', 2, 3)).toBe(0);
    expect(viewerStep('End', 0, 3)).toBe(2);
  });
  it('ignores other keys and empty lists', () => {
    expect(viewerStep('Enter', 0, 3)).toBeUndefined();
    expect(viewerStep('ArrowDown', 0, 0)).toBeUndefined();
  });
});

describe('gridStep', () => {
  // 10 images in 4 columns:
  // 0 1 2 3
  // 4 5 6 7
  // 8 9
  it('moves by one tile horizontally and by one row vertically', () => {
    expect(gridStep('ArrowRight', 3, 10, 4)).toBe(4);
    expect(gridStep('ArrowRight', 9, 10, 4)).toBe(9);
    expect(gridStep('ArrowLeft', 0, 10, 4)).toBe(0);
    expect(gridStep('ArrowDown', 1, 10, 4)).toBe(5);
    expect(gridStep('ArrowUp', 5, 10, 4)).toBe(1);
    expect(gridStep('ArrowUp', 2, 10, 4)).toBe(2);
  });
  it('goes to the last image from above a shorter last row', () => {
    expect(gridStep('ArrowDown', 6, 10, 4)).toBe(9);
    expect(gridStep('ArrowDown', 5, 10, 4)).toBe(9);
    expect(gridStep('ArrowDown', 4, 10, 4)).toBe(8);
    expect(gridStep('ArrowDown', 9, 10, 4)).toBe(9);
  });
  it('moves by a page and to both ends', () => {
    expect(gridStep('PageDown', 0, 10, 4, 2)).toBe(8);
    expect(gridStep('PageDown', 5, 10, 4, 2)).toBe(9);
    expect(gridStep('PageUp', 9, 10, 4, 2)).toBe(1);
    expect(gridStep('PageUp', 3, 10, 4, 0)).toBe(0);
    expect(gridStep('Home', 7, 10, 4)).toBe(0);
    expect(gridStep('End', 0, 10, 4)).toBe(9);
  });
  it('treats an unknown column count as one column and ignores other keys', () => {
    expect(gridStep('ArrowDown', 0, 3, 0)).toBe(1);
    expect(gridStep('Enter', 0, 3, 4)).toBeUndefined();
    expect(gridStep('ArrowDown', 0, 0, 4)).toBeUndefined();
  });
});

describe('gridWindow', () => {
  it('renders the rows in view plus two rows on each side', () => {
    // 1,000 rows of 100px in a 450px viewport scrolled to row 50.
    expect(gridWindow(5000, 450, 100, 1000)).toEqual({
      firstRow: 48,
      lastRow: 57,
    });
    expect(gridWindow(0, 450, 100, 1000)).toEqual({ firstRow: 0, lastRow: 7 });
    expect(gridWindow(99_950, 450, 100, 1000)).toEqual({
      firstRow: 997,
      lastRow: 1000,
    });
  });
  it('stays inside the grid when it shrinks below the scroll position', () => {
    expect(gridWindow(5000, 450, 100, 3)).toEqual({ firstRow: 3, lastRow: 3 });
  });
  it('renders the first rows until a row has been measured', () => {
    expect(gridWindow(5000, 450, 0, 1000)).toEqual({ firstRow: 0, lastRow: 8 });
    expect(gridWindow(0, 0, 0, 2)).toEqual({ firstRow: 0, lastRow: 2 });
  });
});
