<script lang="ts">
  import { onDestroy, onMount, type Snippet } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { api, errorMessage } from '../filesystem/api';
  import {
    summarizeDirectory,
    summarizeLocalDirectory,
  } from '../utils/directorySummary';
  import {
    isLocalPath,
    parseLocation,
    providerFor,
  } from '../filesystem/providers';
  import { bytes } from '../utils/format';

  let {
    path,
    name,
    headerActions,
  }: { path: string; name: string; headerActions?: Snippet } = $props();

  let loading = $state(true);
  let error = $state('');
  let files = $state(0);
  let directories = $state(0);
  let size = $state(0);
  let incomplete = $state(false);
  let disposed = false;
  const requestIds = [crypto.randomUUID(), crypto.randomUUID()];
  let sizing = false;

  onMount(() => {
    void load();
  });

  onDestroy(() => {
    disposed = true;
    if (sizing) void api.cancelDirectorySizing(requestIds).catch(() => {});
  });

  async function load() {
    try {
      sizing = isLocalPath(path);
      const local = sizing
        ? await summarizeLocalDirectory(
            () =>
              invoke('count_delete_entries', {
                paths: [path],
                requestId: requestIds[0],
              }),
            () => api.measureDirectory(path, requestIds[1]),
          ).catch(() => undefined)
        : undefined;
      sizing = false;
      const summary =
        local ??
        (await summarizeDirectory(
          path,
          (dir) => {
            const location = parseLocation(dir);
            return location.kind === 'sftp'
              ? providerFor(location).list(location)
              : api.list(dir);
          },
          () => disposed,
        ));
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
    <div class="viewer-toolbar">{@render headerActions?.()}</div>
  </div>
  <div class="viewer-editor-shell">
    {#if error}
      <div class="viewer-message" role="alert"><p>{error}</p></div>
    {:else if loading}
      <div class="viewer-loading" role="status">Counting files…</div>
    {:else}
      <dl class="directory-summary">
        <div>
          <dt>Files</dt>
          <dd>{files.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Subfolders</dt>
          <dd>{directories.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Total size</dt>
          <dd>{bytes(size)}</dd>
        </div>
      </dl>
      {#if incomplete}
        <p class="directory-summary-note">
          Partial result — some folders could not be read.
        </p>
      {/if}
    {/if}
  </div>
</div>
