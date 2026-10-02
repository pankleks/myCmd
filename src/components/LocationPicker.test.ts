// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import LocationPicker from './LocationPicker.svelte';
import { preferences } from '../state/preferences.svelte';

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
