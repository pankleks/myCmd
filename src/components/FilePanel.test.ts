// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import FilePanel from './FilePanel.svelte';
import { commander, createPanel } from '../state/commander.svelte';

vi.mock('../filesystem/api', () => ({ api: {}, errorMessage: String }));
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const oncontext = vi.fn();
beforeEach(async () => {
  vi.clearAllMocks();
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
  commander.activePanel = 'right';
  commander.left.path = '/left';
  commander.left.entries = ['a.txt', 'b.txt'].map((name) => ({
    name,
    path: `/left/${name}`,
    type: 'file',
    size: 0,
    extension: 'txt',
    hidden: false,
    directoryTarget: false,
  }));
  target = document.createElement('div');
  document.body.append(target);
  component = mount(FilePanel, {
    target,
    props: { panel: commander.left, side: 'left', roots: [], oncontext },
  });
  flushSync();
  await tick();
});
afterEach(async () => {
  await unmount(component);
  target.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
function rightClick(selector: string) {
  target.querySelector(selector)!.dispatchEvent(
    new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 45,
      clientY: 80,
    }),
  );
  flushSync();
}

it('preserves selected items and activates the clicked panel', () => {
  commander.left.selected = new Set(['/left/a.txt', '/left/b.txt']);
  rightClick('#left-1 .filename');
  expect(commander.activePanel).toBe('left');
  expect(commander.left.cursor).toBe(1);
  expect([...commander.left.selected]).toEqual(['/left/a.txt', '/left/b.txt']);
  expect(oncontext).toHaveBeenCalledWith('left', 45, 80, true);
});

it('clears the previous selection when right-clicking another item', () => {
  commander.left.selected = new Set(['/left/a.txt']);
  rightClick('#left-1');
  expect(commander.left.selected.size).toBe(0);
  expect(commander.left.cursor).toBe(1);
});

it('requests the empty-space menu without an item', () => {
  rightClick('#list-left');
  expect(oncontext).toHaveBeenCalledWith('left', 45, 80, false);
});

it.each([{ key: 'F10', shiftKey: true }, { key: 'ContextMenu' }])(
  'opens at the focused item with keyboard: %j',
  (options) => {
    const list = target.querySelector<HTMLElement>('#list-left')!;
    list.focus();
    const event = new KeyboardEvent('keydown', {
      ...options,
      bubbles: true,
      cancelable: true,
    });
    list.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(oncontext).toHaveBeenCalledWith('left', 24, 0, true);
  },
);
