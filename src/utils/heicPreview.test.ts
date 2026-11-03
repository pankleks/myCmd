// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { heicPreview } from './heicPreview';

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
const context = { createImageData: vi.fn(), putImageData: vi.fn() };
const createUrl = vi.fn();
const dataUrl = 'data:image/heic;base64,AQ==';
function decoded() {
  workers[0].onmessage?.({
    data: {
      width: 1,
      height: 1,
      data: new Uint8ClampedArray([21, 21, 21, 255]),
    },
  } as MessageEvent);
}
beforeEach(() => {
  vi.clearAllMocks();
  workers.length = 0;
  vi.stubGlobal('Worker', FakeWorker);
  vi.stubGlobal(
    'URL',
    class extends URL {
      static createObjectURL = createUrl;
    },
  );
  createUrl.mockReturnValue('blob:heic-preview');
  context.createImageData.mockImplementation((width, height) => ({
    width,
    height,
    data: new Uint8ClampedArray(width * height * 4),
  }));
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
    context as unknown as CanvasRenderingContext2D,
  );
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(
    (callback) => callback(new Blob(['png'], { type: 'image/png' })),
  );
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it('decodes in a worker and returns a browser-readable PNG URL', async () => {
  const pending = heicPreview(dataUrl, new AbortController().signal);
  expect(workers[0].postMessage).toHaveBeenCalledOnce();
  decoded();
  expect(await pending).toBe('blob:heic-preview');
  expect(context.putImageData).toHaveBeenCalledOnce();
  expect(workers[0].terminate).toHaveBeenCalledOnce();
  expect(createUrl).toHaveBeenCalledWith(expect.any(Blob));
});

it('terminates decoding immediately when the preview closes', async () => {
  const controller = new AbortController();
  const pending = heicPreview(dataUrl, controller.signal);
  controller.abort();
  await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  expect(workers[0].terminate).toHaveBeenCalledOnce();
  expect(createUrl).not.toHaveBeenCalled();
});

it('times out stalled decoders', async () => {
  vi.useFakeTimers();
  const pending = heicPreview(dataUrl, new AbortController().signal);
  const rejected = expect(pending).rejects.toThrow('timed out');
  await vi.advanceTimersByTimeAsync(30_000);
  await rejected;
  expect(workers[0].terminate).toHaveBeenCalledOnce();
});

it('reports decoder errors and frees the worker', async () => {
  const pending = heicPreview(dataUrl, new AbortController().signal);
  workers[0].onmessage?.({ data: { error: 'Invalid HEIC' } } as MessageEvent);
  await expect(pending).rejects.toThrow('Invalid HEIC');
  expect(workers[0].terminate).toHaveBeenCalledOnce();
});

it('does not create object URLs if closed during PNG encoding', async () => {
  let encode!: BlobCallback;
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(
    (callback) => {
      encode = callback;
    },
  );
  const controller = new AbortController();
  const pending = heicPreview(dataUrl, controller.signal);
  decoded();
  await vi.waitFor(() => expect(encode).toBeDefined());
  controller.abort();
  encode(new Blob(['png']));
  await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  expect(createUrl).not.toHaveBeenCalled();
});

it('rejects non-HEIC inputs before creating a worker', async () => {
  await expect(
    heicPreview('data:image/png;base64,AQ==', new AbortController().signal),
  ).rejects.toThrow('Invalid');
  expect(workers).toHaveLength(0);
});
