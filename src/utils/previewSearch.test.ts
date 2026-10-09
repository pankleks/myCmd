// @vitest-environment jsdom
import { expect, it } from 'vitest';
import { highlightPreviewMatches } from './previewSearch';

it('highlights literal case-insensitive matches without changing Markdown markup', () => {
  const container = document.createElement('article');
  container.innerHTML = '<h1>A.b a.B</h1><p><code>a.b</code> untouched</p>';
  const original = container.innerHTML;
  expect(highlightPreviewMatches(container, 'a.b')).toHaveLength(3);
  expect(container.querySelector('code mark')?.textContent).toBe('a.b');
  expect(highlightPreviewMatches(container, 'missing')).toHaveLength(0);
  expect(container.innerHTML).toBe(original);
  expect(highlightPreviewMatches(container, '<img>')).toHaveLength(0);
  expect(highlightPreviewMatches(container, 'a.b')).toHaveLength(3);
  expect(highlightPreviewMatches(container, '')).toHaveLength(0);
  expect(container.innerHTML).toBe(original);
});
