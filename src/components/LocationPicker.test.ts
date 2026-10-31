// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import LocationPicker from './LocationPicker.svelte';
import { preferences } from '../state/preferences.svelte';
import styles from '../style.css?inline';
import { locationKey, parseLocation } from '../filesystem/providers';

vi.mock('../filesystem/api', () => ({ api: { saveConfig: vi.fn() } }));
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
let navigate = vi.fn(async (_path: string) => {});
beforeEach(() => {
  preferences.pinnedDirectoryNames = {};
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

it('interprets typed absolute paths in an SFTP panel as remote, not local roots or pins', async () => {
  await unmount(component);
  const path = locationKey({
    kind: 'sftp',
    connectionId: 'test',
    path: '/data',
  });
  component = mount(LocationPicker, {
    target,
    props: {
      path,
      label: 'SFTP · Work — /data',
      side: 'left',
      roots: [{ name: 'Home', path: '/home', type: 'home' }],
      onnavigate: navigate,
      onescape: vi.fn(),
    },
  });
  flushSync();
  const input = target.querySelector('input')!;
  input.value = '/home';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  target
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  expect(parseLocation(navigate.mock.calls[0][0])).toEqual({
    kind: 'sftp',
    connectionId: 'test',
    path: '/home',
  });
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

it('keeps the menu open when a WebKit button click leaves focus on the body', async () => {
  const input = target.querySelector<HTMLInputElement>('input')!;
  input.focus();
  flushSync();
  const option = target.querySelector<HTMLButtonElement>('.location-option')!;
  option.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
  // WebKit blurs the input without focusing the pressed button.
  input.blur();
  input.dispatchEvent(
    new FocusEvent('focusout', { bubbles: true, relatedTarget: null }),
  );
  // Let the async focusout handler finish before the click, as in a browser.
  await new Promise((resolve) => setTimeout(resolve));
  flushSync();
  expect(document.activeElement).toBe(document.body);
  expect(target.querySelector('.location-menu')).not.toBeNull();
  option.click();
  await Promise.resolve();
  expect(navigate).toHaveBeenCalledWith('/');
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

it('sorts pinned directories in the dropdown', () => {
  target.querySelector<HTMLInputElement>('input')!.focus();
  flushSync();
  const pins = Array.from(target.querySelectorAll('.location-option small'))
    .map((element) => element.textContent)
    .filter((text) => text?.startsWith('/home'));
  expect(pins).toEqual(['/home/user/Downloads', '/home/user/Projects']);
});

async function editFavorite(name = 'Projects') {
  target.querySelector<HTMLInputElement>('input')!.focus();
  flushSync();
  target
    .querySelector<HTMLButtonElement>(`[aria-label="Rename favorite ${name}"]`)!
    .click();
  await vi.waitFor(() => {
    flushSync();
    expect(document.activeElement).toBe(
      target.querySelector('[aria-label="Favorite name"]'),
    );
  });
  return target.querySelector<HTMLInputElement>(
    '[aria-label="Favorite name"]',
  )!;
}

it('renames a favorite with Enter without navigating or changing its path', async () => {
  const editor = await editFavorite();
  expect(editor.value).toBe('Projects');
  editor.value = ' Work projects ';
  editor.dispatchEvent(new Event('input', { bubbles: true }));
  editor.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'Enter',
      bubbles: true,
      cancelable: true,
    }),
  );
  flushSync();
  expect(preferences.pinnedDirectoryNames['/home/user/Projects']).toBe(
    'Work projects',
  );
  expect(preferences.pinnedDirectories).toContain('/home/user/Projects');
  expect(navigate).not.toHaveBeenCalled();
  expect(target.querySelector('.location-menu')).not.toBeNull();
  expect(target.textContent).toContain('/home/user/Projects');
});

it('cancels with Escape and resets a name by saving empty text', async () => {
  preferences.pinnedDirectoryNames = { '/home/user/Projects': 'Work' };
  let editor = await editFavorite('Work');
  editor.value = 'Discard';
  editor.dispatchEvent(new Event('input', { bubbles: true }));
  editor.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    }),
  );
  flushSync();
  expect(preferences.pinnedDirectoryNames['/home/user/Projects']).toBe('Work');
  expect(target.querySelector('.location-menu')).not.toBeNull();
  editor = await editFavorite('Work');
  editor.value = '  ';
  editor.dispatchEvent(new Event('input', { bubbles: true }));
  target
    .querySelector<HTMLButtonElement>('[aria-label="Save favorite name"]')!
    .click();
  flushSync();
  expect(
    preferences.pinnedDirectoryNames['/home/user/Projects'],
  ).toBeUndefined();
  expect(
    target.querySelector('[aria-label="Rename favorite Projects"]'),
  ).not.toBeNull();
  expect(navigate).not.toHaveBeenCalled();
});

it.each(['work', '/home/user/Projects'])(
  'finds renamed favorites by name or path: %s',
  (query) => {
    preferences.pinnedDirectoryNames = { '/home/user/Projects': 'Work' };
    const input = target.querySelector<HTMLInputElement>('input')!;
    input.value = query;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    const options =
      target.querySelectorAll<HTMLButtonElement>('.location-option');
    expect(options).toHaveLength(1);
    expect(options[0].textContent).toContain('Work');
    options[0].click();
    expect(navigate).toHaveBeenCalledWith('/home/user/Projects');
  },
);

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
