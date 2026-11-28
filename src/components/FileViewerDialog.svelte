<script lang="ts">
  import Icon from './Icon.svelte';
  import { icons } from '../utils/uiIcons';
  import { onMount, tick } from 'svelte';
  import { viewerStep, type GalleryImage } from '../utils/imageGallery';
  import FilePreview from './FilePreview.svelte';
  import ImageGrid from './ImageGrid.svelte';

  let {
    path,
    name,
    extension,
    isDirectory = false,
    images = [],
    index = -1,
    onnavigate,
    onclose,
  }: {
    path: string;
    name: string;
    extension: string;
    isDirectory?: boolean;
    /** Images of the folder; given when the viewed file is one of them. */
    images?: GalleryImage[];
    index?: number;
    onnavigate?: (index: number) => void;
    onclose: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  let mode = $state<'single' | 'grid'>('single');
  let gallery = $derived(!!images[index] && !!onnavigate);
  onMount(() => dialog.showModal());

  // Replacing the preview or the grid removes the focused element with it.
  async function keepFocus() {
    await tick();
    if (dialog && !dialog.contains(document.activeElement))
      dialog.focus({ preventScroll: true });
  }

  function navigate(next: number) {
    if (next !== index) onnavigate?.(next);
    void keepFocus();
  }

  function toggleGrid() {
    mode = mode === 'grid' ? 'single' : 'grid';
    void keepFocus();
  }

  function openFromGrid(next: number) {
    if (next !== index) onnavigate?.(next);
    mode = 'single';
    void keepFocus();
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onclose();
      return;
    }
    if (!gallery || event.defaultPrevented || event.isComposing) return;
    if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey)
      return;
    if (event.key === 'F3') {
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat) toggleGrid();
      return;
    }
    if (mode !== 'single') return;
    const next = viewerStep(event.key, index, images.length);
    if (next === undefined) return;
    event.preventDefault();
    navigate(next);
  }
</script>

{#snippet closeButton()}
  <button
    type="button"
    class="viewer-close"
    onclick={onclose}
    aria-label="Close viewer"
    ><Icon icon={icons.x} width="20" height="20" /></button
  >
{/snippet}

{#snippet headerActions()}
  {#if gallery}
    <span class="viewer-position" aria-label="Image position"
      >{index + 1} / {images.length}</span
    >
    <button
      type="button"
      class="viewer-icon"
      onclick={toggleGrid}
      aria-label="Show thumbnails"
      title="Show thumbnails (F3)"
      ><Icon icon={icons['layout-grid']} width="18" height="18" /></button
    >
  {/if}
  {@render closeButton()}
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_tabindex (keeps image keys working after the focused control is replaced) -->
<dialog
  class="file-viewer"
  bind:this={dialog}
  tabindex="-1"
  aria-label={`View ${name}`}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
  onkeydown={keydown}
>
  {#if gallery && mode === 'grid'}
    <div class="file-preview-content">
      <div class="viewer-header">
        <div class="viewer-heading">
          <h2>Thumbnails</h2>
          <span title={path}>{name}</span>
        </div>
        <div class="viewer-toolbar">
          <span class="viewer-position" aria-label="Image position"
            >{index + 1} / {images.length}</span
          >
          <button
            type="button"
            class="viewer-icon"
            onclick={toggleGrid}
            aria-label="Show image"
            title="Show image (F3)"
            ><Icon icon={icons['layout-grid']} width="18" height="18" /></button
          >
          {@render closeButton()}
        </div>
      </div>
      <ImageGrid
        {images}
        {index}
        onselect={(next) => onnavigate?.(next)}
        onopen={openFromGrid}
      />
    </div>
  {:else}
    {#key path}
      <FilePreview
        {path}
        {name}
        {extension}
        {isDirectory}
        autofocus
        onopened={onclose}
        {headerActions}
      />
    {/key}
  {/if}
</dialog>
