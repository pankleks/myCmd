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
  let close: HTMLButtonElement;
  onMount(() => {
    dialog.showModal();
    close.focus(); // Enter closes; the output itself is the next Tab stop.
  });
</script>

<dialog
  aria-labelledby="error-title"
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <h2 id="error-title">{title}</h2>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (long text scrolls with the keyboard) -->
  {#if command}<p class="error-command">{command}</p>
    <pre class="error-output" tabindex="0">{output}</pre>
  {:else}
    <!-- A message, not program output: plain text, not a monospace block. -->
    <p class="error-message" tabindex="0">{output}</p>
  {/if}
  <div class="dialog-actions">
    <button type="button" class="primary" bind:this={close} onclick={onclose}
      >Close</button
    >
  </div>
</dialog>
