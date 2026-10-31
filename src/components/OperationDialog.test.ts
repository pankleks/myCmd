// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import OperationDialog, { type Action } from './OperationDialog.svelte';
import type { FileEntry } from '../filesystem/types';

let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
const submit = vi.fn();
const close = vi.fn();

function entry(name: string, type: FileEntry['type'] = 'directory'): FileEntry {
  return {
    name,
    path: `/source/${name}`,
    type,
    extension: '',
    size: 0,
    hidden: false,
    directoryTarget: type === 'directory',
  };
}

function show(entries: FileEntry[], action: Action = 'compress') {
  component = mount(OperationDialog, {
    target,
    props: {
      action,
      entries,
      parent: '/source',
      destination: '/target',
      permanent: false,
      onsubmit: submit,
      onclose: close,
    },
  });
  flushSync();
}

beforeEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  target = document.createElement('div');
  document.body.append(target);
});

afterEach(async () => {
  if (component) await unmount(component);
  target.remove();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});

it.each([
  [[entry('My folder')], 'My folder.zip'],
  [[entry('notes.txt', 'file')], 'notes.txt.zip'],
  [[entry('One'), entry('Two')], 'Archive.zip'],
])('defaults the ZIP name for %j to %s', async (entries, name) => {
  show(entries as FileEntry[]);
  const inputs = target.querySelectorAll('input');
  expect(inputs[0].value).toBe(name);
  expect(inputs[1].value).toBe('/target');
  await tick();
  expect(inputs[0].selectionEnd).toBe(name.length);
  target.querySelector<HTMLButtonElement>('[type="submit"]')!.click();
  expect(submit).toHaveBeenCalledExactlyOnceWith({
    type: 'compress',
    sources: (entries as FileEntry[]).map((e) => e.path),
    destination: '/target',
    name,
  });
});

it('submits an edited name and destination', () => {
  show([entry('Folder')]);
  const inputs = target.querySelectorAll('input');
  for (const [index, value] of ['Backup.zip', '/another target'].entries()) {
    inputs[index].value = value;
    inputs[index].dispatchEvent(new Event('input', { bubbles: true }));
  }
  target.querySelector<HTMLButtonElement>('[type="submit"]')!.click();
  expect(submit).toHaveBeenCalledExactlyOnceWith({
    type: 'compress',
    sources: ['/source/Folder'],
    destination: '/another target',
    name: 'Backup.zip',
  });
});

it('cancels without starting compression', () => {
  show([entry('Folder')]);
  target
    .querySelector('dialog')!
    .dispatchEvent(new Event('cancel', { cancelable: true }));
  expect(close).toHaveBeenCalledOnce();
  expect(submit).not.toHaveBeenCalled();
});

it('keeps the existing copy dialog unchanged', () => {
  show([entry('Folder')], 'copy');
  expect(target.querySelectorAll('input')).toHaveLength(1);
  expect(target.querySelector('input')!.value).toBe('/target');
  target.querySelector<HTMLButtonElement>('[type="submit"]')!.click();
  expect(submit).toHaveBeenCalledExactlyOnceWith({
    type: 'copy',
    sources: ['/source/Folder'],
    destination: '/target',
  });
});
