<script lang="ts">
  import { onMount } from 'svelte';
  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  const shortcuts = [
    ['F1', 'Keyboard shortcuts'],
    ['F2', 'Rename selected item'],
    ['F3', 'View file'],
    ['Shift+F3', 'Toggle preview pane'],
    ['F5', 'Copy / extract from archive'],
    ['F6', 'Move'],
    ['F7', 'Create folder'],
    ['Alt+F7', 'Search file and folder names by glob'],
    ['F8 / Delete', 'Delete to Recycle Bin; permanent inside archives'],
    ['Shift+F8 / Shift+Delete', 'Delete permanently'],
    ['F9', 'Settings'],
    ['↑ / ↓ / ← / →', 'Move cursor; left/right go to first/last item'],
    ['Home / End', 'First / last item'],
    ['Page Up / Page Down', 'Move cursor by one page'],
    [
      'Enter / double-click',
      'Open file, folder or archive; locate search result',
    ],
    ['Backspace', 'Parent folder; leave search results'],
    ['Tab', 'Switch active panel'],
    ['Shift+Tab', 'Swap panels'],
    ['Alt+F1 / Alt+F2', 'Choose drive in left / right panel'],
    ['Space', 'Toggle selection; calculate directory size'],
    ['Insert', 'Toggle selection and move cursor down'],
    ['*', 'Invert selection'],
    ['+ / -', 'Add / remove selection by glob'],
    ['Ctrl+A', 'Select all visible items'],
    [
      'Esc',
      'Clear command / selection; close quick find or dialog; leave search',
    ],
    ['Ctrl+click / Shift+click', 'Toggle selection / select range'],
    ['Alt+letter', 'Quick find (outside text fields)'],
    ['Ctrl+H', 'Show / hide hidden files'],
    ['Ctrl+R', 'Refresh panel / repeat search'],
    ['Ctrl+I', 'Open same location in opposite panel'],
    ['Ctrl+L', 'Edit directory path'],
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
  class="shortcuts-dialog"
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <h2>Keyboard shortcuts</h2>
  <div class="shortcuts-list">
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
