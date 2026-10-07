// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import SettingsDialog from './SettingsDialog.svelte';
import OperationDialog from './OperationDialog.svelte';
import { version } from '../../package.json';
import type { FileEntry } from '../filesystem/types';

let component: ReturnType<typeof mount>;
let target: HTMLDivElement;

beforeEach(() => {
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    value: vi.fn(),
    configurable: true,
  });
  target = document.createElement('div');
  document.body.append(target);
});

afterEach(async () => {
  await unmount(component);
  target.remove();
  vi.restoreAllMocks();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});

it.each([
  [' /opt/My Editor/editor ', '/opt/My Editor/editor'],
  ['   ', null],
])('saves editor setting %s', (value, editor) => {
  const onsubmit = vi.fn();
  component = mount(SettingsDialog, {
    target,
    props: {
      fileFontSize: 18,
      showHidden: false,
      showFunctionBar: true,
      editor: '/usr/bin/editor',
      onsubmit,
      onclose: vi.fn(),
    },
  });
  flushSync();
  expect(target.querySelector('footer')?.textContent).toContain(
    `myCmd · v${version}`,
  );
  expect(target.textContent).not.toContain('Customize your file manager.');
  expect(
    [...target.querySelectorAll('h3')].map((heading) => heading.textContent),
  ).toEqual(['Appearance', 'Editor']);
  const input = target.querySelector<HTMLInputElement>('input[placeholder]')!;
  expect(input.value).toBe('/usr/bin/editor');
  input.value = value as string;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  target
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { cancelable: true }));
  expect(onsubmit).toHaveBeenCalledWith({
    fileFontSize: 18,
    showHidden: false,
    showFunctionBar: true,
    editor,
  });
});

it.each(['directory', 'file', undefined] as const)(
  'defaults the new folder name for a cursor on %s',
  (type) => {
    const entry: FileEntry = {
      name: 'Projects',
      path: '/current/Projects',
      type: type ?? 'file',
      extension: '',
      size: 0,
      hidden: false,
      readonly: false,
      directoryTarget: type === 'directory',
    };
    const onsubmit = vi.fn();
    component = mount(OperationDialog, {
      target,
      props: {
        action: 'createDirectory',
        entries: type ? [entry] : [],
        parent: '/current',
        destination: '/opposite',
        permanent: false,
        onsubmit,
        onclose: vi.fn(),
      },
    });
    flushSync();
    const input = target.querySelector('input')!;
    expect(input.value).toBe(type === 'directory' ? 'Projects' : '');
    input.value = 'New Projects';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    target
      .querySelector('form')!
      .dispatchEvent(new Event('submit', { cancelable: true }));
    expect(onsubmit).toHaveBeenCalledWith({
      type: 'createDirectory',
      parent: '/current',
      name: 'New Projects',
    });
  },
);

it('asks for a new filename even in an empty directory', () => {
  const onsubmit = vi.fn();
  component = mount(OperationDialog, {
    target,
    props: {
      action: 'createFile',
      entries: [],
      parent: '/current',
      destination: '/opposite',
      permanent: false,
      onsubmit,
      onclose: vi.fn(),
    },
  });
  flushSync();
  expect(target.querySelector('h2')?.textContent).toBe('New file and edit');
  const input = target.querySelector('input')!;
  expect(input.value).toBe('');
  input.value = 'new file.txt';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  target
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { cancelable: true }));
  expect(onsubmit).toHaveBeenCalledWith({
    type: 'createFile',
    parent: '/current',
    name: 'new file.txt',
  });
});
