// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import App from './App.svelte';
import { api } from './filesystem/api';
import { commander, createPanel, rows } from './state/commander.svelte';
import { locationKey, searchSessions } from './filesystem/providers';
import { listen } from '@tauri-apps/api/event';

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => vi.fn()),
}));
vi.mock('./filesystem/api', () => ({
  api: {
    roots: vi.fn(async () => [
      { name: 'Home', path: '/home/user', type: 'home' },
    ]),
    loadConfig: vi.fn(async () => ({
      version: 1,
      leftPath: '/left',
      rightPath: '/right',
    })),
    saveConfig: vi.fn(async () => {}),
    watch: vi.fn(async () => {}),
    list: vi.fn(async (path: string) => ({
      path,
      entries: ['one.txt', 'two.txt'].map((name) => ({
        name,
        path: `${path}/${name}`,
        type: 'file',
        extension: 'txt',
        size: 5,
        hidden: false,
        directoryTarget: false,
      })),
    })),
    systemActions: vi.fn(async () => {}),
    start: vi.fn(async () => 'operation'),
    cancel: vi.fn(async () => {}),
    resolve: vi.fn(async () => {}),
    readFilesFromClipboard: vi.fn(async () => ({ paths: [], cut: false })),
    readImagePreview: vi.fn(async () => 'data:image/png;base64,cG5n'),
    readImageThumbnail: vi.fn(
      async (path: string) => `data:image/jpeg;base64,${btoa(path)}`,
    ),
  },
  errorMessage: (error: Error) => error.message,
}));

let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
beforeEach(async () => {
  vi.clearAllMocks();
  vi.mocked(api.systemActions).mockResolvedValue(undefined);
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  commander.left = createPanel();
  commander.right = createPanel();
  commander.activePanel = 'left';
  target = document.createElement('div');
  document.body.append(target);
  component = mount(App, { target });
  await vi.waitFor(() => {
    flushSync();
    expect(
      target.querySelector<HTMLInputElement>('#system-command')?.disabled,
    ).toBe(false);
  });
});

const remoteLocation = {
  kind: 'sftp' as const,
  connectionId: 'test',
  path: '/data',
};
function remotePanel(side: 'left' | 'right') {
  commander[side].path = locationKey(remoteLocation);
  commander[side].parent = undefined;
  commander[side].entries = [
    {
      name: 'remote.txt',
      path: locationKey({ ...remoteLocation, path: '/data/remote.txt' }),
      type: 'file',
      extension: 'txt',
      size: 5,
      hidden: false,
      directoryTarget: false,
    },
  ];
  flushSync();
}
function press(key: string) {
  target
    .querySelector(`#list-${commander.activePanel}`)!
    .dispatchEvent(
      new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }),
    );
  flushSync();
}
it('opens the remote file viewer when F3 is pressed on an SFTP panel', async () => {
  remotePanel('left');
  const path = locationKey({ ...remoteLocation, path: '/data/remote.png' });
  commander.left.entries = [
    {
      ...commander.left.entries[0],
      name: 'remote.png',
      path,
      extension: 'png',
    },
  ];
  flushSync();
  press('F3');
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('dialog img')?.getAttribute('src')).toBe(
      'data:image/png;base64,cG5n',
    );
  });
  expect(api.readImagePreview).toHaveBeenCalledWith(path);
});
it('steps through the images of the folder and shows them as thumbnails with F3', async () => {
  const file = (name: string, extension: string) => ({
    name,
    path: `/left/${name}`,
    type: 'file' as const,
    extension,
    size: 5,
    hidden: false,
    directoryTarget: false,
  });
  commander.left.entries = [
    file('a.png', 'png'),
    file('b.txt', 'txt'),
    file('c.jpg', 'jpg'),
    file('d.gif', 'gif'),
  ];
  flushSync();
  const cursorOn = (name: string) =>
    rows(commander.left).findIndex((row) => row.name === name);
  commander.left.cursor = cursorOn('a.png');
  flushSync();
  press('F3');
  const dialog = () => target.querySelector('dialog.file-viewer')!;
  const key = (key: string) => {
    (document.activeElement && dialog().contains(document.activeElement)
      ? document.activeElement
      : dialog()
    ).dispatchEvent(
      new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }),
    );
    flushSync();
  };
  const shown = async (path: string) =>
    vi.waitFor(() => {
      flushSync();
      expect(api.readImagePreview).toHaveBeenLastCalledWith(path);
      expect(dialog().querySelector('img')).not.toBeNull();
    });
  await shown('/left/a.png');
  expect(dialog().querySelector('.viewer-position')?.textContent).toBe('1 / 3');
  // The desktop smoke test closes viewers through the first .viewer-close.
  expect(
    [...dialog().querySelectorAll('.viewer-close')].map((button) =>
      button.getAttribute('aria-label'),
    ),
  ).toEqual(['Close viewer']);

  key('ArrowDown');
  await shown('/left/c.jpg');
  expect(commander.left.cursor).toBe(cursorOn('c.jpg'));
  key('ArrowDown');
  key('ArrowDown');
  await shown('/left/d.gif');
  expect(dialog().querySelector('.viewer-position')?.textContent).toBe('3 / 3');
  key('ArrowUp');
  await shown('/left/c.jpg');

  key('F3');
  const tiles = () => [...dialog().querySelectorAll('.image-tile')];
  expect(tiles().map((tile) => tile.textContent?.trim())).toEqual([
    'a.png',
    'c.jpg',
    'd.gif',
  ]);
  expect(document.activeElement?.classList.contains('image-grid')).toBe(true);
  expect(tiles()[1].getAttribute('aria-selected')).toBe('true');
  await vi.waitFor(() => {
    flushSync();
    expect(dialog().querySelectorAll('.image-thumbnail img')).toHaveLength(3);
  });
  expect(api.readImageThumbnail).toHaveBeenCalledWith('/left/a.png', 320);

  key('ArrowLeft');
  expect(tiles()[0].getAttribute('aria-selected')).toBe('true');
  expect(commander.left.cursor).toBe(cursorOn('a.png'));
  key('End');
  key('Enter');
  await shown('/left/d.gif');
  expect(dialog().querySelector('.image-grid')).toBeNull();

  key('Escape');
  expect(target.querySelector('dialog.file-viewer')).toBeNull();
  expect(commander.left.cursor).toBe(cursorOn('d.gif'));
});
it('keeps arrow keys for scrolling when viewing a file that is not an image', async () => {
  commander.left.cursor = rows(commander.left).findIndex(
    (row) => row.name === 'one.txt',
  );
  flushSync();
  press('F3');
  const dialog = target.querySelector('dialog.file-viewer')!;
  const event = new KeyboardEvent('keydown', {
    key: 'ArrowDown',
    bubbles: true,
    cancelable: true,
  });
  dialog.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(false);
  expect(dialog.querySelector('.viewer-position')).toBeNull();
});
it('routes local-to-SFTP F5 to a remote upload without passing resource URIs to local copy', async () => {
  remotePanel('right');
  press('F5');
  expect(target.querySelector<HTMLInputElement>('dialog input')!.value).toBe(
    '/data',
  );
  target
    .querySelector('dialog form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  await vi.waitFor(() =>
    expect(api.start).toHaveBeenCalledWith({
      type: 'sftp',
      connectionId: 'test',
      operation: {
        type: 'copy',
        sources: ['/left/one.txt'],
        destination: '/data',
        upload: true,
      },
    }),
  );
});

