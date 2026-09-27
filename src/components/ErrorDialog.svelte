<script lang="ts">
  import { onMount } from 'svelte';
  let {
    command,
    title = 'Command failed',
    output,
    onclose,
  }: {
    command?: string;
    title?: string;
    output: string;
    onclose: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  onMount(() => dialog.showModal());
</script>

<dialog
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <h2>{title}</h2>
  {#if command}<p class="error-command">{command}</p>{/if}
  <pre class="error-output">{output}</pre>
  <div class="dialog-actions">
    <button type="button" class="primary" onclick={onclose}>Close (Esc)</button>
  </div>
</dialog>
