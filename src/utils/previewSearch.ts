export function highlightPreviewMatches(
  container: HTMLElement,
  query: string,
): HTMLElement[] {
  for (const mark of container.querySelectorAll('mark[data-preview-search]'))
    mark.replaceWith(...mark.childNodes);
  container.normalize();
  if (!query) return [];

  const pattern = new RegExp(
    query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'),
    'gi',
  );
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  while (walker.nextNode()) nodes.push(walker.currentNode as Text);
  const marks: HTMLElement[] = [];
  // ponytail: matches stay within text nodes; use DOM ranges if cross-inline searches are needed.
  for (const node of nodes) {
    const text = node.data;
    const matches = [...text.matchAll(pattern)];
    if (!matches.length) continue;
    const fragment = document.createDocumentFragment();
    let offset = 0;
    for (const match of matches) {
      fragment.append(text.slice(offset, match.index));
      const mark = document.createElement('mark');
      mark.dataset.previewSearch = '';
      mark.textContent = match[0];
      fragment.append(mark);
      marks.push(mark);
      offset = match.index + match[0].length;
    }
    fragment.append(text.slice(offset));
    node.replaceWith(fragment);
  }
  return marks;
}
