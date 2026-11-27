import { describe, expect, it } from 'vitest';
import { noteBlocks, noteSpans } from './releaseNotes';

// Release notes are rendered from these blocks as plain text nodes, so the
// formatter only has to keep structure; it never produces HTML.
describe('release notes', () => {
  it('turns inline code, bold and links into plain spans', () => {
    expect(
      noteSpans('Use `F12` to **open** [the docs](https://x.example) now'),
    ).toEqual([
      { text: 'Use ' },
      { text: 'F12', code: true },
      { text: ' to ' },
      { text: 'open', bold: true },
      { text: ' the docs now' },
    ]);
    expect(noteSpans('a `**not bold**` b')).toEqual([
      { text: 'a ' },
      { text: '**not bold**', code: true },
      { text: ' b' },
    ]);
    expect(noteSpans('![logo](x.png) __strong__')).toEqual([
      { text: 'logo ' },
      { text: 'strong', bold: true },
    ]);
  });

  it('reduces HTML to text, never keeping it as markup', () => {
    const spans = noteSpans(
      '<img src=x onerror=alert(1)>Hi <script>alert(1)</script><b>there</b>',
    );
    expect(spans.map((span) => span.text).join('')).toBe('Hi alert(1)there');
    // Code spans keep their characters; the dialog shows them as text.
    expect(noteSpans('`<script>`')).toEqual([{ text: '<script>', code: true }]);
  });

  it('keeps headings, nested items, quotes and joined paragraphs', () => {
    const blocks = noteBlocks(
      [
        "## What's new",
        '',
        '- **Open Definition.** Select a name',
        '  - nested detail',
        '* star item',
        '1. numbered',
        '',
        '> **Note:** first line',
        '> second line',
        '',
        'A paragraph',
        'continues here.',
        '---',
        '#### Deep heading ####',
      ].join('\r\n'),
    );
    expect(
      blocks.map((block) => [
        block.kind,
        'level' in block ? block.level : 'depth' in block ? block.depth : null,
        block.spans.map((span) => span.text).join(''),
      ]),
    ).toEqual([
      ['heading', 2, "What's new"],
      ['item', 0, 'Open Definition. Select a name'],
      ['item', 1, 'nested detail'],
      ['item', 0, 'star item'],
      ['item', 0, 'numbered'],
      ['quote', null, 'Note: first line second line'],
      ['paragraph', null, 'A paragraph continues here.'],
      ['heading', 3, 'Deep heading'],
    ]);
    expect(blocks[1].spans[0].bold).toBe(true);
  });

  it('gives no blocks for empty notes', () => {
    expect(noteBlocks('')).toEqual([]);
    expect(noteBlocks('\n\n   \n')).toEqual([]);
  });
});
