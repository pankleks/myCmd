// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import SearchDialog from './SearchDialog.svelte';
import { preferences } from '../state/preferences.svelte';

vi.mock('../filesystem/api', () => ({ api: { saveConfig: vi.fn() } }));
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
let submit = vi.fn((_folder: string, _pattern: string) => {});
let close = vi.fn(() => {});

beforeEach(() => {
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  preferences.pinnedDirectories = ['/home/user/Projects'];
  target = document.createElement('div');
  document.body.append(target);
  submit = vi.fn();
  close = vi.fn();
  component = mount(SearchDialog, {
    target,
    props: {
      folder: '/current',
      roots: [{ name: 'System', path: '/', type: 'root' }],
      onsubmit: submit,
      onclose: close,
    },
  });
  flushSync();
});

afterEach(async () => {
  await unmount(component);
  target.remove();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});

function folderInput() {
  return target.querySelector<HTMLInputElement>(
    '[aria-label="Search folder"]',
  )!;
}
function typeFolder(value: string) {
  const input = folderInput();
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}
function search() {
  target.querySelector<HTMLButtonElement>('.dialog-actions .primary')!.click();
  flushSync();
}

it('uses a typed folder when Search is clicked, without nested forms', () => {
  expect(target.querySelectorAll('form')).toHaveLength(1);
  expect(folderInput().value).toBe('/current');
  typeFolder('/directory with spaces');
  search();
  expect(submit).toHaveBeenCalledExactlyOnceWith(
    '/directory with spaces',
    '*.*',
    undefined,
  );
});

it.each([
  ['System', '/'],
  ['Projects', '/home/user/Projects'],
])(
  'chooses %s without starting the search or navigating a panel',
  (label, path) => {
    target.querySelector<HTMLButtonElement>('#search-folder-picker')!.click();
    flushSync();
    const option = [
      ...target.querySelectorAll<HTMLButtonElement>('.location-option'),
    ].find((button) => button.textContent?.includes(label))!;
    option.click();
    flushSync();
    expect(folderInput().value).toBe(path);
    expect(submit).not.toHaveBeenCalled();
    expect(target.querySelector('.location-menu')).toBeNull();
    search();
    expect(submit).toHaveBeenCalledExactlyOnceWith(path, '*.*', undefined);
  },
);

it('chooses the filtered result with Enter instead of submitting the search', () => {
  typeFolder('projects');
  expect(target.querySelectorAll('.location-option')).toHaveLength(1);
  const event = new KeyboardEvent('keydown', {
    key: 'Enter',
    bubbles: true,
    cancelable: true,
  });
  folderInput().dispatchEvent(event);
  flushSync();
  expect(event.defaultPrevented).toBe(true);
  expect(folderInput().value).toBe('/home/user/Projects');
  expect(submit).not.toHaveBeenCalled();
  search();
  expect(submit).toHaveBeenCalledWith('/home/user/Projects', '*.*', undefined);
});

it('Escape closes the dropdown first and preserves the typed folder', () => {
  typeFolder('/new/folder');
  const first = new KeyboardEvent('keydown', {
    key: 'Escape',
    bubbles: true,
    cancelable: true,
  });
  folderInput().dispatchEvent(first);
  flushSync();
  expect(first.defaultPrevented).toBe(true);
  expect(target.querySelector('.location-menu')).toBeNull();
  expect(folderInput().value).toBe('/new/folder');
  expect(close).not.toHaveBeenCalled();
  const second = new KeyboardEvent('keydown', {
    key: 'Escape',
    bubbles: true,
    cancelable: true,
  });
  folderInput().dispatchEvent(second);
  expect(second.defaultPrevented).toBe(false);
  target
    .querySelector('dialog')!
    .dispatchEvent(new Event('cancel', { cancelable: true }));
  expect(close).toHaveBeenCalledOnce();
});

it('supports arrow-key navigation and Escape from a dropdown option', async () => {
  await tick();
  folderInput().focus();
  folderInput().dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'ArrowDown',
      bubbles: true,
      cancelable: true,
    }),
  );
  await tick();
  flushSync();
  const option = target.querySelector<HTMLButtonElement>('.location-option')!;
  await vi.waitFor(() => expect(document.activeElement).toBe(option));
  option.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    }),
  );
  flushSync();
  expect(target.querySelector('.location-menu')).toBeNull();
  expect(close).not.toHaveBeenCalled();
});

it('requires text only when content search is enabled and ignores it when disabled', () => {
  const checkbox = target.querySelector<HTMLInputElement>('[type="checkbox"]')!;
  const text = target.querySelector<HTMLInputElement>(
    '[placeholder="Enter text to find…"]',
  )!;
  expect(text.disabled).toBe(true);
  checkbox.click();
  flushSync();
  expect(text.disabled).toBe(false);
  expect(text.required).toBe(true);
  search();
  expect(submit).not.toHaveBeenCalled();
  text.value = 'literal .* text';
  text.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  search();
  expect(submit).toHaveBeenLastCalledWith('/current', '*.*', 'literal .* text');
  checkbox.click();
  flushSync();
  expect(text.disabled).toBe(true);
  expect(text.value).toBe('literal .* text');
  search();
  expect(submit).toHaveBeenLastCalledWith('/current', '*.*', undefined);
});
