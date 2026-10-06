// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import PdfPreview from './PdfPreview.svelte';
import FilePreview from './FilePreview.svelte';

const mocks = vi.hoisted(() => ({
  read: vi.fn(),
  load: vi.fn(),
  getPage: vi.fn(),
  render: vi.fn(),
  cleanup: vi.fn(),
  destroy: vi.fn(),
  cancel: vi.fn(),
}));
vi.mock('../filesystem/api', () => ({
  api: { readPdfPreview: mocks.read },
  errorMessage: (error: Error) => error.message,
}));
vi.mock('pdfjs-dist/legacy/build/pdf.mjs', () => ({
  GlobalWorkerOptions: {},
  getDocument: mocks.load,
}));
let target: HTMLDivElement;
let component: ReturnType<typeof mount> | undefined;
let onerror = vi.fn<(message: string) => void>();
let entries: IntersectionObserverCallback;

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal(
    'IntersectionObserver',
    class {
      constructor(callback: IntersectionObserverCallback) {
        entries = callback;
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  );
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(640);
  Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
    value: vi.fn(),
    configurable: true,
    writable: true,
  });
  mocks.read.mockResolvedValue(new ArrayBuffer(10));
  mocks.render.mockReturnValue({
    promise: Promise.resolve(),
    cancel: mocks.cancel,
  });
  mocks.getPage.mockResolvedValue({
    getViewport: ({ scale }: { scale: number }) => ({
      width: 600 * scale,
      height: 800 * scale,
    }),
    render: mocks.render,
    cleanup: mocks.cleanup,
  });
  mocks.destroy.mockResolvedValue(undefined);
  mocks.load.mockReturnValue({
    promise: Promise.resolve({ numPages: 3, getPage: mocks.getPage }),
    destroy: mocks.destroy,
  });
  target = document.createElement('div');
  document.body.append(target);
  onerror = vi.fn();
  component = mount(PdfPreview, {
    target,
    props: {
      path: '/sample.pdf',
      name: 'sample.pdf',
      autofocus: true,
      onerror,
    },
  });
  flushSync();
});
afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  target.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(HTMLElement.prototype, 'scrollIntoView');
});
async function loaded() {
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelectorAll('canvas')).toHaveLength(3);
  });
}
function intersect(canvas: HTMLCanvasElement, visible: boolean) {
  entries(
    [
      {
        target: canvas.parentElement!,
        isIntersecting: visible,
      } as unknown as IntersectionObserverEntry,
    ],
    {} as IntersectionObserver,
  );
}

it('renders only nearby pages, limits canvas pixels and releases pages outside the viewport', async () => {
  await loaded();
  expect(mocks.render).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(target.querySelector('.pdf-pages'));
  const canvas = target.querySelector('canvas')!;
  intersect(canvas, true);
  await vi.waitFor(() => expect(mocks.render).toHaveBeenCalledOnce());
  expect(canvas.width * canvas.height).toBeLessThanOrEqual(4_000_000);
  expect(mocks.load).toHaveBeenCalledWith(
    expect.objectContaining({ enableXfa: false, cMapUrl: '/pdfjs/cmaps/' }),
  );
  intersect(canvas, false);
  expect(canvas.width).toBe(0);
  expect(canvas.height).toBe(0);
  expect(onerror).not.toHaveBeenCalled();
});

it('supports zoom, fit width and page navigation', async () => {
  await loaded();
  expect(
    parseFloat(target.querySelector<HTMLElement>('.pdf-page')!.style.width),
  ).toBe(600);
  expect(target.querySelector('.pdf-toolbar')!.textContent).toContain('100%');
  const scroll = vi.fn();
  target.querySelectorAll('.pdf-page')[1].scrollIntoView = scroll;
  target.querySelector<HTMLButtonElement>('[aria-label="Next page"]')!.click();
  flushSync();
  expect(scroll).toHaveBeenCalled();
  expect(target.querySelector<HTMLInputElement>('input')!.value).toBe('2');
  target.querySelector<HTMLButtonElement>('[aria-label="Zoom in"]')!.click();
  flushSync();
  expect(
    parseFloat(target.querySelector<HTMLElement>('.pdf-page')!.style.width),
  ).toBeCloseTo(750);
  Array.from(target.querySelectorAll('button'))
    .find((button) => button.textContent === 'Fit width')!
    .click();
  flushSync();
  expect(
    parseFloat(target.querySelector<HTMLElement>('.pdf-page')!.style.width),
  ).toBeCloseTo(608);
});

it('places PDF controls in the shared header without the external-open button', async () => {
  await unmount(component!);
  component = mount(FilePreview, {
    target,
    props: { path: '/sample.pdf', name: 'sample.pdf', extension: 'pdf' },
  });
  flushSync();
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('.viewer-header .pdf-toolbar')).not.toBeNull();
  });
  expect(target.querySelector('.viewer-editor-shell .pdf-toolbar')).toBeNull();
  expect(target.textContent).not.toContain('Open in default app');
});

it.each(['PasswordException', 'InvalidPDFException'])(
  'reports %s with no unhandled load failure',
  async (name) => {
    const error = Object.assign(new Error('Invalid document'), { name });
    mocks.load.mockReturnValue({
      promise: Promise.reject(error),
      destroy: mocks.destroy,
    });
    await vi.waitFor(() => expect(onerror).toHaveBeenCalled());
    expect(onerror.mock.calls[0][0]).toContain(
      name === 'PasswordException' ? 'password-protected' : 'Invalid document',
    );
  },
);

it('ignores a file read which completes after unmount', async () => {
  let resolve!: (data: ArrayBuffer) => void;
  mocks.read.mockReturnValue(
    new Promise((done) => {
      resolve = done;
    }),
  );
  await unmount(component!);
  mocks.load.mockClear();
  component = mount(PdfPreview, {
    target,
    props: { path: '/late.pdf', name: 'late.pdf', onerror },
  });
  flushSync();
  await unmount(component);
  component = undefined;
  resolve(new ArrayBuffer(10));
  await Promise.resolve();
  flushSync();
  expect(mocks.load).not.toHaveBeenCalled();
  expect(onerror).not.toHaveBeenCalled();
});