it('routes SFTP-to-local F5 to a remote download', async () => {
  remotePanel('left');
  press('F5');
  target
    .querySelector('dialog form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  await vi.waitFor(() =>
    expect(api.start).toHaveBeenCalledWith({
      type: 'sftp',
      connectionId: 'test',
      operation: {
        type: 'copy',
        sources: ['/data/remote.txt'],
        destination: '/right',
        upload: false,
      },
    }),
  );
});

it('blocks remote Move and disables local-only actions on remote panels', () => {
  remotePanel('left');
  expect(
    target.querySelector<HTMLInputElement>('#system-command')!.disabled,
  ).toBe(true);
  expect(menu()).toBeUndefined();
  const menuNode = target.querySelector('[role="menu"]')!;
  menuNode.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    }),
  );
  flushSync();
  press('F6');
  expect(target.textContent).toContain('Remote Move');
  expect(api.start).not.toHaveBeenCalled();
});
afterEach(async () => {
  await unmount(component);
  target.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});

function menu() {
  target.querySelector('#left-0')!.dispatchEvent(
    new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 10,
      clientY: 10,
    }),
  );
  flushSync();
  return [
    ...target.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'),
  ].find((button) => button.textContent?.startsWith('System actions'));
}

it('opens native system actions for one local item and refreshes panels afterwards', async () => {
  const before = vi.mocked(api.list).mock.calls.length;
  menu()!.click();
  await vi.waitFor(() => expect(api.list).toHaveBeenCalledTimes(before + 2));
  expect(api.systemActions).toHaveBeenCalledExactlyOnceWith('/left/one.txt');
});

it('disables native system actions for multiple selected items', () => {
  commander.left.selected = new Set(['/left/one.txt', '/left/two.txt']);
  expect(menu()!.disabled).toBe(true);
  expect(api.systemActions).not.toHaveBeenCalled();
});

it('does not show native actions for archive members', () => {
  commander.left.path = 'archive:/archive.zip!/';
  commander.left.entries = commander.left.entries.map((entry) => ({
    ...entry,
    path: `archive:/archive.zip!/${entry.name}`,
  }));
  flushSync();
  expect(menu()).toBeUndefined();
});

it('prevents duplicate helpers while native actions are open', async () => {
  let finish!: () => void;
  vi.mocked(api.systemActions).mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  menu()!.click();
  flushSync();
  expect(menu()!.disabled).toBe(true);
  expect(api.systemActions).toHaveBeenCalledOnce();
  finish();
  await vi.waitFor(() => {
    flushSync();
    expect(menu()!.disabled).toBe(false);
  });
});

