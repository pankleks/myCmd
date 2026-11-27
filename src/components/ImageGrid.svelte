<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { gridStep, type GalleryImage } from '../utils/imageGallery';
  import ImageThumbnail from './ImageThumbnail.svelte';

  let {
    images,
    index,
    onselect,
    onopen,
  }: {
    images: GalleryImage[];
    index: number;
    onselect: (index: number) => void;
    onopen: (index: number) => void;
  } = $props();
  let grid: HTMLDivElement;

  onMount(() => grid.focus({ preventScroll: true }));

  $effect(() => {
    const selected = index;
    void tick().then(() =>
      grid
        ?.querySelector<HTMLElement>(`[data-index="${selected}"]`)
        ?.scrollIntoView?.({ block: 'nearest' }),
    );
  });

  function layout() {
    const columns =
      getComputedStyle(grid).gridTemplateColumns.split(' ').filter(Boolean)
        .length || 1;
    const tile = grid.querySelector<HTMLElement>('.image-tile');
    const rows = tile?.offsetHeight
      ? Math.floor(grid.clientHeight / tile.offsetHeight)
      : 1;
    return { columns, rows };
  }

  function keydown(event: KeyboardEvent) {
    if (
      event.defaultPrevented ||
      event.isComposing ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey
    )
      return;
    if (event.key === 'Enter') {
      event.preventDefault();
      if (images[index]) onopen(index);
      return;
    }
    const { columns, rows } = layout();
    const next = gridStep(event.key, index, images.length, columns, rows);
    if (next === undefined) return;
    event.preventDefault();
    if (next !== index) onselect(next);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex (the listbox owns keyboard navigation) -->
<div
  class="image-grid"
  role="listbox"
  aria-label="Images in folder"
  tabindex="0"
  aria-activedescendant={images[index] ? `image-tile-${index}` : undefined}
  bind:this={grid}
  onkeydown={keydown}
>
  {#each images as image, i (image.path)}
    <!-- svelte-ignore a11y_click_events_have_key_events (keys are handled by the listbox) -->
    <div
      id={`image-tile-${i}`}
      class="image-tile"
      class:selected={i === index}
      role="option"
      tabindex="-1"
      aria-selected={i === index}
      data-index={i}
      title={image.name}
      onclick={() => onselect(i)}
      ondblclick={() => onopen(i)}
    >
      <ImageThumbnail {image} />
      <span class="image-tile-name">{image.name}</span>
    </div>
  {/each}
</div>
