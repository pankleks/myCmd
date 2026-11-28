<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { version } from '../../package.json';
  import { api, errorMessage, type AvailableUpdate } from '../filesystem/api';
  import UpdateDialog from './UpdateDialog.svelte';

  let {
    // Development builds never ask GitHub on their own; Check now still works.
    autoCheck = !import.meta.env.DEV,
  }: { autoCheck?: boolean } = $props();

  let current = $state(version);
  let update = $state<AvailableUpdate | null>(null);
  let checking = $state(false);
  let checked = $state(false);
  let error = $state('');
  let dialogOpen = $state(false);
  let disposed = false;

  let status = $derived(
    checking
      ? 'Checking GitHub for a newer version…'
      : error
        ? `The check failed: ${error}`
        : update
          ? `myCmd ${update.version} is available.`
          : checked
            ? 'You have the newest version.'
            : '',
  );

  async function check(force: boolean) {
    if (checking) return;
    checking = true;
    error = '';
    try {
      const result = await api.checkForUpdate(force);
      if (disposed) return;
      current = result.current;
      update = result.update;
      checked = true;
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      if (!disposed) checking = false;
    }
  }

  // The backend keeps the answer for an hour, so reopening Settings is cheap.
  onMount(() => {
    if (autoCheck) void check(false);
  });
  onDestroy(() => (disposed = true));
</script>

<div class="update-row">
  <div class="update-text">
    <strong>myCmd {current}</strong>
    <small role="status">{status}</small>
  </div>
  <div class="update-actions">
    {#if update}<button
        type="button"
        class="primary"
        onclick={() => (dialogOpen = true)}>View…</button
      >{/if}
    <button type="button" disabled={checking} onclick={() => check(true)}
      >Check now</button
    >
  </div>
</div>
<p class="hint">
  Asks GitHub for a newer release. Nothing is downloaded or installed
  automatically.
</p>
{#if dialogOpen && update}
  <UpdateDialog {update} {current} onclose={() => (dialogOpen = false)} />
{/if}

<style>
  .update-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .update-text {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 4px;
  }
  .update-text strong {
    font-size: 14px;
    font-weight: 600;
  }
  .update-text small {
    color: var(--text-muted);
    font-size: 12px;
  }
  .update-text small:empty {
    display: none;
  }
  .update-actions {
    display: flex;
    flex: none;
    gap: 10px;
  }
</style>
