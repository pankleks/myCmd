import { api } from '../filesystem/api';
import type { GalleryImage } from './imageGallery';
import { hasNativeThumbnail, loadImagePreview } from './imagePreview';

/** Pixel size of the longer side; covers grid tiles on 2x displays. */
export const THUMBNAIL_SIDE = 320;
const MAX_PARALLEL = 3;
const MAX_CACHED = 400;

let running = 0;
const waiting: (() => void)[] = [];
const cache = new Map<string, string>();

function acquire(signal: AbortSignal): Promise<void> {
  signal.throwIfAborted();
  if (running < MAX_PARALLEL) {
    running++;
    return Promise.resolve();
  }
  return new Promise((resolve, reject) => {
    const start = () => {
      signal.removeEventListener('abort', abort);
      running++;
      resolve();
    };
    const abort = () => {
      waiting.splice(waiting.indexOf(start), 1);
      reject(signal.reason);
    };
    waiting.push(start);
    signal.addEventListener('abort', abort, { once: true });
  });
}

function release() {
  running--;
  waiting.shift()?.();
}

function cacheKey(image: GalleryImage) {
  return `${image.path}\0${image.size}\0${image.modified ?? ''}`;
}

/**
 * Loads a small image for the grid. Backend thumbnails are cached data URLs.
 * Other formats (HEIC, AVIF, SVG) use the full preview; a blob URL result
 * belongs to the caller, who must revoke it.
 */
export async function loadThumbnail(
  image: GalleryImage,
  signal: AbortSignal,
): Promise<string> {
  const key = cacheKey(image);
  const cached = cache.get(key);
  if (cached !== undefined) {
    // Refresh the entry so the cache drops the least recently used image.
    cache.delete(key);
    cache.set(key, cached);
    return cached;
  }
  await acquire(signal);
  try {
    signal.throwIfAborted();
    if (!hasNativeThumbnail(image.extension))
      return await loadImagePreview(image.path, image.extension, signal);
    const url = await api.readImageThumbnail(image.path, THUMBNAIL_SIDE);
    cache.set(key, url);
    if (cache.size > MAX_CACHED) cache.delete(cache.keys().next().value!);
    return url;
  } finally {
    release();
  }
}

/** For tests. */
export function resetThumbnails() {
  cache.clear();
  waiting.length = 0;
  running = 0;
}
