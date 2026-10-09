// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import FilePreview from './FilePreview.svelte';

const mocks = vi.hoisted(() => ({
  image: vi.fn(),
  text: vi.fn(),
  markdownImage: vi.fn(),
  open: vi.fn(),
  editor: vi.fn(),
  dispose: vi.fn(),
  focus: vi.fn(),
  setValue: vi.fn(),
  getAction: vi.fn(),
  find: vi.fn(),
}));
vi.mock('../filesystem/api', () => ({
  api: {
    readImagePreview: mocks.image,
    readTextPreview: mocks.text,
    readMarkdownImage: mocks.markdownImage,
    open: mocks.open,
  },
  errorMessage: (cause: unknown) =>
    cause instanceof Error ? cause.message : String(cause),
}));
vi.mock('../monaco', () => ({ createViewerEditor: mocks.editor }));

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;
function preview(extension = 'png', autofocus = false, onopened?: () => void) {
  component = mount(FilePreview, {
    target,
    props: {
      path: `/file.${extension}`,
      name: `file.${extension}`,
      extension,
      autofocus,
      onopened,
    },
  });
  flushSync();
}
async function settle(check: () => void) {
  await vi.waitFor(() => {
    flushSync();
    check();
  });
}
beforeEach(() => {
  vi.resetAllMocks();
  mocks.editor.mockReturnValue({
    dispose: mocks.dispose,
    focus: mocks.focus,
    setValue: mocks.setValue,
    getAction: mocks.getAction,
  });
  mocks.getAction.mockReturnValue({ run: mocks.find });
  target = document.createElement('div');
  document.body.append(target);
});
afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  target.remove();
});

