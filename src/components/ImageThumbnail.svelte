<script lang="ts">
  import Icon from '@iconify/svelte';
  import { onMount } from 'svelte';
  import { errorMessage } from '../filesystem/api';
  import { icons } from '../utils/uiIcons';
  import type { GalleryImage } from '../utils/imageGallery';
  import { loadThumbnail } from '../utils/thumbnails';

  let { image }: { image: GalleryImage } = $props();
  let url = $state('');
  let error = $state('');

  // The grid mounts only tiles near the viewport: load at once, and give the
  // place in the queue back when the tile scrolls away.
  onMount(() => {
    const controller = new AbortController();
    loadThumbnail(image, controller.signal).then(
      (result) => (url = result),
      (cause) => {
        if (!controller.signal.aborted) error = errorMessage(cause);
      },
    );
    return () => controller.abort();
  });
</script>

<div class="image-thumbnail">
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
