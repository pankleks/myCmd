// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import LocationPicker from './LocationPicker.svelte';
import { preferences } from '../state/preferences.svelte';
import styles from '../style.css?inline';

vi.mock('../filesystem/api', () => ({ api: { saveConfig: vi.fn() } }));
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
let navigate = vi.fn(async (_path: string) => {});
beforeEach(() => {
  preferences.pinnedDirectories = [
    '/home/user/Projects',
    '/home/user/Downloads',
  ];
  target = document.createElement('div');
  document.body.append(target);
  navigate = vi.fn(async (_path: string) => {});
  component = mount(LocationPicker, {
    target,
    props: {
      path: '/tmp',
      label: '/tmp',
      side: 'left',
      roots: [{ name: 'System', path: '/', type: 'root' }],
      onnavigate: navigate,
      onescape: vi.fn(),
    },
  });
  flushSync();
});
afterEach(async () => {
  await unmount(component);
  target.remove();
});
it('filters shared pins while typing and navigates when a result is clicked', async () => {
  const input = target.querySelector('input')!;
  input.value = 'projects';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  const results =
    target.querySelectorAll<HTMLButtonElement>('.location-option');
  expect(results).toHaveLength(1);
  results[0].click();
  await Promise.resolve();
  expect(navigate).toHaveBeenCalledWith('/home/user/Projects');
});
it('opens the full dropdown when the path input receives focus', () => {
  const input = target.querySelector<HTMLInputElement>('input')!;
  input.focus();
  flushSync();
  expect(input.selectionStart).toBe(0);
  expect(input.selectionEnd).toBe(input.value.length);
  expect(target.querySelector('.location-menu')).not.toBeNull();
  expect(target.querySelectorAll('.location-option')).toHaveLength(4);
});

it.each(['input', 'button'])(
  'moves focus into dropdown options from the %s and wraps arrow navigation',
  async (origin) => {
    const trigger = target.querySelector<HTMLButtonElement>('#drive-left')!;
    trigger.focus();
    trigger.click();
    flushSync();
    const start =
      origin === 'input'
        ? target.querySelector<HTMLInputElement>('input')!
        : trigger;
    start.focus();
    const key = async (element: HTMLElement, name: string) => {
      const event = new KeyboardEvent('keydown', {
        key: name,
        bubbles: true,
        cancelable: true,
      });
      element.dispatchEvent(event);
      await vi.waitFor(() => {
        flushSync();
        expect(document.activeElement).not.toBe(element);
      });
      expect(event.defaultPrevented).toBe(true);
    };
    await key(start, 'ArrowDown');
    const options = [
      ...target.querySelectorAll<HTMLButtonElement>('.location-option'),
    ];
    expect(document.activeElement).toBe(options[0]);
    await key(options[0], 'ArrowDown');
    expect(document.activeElement).toBe(options[1]);
    await key(options[1], 'ArrowUp');
    expect(document.activeElement).toBe(options[0]);
    await key(options[0], 'ArrowUp');
    expect(document.activeElement).toBe(options.at(-1));
  },
);

it('keeps the menu open when focusout has no related target but focus moves to an option', async () => {
  const input = target.querySelector<HTMLInputElement>('input')!;
  input.focus();
  flushSync();
  const option = target.querySelector<HTMLButtonElement>('.location-option')!;
  input.dispatchEvent(
    new FocusEvent('focusout', { bubbles: true, relatedTarget: null }),
  );
  flushSync();
  option.focus();
  await Promise.resolve();
  flushSync();
  expect(document.activeElement).toBe(option);
  expect(target.querySelector('.location-menu')).not.toBeNull();
  const outside = document.createElement('button');
  target.append(outside);
  outside.focus();
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('.location-menu')).toBeNull();
  });
});

it('visibly highlights focused options after clicking the input and reopening the picker', async () => {
  const stylesheet = document.createElement('style');
  stylesheet.textContent = styles;
  document.head.append(stylesheet);
  try {
    const input = target.querySelector<HTMLInputElement>('input')!;
    const trigger = target.querySelector<HTMLButtonElement>('#drive-left')!;
    for (let attempt = 0; attempt < 2; attempt++) {
      trigger.focus();
      trigger.click();
      flushSync();
      input.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
      input.focus();
      input.click();
      input.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'ArrowDown',
          bubbles: true,
          cancelable: true,
        }),
      );
      await vi.waitFor(() => {
        flushSync();
        const option =
          target.querySelector<HTMLButtonElement>('.location-option')!;
        expect(document.activeElement).toBe(option);
        expect(getComputedStyle(option).backgroundColor).toBe(
          'rgb(52, 64, 86)',
        );
      });
      document.activeElement!.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Escape',
          bubbles: true,
          cancelable: true,
        }),
      );
      flushSync();
      expect(target.querySelector('.location-menu')).toBeNull();
    }
  } finally {
    stylesheet.remove();
  }
});

it('submits a typed path directly', () => {
  const input = target.querySelector('input')!;
  input.value = '/new/path';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  target
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  expect(navigate).toHaveBeenCalledWith('/new/path');
});

it('highlights the first filtered result and opens it with Enter', () => {
  const input = target.querySelector<HTMLInputElement>('input')!;
  input.focus();
  input.value = 'projects';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  expect(document.activeElement).toBe(input);
  expect(target.querySelector('.location-match')?.textContent).toContain(
    'Projects',
  );
  target
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  expect(navigate).toHaveBeenCalledWith('/home/user/Projects');
});
it('pins the actual directory rather than the unfinished input', () => {
  const input = target.querySelector('input')!;
  input.value = '/unfinished';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  target.querySelector<HTMLButtonElement>('.location-pin')!.click();
  flushSync();
  expect(preferences.pinnedDirectories).toContain('/tmp');
  expect(preferences.pinnedDirectories).not.toContain('/unfinished');
  target.querySelector<HTMLButtonElement>('.location-pin')!.click();
  flushSync();
  expect(preferences.pinnedDirectories).not.toContain('/tmp');
});
it('shows drives before pins and restores current path on Escape', () => {
  target.querySelector<HTMLButtonElement>('#drive-left')!.click();
  flushSync();
  expect(target.querySelector('.location-option')?.textContent).toContain(
    'System',
  );
  const input = target.querySelector('input')!;
  input.value = 'draft';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
  );
  flushSync();
  expect(input.value).toBe('/tmp');
  expect(target.querySelector('.location-menu')).toBeNull();
});
