import { marked } from 'marked';
import DOMPurify from 'dompurify';
import type { PreviewImage } from '../filesystem/api';

const MAX_IMAGES = 24;
const MAX_IMAGE_BYTES = 12 * 1024 * 1024;
const MAX_IMAGE_PIXELS = 32_000_000;
let diagramId = 0;
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
  loadImage: (source: string) => Promise<PreviewImage>,
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
  let totalPixels = 0;
  for (const [index, image] of images.entries()) {
    if (isCancelled()) return '';
    const source = sources[index];
    if (
      !source ||
      index >= MAX_IMAGES ||
      totalBytes >= MAX_IMAGE_BYTES ||
      totalPixels >= MAX_IMAGE_PIXELS
    )
      continue;
    try {
      const { dataUrl, width, height, frames } = await loadImage(source);
      if (isCancelled()) return '';
      const pixels = width * height * frames;
      if (
        !Number.isSafeInteger(width) ||
        !Number.isSafeInteger(height) ||
        !Number.isSafeInteger(frames) ||
        frames <= 0 ||
        frames > 100 ||
        width <= 0 ||
        height <= 0 ||
        !Number.isSafeInteger(pixels)
      )
        continue;
      if (totalPixels + pixels > MAX_IMAGE_PIXELS) break;
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
      totalPixels += pixels;
      image.setAttribute('src', dataUrl);
      image.setAttribute('loading', 'lazy');
      image.setAttribute('decoding', 'async');
    } catch {
      if (!image.getAttribute('alt'))
        image.setAttribute('alt', 'Image unavailable in preview');
    }
  }
  const diagrams = preview.body.querySelectorAll('pre > code.language-mermaid');
  if (diagrams.length && !isCancelled()) {
    try {
      const { default: mermaid } = await import('mermaid');
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: 'strict',
        suppressErrorRendering: true,
        maxTextSize: 50_000,
        maxEdges: 500,
        theme: 'dark',
        htmlLabels: false,
        flowchart: { htmlLabels: false },
        secure: [
          'secure',
          'securityLevel',
          'startOnLoad',
          'suppressErrorRendering',
          'maxTextSize',
          'maxEdges',
          'htmlLabels',
          'flowchart',
        ],
      });
      for (const [index, code] of Array.from(diagrams).entries()) {
        if (isCancelled()) return '';
        try {
          if (index >= 24 || (code.textContent?.length ?? 0) > 50_000)
            throw new Error('Diagram preview limit exceeded');
          const { svg } = await mermaid.render(
            `preview-diagram-${++diagramId}`,
            code.textContent ?? '',
          );
          if (isCancelled()) return '';
          // An image keeps diagram CSS, links and scripts out of the app DOM.
          const safeSvg = DOMPurify.sanitize(svg, {
            USE_PROFILES: { svg: true, svgFilters: true },
            FORBID_TAGS: ['foreignObject', 'a', 'image'],
          });
          const image = preview.createElement('img');
          const svgRoot = new DOMParser().parseFromString(
            safeSvg,
            'image/svg+xml',
          ).documentElement;
          const [, , width, height] = (svgRoot.getAttribute('viewBox') ?? '')
            .trim()
            .split(/[\s,]+/)
            .map(Number);
          if (
            Number.isFinite(width) &&
            width > 0 &&
            Number.isFinite(height) &&
            height > 0
          ) {
            image.width = Math.ceil(width);
            image.height = Math.ceil(height);
          }
          image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(safeSvg)}`;
          image.alt = 'Mermaid diagram (source available in source view)';
          code.parentElement?.replaceWith(image);
        } catch {
          const error = preview.createElement('p');
          error.textContent =
            'Diagram unavailable: invalid Mermaid syntax or preview limit exceeded.';
          code.parentElement?.after(error);
        }
      }
    } catch {
      const error = preview.createElement('p');
      error.textContent = 'Mermaid diagram renderer unavailable.';
      diagrams[0]?.parentElement?.before(error);
    }
  }
  return isCancelled()
    ? ''
    : DOMPurify.sanitize(preview.body.innerHTML, sanitizeOptions);
}
