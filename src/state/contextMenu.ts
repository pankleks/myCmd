import type { MenuItem } from '../components/ContextMenu.svelte';
import type { Action } from '../components/OperationDialog.svelte';
import { isLocalPath, parseLocation } from '../filesystem/providers';
import type { Row } from './commander.svelte';

export interface ContextMenuState {
  panelPath: string;
  /** Opposite panel path as shown (may be a search session). */
  oppositePath: string;
  /** Opposite panel's real folder (search root when searching). */
  oppositeRoot: string;
  /** Right-click was on an item, not on empty space. */
  item: boolean;
  /** The clipboard holds files. */
  paste: boolean;
  entry: Row | undefined;
  entries: Row[];
  systemActionsRunning: boolean;
}

export interface ContextMenuActions {
  request: (action: Action, permanent?: boolean) => void;
  connections: () => void;
  refresh: () => void;
  open: () => void;
  openTerminal: () => void;
  paste: () => void;
  copy: () => void;
  cut: () => void;
  view: () => void;
  edit: () => void;
  copyPath: () => void;
  properties: () => void;
  systemActions: () => void;
}

export function contextMenuItems(
  state: ContextMenuState,
  actions: ContextMenuActions,
): MenuItem[] {
  const { entry, entries } = state;
  const localFolder = isLocalPath(state.panelPath);
  const remote = parseLocation(state.panelPath).kind === 'sftp';
  const refresh: MenuItem = {
    label: 'Refresh',
    shortcut: 'Ctrl+R',
    run: actions.refresh,
  };
  const paste: MenuItem = {
    label: 'Paste',
    shortcut: 'Ctrl+V',
    disabled: !state.paste || !localFolder,
    run: actions.paste,
  };
  if (!state.item && remote)
    return [
      {
        label: 'New folder…',
        shortcut: 'F7',
        run: () => actions.request('createDirectory'),
      },
      { label: 'Connections…', run: actions.connections },
      refresh,
    ];
  if (!state.item && !localFolder) return [refresh];
  if (!state.item)
    return [
      {
        label: 'New folder…',
        shortcut: 'F7',
        run: () => actions.request('createDirectory'),
      },
      {
        label: 'New file…',
        shortcut: 'Shift+F4',
        run: () => actions.request('createFile'),
      },
      { ...paste, separator: true },
      {
        label: 'Open terminal here',
        shortcut: 'Ctrl+T',
        separator: true,
        run: actions.openTerminal,
      },
      refresh,
    ];
  const single = entries.length === 1;
  const local =
    entries.length > 0 && entries.every((entry) => isLocalPath(entry.path));
  const localTarget = isLocalPath(state.oppositeRoot);
  const sftpTarget = parseLocation(state.oppositePath).kind === 'sftp';
  if (remote)
    return [
      {
        label: 'Open folder',
        shortcut: 'Enter',
        disabled: !single || !entry?.directoryTarget,
        run: actions.open,
      },
      {
        label: 'Download to opposite panel…',
        shortcut: 'F5',
        disabled: !localTarget,
        separator: true,
        run: () => actions.request('copy'),
      },
      {
        label: 'New folder…',
        shortcut: 'F7',
        run: () => actions.request('createDirectory'),
      },
      {
        label: 'Rename…',
        shortcut: 'F2',
        disabled: !single,
        run: () => actions.request('rename'),
      },
      {
        label: 'Delete permanently…',
        shortcut: 'F8',
        run: () => actions.request('delete', true),
      },
      { label: 'Connections…', separator: true, run: actions.connections },
    ];
  const items: MenuItem[] = [
    { label: 'Open', shortcut: 'Enter', disabled: !single, run: actions.open },
  ];
  if (
    entry &&
    isLocalPath(entry.path) &&
    (entry.type === 'directory' || !entry.directoryTarget)
  )
    items.push({
      label: 'View',
      shortcut: 'F3',
      disabled: !single,
      run: actions.view,
    });
  if (
    entry &&
    isLocalPath(entry.path) &&
    entry.type !== 'directory' &&
    !entry.directoryTarget
  )
    items.push({
      label: 'Edit',
      shortcut: 'F4',
      disabled: !single,
      run: actions.edit,
    });
  if (!local)
    return [
      ...items,
      {
        label: 'Extract to opposite panel…',
        shortcut: 'F5',
        disabled: !localTarget,
        separator: true,
        run: () => actions.request('copy'),
      },
      {
        label: 'Delete…',
        shortcut: 'F8',
        run: () => actions.request('delete'),
      },
    ];
  items.push(
    { label: 'Copy', shortcut: 'Ctrl+C', separator: true, run: actions.copy },
    { label: 'Cut', shortcut: 'Ctrl+X', run: actions.cut },
    paste,
    {
      label: sftpTarget
        ? 'Upload to opposite panel…'
        : 'Copy to opposite panel…',
      shortcut: 'F5',
      disabled: !localTarget && !sftpTarget,
      separator: true,
      run: () => actions.request('copy'),
    },
    {
      label: 'Move to opposite panel…',
      shortcut: 'F6',
      disabled: !localTarget,
      run: () => actions.request('move'),
    },
    {
      label: 'Compress to ZIP…',
      shortcut: 'Alt+F5',
      disabled: !localTarget,
      run: () => actions.request('compress'),
    },
    {
      label: 'Rename…',
      shortcut: 'F2',
      disabled: !single,
      run: () => actions.request('rename'),
    },
    { label: 'Delete…', shortcut: 'F8', run: () => actions.request('delete') },
    {
      label: 'Copy path',
      disabled: !single,
      separator: true,
      run: actions.copyPath,
    },
    {
      label: 'Properties…',
      shortcut: 'Alt+Enter',
      disabled: !single,
      run: actions.properties,
    },
    {
      label: state.systemActionsRunning
        ? 'System actions running…'
        : 'System actions…',
      disabled: !single || state.systemActionsRunning,
      separator: true,
      run: actions.systemActions,
    },
  );
  return items;
}
