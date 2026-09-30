// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { renderMarkdownPreview } from './markdownPreview';

const image = {
  dataUrl: 'data:image/png;base64,cG5n',
  width: 1,
  height: 1,
  frames: 1,
};
function documentFor(html: string) {
  return new DOMParser().parseFromString(html, 'text/html');
}

describe('Markdown preview', () => {
  it('counts all GIF frames toward the aggregate pixel budget', async () => {
    const loader = vi.fn().mockResolvedValue({
      ...image,
      dataUrl: 'data:image/gif;base64,R0lG',
      width: 4000,
      height: 2000,
      frames: 2,
    });
    const html = await renderMarkdownPreview(
      '![](a.gif) ![](b.gif) ![](c.gif)',
      loader,
    );
    expect(loader).toHaveBeenCalledTimes(2);
    expect(documentFor(html).querySelectorAll('img[src]')).toHaveLength(2);
  });

  it.each([0, -1, 1.5, NaN, 101])(
    'rejects invalid frame counts (%s)',
    async (frames) => {
      const html = await renderMarkdownPreview(
        '![](a.gif)',
        vi.fn().mockResolvedValue({ ...image, frames }),
      );
      expect(documentFor(html).querySelector('img')?.hasAttribute('src')).toBe(
        false,
      );
    },
  );
  it('stops loading at the aggregate pixel budget even for tiny compressed images', async () => {
    const loader = vi
      .fn()
      .mockResolvedValue({ ...image, width: 4000, height: 4000 });
    const html = await renderMarkdownPreview(
      '![](a.png) ![](b.png) ![](c.png)',
      loader,
    );
    expect(loader).toHaveBeenCalledTimes(2);
    expect(documentFor(html).querySelectorAll('img[src]')).toHaveLength(2);
  });

  it('does not publish an image that would cross the pixel budget', async () => {
    const loader = vi
      .fn()
      .mockResolvedValue({ ...image, width: 4000, height: 3000 });
    const html = await renderMarkdownPreview(
      '![](a.png) ![](b.png) ![](c.png)',
      loader,
    );
    expect(loader).toHaveBeenCalledTimes(3);
    expect(documentFor(html).querySelectorAll('img[src]')).toHaveLength(2);
  });

  it.each([0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER])(
    'rejects invalid image dimensions (%s)',
    async (width) => {
      const loader = vi.fn().mockResolvedValue({ ...image, width, height: 2 });
      const html = await renderMarkdownPreview('![](a.png)', loader);
      expect(documentFor(html).querySelector('img')?.hasAttribute('src')).toBe(
        false,
      );
    },
  );
  it('does not load images when rendering is already cancelled', async () => {
    const loader = vi.fn();
    expect(
      await renderMarkdownPreview('![](image.png)', loader, () => true),
    ).toBe('');
    expect(loader).not.toHaveBeenCalled();
  });

  it('adds a fallback description when an undescribed image cannot be loaded', async () => {
    const html = await renderMarkdownPreview(
      '![](missing.png)',
      vi.fn().mockRejectedValue(new Error('missing')),
    );
    const img = documentFor(html).querySelector('img');
    expect(img?.getAttribute('alt')).toBe('Image unavailable in preview');
    expect(img?.hasAttribute('src')).toBe(false);
  });

  it('does not load images without a source attribute', async () => {
    const loader = vi.fn();
    const html = await renderMarkdownPreview('<img alt="no source">', loader);
    expect(loader).not.toHaveBeenCalled();
    expect(documentFor(html).querySelector('img')?.hasAttribute('src')).toBe(
      false,
    );
  });
  it('renders Markdown while removing active HTML and navigable links', async () => {
    const html = await renderMarkdownPreview(
      '# Heading\n\n**bold** [link](https://example.com)\n<script>alert(1)</script><iframe src="https://example.com"></iframe><img src="picture.png" onerror="alert(1)" style="color:red" srcset="other.png 2x">',
      vi.fn().mockResolvedValue(image),
    );
    const doc = documentFor(html);
    expect(doc.querySelector('h1')?.textContent).toBe('Heading');
    expect(doc.querySelector('strong')?.textContent).toBe('bold');
    expect(doc.querySelector('script, iframe, a')).toBeNull();
    expect(doc.querySelector('[onerror], [style], [srcset]')).toBeNull();
    expect(doc.querySelector('img')?.getAttribute('src')).toBe(image.dataUrl);
  });

  it('does not retain original image URLs when local loading fails', async () => {
    const html = await renderMarkdownPreview(
      '![alt](https://example.com/image.png)',
      vi.fn().mockRejectedValue(new Error('not local')),
    );
    const img = documentFor(html).querySelector('img');
    expect(img?.hasAttribute('src')).toBe(false);
    expect(img?.getAttribute('alt')).toBe('alt');
  });

  it('rejects non-raster URLs returned by the loader', async () => {
    const html = await renderMarkdownPreview(
      '![](image.svg)',
      vi.fn().mockResolvedValue({
        ...image,
        dataUrl: 'data:image/svg+xml;base64,PHN2Zz4=',
      }),
    );
    expect(documentFor(html).querySelector('img')?.hasAttribute('src')).toBe(
      false,
    );
  });

  it('limits image loading to 24 images', async () => {
    const loader = vi.fn().mockResolvedValue(image);
    const html = await renderMarkdownPreview(
      Array.from({ length: 30 }, (_, index) => `![](image${index}.png)`).join(
        '\n',
      ),
      loader,
    );
    expect(loader).toHaveBeenCalledTimes(24);
    expect(documentFor(html).querySelectorAll('img[src]')).toHaveLength(24);
  });

  it('stops reading images once the aggregate budget is exhausted', async () => {
    const loader = vi.fn().mockResolvedValue({
      ...image,
      dataUrl:
        'data:image/png;base64,' +
        'AAAA'.repeat((4 * 1024 * 1024 - 1) / 3) +
        'AA==',
    });
    await renderMarkdownPreview(
      Array.from({ length: 10 }, () => '![](image.png)').join('\n'),
      loader,
    );
    expect(loader).toHaveBeenCalledTimes(3);
  });

  it('discards a preview cancelled during image loading', async () => {
    let cancelled = false;
    const loader = vi.fn().mockImplementation(async () => {
      cancelled = true;
      return image;
    });
    expect(
      await renderMarkdownPreview(
        '![](first.png) ![](second.png)',
        loader,
        () => cancelled,
      ),
    ).toBe('');
    expect(loader).toHaveBeenCalledOnce();
  });
});
