// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import ContextMenu from './ContextMenu.svelte';

let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const close = vi.fn();
const run = vi.fn();
beforeEach(async () => {
  vi.clearAllMocks();
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(240);
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(200);
  target = document.createElement('div');
  document.body.append(target);
  component = mount(ContextMenu, {
    target,
    props: {
      x: window.innerWidth - 10,
      y: window.innerHeight - 10,
      onclose: close,
      items: [
        { label: 'Disabled', disabled: true, run },
        { label: 'Copy', shortcut: 'Ctrl+C', run },
        { label: 'Paste', separator: true, run },
      ],
    },
  });
  flushSync();
  await tick();
});
afterEach(async () => {
  await unmount(component);
  target.remove();
  vi.restoreAllMocks();
});
function key(name: string) {
  document.activeElement!.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: name,
      bubbles: true,
      cancelable: true,
    }),
  );
  flushSync();
}

it('stays within the viewport and focuses the first enabled action', () => {
  const menu = target.querySelector<HTMLElement>('[role="menu"]')!;
  expect(menu.style.left).toBe(`${window.innerWidth - 244}px`);
  expect(menu.style.top).toBe(`${window.innerHeight - 204}px`);
  expect(document.activeElement?.textContent).toContain('Copy');
  expect(target.querySelector('[role="separator"]')).not.toBeNull();
});

it('supports arrows, Home, End and wrapping, skipping disabled items', () => {
  key('ArrowUp');
  expect(document.activeElement?.textContent).toContain('Paste');
  key('ArrowDown');
  expect(document.activeElement?.textContent).toContain('Copy');
  key('End');
  expect(document.activeElement?.textContent).toContain('Paste');
  key('Home');
  expect(document.activeElement?.textContent).toContain('Copy');
});

it.each(['Escape', 'Tab'])(
  'closes with %s without invoking actions',
  (name) => {
    key(name);
    expect(close).toHaveBeenCalledOnce();
    expect(run).not.toHaveBeenCalled();
  },
);

it('closes before invoking the chosen action', () => {
  (document.activeElement as HTMLButtonElement).click();
  expect(close).toHaveBeenCalledOnce();
  expect(run).toHaveBeenCalledOnce();
  expect(close.mock.invocationCallOrder[0]).toBeLessThan(
    run.mock.invocationCallOrder[0],
  );
});

it('closes on outside clicks but not clicks inside the menu', () => {
  target
    .querySelector('[role="menu"]')!
    .dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(close).not.toHaveBeenCalled();
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(close).toHaveBeenCalledWith(false);
});

it('does not leak navigation keys to global shortcuts', () => {
  const listener = vi.fn();
  window.addEventListener('keydown', listener);
  try {
    key('ArrowDown');
    expect(listener).not.toHaveBeenCalled();
  } finally {
    window.removeEventListener('keydown', listener);
  }
});
