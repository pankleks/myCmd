// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import {
  checkMarkdownNesting,
  MarkdownTooComplex,
  parseMarkdown,
} from './markdownParse';

const workers: FakeWorker[] = [];
class FakeWorker {
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;
  postMessage = vi.fn();
  terminate = vi.fn();
  constructor() {
    workers.push(this);
  }
}
beforeEach(() => {
  workers.length = 0;
  vi.useFakeTimers();
  vi.stubGlobal('Worker', FakeWorker);
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

it('returns HTML from the worker and stops it', async () => {
  const result = parseMarkdown('# hi', () => false);
  expect(workers[0].postMessage).toHaveBeenCalledWith('# hi');
  workers[0].onmessage!({ data: { html: '<h1>hi</h1>' } } as MessageEvent);
  await expect(result).resolves.toBe('<h1>hi</h1>');
  expect(workers[0].terminate).toHaveBeenCalled();
});

it('stops a worker that takes too long', async () => {
  const result = parseMarkdown('*a **'.repeat(10), () => false, 1000);
  const settled = expect(result).rejects.toBeInstanceOf(MarkdownTooComplex);
  await vi.advanceTimersByTimeAsync(1200);
  await settled;
  expect(workers[0].terminate).toHaveBeenCalled();
});

it('stops the worker when the preview closes', async () => {
  let closed = false;
  const result = parseMarkdown('text', () => closed);
  const settled = expect(result).rejects.toThrow('Preview closed');
  closed = true;
  await vi.advanceTimersByTimeAsync(150);
  await settled;
  expect(workers[0].terminate).toHaveBeenCalled();
});

it('treats worker failures as unrenderable Markdown', async () => {
  const failed = parseMarkdown('x', () => false);
  workers[0].onmessage!({ data: { error: true } } as MessageEvent);
  await expect(failed).rejects.toBeInstanceOf(MarkdownTooComplex);
  const crashed = parseMarkdown('x', () => false);
  workers[1].onerror!();
  await expect(crashed).rejects.toBeInstanceOf(MarkdownTooComplex);
});

it('rejects deep nesting before starting a worker', async () => {
  expect(() => checkMarkdownNesting('>'.repeat(300) + ' x')).toThrow(
    MarkdownTooComplex,
  );
  expect(() =>
    checkMarkdownNesting(
      Array.from({ length: 200 }, (_, i) => ' '.repeat(i * 2) + '- a').join(
        '\n',
      ),
    ),
  ).toThrow(MarkdownTooComplex);
  expect(() =>
    checkMarkdownNesting('    code\n> quote\n  - item'),
  ).not.toThrow();
  expect(() => checkMarkdownNesting('a'.repeat(10_000))).not.toThrow();
  await expect(
    parseMarkdown('>'.repeat(300), () => false),
  ).rejects.toBeInstanceOf(MarkdownTooComplex);
  expect(workers).toHaveLength(0);
});
