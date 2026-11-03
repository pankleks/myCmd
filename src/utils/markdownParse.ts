import { marked } from 'marked';

/** Markdown that cannot be rendered safely; the caller shows source text instead. */
export class MarkdownTooComplex extends Error {
  name = 'MarkdownTooComplex';
}

// Deep nesting makes marked exhaust memory, which a worker cannot contain.
const MAX_LINE_PREFIX = 256;
const PARSE_TIMEOUT_MS = 5000;

export function checkMarkdownNesting(content: string) {
  const prefix = new RegExp(`^[ \\t>]{${MAX_LINE_PREFIX + 1}}`, 'm');
  if (prefix.test(content))
    throw new MarkdownTooComplex('Markdown is nested too deeply to preview.');
}

/** Parses Markdown off the main thread with a time limit, so hostile files cannot freeze the app. */
export async function parseMarkdown(
  content: string,
  isCancelled: () => boolean,
  timeoutMs = PARSE_TIMEOUT_MS,
): Promise<string> {
  checkMarkdownNesting(content);
  // Test environments without workers; the desktop webview always has them.
  if (typeof Worker === 'undefined')
    return marked.parse(content, { gfm: true });
  return new Promise<string>((resolve, reject) => {
    const worker = new Worker(
      new URL('../workers/markdown.worker.ts', import.meta.url),
      { type: 'module' },
    );
    const started = Date.now();
    const timer = setInterval(() => {
      if (isCancelled())
        finish(new DOMException('Preview closed', 'AbortError'));
      else if (Date.now() - started > timeoutMs)
        finish(new MarkdownTooComplex('Markdown took too long to render.'));
    }, 100);
    function finish(error?: Error, html?: string) {
      clearInterval(timer);
      worker.terminate();
      if (error) reject(error);
      else resolve(html!);
    }
    worker.onmessage = (
      event: MessageEvent<{ html?: string; error?: true }>,
    ) => {
      if (typeof event.data.html === 'string')
        finish(undefined, event.data.html);
      else finish(new MarkdownTooComplex('Markdown could not be rendered.'));
    };
    worker.onerror = () =>
      finish(new MarkdownTooComplex('Markdown could not be rendered.'));
    worker.postMessage(content);
  });
}
