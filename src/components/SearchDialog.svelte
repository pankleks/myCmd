<script lang="ts">
  import { onMount, tick } from 'svelte';
  let {
    folder,
    onsubmit,
    onclose,
  }: {
    folder: string;
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
    <label
      >Search folder<input
        bind:value={root}
        required
        spellcheck="false"
      /></label
    >
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
