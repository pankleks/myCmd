// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ConnectionsDialog from './ConnectionsDialog.svelte';
import { api } from '../filesystem/api';
import { preferences } from '../state/preferences.svelte';
import type { SftpProfile } from '../filesystem/types';

vi.mock('../filesystem/api', () => ({
  api: { connectSftp: vi.fn(), disconnectSftp: vi.fn(), saveConfig: vi.fn() },
  errorMessage: (error: Error) => error.message,
}));
const profile: SftpProfile = {
  id: 'b02db8da-ccdc-41bb-8db0-5c7b0e798d7a',
  name: 'Work',
  host: 'server',
  port: 22,
  username: 'user',
  directory: '/data',
  keyPath: null,
};
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const connected = vi.fn(async () => {});
const disconnected = vi.fn(async () => {});
const close = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  preferences.sftpProfiles = [profile];
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  target = document.createElement('div');
  document.body.append(target);
  vi.mocked(api.connectSftp).mockResolvedValue({
    status: 'connected',
    id: profile.id,
    path: '/data',
    name: 'Work',
  });
  vi.mocked(api.disconnectSftp).mockResolvedValue(undefined);
  component = mount(ConnectionsDialog, {
    target,
    props: {
      initialId: profile.id,
      onconnected: connected,
      ondisconnected: disconnected,
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
function submit() {
  target
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  flushSync();
}
function labelInput(label: string) {
  return [...target.querySelectorAll('label')]
    .find((node) => node.textContent?.startsWith(label))!
    .querySelector('input, select') as HTMLInputElement | HTMLSelectElement;
}
function change(label: string, value: string) {
  const input = labelInput(label);
  input.value = value;
  input.dispatchEvent(
    new Event(input.tagName === 'SELECT' ? 'change' : 'input', {
      bubbles: true,
    }),
  );
  flushSync();
}

it('loads saved details and sends agent authentication without secrets', async () => {
  expect(labelInput('Host').value).toBe('server');
  submit();
  await vi.waitFor(() => expect(connected).toHaveBeenCalledOnce());
  expect(api.connectSftp).toHaveBeenCalledWith({
    profile,
    auth: 'agent',
    password: undefined,
    passphrase: undefined,
    trustFingerprint: undefined,
  });
});

it('requires explicit host-key confirmation before retrying with trust', async () => {
  vi.mocked(api.connectSftp).mockResolvedValueOnce({
    status: 'hostKey',
    fingerprint: 'SHA256:verified',
  });
  submit();
  await vi.waitFor(() => {
    flushSync();
    expect(target.textContent).toContain('SHA256:verified');
  });
  expect(connected).not.toHaveBeenCalled();
  expect(target.querySelector('fieldset')!.disabled).toBe(true);
  expect(api.connectSftp).toHaveBeenCalledTimes(1);
  submit();
  await vi.waitFor(() => expect(connected).toHaveBeenCalledOnce());
  expect(api.connectSftp).toHaveBeenLastCalledWith(
    expect.objectContaining({ trustFingerprint: 'SHA256:verified' }),
  );
});

it('does not save passwords or passphrases in remembered profiles', async () => {
  change('Authentication', 'password');
  change('Password', 'secret');
  change('Name', 'Changed');
  submit();
  await vi.waitFor(() => expect(connected).toHaveBeenCalledOnce());
  expect(api.connectSftp).toHaveBeenCalledWith(
    expect.objectContaining({ password: 'secret', auth: 'password' }),
  );
  expect(preferences.sftpProfiles).toEqual([{ ...profile, name: 'Changed' }]);
  expect(
    target.querySelector<HTMLInputElement>('input[type="password"]')?.value,
  ).toBe('');
});

it('reports changed keys or auth failures in place without remembering edited details', async () => {
  vi.mocked(api.connectSftp).mockRejectedValue(
    new Error('Host key changed; blocked'),
  );
  change('Name', 'Do not save');
  submit();
  await vi.waitFor(() => {
    flushSync();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain(
      'Host key changed',
    );
  });
  expect(preferences.sftpProfiles).toEqual([profile]);
  expect(connected).not.toHaveBeenCalled();
  expect(close).not.toHaveBeenCalled();
});

it('does not save edits when Remember is unchecked', async () => {
  change('Name', 'Temporary');
  target.querySelector<HTMLInputElement>('input[type="checkbox"]')!.click();
  submit();
  await vi.waitFor(() => expect(connected).toHaveBeenCalledOnce());
  expect(preferences.sftpProfiles).toEqual([profile]);
});

it('disconnects the selected connection', async () => {
  [...target.querySelectorAll<HTMLButtonElement>('button')]
    .find((button) => button.textContent === 'Disconnect')!
    .click();
  await vi.waitFor(() => expect(disconnected).toHaveBeenCalledWith(profile.id));
  expect(api.disconnectSftp).toHaveBeenCalledWith(profile.id);
});
