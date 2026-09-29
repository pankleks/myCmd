<script lang="ts">
  import { onMount } from 'svelte';
  import type { Conflict, Resolution } from '../operations/types';
  import { bytes, date } from '../utils/format';
  let {
    conflict,
    onresolve,
  }: {
    conflict: Conflict;
    onresolve: (resolution: Resolution) => Promise<void>;
  } = $props();
  let dialog: HTMLDialogElement;
  let applyToAll = $state(false);
  let name = $state('');
  let submitting = $state(false);
  let error = $state('');
  onMount(() => {
    dialog.showModal();
  });
  async function resolve(action: Resolution['action']) {
    submitting = true;
    error = '';
    try {
      await onresolve({
        action,
        applyToAll,
        name: action === 'rename' && name ? name : undefined,
      });
    } catch (e) {
      error = String(e);
    } finally {
      submitting = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  oncancel={(e) => {
    e.preventDefault();
    void resolve('cancel');
  }}
>
  <h2>Item already exists</h2>
  <p class="conflict-path">{conflict.destination.path}</p>
  <dl>
    <dt>Source</dt>
    <dd>
      {bytes(conflict.source.size)} · {date(conflict.source.modified)} · {conflict
        .source.type}
    </dd>
    <dt>Destination</dt>
    <dd>
      {bytes(conflict.destination.size)} · {date(conflict.destination.modified)} ·
      {conflict.destination.type}
    </dd>
  </dl>
  <label
    >New name (leave blank for automatic)<input
      bind:value={name}
      spellcheck="false"
    /></label
  >
  <label class="check-label"
    ><input type="checkbox" bind:checked={applyToAll} /> Apply to all (automatic names
    for subsequent conflicts)</label
  >
  {#if error}<p class="danger-text" role="alert">{error}</p>{/if}
  <div class="dialog-actions">
    <button
      data-conflict-action="skip"
      disabled={submitting}
      onclick={() => resolve('skip')}>Skip</button
    ><button disabled={submitting} onclick={() => resolve('rename')}
      >Rename</button
    ><button disabled={submitting} onclick={() => resolve('overwrite')}
      >Overwrite</button
    ><button disabled={submitting} onclick={() => resolve('cancel')}
      >Cancel operation</button
    >
  </div>
</dialog>
