// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { renderMarkdownPreview } from './markdownPreview';

const image = 'data:image/png;base64,cG5n';
function documentFor(html: string) {
  return new DOMParser().parseFromString(html, 'text/html');
}

describe('Markdown preview', () => {
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
    expect(doc.querySelector('img')?.getAttribute('src')).toBe(image);
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
      vi.fn().mockResolvedValue('data:image/svg+xml;base64,PHN2Zz4='),
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
    const loader = vi
      .fn()
      .mockResolvedValue(
        'data:image/png;base64,' +
          'AAAA'.repeat((4 * 1024 * 1024 - 1) / 3) +
          'AA==',
      );
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
