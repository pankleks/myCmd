export type ViewerLanguage =
  | 'markdown'
  | 'json'
  | 'javascript'
  | 'typescript'
  | 'csharp'
  | 'rust'
  | 'c'
  | 'cpp'
  | 'plaintext';

export function isMarkdownFile(extension: string): boolean {
  return ['md', 'markdown', 'mdown', 'mkd'].includes(
    extension.toLowerCase().replace(/^\./, ''),
  );
}

export function isImageFile(extension: string): boolean {
  return ['png', 'jpg', 'jpeg', 'gif', 'webp', 'avif', 'bmp'].includes(
    extension.toLowerCase().replace(/^\./, ''),
  );
}

export function viewerLanguage(extension: string): ViewerLanguage {
  const original = extension.replace(/^\./, '');
  if (original === 'C' || original === 'H') return 'cpp';
  const normalized = extension.toLowerCase().replace(/^\./, '');
  if (isMarkdownFile(normalized)) return 'markdown';
  switch (normalized) {
    case 'json':
    case 'jsonc':
      return 'json';
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return 'javascript';
    case 'ts':
    case 'tsx':
    case 'mts':
    case 'cts':
      return 'typescript';
    case 'cs':
    case 'csx':
    case 'cake':
      return 'csharp';
    case 'rs':
    case 'rlib':
      return 'rust';
    case 'c':
    case 'h':
      return 'c';
    case 'cpp':
    case 'cc':
    case 'cxx':
    case 'hpp':
    case 'hh':
    case 'hxx':
      return 'cpp';
    default:
      return 'plaintext';
  }
}
