import { marked } from 'marked';
import DOMPurify from 'dompurify';

const MAX_IMAGES = 24;
const MAX_IMAGE_BYTES = 12 * 1024 * 1024;
const sanitizeOptions = {
  USE_PROFILES: { html: true },
  FORBID_TAGS: [
    'style',
    'iframe',
    'object',
    'embed',
    'a',
    'source',
    'video',
    'audio',
    'form',
  ],
  FORBID_ATTR: ['style', 'target', 'srcset', 'action', 'formaction'],
};

export async function renderMarkdownPreview(
  content: string,
  loadImage: (source: string) => Promise<string>,
  isCancelled: () => boolean = () => false,
): Promise<string> {
  const html = await marked.parse(content, { gfm: true });
  if (isCancelled()) return '';
  const preview = new DOMParser().parseFromString(
    DOMPurify.sanitize(html, sanitizeOptions),
    'text/html',
  );
  const images = Array.from(preview.body.querySelectorAll('img'));
  const sources = images.map((image) => image.getAttribute('src'));
  // Strip original URLs before inserting the document into the live DOM.
  for (const image of images) image.removeAttribute('src');
  let totalBytes = 0;
  for (const [index, image] of images.entries()) {
    if (isCancelled()) return '';
    const source = sources[index];
    if (!source || index >= MAX_IMAGES || totalBytes >= MAX_IMAGE_BYTES)
      continue;
    try {
      const dataUrl = await loadImage(source);
      if (isCancelled()) return '';
      // Accept only raster data returned by the local image reader.
      if (
        !/^data:image\/(png|jpeg|gif|webp|avif|bmp);base64,[A-Za-z0-9+/]*={0,2}$/.test(
          dataUrl,
        )
      )
        continue;
      const payload = dataUrl.slice(dataUrl.indexOf(',') + 1);
      const padding = payload.endsWith('==')
        ? 2
        : payload.endsWith('=')
          ? 1
          : 0;
      const bytes = Math.floor((payload.length * 3) / 4) - padding;
      if (totalBytes + bytes > MAX_IMAGE_BYTES) break;
      totalBytes += bytes;
      image.setAttribute('src', dataUrl);
      image.setAttribute('loading', 'lazy');
      image.setAttribute('decoding', 'async');
    } catch {
      if (!image.getAttribute('alt'))
        image.setAttribute('alt', 'Image unavailable in preview');
    }
  }
  return isCancelled()
    ? ''
    : DOMPurify.sanitize(preview.body.innerHTML, sanitizeOptions);
}
