<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { FileEntry } from '../filesystem/types';
  import type { FileOperation } from '../operations/types';
  export type Action =
    'copy' | 'move' | 'rename' | 'createDirectory' | 'delete';
  let {
    action,
    entries,
    parent,
    destination,
    onsubmit,
    onclose,
  }: {
    action: Action;
    entries: FileEntry[];
    parent: string;
    destination: string;
    onsubmit: (operation: FileOperation) => void;
    onclose: () => void;
  } = $props();
  const labels = {
    copy: 'Copy',
    move: 'Move',
    rename: 'Rename',
    createDirectory: 'New folder',
    delete: 'Delete permanently',
  };
  let value = $state('');
  let dialog: HTMLDialogElement;
  onMount(() => {
    value =
      action === 'copy' || action === 'move'
        ? destination
        : action === 'rename'
          ? entries[0].name
          : '';
    dialog.showModal();
    void tick().then(() => dialog.querySelector('input')?.select());
  });
  function submit() {
    const paths = entries.map((e) => e.path);
    const operation: FileOperation =
      action === 'copy' || action === 'move'
        ? { type: action, sources: paths, destination: value }
        : action === 'rename'
          ? { type: action, path: paths[0], name: value }
          : action === 'createDirectory'
            ? { type: action, parent, name: value }
            : { type: 'delete', sources: paths };
    onsubmit(operation);
  }
</script>

<dialog
  bind:this={dialog}
  oncancel={(e) => {
    e.preventDefault();
    onclose();
  }}
>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <h2>{labels[action]}</h2>
    {#if action === 'delete'}
      <p>Delete {entries.length} items permanently?</p>
      <p>
        {entries.filter((e) => e.type === 'directory').length} folders, {entries.filter(
          (e) => e.type !== 'directory',
        ).length} files / links.
      </p>
      <p class="danger-text">
        Folders and all their contents will be deleted. This cannot be undone.
      </p>
    {:else}
      {#if action !== 'createDirectory'}<p>
          {entries.length === 1
            ? entries[0].name
            : `${entries.length} selected items`}
        </p>{/if}
      <label
        >{action === 'copy' || action === 'move'
          ? 'Destination folder'
          : 'Name'}<input bind:value required spellcheck="false" /></label
      >
    {/if}
    <div class="dialog-actions">
      <button type="button" onclick={onclose}>Cancel</button><button
        class:danger={action === 'delete'}
        class="primary"
        type="submit">{labels[action]}</button
      >
    </div>
  </form>
</dialog>
