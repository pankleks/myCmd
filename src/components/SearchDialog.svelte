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
    onsubmit: (folder: string, pattern: string, text?: string) => void;
    onclose: () => void;
  } = $props();
  let root = $state('');
  let pattern = $state('*.*');
  let searchContent = $state(false);
  let searchText = $state('');
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
      if (searchContent && !searchText) return;
      onsubmit(root, pattern, searchContent ? searchText : undefined);
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
    <label class="check-label content-toggle">
      <input type="checkbox" bind:checked={searchContent} />
      Search text inside files
    </label>
    <label class="content-query">
      Text to find
      <input
        bind:value={searchText}
        disabled={!searchContent}
        required={searchContent}
        placeholder="Enter text to find…"
        autocomplete="off"
        spellcheck="false"
        aria-describedby="content-search-hint"
      />
    </label>
    <p id="content-search-hint" class="content-hint">
      Literal, case-insensitive text search. Searches textual files matching the
      glob pattern; binary files are skipped.
    </p>
    <p>
      {searchContent
        ? 'Searches recursively and returns files containing the text.'
        : 'Searches names recursively, including folders.'}
      Archives and directory links are not traversed.
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
  .content-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 20px;
  }
  .content-query {
    margin-top: 14px;
  }
  .content-query input:disabled {
    opacity: 0.45;
  }
  .content-hint {
    margin-top: 7px;
    font-size: 12px;
  }
</style>
