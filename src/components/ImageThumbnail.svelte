<script lang="ts">
  import Icon from '@iconify/svelte';
  import { onDestroy, onMount } from 'svelte';
  import { errorMessage } from '../filesystem/api';
  import { icons } from '../utils/uiIcons';
  import type { GalleryImage } from '../utils/imageGallery';
  import { loadThumbnail } from '../utils/thumbnails';

  let { image }: { image: GalleryImage } = $props();
  let element: HTMLDivElement;
  let url = $state('');
  let error = $state('');
  let controller: AbortController | undefined;
  let observer: IntersectionObserver | undefined;

  function start() {
    if (url || error || controller) return;
    const current = (controller = new AbortController());
    loadThumbnail(image, current.signal)
      .then((result) => {
        if (current.signal.aborted) {
          if (result.startsWith('blob:')) URL.revokeObjectURL(result);
        } else url = result;
      })
      .catch((cause) => {
        if (!current.signal.aborted) error = errorMessage(cause);
      })
      .finally(() => {
        if (controller === current) controller = undefined;
      });
  }

  // Tiles scrolled away before their turn give their place to visible ones.
  function stop() {
    if (url || !controller) return;
    controller.abort();
    controller = undefined;
  }

  onMount(() => {
    if (typeof IntersectionObserver === 'undefined') {
      start();
      return;
    }
    observer = new IntersectionObserver(
      (entries) =>
        entries.some((entry) => entry.isIntersecting) ? start() : stop(),
      { root: element.closest('.image-grid'), rootMargin: '200px' },
    );
    observer.observe(element);
  });

  onDestroy(() => {
    observer?.disconnect();
    controller?.abort();
    if (url.startsWith('blob:')) URL.revokeObjectURL(url);
  });
</script>

<div class="image-thumbnail" bind:this={element}>
  {#if url && !error}
    <img
      src={url}
      alt=""
      draggable="false"
      onerror={() => (error = 'This image could not be displayed.')}
    />
  {:else if error}
    <span class="image-thumbnail-error" title={error}
      ><Icon icon={icons['image-off']} width="28" height="28" /></span
    >
  {/if}
</div>
