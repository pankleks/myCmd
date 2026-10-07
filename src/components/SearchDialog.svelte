<script lang="ts">
  import { onMount, tick } from 'svelte';
  import FolderPicker from './FolderPicker.svelte';
  import type { Root } from '../filesystem/types';
  let {
    folder,
    roots,
    onsubmit,
    onclose,
  }: {
    folder: string;
    roots: Root[];
    onsubmit: (folder: string, pattern: string) => void;
    onclose: () => void;
  } = $props();
  let root = $state('');
  let pattern = $state('*.*');
  let dialog: HTMLDialogElement;
  onMount(() => {
    root = folder;
    dialog.showModal();
    void tick().then(() => dialog.querySelector('input')?.select());
  });
</script>

<dialog
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      onsubmit(root, pattern);
    }}
  >
    <h2>Search files and folders</h2>
    <label
      >Glob pattern<input
        bind:value={pattern}
        required
        autocomplete="off"
        spellcheck="false"
      /></label
    >
    <div class="search-folder">
      <span>Search folder</span>
      <FolderPicker
        bind:value={root}
        path={folder}
        label={folder}
        {roots}
        id="search-folder-picker"
        ariaLabel="Search folder"
        required
        onchoose={(path) => {
          root = path;
        }}
      />
    </div>
    <p>
      Searches names recursively, including folders. Archives and directory
      links are not traversed.
    </p>
    <div class="dialog-actions">
      <button type="button" onclick={onclose}>Cancel</button><button
        type="submit"
        class="primary">Search</button
      >
    </div>
  </form>
</dialog>

<style>
  dialog {
    overflow: visible;
  }
  .search-folder {
    display: grid;
    gap: 8px;
    margin-top: 14px;
  }
  .search-folder :global(.location-menu) {
    max-height: min(240px, 30vh);
  }
</style>
