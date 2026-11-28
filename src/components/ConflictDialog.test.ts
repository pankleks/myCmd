// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ConflictDialog from './ConflictDialog.svelte';

const entry = (path: string) => ({
  name: 'a.txt',
  path,
  type: 'file' as const,
  extension: 'txt',
  size: 5,
  hidden: false,
  directoryTarget: false,
});
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const resolve = vi.fn(async () => {});

beforeEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  target = document.createElement('div');
  document.body.append(target);
  component = mount(ConflictDialog, {
    target,
    props: {
      conflict: {
        operationId: 'op',
        source: entry('/a/a.txt'),
        destination: entry('/b/a.txt'),
      },
      onresolve: resolve,
    },
  });
  flushSync();
});
afterEach(async () => {
  await unmount(component);
  target.remove();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});

it('renames with the typed name when Enter submits the form', () => {
  const input = target.querySelector<HTMLInputElement>('input:not([type])')!;
  input.value = 'b.txt';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  target
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  expect(resolve).toHaveBeenCalledWith({
    action: 'rename',
    applyToAll: false,
    name: 'b.txt',
  });
});

it('marks Overwrite as destructive and Rename as the default', () => {
  const buttons = [...target.querySelectorAll('button')];
  const overwrite = buttons.find((b) => b.textContent === 'Overwrite')!;
  const rename = buttons.find((b) => b.textContent === 'Rename')!;
  expect(overwrite.classList.contains('danger')).toBe(true);
  expect(rename.classList.contains('primary')).toBe(true);
  expect(rename.type).toBe('submit');
});

it('shows a failed answer without an "Error:" prefix', async () => {
  resolve.mockRejectedValueOnce(new Error('Name is not valid'));
  target
    .querySelector<HTMLButtonElement>('[data-conflict-action="skip"]')!
    .click();
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('[role="alert"]')?.textContent).toBe(
      'Name is not valid',
    );
  });
});
