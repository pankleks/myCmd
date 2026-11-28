<script lang="ts">
  import { onMount } from 'svelte';
  import { trashName } from '../utils/format';
  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  const shortcuts = [
    ['F1', 'Keyboard shortcuts'],
    ['F2', 'Rename selected item'],
    ['F3', 'View file; summary for a folder'],
    [
      'Ctrl+F / Cmd+F in view',
      'Search the file; Enter / Shift+Enter: next / previous',
    ],
    ['↑ / ↓ in image view', 'Previous / next image in the folder'],
    ['F3 in image view', 'Switch between the image and a thumbnail grid'],
    ['Shift+F3', 'Toggle preview pane'],
    ['F4', 'Edit file in configured or system default application'],
    ['Shift+F4', 'Create a new file and edit it'],
    ['F5', 'Copy / extract from archive'],
    ['Alt+F5', 'Compress selected items to ZIP in the opposite panel'],
    ['F6', 'Move'],
    ['F7', 'New folder'],
    ['Alt+F7', 'Search names by glob, optionally text inside files'],
    [
      'F8 / Delete',
      `Delete to ${trashName}; permanent inside archives and on SFTP`,
    ],
    ['Shift+F8 / Shift+Delete', 'Delete permanently'],
    ['F9', 'Settings'],
    ['Alt+Enter', 'Properties of the focused local file or folder'],
    ['↑ / ↓ / ← / →', 'Move cursor; left/right go to first/last item'],
    ['Home / End', 'First / last item'],
    ['Page Up / Page Down', 'Move cursor by one page'],
    [
      'Enter / double-click',
      'Open or run file, folder or archive; locate search result',
    ],
    ['Backspace', 'Parent folder; leave search results'],
    ['Tab', 'Switch active panel (closes the preview pane)'],
    ['Shift+Tab', 'Swap panels'],
    ['Alt+F1 / Alt+F2', 'Choose drive in left / right panel'],
    ['Space', 'Toggle selection; calculate folder size'],
    ['Insert', 'Toggle selection and move cursor down'],
    ['*', 'Invert selection'],
    ['+ / -', 'Add / remove selection by glob'],
    ['Ctrl+A', 'Select all visible items'],
    ['Ctrl+C / Cmd+C', 'Copy selected files to the system clipboard'],
    [
      'Ctrl+X / Cmd+X',
      'Cut selected files for moving through the system clipboard',
    ],
    ['Shift+F10 / Menu key', 'Open the file context menu'],
    [
      'Ctrl+V / Cmd+V',
      'Paste files into the active folder; move files marked as Cut',
    ],
    [
      'Esc',
      'Clear command / selection; close quick find or dialog; leave search',
    ],
    ['F-keys, ↑ / ↓, Tab while typing a command', 'Work on the panel as usual'],
    ['Ctrl+click / Shift+click', 'Toggle selection / select range'],
    ['Alt+letter', 'Quick find (outside text fields); Enter or Esc closes it'],
    ['Ctrl+H', 'Show / hide hidden files'],
    ['Ctrl+R', 'Refresh panel / repeat search'],
    ['Ctrl+I', 'Open same location in opposite panel'],
    ['Ctrl+P', 'Edit folder path'],
    ['Ctrl+Enter', 'Append cursor item filename to command input'],
    ['Ctrl+T', 'Open system terminal in current folder'],
    [
      'Ctrl++ / Ctrl+- / Ctrl+0',
      'Increase / decrease / reset file-list font size',
    ],
    ['Arrow Left / Right on column divider', 'Resize column'],
    ['Double-click column header', 'Reset column widths'],
    [
      'Typing / Enter in command field',
      'Enter / run command in active local folder',
    ],
  ];
  onMount(() => dialog.showModal());
</script>

<dialog
  class="shortcuts-dialog dialog-lg"
  aria-labelledby="shortcuts-title"
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <h2 id="shortcuts-title">Keyboard shortcuts</h2>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (the list scrolls with the keyboard) -->
  <div class="shortcuts-list" tabindex="0">
    <table>
      <tbody
        >{#each shortcuts as [key, description]}<tr
            ><th scope="row"><kbd>{key}</kbd></th><td>{description}</td></tr
          >{/each}</tbody
      >
    </table>
  </div>
  <div class="dialog-actions">
    <button class="primary" onclick={onclose}>Close</button>
  </div>
</dialog>
