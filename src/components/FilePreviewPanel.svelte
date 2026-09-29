<script lang="ts">
  import FilePreview from './FilePreview.svelte';

  let {
    target,
    loading = false,
  }: {
    target?: { path: string; name: string; extension: string; key: string };
    loading?: boolean;
  } = $props();
</script>

<section
  class="panel preview-panel preview-inline-content"
  aria-label={target ? `Preview ${target.name}` : 'File preview'}
>
  {#if target}
    {#key target.key}
      <FilePreview
        path={target.path}
        name={target.name}
        extension={target.extension}
        title="Preview"
      />
    {/key}
  {:else}
    <div class="viewer-header">
      <div class="viewer-heading"><h2>Preview</h2></div>
    </div>
    <div class="preview-empty">
      {loading ? 'Loading files…' : 'Select a file to preview.'}
    </div>
  {/if}
</section>