it('reports helper failures without claiming that an action succeeded', async () => {
  vi.mocked(api.systemActions).mockRejectedValue(
    new Error('Native helper failed'),
  );
  menu()!.click();
  await vi.waitFor(() => {
    flushSync();
    expect(target.textContent).toContain('Native helper failed');
  });
});

it('keeps Shift+Tab inside the search dialog instead of swapping panels', () => {
  target.querySelector(`#list-${commander.activePanel}`)!.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'F7',
      altKey: true,
      bubbles: true,
      cancelable: true,
    }),
  );
  flushSync();
  const field = target.querySelector('dialog input')!;
  expect(field).not.toBeNull();
  const leftPath = commander.left.path;
  const shiftTab = new KeyboardEvent('keydown', {
    key: 'Tab',
    shiftKey: true,
    bubbles: true,
    cancelable: true,
  });
  field.dispatchEvent(shiftTab);
  flushSync();
  expect(commander.left.path).toBe(leftPath);
  expect(shiftTab.defaultPrevented).toBe(false);
});

function keydown(element: Element, key: string, init: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
    ...init,
  });
  element.dispatchEvent(event);
  flushSync();
  return event;
}

it('keeps Shift+Tab in the path bar for moving focus', () => {
  const leftPath = commander.left.path;
  const event = keydown(
    target.querySelector('.panel.active .pathbar input')!,
    'Tab',
    { shiftKey: true },
  );
  expect(event.defaultPrevented).toBe(false);
  expect(commander.left.path).toBe(leftPath);
});

it('runs function-key actions only without Ctrl, Alt or Meta', () => {
  const list = target.querySelector(`#list-${commander.activePanel}`)!;
  commander.left.cursor = 1;
  flushSync();
  for (const init of [{ ctrlKey: true }, { metaKey: true }, { shiftKey: true }])
    keydown(list, 'F5', init);
  keydown(list, 'F8', { ctrlKey: true });
  expect(target.querySelector('dialog form')).toBeNull();
  keydown(list, 'F8', { shiftKey: true });
  expect(target.querySelector('dialog h2')?.textContent).toBe(
    'Delete permanently',
  );
});

it('shows a Close button when F3 opens a folder summary', async () => {
  commander.left.entries = [
    {
      name: 'docs',
      path: `${commander.left.path}/docs`,
      type: 'directory',
      extension: '',
      size: 0,
      hidden: false,
      directoryTarget: true,
    },
  ];
  commander.left.cursor = rows(commander.left).findIndex(
    (row) => row.name === 'docs',
  );
  flushSync();
  press('F3');
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('[aria-label="Close viewer"]')).not.toBeNull();
  });
});

it('keeps F-keys, arrows and Esc working after typing a command', () => {
  const input = target.querySelector<HTMLInputElement>('#system-command')!;
  commander.left.cursor = 0;
  press('l');
  expect(document.activeElement).toBe(input);
  expect(input.value).toBe('l');
  keydown(input, 'ArrowDown');
  expect(commander.left.cursor).toBe(1);
  input.focus();
  keydown(input, 'Escape');
  expect(input.value).toBe('');
  expect(document.activeElement?.id).toBe('list-left');
  press('l');
  keydown(input, 'F7');
  expect(target.querySelector('dialog h2')?.textContent).toBe('New folder');
});

it('reopens New folder with the typed name when creating it fails', async () => {
  press('F7');
  const input = target.querySelector<HTMLInputElement>('dialog input')!;
  input.value = 'taken';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  target
    .querySelector('dialog form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  await vi.waitFor(() => expect(api.start).toHaveBeenCalled());
  flushSync();
  expect(target.querySelector('dialog form')).toBeNull();
  const onProgress = vi
    .mocked(listen)
    .mock.calls.find(([event]) => event === 'operation-progress')![1];
  onProgress({
    event: 'operation-progress',
    id: 1,
    payload: {
      operationId: 'operation',
      state: 'failed',
      processedBytes: 0,
      totalBytes: 0,
      processedItems: 0,
      totalItems: 0,
      error: { code: 'already_exists', message: 'File exists' },
    },
  });
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('dialog h2')?.textContent).toBe('New folder');
  });
  expect(target.querySelector<HTMLInputElement>('dialog input')!.value).toBe(
    'taken',
  );
  expect(target.querySelector('dialog [role="alert"]')?.textContent).toBe(
    'File exists',
  );
});

it('keeps search progress and Cancel visible after switching panels', () => {
  searchSessions.set('progress-test', {
    root: '/left',
    pattern: '*',
    returnPath: '/left',
  });
  commander.left.path = 'search:progress-test';
  commander.left.entries = [];
  commander.left.loading = true;
  commander.activePanel = 'right';
  flushSync();
  expect(target.textContent).toContain('Cancel search');
  expect(target.textContent?.replace(/\s+/g, ' ')).toContain('0 matches found');
  searchSessions.delete('progress-test');
});
