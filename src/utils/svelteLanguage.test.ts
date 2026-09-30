// @vitest-environment jsdom
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import { svelteLanguage } from './svelteLanguage';

let monaco: typeof import('monaco-editor/editor/editor.api.js');
beforeAll(async () => {
  vi.stubGlobal('CSS', { escape: (value: string) => value });
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
    })),
  );
  monaco = await import('monaco-editor/editor/editor.api.js');
  // Deterministic embedded grammars verify language switching independently
  // of Monaco's asynchronous loading of its bundled language definitions.
  for (const id of ['javascript', 'typescript', 'css']) {
    monaco.languages.register({ id });
    monaco.languages.setMonarchTokensProvider(id, {
      tokenPostfix: `.${id}`,
      tokenizer: {
        root: [
          [/\w+/, 'embedded'],
          [/./, ''],
        ],
      },
    });
  }
  monaco.languages.register({ id: 'svelte' });
  monaco.languages.setMonarchTokensProvider('svelte', svelteLanguage);
});
afterAll(() => vi.unstubAllGlobals());
function tokens(text: string) {
  return monaco.editor
    .tokenize(text, 'svelte')
    .flat()
    .map((token) => token.type);
}
describe('Svelte syntax highlighting', () => {
  it('highlights markup, attributes, template blocks and expressions', () => {
    const result = tokens(
      '{#if visible}\n<Button on:click={handle} disabled={count > 2}>{count}</Button>\n{/if}',
    );
    expect(result).toContain('keyword.svelte');
    expect(result).toContain('tag.svelte');
    expect(result).toContain('attribute.name.svelte');
    expect(result).toContain('identifier.svelte');
    expect(result).toContain('number.svelte');
  });
  it('keeps nested objects and braces in strings inside expressions', () => {
    const lines = monaco.editor.tokenize(
      '<div class="item-{fn({ label: \'}\' })}">done</div>',
      'svelte',
    );
    expect(lines[0].map((token) => token.type)).toContain('string.svelte');
    expect(lines[0].at(-1)?.type).toBe('delimiter.svelte');
  });
  it('highlights runes and render/snippet directives', () => {
    const result = tokens(
      '{@const count = $state(1)}\n{#snippet item(value)}{value}{/snippet}\n{@render item(count)}',
    );
    expect(result).toContain('predefined.svelte');
    expect(result).toContain('keyword.svelte');
  });
  it('embeds TypeScript, JavaScript and CSS and returns to markup', () => {
    for (const [tag, attribute, language] of [
      ['script', ' lang="ts"', 'typescript'],
      ['script', '', 'javascript'],
      ['style', '', 'css'],
    ]) {
      const result = tokens(
        `<${tag}${attribute}>\nvalue\n</${tag}>\n<p>after</p>`,
      );
      expect(result).toContain(`embedded.${language}`);
      expect(result).toContain('tag.svelte');
      expect(result.at(-1)).toBe('delimiter.svelte');
    }
  });
});
