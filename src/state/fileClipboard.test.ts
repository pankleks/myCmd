// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from '../filesystem/api';
import { commander, createPanel } from './commander.svelte';
import { createFileClipboard } from './fileClipboard';

vi.mock('../filesystem/api', () => ({
  api: {
    copyFilesToClipboard: vi.fn().mockResolvedValue(undefined),
    readFilesFromClipboard: vi.fn().mockResolvedValue(['/external/a.txt']),
  },
  errorMessage: String,
}));

function setup() {
  const dependencies = {
    available: vi.fn(() => true),
    start: vi.fn().mockResolvedValue(undefined),
    failed: vi.fn(),
  };
  const controller = createFileClipboard(dependencies);
  function press(
    key: string,
    options: KeyboardEventInit = {},
    target?: HTMLElement,
  ) {
    const event = new KeyboardEvent('keydown', {
      key,
      ctrlKey: true,
      cancelable: true,
      ...options,
    });
    if (target) Object.defineProperty(event, 'target', { value: target });
    return { handled: controller.handle(event), event };
  }
  return { ...dependencies, press };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.copyFilesToClipboard).mockResolvedValue(undefined);
  vi.mocked(api.readFilesFromClipboard).mockResolvedValue(['/external/a.txt']);
  commander.left = createPanel();
  commander.right = createPanel();
  commander.activePanel = 'left';
  commander.left.path = '/left';
  commander.right.path = '/right';
  commander.left.entries = ['a.txt', 'folder', '日本語 # %.txt'].map(
    (name) => ({
      name,
      path: `/left/${name}`,
      type: name === 'folder' ? 'directory' : 'file',
      size: 0,
      extension: '',
      hidden: false,
      directoryTarget: name === 'folder',
    }),
  );
});

describe('system file clipboard shortcuts', () => {
  it('copies all selected files and directories, not only the cursor item', async () => {
    commander.left.selected = new Set(['/left/folder', '/left/日本語 # %.txt']);
    const { press } = setup();
    expect(press('c').event.defaultPrevented).toBe(true);
    expect(api.copyFilesToClipboard).toHaveBeenCalledWith([
      '/left/folder',
      '/left/日本語 # %.txt',
    ]);
  });

  it('falls back to the cursor and supports Command+C', () => {
    const { press } = setup();
    press('C', { ctrlKey: false, metaKey: true });
    expect(api.copyFilesToClipboard).toHaveBeenCalledWith(['/left/folder']);
  });

  it('does not copy the parent entry or an empty selection', () => {
    commander.left.parent = '/';
    const { press } = setup();
    press('c');
    expect(api.copyFilesToClipboard).not.toHaveBeenCalled();
  });

  it('pastes into the active panel, not the opposite panel', async () => {
    commander.activePanel = 'right';
    const { press, start } = setup();
    press('v', { ctrlKey: false, metaKey: true });
    await vi.waitFor(() =>
      expect(start).toHaveBeenCalledWith(
        {
          type: 'copy',
          sources: ['/external/a.txt'],
          destination: '/right',
        },
        'right',
      ),
    );
  });

  it('captures the destination before reading the clipboard and suppresses duplicate pastes', async () => {
    let resolve!: (paths: string[]) => void;
    vi.mocked(api.readFilesFromClipboard).mockReturnValue(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const { press, start } = setup();
    press('v');
    press('v');
    commander.activePanel = 'right';
    commander.left.path = '/changed';
    resolve(['/external/a.txt']);
    await vi.waitFor(() =>
      expect(start).toHaveBeenCalledWith(
        {
          type: 'copy',
          sources: ['/external/a.txt'],
          destination: '/left',
        },
        'left',
      ),
    );
    expect(api.readFilesFromClipboard).toHaveBeenCalledTimes(1);
  });

  it.each(['input', 'textarea', 'select', 'dialog', 'editable', 'editor'])(
    'preserves native clipboard behavior in %s',
    (kind) => {
      const target = document.createElement(
        ['editable', 'editor'].includes(kind) ? 'div' : kind,
      );
      if (kind === 'editable') target.setAttribute('contenteditable', 'true');
      if (kind === 'editor') target.className = 'monaco-editor';
      const { press } = setup();
      for (const key of ['c', 'v']) {
        const { handled, event } = press(key, {}, target);
        expect(handled).toBe(false);
        expect(event.defaultPrevented).toBe(false);
      }
      expect(api.copyFilesToClipboard).not.toHaveBeenCalled();
      expect(api.readFilesFromClipboard).not.toHaveBeenCalled();
    },
  );

  it.each([
    { ctrlKey: false },
    { altKey: true },
    { shiftKey: true },
    { isComposing: true },
  ])('ignores other modifiers or composition: %j', (options) => {
    const { press } = setup();
    expect(press('v', options).handled).toBe(false);
    expect(api.readFilesFromClipboard).not.toHaveBeenCalled();
  });

  it('ignores repeats, loading panels, and unavailable operations', () => {
    const { press, available } = setup();
    press('v', { repeat: true });
    commander.left.loading = true;
    press('v');
    commander.left.loading = false;
    available.mockReturnValue(false);
    press('v');
    expect(api.readFilesFromClipboard).not.toHaveBeenCalled();
  });

  it('does nothing when the clipboard contains no files', async () => {
    vi.mocked(api.readFilesFromClipboard).mockResolvedValue([]);
    const { press, start, failed } = setup();
    press('v');
    await vi.waitFor(() =>
      expect(api.readFilesFromClipboard).toHaveBeenCalled(),
    );
    expect(start).not.toHaveBeenCalled();
    expect(failed).not.toHaveBeenCalled();
  });

  it('does not start a paste if an operation becomes unavailable while reading', async () => {
    const { press, available, start } = setup();
    press('v');
    available.mockReturnValue(false);
    await Promise.resolve();
    expect(start).not.toHaveBeenCalled();
  });

  it.each(['archive:/tmp/a.zip!/', 'search:results', ''])(
    'rejects non-folder paste destinations: %s',
    async (path) => {
      commander.left.path = path;
      const { press, failed } = setup();
      press('v');
      expect(failed).toHaveBeenCalled();
      expect(api.readFilesFromClipboard).not.toHaveBeenCalled();
    },
  );

  it('rejects virtual archive members on copy', () => {
    commander.left.entries[0].path = 'archive:/tmp/a.zip!/a.txt';
    commander.left.selected = new Set([commander.left.entries[0].path]);
    const { press, failed } = setup();
    press('c');
    expect(failed).toHaveBeenCalled();
    expect(api.copyFilesToClipboard).not.toHaveBeenCalled();
  });

  it('reports clipboard failures and allows retrying', async () => {
    vi.mocked(api.readFilesFromClipboard).mockRejectedValueOnce(
      new Error('Clipboard unavailable'),
    );
    const { press, failed, start } = setup();
    press('v');
    await vi.waitFor(() => expect(failed).toHaveBeenCalled());
    press('v');
    await vi.waitFor(() => expect(start).toHaveBeenCalledOnce());
  });
});
