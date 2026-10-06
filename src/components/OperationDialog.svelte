<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { FileEntry } from '../filesystem/types';
  import type { FileOperation } from '../operations/types';
  import { isLocalPath } from '../filesystem/providers';
  import {
    countDeleteEntries,
    type DeleteCounts,
  } from '../filesystem/deleteCounts';
  export type Action =
    'copy' | 'move' | 'rename' | 'createDirectory' | 'delete';
  let {
    action,
    entries,
    parent,
    destination,
    permanent,
    onsubmit,
    onclose,
  }: {
    action: Action;
    entries: FileEntry[];
    parent: string;
    destination: string;
    permanent: boolean;
    onsubmit: (operation: FileOperation) => void;
    onclose: () => void;
  } = $props();
  const labels = {
    copy: 'Copy',
    move: 'Move',
    rename: 'Rename',
    createDirectory: 'New folder',
  };
  let title = $derived(
    action === 'delete'
      ? permanent
        ? 'Delete permanently'
        : 'Move to Recycle Bin'
      : labels[action],
  );
  let value = $state('');
  let dialog: HTMLDialogElement;
  let counts = $state<DeleteCounts>();
  let countFailed = $state(false);
  onMount(() => {
    value =
      action === 'copy' || action === 'move'
        ? destination
        : action === 'rename'
          ? entries[0].name
          : '';
    dialog.showModal();
    void tick().then(() => dialog.querySelector('input')?.select());
    let disposed = false;
    if (action === 'delete') {
      void countDeleteEntries(entries, () => disposed)
        .then((result) => {
          if (!disposed) counts = result;
        })
        .catch(() => {
          if (!disposed) countFailed = true;
        });
    }
    return () => {
      disposed = true;
    };
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
            : { type: 'delete', sources: paths, permanent };
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
    <h2>{title}</h2>
    {#if action === 'delete'}
      {#if permanent}
        <p>Delete {entries.length} items permanently?</p>
        <p>
          {#if counts}{counts.folders} folders, {counts.files} files / links (including
            all children).
          {:else if countFailed}Unable to count all contents.
          {:else}Counting contents…{/if}
        </p>
        <p class="danger-text">
          Folders and all their contents will be deleted. This cannot be undone.
        </p>
        {#if !isLocalPath(parent)}<p>
            The archive will be rewritten. Deleted entries do not go to the
            Recycle Bin.
          </p>{/if}
      {:else}
        <p>Move {entries.length} items to the Recycle Bin?</p>
        <p>
          {#if counts}{counts.folders} folders, {counts.files} files / links (including
            all children).
          {:else if countFailed}Unable to count all contents.
          {:else}Counting contents…{/if}
        </p>
        <p>You can restore them from the Recycle Bin.</p>
      {/if}
      {#if counts?.skipped}<p class="danger-text">
          Count is incomplete: {counts.skipped} entries could not be read.
        </p>{/if}
    {:else}
      {#if action !== 'createDirectory'}<p>
          {entries.length === 1
            ? entries[0].name
            : `${entries.length} selected items`}
        </p>{/if}
      <label
        >{action === 'copy' || action === 'move'
          ? entries.length === 1 &&
            isLocalPath(parent) &&
            entries[0].type !== 'directory'
            ? 'Destination folder or filename'
            : 'Destination folder'
          : 'Name'}<input bind:value required spellcheck="false" /></label
      >
    {/if}
    <div class="dialog-actions">
      <button type="button" onclick={onclose}>Cancel</button><button
        class:danger={action === 'delete' && permanent}
        class="primary"
        type="submit">{title}</button
      >
    </div>
  </form>
</dialog>
