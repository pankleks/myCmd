<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { SelectionMode } from '../state/commander.svelte';

  let {
    mode,
    onsubmit,
    onclose,
  }: {
    mode: SelectionMode;
    onsubmit: (pattern: string) => void;
    onclose: () => void;
  } = $props();

  let pattern = $state('*.*');
  let dialog: HTMLDialogElement;
  const title = $derived(
    mode === 'extend' ? 'Add to selection' : 'Remove from selection',
  );

  onMount(() => {
    dialog.showModal();
    void tick().then(() => {
      const input = dialog.querySelector('input');
      input?.focus();
      input?.select();
    });
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
      onsubmit(pattern);
    }}
  >
    <h2>{title}</h2>
    <label
      >Glob pattern<input
        bind:value={pattern}
        required
        autocomplete="off"
        spellcheck="false"
      /></label
    >
    <p>Example: *.exe (case-insensitive)</p>
    <div class="dialog-actions">
      <button type="button" onclick={onclose}>Cancel</button><button
        class="primary"
        type="submit">{title}</button
      >
    </div>
  </form>
</dialog>
