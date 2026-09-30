<script lang="ts">
  import FilePreview from './FilePreview.svelte';

  let {
    target,
    loading = false,
  }: {
    target?: { path: string; name: string; extension: string; key: string };
    loading?: boolean;
  } = $props();

  let settledTarget = $state<typeof target>();

  $effect(() => {
    const next = target;
    // Dispose the previous preview immediately; only open a file once the
    // cursor has stopped moving. Effect cleanup also handles panel unmount.
    settledTarget = undefined;
    if (!next) return;
    const timer = setTimeout(() => {
      settledTarget = next;
    }, 200);
    return () => clearTimeout(timer);
  });
</script>

<section
  class="panel preview-panel preview-inline-content"
  aria-label={target ? `Preview ${target.name}` : 'File preview'}
>
  {#if settledTarget}
    {#key settledTarget.key}
      <FilePreview
        path={settledTarget.path}
        name={settledTarget.name}
        extension={settledTarget.extension}
        title="Preview"
      />
    {/key}
  {:else}
    <div class="viewer-header">
      <div class="viewer-heading"><h2>Preview</h2></div>
    </div>
    <div class="preview-empty">
      {loading
        ? 'Loading files…'
        : target
          ? 'Waiting to preview…'
          : 'Select a file to preview.'}
    </div>
  {/if}
</section>
