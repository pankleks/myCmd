<script lang="ts">
  import Icon from '@iconify/svelte';
  import { icons } from '../utils/uiIcons';
  import { onMount } from 'svelte';
  import FilePreview from './FilePreview.svelte';

  let {
    path,
    name,
    extension,
    isDirectory = false,
    onclose,
  }: {
    path: string;
    name: string;
    extension: string;
    isDirectory?: boolean;
    onclose: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  onMount(() => dialog.showModal());
</script>

{#snippet headerActions()}
  <button
    type="button"
    class="viewer-close"
    onclick={onclose}
    aria-label="Close viewer"
    ><Icon icon={icons.x} width="20" height="20" /></button
  >
{/snippet}

<dialog
  class="file-viewer"
  bind:this={dialog}
  aria-label={`View ${name}`}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
  onkeydown={(event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onclose();
    }
  }}
>
  <FilePreview
    {path}
    {name}
    {extension}
    {isDirectory}
    autofocus
    onopened={onclose}
    {headerActions}
  />
</dialog>
