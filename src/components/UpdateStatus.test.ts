// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import UpdateStatus from './UpdateStatus.svelte';
import { version } from '../../package.json';
import type { AvailableUpdate } from '../filesystem/api';

const mocks = vi.hoisted(() => ({
  check: vi.fn(),
  copy: vi.fn(),
  open: vi.fn(),
}));
vi.mock('../filesystem/api', () => ({
  api: {
    checkForUpdate: mocks.check,
    copyText: mocks.copy,
    openReleasePage: mocks.open,
  },
  errorMessage: (cause: unknown) =>
    typeof cause === 'object' && cause && 'message' in cause
      ? String(cause.message)
      : String(cause),
}));

const update: AvailableUpdate = {
  version: '9.0.0',
  name: 'myCmd v9.0.0',
  publishedAt: '2026-12-01T10:00:00Z',
  notes:
    '# myCmd 9.0.0\n\n- **New** image grid\n- <img src=x onerror=alert(1)>Safe `code`',
  notesTruncated: true,
  instructions: [
    {
      target: 'deb',
      label: 'Debian / Ubuntu (.deb)',
      note: 'Run in a terminal, then restart myCmd.',
      command: "curl -fLO 'https://example/x.deb'",
    },
  ],
};

let component: ReturnType<typeof mount> | undefined;
let target: HTMLDivElement;
function show(autoCheck = true) {
  component = mount(UpdateStatus, { target, props: { autoCheck } });
  flushSync();
}
async function settle(check: () => void) {
  await vi.waitFor(() => {
    flushSync();
    check();
  });
}
const status = () => target.querySelector('[role="status"]')?.textContent;
const button = (text: string) =>
  [...target.querySelectorAll('button')].find(
    (candidate) => candidate.textContent?.trim() === text,
  );

beforeEach(() => {
  vi.resetAllMocks();
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  target = document.createElement('div');
  document.body.append(target);
});
afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  target.remove();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});

it('checks once on open and offers the dialog for a newer version', async () => {
  mocks.check.mockResolvedValue({ current: '0.2.17', update });
  show();
  expect(mocks.check).toHaveBeenCalledExactlyOnceWith(false);
  expect(status()).toBe('Checking GitHub for a newer version…');
  expect(button('Check now')?.disabled).toBe(true);
  await settle(() => expect(status()).toBe('myCmd 9.0.0 is available.'));
  expect(target.querySelector('.update-text strong')?.textContent).toBe(
    'myCmd 0.2.17',
  );

  button('View…')!.click();
  flushSync();
  const dialog = target.querySelector('dialog.update-dialog')!;
  expect(dialog.querySelector('h2')?.textContent).toBe(
    'myCmd 9.0.0 is available',
  );
  expect(dialog.querySelector('header p')?.textContent).toContain(
    'You have 0.2.17',
  );
  expect(dialog.querySelector('.update-command code')?.textContent).toBe(
    "curl -fLO 'https://example/x.deb'",
  );
  // Notes are text only: HTML in them never becomes elements.
  const notes = dialog.querySelector('.release-notes')!;
  expect(notes.querySelector('img, script')).toBeNull();
  expect(notes.querySelector('strong')?.textContent).toBe('New');
  expect(notes.querySelector('code')?.textContent).toBe('code');
  expect(notes.textContent).toContain('The notes are shortened');

  mocks.copy.mockResolvedValue(undefined);
  button('Copy')!.click();
  await settle(() =>
    expect(dialog.querySelector('footer [role="status"]')?.textContent).toBe(
      'Command copied.',
    ),
  );
  expect(mocks.copy).toHaveBeenCalledWith("curl -fLO 'https://example/x.deb'");

  mocks.open.mockRejectedValue({ message: 'No browser' });
  button('Release page')!.click();
  await settle(() =>
    expect(dialog.querySelector('footer [role="status"]')?.textContent).toBe(
      'No browser',
    ),
  );
  expect(mocks.open).toHaveBeenCalledOnce();

  dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
  flushSync();
  expect(target.querySelector('dialog.update-dialog')).toBeNull();
  // Focus goes back to the button that opened the dialog.
  await settle(() => expect(document.activeElement).toBe(button('View…')));
});

it('reports the newest version and failed checks from Check now', async () => {
  show(false);
  expect(mocks.check).not.toHaveBeenCalled();
  expect(status()).toBe('');
  expect(target.querySelector('.update-text strong')?.textContent).toBe(
    `myCmd ${version}`,
  );

  mocks.check.mockRejectedValueOnce({ message: 'Could not reach GitHub' });
  button('Check now')!.click();
  await settle(() =>
    expect(status()).toBe('The check failed: Could not reach GitHub'),
  );
  expect(mocks.check).toHaveBeenCalledWith(true);

  mocks.check.mockResolvedValueOnce({ current: version, update: null });
  button('Check now')!.click();
  await settle(() => expect(status()).toBe('You have the newest version.'));
  expect(button('View…')).toBeUndefined();
});

it('shows a hint when there is no command for this system', async () => {
  mocks.check.mockResolvedValue({
    current: '0.2.17',
    update: { ...update, instructions: [], notes: '', notesTruncated: false },
  });
  show();
  await settle(() => expect(button('View…')).toBeDefined());
  button('View…')!.click();
  flushSync();
  expect(target.querySelector('dialog.update-dialog')?.textContent).toContain(
    'No ready-made command for this system',
  );
  expect(target.textContent).toContain('No release notes.');
  button('Close')!.click();
  flushSync();
  expect(target.querySelector('dialog.update-dialog')).toBeNull();
});
