<script lang="ts">
  import { onMount, tick } from 'svelte';
  import {
    gridStep,
    gridWindow,
    type GalleryImage,
  } from '../utils/imageGallery';
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
  let scrollTop = $state(0);
  let height = $state(0);
  // Measured from the rendered grid; rowStep is 0 until a tile is laid out.
  let columns = $state(1);
  let rowStep = $state(0);
  let gap = $state(0);

  // Only rows near the viewport are rendered, so a folder of thousands of
  // images keeps a few dozen tiles (and thumbnail loads) alive.
  let totalRows = $derived(Math.ceil(images.length / columns));
  let { firstRow, lastRow } = $derived(
    gridWindow(scrollTop, height, rowStep, totalRows),
  );
  let shown = $derived(images.slice(firstRow * columns, lastRow * columns));

  function measure() {
    if (!grid) return;
    const style = getComputedStyle(grid);
    columns = style.gridTemplateColumns.split(' ').filter(Boolean).length || 1;
    gap = parseFloat(style.rowGap) || 0;
    const tile = grid.querySelector<HTMLElement>('.image-tile');
    rowStep = tile?.offsetHeight ? tile.offsetHeight + gap : 0;
  }

  onMount(() => grid.focus({ preventScroll: true }));

  $effect(() => {
    // Tiles appear (and can be measured) once there are images.
    void images.length;
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    return () => observer.disconnect();
  });

  $effect(() => {
    const selected = index;
    // Render the selected row first; then scroll it exactly into view.
    if (grid && rowStep) {
      const top = Math.floor(selected / columns) * rowStep;
      if (top < grid.scrollTop) grid.scrollTop = top;
      else if (top + rowStep > grid.scrollTop + grid.clientHeight)
        grid.scrollTop = top + rowStep - grid.clientHeight;
      scrollTop = grid.scrollTop;
    }
    void tick().then(() =>
      grid
        ?.querySelector<HTMLElement>(`[data-index="${selected}"]`)
        ?.scrollIntoView?.({ block: 'nearest' }),
    );
  });

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
    const rows = rowStep ? Math.floor(grid.clientHeight / rowStep) : 1;
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
  bind:clientHeight={height}
  onscroll={() => (scrollTop = grid.scrollTop)}
  onkeydown={keydown}
>
  {#if firstRow}
    <div
      aria-hidden="true"
      style:grid-column="1 / -1"
      style:height={`${firstRow * rowStep - gap}px`}
    ></div>
  {/if}
  {#each shown as image, offset (image.path)}
    {@const i = firstRow * columns + offset}
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
  {#if lastRow < totalRows}
    <div
      aria-hidden="true"
      style:grid-column="1 / -1"
      style:height={`${(totalRows - lastRow) * rowStep - gap}px`}
    ></div>
  {/if}
</div>
