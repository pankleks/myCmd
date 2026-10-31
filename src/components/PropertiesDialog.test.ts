// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import PropertiesDialog from './PropertiesDialog.svelte';
import { api } from '../filesystem/api';
import type { FileProperties } from '../filesystem/types';

vi.mock('../filesystem/api', () => ({
  api: {
    properties: vi.fn(),
    setPermissions: vi.fn(),
    copyText: vi.fn(),
    measureDirectory: vi.fn(),
    cancelDirectorySizing: vi.fn(),
  },
  errorMessage: (error: Error) => error.message,
}));

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;
let details: FileProperties;
const close = vi.fn();
const applied = vi.fn(async () => {});

beforeEach(() => {
  vi.resetAllMocks();
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  details = {
    name: 'notes.txt',
    path: '/data/notes.txt',
    type: 'file',
    extension: 'txt',
    size: 5,
    modified: 1000,
    hidden: false,
    directoryTarget: false,
    mode: 0o640,
    owner: 'user (1000)',
    group: 'users (1000)',
    readOnly: false,
    linkTarget: null,
    linkExists: null,
  };
  vi.mocked(api.properties).mockImplementation(async () => details);
  vi.mocked(api.setPermissions).mockImplementation(
    async (_path, permissions) => ({ ...details, ...permissions }),
  );
  vi.mocked(api.copyText).mockResolvedValue(undefined);
  vi.mocked(api.cancelDirectorySizing).mockResolvedValue(undefined);
  target = document.createElement('div');
  document.body.append(target);
});

afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  target.remove();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});

async function show() {
  component = mount(PropertiesDialog, {
    target,
    props: { path: details.path, onclose: close, onapplied: applied },
  });
  await vi.waitFor(() => {
    flushSync();
    expect(
      target.querySelector('[role="status"]')?.textContent ?? '',
    ).not.toContain('Loading');
  });
}
function button(label: string) {
  return [...target.querySelectorAll<HTMLButtonElement>('button')].find(
    (button) => button.textContent?.trim() === label,
  )!;
}
function toggle(label: string) {
  target.querySelector<HTMLInputElement>(`[aria-label="${label}"]`)!.click();
  flushSync();
}

it('shows general information and Unix permission checkboxes', async () => {
  await show();
  expect(api.properties).toHaveBeenCalledWith('/data/notes.txt');
  expect(target.textContent).toContain('notes.txt');
  expect(target.textContent).toContain('5 bytes');
  expect(target.textContent).toContain('user (1000)');
  expect(target.querySelectorAll('input[type="checkbox"]')).toHaveLength(9);
  expect(
    target.querySelector<HTMLInputElement>('[aria-label="Owner read"]')!
      .checked,
  ).toBe(true);
  expect(
    target.querySelector<HTMLInputElement>('[aria-label="Group write"]')!
      .checked,
  ).toBe(false);
  expect(button('Apply').disabled).toBe(true);
});

it('copies a displayable Windows path as text', async () => {
  details.path = '\\\\?\\C:\\Data\\notes.txt';
  await show();
  button('Copy path').click();
  await vi.waitFor(() => {
    flushSync();
    expect(target.textContent).toContain('Path copied.');
  });
  expect(api.copyText).toHaveBeenCalledExactlyOnceWith('C:\\Data\\notes.txt');
});

it('applies Unix permissions and refreshes without closing the dialog', async () => {
  await show();
  toggle('Owner execute');
  button('Apply').click();
  await vi.waitFor(() => {
    flushSync();
    expect(applied).toHaveBeenCalledOnce();
  });
  expect(api.setPermissions).toHaveBeenCalledExactlyOnceWith(details.path, {
    mode: 0o740,
  });
  expect(button('Apply').disabled).toBe(true);
  expect(close).not.toHaveBeenCalled();
  expect(target.textContent).toContain('Permissions updated.');
});

it('shows save failures in place and allows retry', async () => {
  vi.mocked(api.setPermissions).mockRejectedValueOnce(
    new Error('Permission denied'),
  );
  await show();
  toggle('Group write');
  button('Apply').click();
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain(
      'Permission denied',
    );
  });
  expect(button('Apply').disabled).toBe(false);
  expect(close).not.toHaveBeenCalled();
  button('Apply').click();
  await vi.waitFor(() => expect(applied).toHaveBeenCalledOnce());
});

it('uses the read-only attribute on Windows instead of Unix permissions', async () => {
  details.mode = null;
  details.owner = null;
  details.group = null;
  await show();
  const checkbox = target.querySelector<HTMLInputElement>(
    'input[type="checkbox"]',
  )!;
  expect(checkbox.closest('label')?.textContent).toContain('Read-only');
  checkbox.click();
  flushSync();
  button('Apply').click();
  await vi.waitFor(() => expect(applied).toHaveBeenCalledOnce());
  expect(api.setPermissions).toHaveBeenCalledExactlyOnceWith(details.path, {
    readOnly: true,
  });
});

it.each([true, false, null])(
  'shows symlink targets and disables editing when target status is %s',
  async (linkExists) => {
    details.type = 'symlink';
    details.linkTarget = '../target';
    details.linkExists = linkExists;
    await show();
    expect(target.textContent).toContain('../target');
    expect(target.textContent).toContain(
      linkExists === true
        ? 'Resolves'
        : linkExists === false
          ? 'Missing target'
          : 'Unable to resolve target',
    );
    expect(target.querySelector('fieldset')!.disabled).toBe(true);
    expect(button('Apply').disabled).toBe(true);
    expect(button('Calculate size')).toBeUndefined();
  },
);

it('calculates folder size only on request', async () => {
  details.type = 'directory';
  vi.mocked(api.measureDirectory).mockResolvedValue(2048);
  await show();
  expect(api.measureDirectory).not.toHaveBeenCalled();
  button('Calculate size').click();
  await vi.waitFor(() => {
    flushSync();
    expect(target.textContent).toContain('2.0 KB');
  });
  expect(api.measureDirectory).toHaveBeenCalledWith(
    details.path,
    expect.any(String),
  );
});

it('cancels pending size calculation when unmounted', async () => {
  details.type = 'directory';
  vi.mocked(api.measureDirectory).mockReturnValue(new Promise(() => {}));
  await show();
  button('Calculate size').click();
  const requestId = vi.mocked(api.measureDirectory).mock.calls[0][1];
  await unmount(component!);
  component = undefined;
  expect(api.cancelDirectorySizing).toHaveBeenCalledExactlyOnceWith([
    requestId,
  ]);
});

it('reports size errors without showing a partial result', async () => {
  details.type = 'directory';
  vi.mocked(api.measureDirectory).mockRejectedValue(
    new Error('Cannot read folder'),
  );
  await show();
  button('Calculate size').click();
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain(
      'Cannot read folder',
    );
  });
  expect(target.textContent).toContain('Not calculated');
  expect(button('Calculate size').disabled).toBe(false);
});

it('shows load errors and still allows closing', async () => {
  vi.mocked(api.properties).mockRejectedValue(
    new Error('File no longer exists'),
  );
  await show();
  expect(target.querySelector('[role="alert"]')?.textContent).toContain(
    'File no longer exists',
  );
  button('Close').click();
  expect(close).toHaveBeenCalledOnce();
});

it('Escape closes without applying unsaved changes', async () => {
  await show();
  toggle('Others read');
  target
    .querySelector('dialog')!
    .dispatchEvent(new Event('cancel', { cancelable: true }));
  expect(close).toHaveBeenCalledOnce();
  expect(api.setPermissions).not.toHaveBeenCalled();
});
