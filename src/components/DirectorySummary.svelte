<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { api, errorMessage } from '../filesystem/api';
  import { summarizeDirectory } from '../utils/directorySummary';
  import { bytes } from '../utils/format';

  let { path, name }: { path: string; name: string } = $props();

  let loading = $state(true);
  let error = $state('');
  let files = $state(0);
  let directories = $state(0);
  let size = $state(0);
  let incomplete = $state(false);
  let disposed = false;

  onMount(() => {
    void load();
  });

  onDestroy(() => {
    disposed = true;
  });

  async function load() {
    try {
      const summary = await summarizeDirectory(
        path,
        (dir) => api.list(dir),
        () => disposed,
      );
      if (disposed) return;
      files = summary.files;
      directories = summary.directories;
      size = summary.size;
      incomplete = summary.incomplete;
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      if (!disposed) loading = false;
    }
  }
</script>

<div class="file-preview-content">
  <div class="viewer-header">
    <div class="viewer-heading">
      <h2>Summary</h2>
      <span title={path}>{name}</span>
    </div>
  </div>
  <div class="viewer-editor-shell">
    {#if error}
      <div class="viewer-message" role="alert"><p>{error}</p></div>
    {:else if loading}
      <div class="viewer-loading" role="status">Counting files…</div>
    {:else}
      <dl class="directory-summary">
        <div><dt>Files</dt><dd>{files.toLocaleString()}</dd></div>
        <div><dt>Subdirectories</dt><dd>{directories.toLocaleString()}</dd></div>
        <div><dt>Total size</dt><dd>{bytes(size)}</dd></div>
      </dl>
      {#if incomplete}
        <p class="directory-summary-note">
          Partial result — some folders could not be read.
        </p>
      {/if}
    {/if}
  </div>
</div>