describe('FilePreview', () => {
  it.each(['txt', 'ts', 'json', 'jsonc', 'rs', 'svelte'])(
    'opens Monaco search from the toolbar and keyboard for %s files',
    async (extension) => {
      mocks.text.mockResolvedValue('searchable text');
      preview(extension);
      await settle(() => expect(mocks.editor).toHaveBeenCalledOnce());
      await settle(() =>
        expect(
          target.querySelector<HTMLButtonElement>(
            '[title="Search file (Ctrl/Cmd+F)"]',
          )?.disabled,
        ).toBe(false),
      );
      target
        .querySelector<HTMLButtonElement>('[title="Search file (Ctrl/Cmd+F)"]')!
        .click();
      expect(mocks.getAction).toHaveBeenCalledWith('actions.find');
      expect(mocks.find).toHaveBeenCalledOnce();
      for (const modifier of ['ctrlKey', 'metaKey']) {
        const event = new KeyboardEvent('keydown', {
          key: 'f',
          [modifier]: true,
          bubbles: true,
          cancelable: true,
        });
        target.querySelector('.viewer-editor')!.dispatchEvent(event);
        expect(event.defaultPrevented).toBe(true);
      }
      expect(mocks.find).toHaveBeenCalledTimes(3);
      expect(target.querySelector('.preview-search')).toBeNull();
      expect(mocks.setValue).not.toHaveBeenCalled();
    },
  );
  it('searches rendered Markdown, wraps navigation, and closes search without closing the viewer', async () => {
    mocks.text.mockResolvedValue('# Hello\n\nHello **world** hello');
    preview('md');
    await settle(() =>
      expect(target.querySelector('h1')?.textContent).toBe('Hello'),
    );
    const article = target.querySelector('article')!;
    const key = (element: Element, name: string, options = {}) => {
      const event = new KeyboardEvent('keydown', {
        key: name,
        bubbles: true,
        cancelable: true,
        ...options,
      });
      element.dispatchEvent(event);
      flushSync();
      return event;
    };
    expect(key(article, 'f', { ctrlKey: true }).defaultPrevented).toBe(true);
    await settle(() =>
      expect(document.activeElement).toBe(target.querySelector('input')),
    );
    const input = target.querySelector('input')!;
    expect(
      target
        .querySelector('[aria-label="Next match"] svg')
        ?.getAttribute('viewBox'),
    ).toBe('0 0 24 24');
    input.value = 'hello';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(target.querySelectorAll('mark')).toHaveLength(3);
    expect(
      target.querySelector('.preview-search [role="status"]')?.textContent,
    ).toBe('1 of 3');
    key(input, 'Enter', { shiftKey: true });
    expect(
      target.querySelector('.preview-search [role="status"]')?.textContent,
    ).toBe('3 of 3');
    target
      .querySelector<HTMLButtonElement>('[aria-label="Next match"]')!
      .click();
    flushSync();
    expect(
      target.querySelector('.preview-search [role="status"]')?.textContent,
    ).toBe('1 of 3');
    expect(target.querySelectorAll('mark.search-current')).toHaveLength(1);
    target.querySelector<HTMLButtonElement>('.viewer-toggle')!.click();
    await settle(() => expect(mocks.editor).toHaveBeenCalledOnce());
    expect(target.querySelector('.preview-search')).toBeNull();
    target
      .querySelector<HTMLButtonElement>('[title="Search file (Ctrl/Cmd+F)"]')!
      .click();
    expect(mocks.find).toHaveBeenCalledOnce();
    target.querySelector<HTMLButtonElement>('.viewer-toggle')!.click();
    await settle(() => expect(target.querySelectorAll('mark')).toHaveLength(3));
    const restoredInput = target.querySelector('input')!;
    restoredInput.value = 'missing';
    restoredInput.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(
      target.querySelector('.preview-search [role="status"]')?.textContent,
    ).toBe('0 of 0');
    expect(
      target.querySelector<HTMLButtonElement>('[aria-label="Next match"]')!
        .disabled,
    ).toBe(true);
    const parentKeydown = vi.fn();
    document.body.addEventListener('keydown', parentKeydown);
    key(restoredInput, 'Escape');
    document.body.removeEventListener('keydown', parentKeydown);
    expect(parentKeydown).not.toHaveBeenCalled();
    expect(target.querySelector('input')).toBeNull();
    expect(target.querySelector('mark')).toBeNull();
    expect(document.activeElement).toBe(target.querySelector('article'));
    expect(target.querySelector('article strong')?.textContent).toBe('world');
  });

  it('formats JSON only in the preview editor', async () => {
    mocks.text.mockResolvedValue('{"a":1}');
    preview('json');
    await settle(() =>
      expect(target.querySelector<HTMLButtonElement>('button')?.disabled).toBe(
        false,
      ),
    );
    target.querySelector<HTMLButtonElement>('button')!.click();
    expect(mocks.setValue).toHaveBeenCalledWith('{\n  "a": 1\n}');
    expect(mocks.text).toHaveBeenCalledOnce();
  });
  it('keeps invalid JSON visible and reports formatting errors', async () => {
    mocks.text.mockResolvedValue('{bad}');
    preview('json');
    await settle(() =>
      expect(target.querySelector<HTMLButtonElement>('button')?.disabled).toBe(
        false,
      ),
    );
    target.querySelector<HTMLButtonElement>('button')!.click();
    flushSync();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain(
      'Cannot format invalid JSON',
    );
    expect(mocks.setValue).not.toHaveBeenCalled();
    expect(mocks.dispose).not.toHaveBeenCalled();
  });
  it('shows the full path only as the filename tooltip', () => {
    mocks.image.mockReturnValue(new Promise(() => {}));
    preview();
    const name = target.querySelector('.viewer-heading span');
    expect(name?.textContent).toBe('file.png');
    expect(name?.getAttribute('title')).toBe('/file.png');
    expect(target.querySelector('.viewer-path')).toBeNull();
    expect(target.textContent).not.toContain('/file.png');
  });
  it('suppresses duplicate external-open requests and ignores completion after unmount', async () => {
    mocks.image.mockRejectedValue(new Error('Cannot preview'));
    let resolve!: () => void;
    mocks.open.mockReturnValue(
      new Promise<void>((done) => {
        resolve = done;
      }),
    );
    const onopened = vi.fn();
    preview('png', false, onopened);
    await settle(() => expect(target.textContent).toContain('Cannot preview'));
    const button = target.querySelector('button')!;
    button.click();
    button.click();
    flushSync();
    expect(mocks.open).toHaveBeenCalledOnce();
    expect(button.disabled).toBe(true);
    await unmount(component!);
    component = undefined;
    resolve();
    await Promise.resolve();
    expect(onopened).not.toHaveBeenCalled();
  });

  it('allows retrying external-open failures and closes only after success', async () => {
    mocks.image.mockRejectedValue(new Error('Cannot preview'));
    mocks.open
      .mockRejectedValueOnce(new Error('Cannot open'))
      .mockResolvedValueOnce(undefined);
    const onopened = vi.fn();
    preview('png', false, onopened);
    await settle(() => expect(target.textContent).toContain('Cannot preview'));
    target.querySelector('button')!.click();
    await settle(() => expect(target.textContent).toContain('Cannot open'));
    expect(onopened).not.toHaveBeenCalled();
    expect(target.querySelector('button')!.disabled).toBe(false);
    target.querySelector('button')!.click();
    await settle(() => expect(onopened).toHaveBeenCalledOnce());
    expect(mocks.open).toHaveBeenCalledTimes(2);
  });
  it('defaults Markdown to rendered preview and disposes editors when toggling back', async () => {
    mocks.text.mockResolvedValue('# Heading');
    preview('md');
    await settle(() =>
      expect(target.querySelector('h1')?.textContent).toBe('Heading'),
    );
    expect(mocks.editor).not.toHaveBeenCalled();
    target.querySelector<HTMLButtonElement>('.viewer-toggle')!.click();
    await settle(() => expect(mocks.editor).toHaveBeenCalledOnce());
    expect(mocks.focus).not.toHaveBeenCalled();
    target.querySelector<HTMLButtonElement>('.viewer-toggle')!.click();
    await settle(() =>
      expect(target.querySelector('h1')?.textContent).toBe('Heading'),
    );
    expect(mocks.dispose).toHaveBeenCalledOnce();
    await unmount(component!);
    component = undefined;
    expect(mocks.dispose).toHaveBeenCalledOnce();
  });

  it('does not create a source editor after its text request outlives the component', async () => {
    let resolve!: (text: string) => void;
    mocks.text.mockReturnValue(
      new Promise<string>((done) => {
        resolve = done;
      }),
    );
    preview('rs');
    await unmount(component!);
    component = undefined;
    resolve('fn main() {}');
    await Promise.resolve();
    flushSync();
    expect(mocks.editor).not.toHaveBeenCalled();
    expect(target.childElementCount).toBe(0);
  });

  it('shows a fallback when the WebView cannot decode an image', async () => {
    mocks.image.mockResolvedValue('data:image/png;base64,cG5n');
    preview();
    await settle(() => expect(target.querySelector('img')).not.toBeNull());
    target.querySelector('img')!.dispatchEvent(new Event('error'));
    flushSync();
    expect(target.textContent).toContain('This image could not be displayed.');
    expect(target.textContent).toContain('Open in default app');
  });
  it('does not create an image with an empty src while loading', async () => {
    let resolve!: (url: string) => void;
    mocks.image.mockReturnValue(
      new Promise<string>((done) => {
        resolve = done;
      }),
    );
    preview();
    expect(target.querySelector('img')).toBeNull();
    expect(target.textContent).toContain('Loading preview');
    resolve('data:image/png;base64,cG5n');
    await settle(() =>
      expect(target.querySelector('img')?.getAttribute('src')).toBe(
        'data:image/png;base64,cG5n',
      ),
    );
    expect(target.querySelector('.viewer-loading')).toBeNull();
  });

  it('ignores image responses after unmount', async () => {
    let resolve!: (url: string) => void;
    mocks.image.mockReturnValue(
      new Promise<string>((done) => {
        resolve = done;
      }),
    );
    preview();
    await unmount(component!);
    component = undefined;
    resolve('data:image/png;base64,cG5n');
    await Promise.resolve();
    flushSync();
    expect(target.childElementCount).toBe(0);
    expect(mocks.editor).not.toHaveBeenCalled();
  });

  it('disposes the source editor and applies requested autofocus', async () => {
    mocks.text.mockResolvedValue('const value = 1;');
    preview('ts', true);
    await settle(() => expect(mocks.editor).toHaveBeenCalledOnce());
    expect(mocks.editor).toHaveBeenCalledWith(
      expect.any(HTMLElement),
      'const value = 1;',
      'ts',
    );
    expect(mocks.focus).toHaveBeenCalledOnce();
    await unmount(component!);
    component = undefined;
    expect(mocks.dispose).toHaveBeenCalledOnce();
  });

  it('shows image errors with an external-open fallback', async () => {
    mocks.image.mockRejectedValue(new Error('Image exceeds preview limits'));
    preview();
    await settle(() =>
      expect(target.textContent).toContain('Image exceeds preview limits'),
    );
    expect(target.querySelector('img')).toBeNull();
    const button = target.querySelector('button')!;
    expect(button.textContent).toBe('Open in default app');
    button.click();
    await settle(() => expect(mocks.open).toHaveBeenCalledWith('/file.png'));
  });
});
