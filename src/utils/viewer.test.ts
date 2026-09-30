import { describe, expect, it } from 'vitest';
import { isImageFile, isMarkdownFile, viewerLanguage } from './viewer';

describe('viewerLanguage', () => {
  it('uses XML highlighting for XML and related document formats', () => {
    for (const extension of [
      'xml',
      '.XML',
      'xsd',
      'xsl',
      'xslt',
      'svg',
      'xaml',
    ])
      expect(viewerLanguage(extension)).toBe('xml');
  });
  it('uses Svelte highlighting for component files', () => {
    for (const extension of ['svelte', '.svelte', 'SVELTE'])
      expect(viewerLanguage(extension)).toBe('svelte');
  });
  it('uses Markdown highlighting for Markdown source files', () => {
    expect(viewerLanguage('md')).toBe('markdown');
    expect(viewerLanguage('.markdown')).toBe('markdown');
  });

  it('uses JSON highlighting for JSON files', () => {
    expect(viewerLanguage('json')).toBe('json');
    expect(viewerLanguage('.jsonc')).toBe('json');
  });

  it('uses JavaScript highlighting for JavaScript files', () => {
    for (const extension of ['js', 'jsx', 'mjs', 'cjs'])
      expect(viewerLanguage(extension)).toBe('javascript');
  });

  it('uses TypeScript highlighting for TypeScript files', () => {
    for (const extension of ['ts', 'tsx', 'mts', 'cts'])
      expect(viewerLanguage(extension)).toBe('typescript');
  });

  it('uses C# highlighting for C# files and scripts', () => {
    for (const extension of ['cs', '.CS', 'csx', 'cake'])
      expect(viewerLanguage(extension)).toBe('csharp');
  });

  it('uses Rust highlighting for Rust source files', () => {
    expect(viewerLanguage('rs')).toBe('rust');
    expect(viewerLanguage('.RLIB')).toBe('rust');
  });

  it('uses C and C++ highlighting for their source and header files', () => {
    for (const extension of ['c', '.h'])
      expect(viewerLanguage(extension)).toBe('c');
    for (const extension of [
      'cpp',
      'cc',
      'cxx',
      'hpp',
      'hh',
      'hxx',
      '.C',
      '.H',
    ])
      expect(viewerLanguage(extension)).toBe('cpp');
  });

  it('falls back to plain text for other file types', () => {
    expect(viewerLanguage('go')).toBe('plaintext');
    expect(viewerLanguage('')).toBe('plaintext');
  });
});

describe('isMarkdownFile', () => {
  it('recognizes common Markdown extensions case-insensitively', () => {
    for (const extension of ['md', '.MD', 'markdown', 'mdown', 'mkd'])
      expect(isMarkdownFile(extension)).toBe(true);
    expect(isMarkdownFile('txt')).toBe(false);
  });
});

describe('isImageFile', () => {
  it('recognizes supported raster image extensions case-insensitively', () => {
    for (const extension of [
      'png',
      '.JPG',
      'jpeg',
      'gif',
      'webp',
      'avif',
      'bmp',
    ])
      expect(isImageFile(extension)).toBe(true);
    expect(isImageFile('svg')).toBe(false);
    expect(isImageFile('txt')).toBe(false);
  });
});
