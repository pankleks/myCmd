// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import FilePreviewPanel from './FilePreviewPanel.svelte';
import { commander, createPanel } from '../state/commander.svelte';

const mocks = vi.hoisted(() => ({ read: vi.fn() }));
vi.mock('../filesystem/api', () => ({
  api: { readTextPreview: mocks.read },
  errorMessage: String,
}));
vi.mock('../monaco', () => ({
  createViewerEditor: vi.fn(() => ({ dispose: vi.fn(), focus: vi.fn() })),
}));

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  vi.useFakeTimers();
  mocks.read.mockReset().mockReturnValue(new Promise(() => {}));
  commander.left = createPanel();
  target = document.createElement('div');
  document.body.append(target);
});
afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  target.remove();
  vi.useRealTimers();
});
function start() {
  component = mount(FilePreviewPanel, {
    target,
    props: {
      get target() {
        const path = commander.left.path;
        return path
          ? { path, name: path, extension: 'txt', key: path }
          : undefined;
      },
    },
  });
  flushSync();
}
async function advance(ms: number) {
  await vi.advanceTimersByTimeAsync(ms);
  flushSync();
}
describe('panel preview debounce', () => {
  it('opens only the final file after cursor movement stops', async () => {
    commander.left.path = '/a.txt';
    start();
    await advance(150);
    commander.left.path = '/b.txt';
    flushSync();
    await advance(150);
    commander.left.path = '/c.txt';
    flushSync();
    await advance(199);
    expect(mocks.read).not.toHaveBeenCalled();
    await advance(1);
    expect(mocks.read).toHaveBeenCalledExactlyOnceWith('/c.txt');
  });
  it('cancels a pending preview when the selection is cleared', async () => {
    commander.left.path = '/a.txt';
    start();
    commander.left.path = '';
    flushSync();
    await advance(200);
    expect(mocks.read).not.toHaveBeenCalled();
  });
  it('cancels a pending preview when the panel is closed', async () => {
    commander.left.path = '/a.txt';
    start();
    await unmount(component!);
    component = undefined;
    await advance(200);
    expect(mocks.read).not.toHaveBeenCalled();
  });
});
