// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import App from './App.svelte';
import { api } from './filesystem/api';
import { commander, createPanel } from './state/commander.svelte';
import { locationKey } from './filesystem/providers';

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
