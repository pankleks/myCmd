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

/** Scales a full-size preview down so the cache holds small data URLs only. */
async function shrink(url: string): Promise<string> {
  const image = new Image();
  image.src = url;
  await image.decode();
  const scale = Math.min(
    1,
    THUMBNAIL_SIDE / Math.max(image.naturalWidth, image.naturalHeight),
  );
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Image preview is unavailable.');
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  return canvas.toDataURL('image/png');
}

async function fullPreview(image: GalleryImage, signal: AbortSignal) {
  const url = await loadImagePreview(image.path, image.extension, signal);
  // SVG stays vector (and may have no intrinsic size to scale by).
  if (url.startsWith('data:image/svg+xml')) return url;
  try {
    signal.throwIfAborted();
    return await shrink(url);
  } finally {
    if (url.startsWith('blob:')) URL.revokeObjectURL(url);
  }
}

/**
 * Loads a small image for the grid as a data URL, cached until the file
 * changes. The backend scales common formats; HEIC and AVIF are decoded in
 * full once and scaled here.
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
    const url = hasNativeThumbnail(image.extension)
      ? await api.readImageThumbnail(image.path, THUMBNAIL_SIDE)
      : await fullPreview(image, signal);
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
