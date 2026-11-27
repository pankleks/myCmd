import { api } from '../filesystem/api';
import { isHeicFile } from './viewer';

function normalized(extension: string) {
  return extension.toLowerCase().replace(/^\./, '');
}

/**
 * Loads a full-size image for display in an `<img>`. HEIC results are blob
 * URLs that the caller must revoke; other results are data URLs.
 */
export async function loadImagePreview(
  path: string,
  extension: string,
  signal: AbortSignal,
): Promise<string> {
  // Keep SVG in an image context: scripts and external resources are disabled.
  if (normalized(extension) === 'svg')
    return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(await api.readTextPreview(path))}`;
  const dataUrl = await api.readImagePreview(path);
  if (!isHeicFile(extension)) return dataUrl;
  signal.throwIfAborted();
  const { heicPreview } = await import('./heicPreview');
  return heicPreview(dataUrl, signal);
}

/** Formats that the backend can scale down to a small thumbnail. */
export function hasNativeThumbnail(extension: string): boolean {
  return ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp'].includes(
    normalized(extension),
  );
}
