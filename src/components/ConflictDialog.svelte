<script lang="ts">
  import { onMount } from 'svelte';
  import type { Conflict, Resolution } from '../operations/types';
  import { bytes, date } from '../utils/format';
  let { conflict, onresolve }: { conflict: Conflict; onresolve: (resolution: Resolution) => Promise<void> } = $props();
  let dialog: HTMLDialogElement;
  let applyToAll = $state(false);
  let name = $state('');
  let submitting = $state(false);
  onMount(() => { dialog.showModal(); });
  async function resolve(action: Resolution['action']) { submitting = true; try { await onresolve({ action, applyToAll, name: action === 'rename' && name ? name : undefined }); } finally { submitting = false; } }
</script>
<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); void resolve('cancel'); }}>
  <h2>Element już istnieje</h2>
  <p class="conflict-path">{conflict.destination.path}</p>
  <dl><dt>Źródło</dt><dd>{bytes(conflict.source.size)} · {date(conflict.source.modified)} · {conflict.source.type}</dd><dt>Cel</dt><dd>{bytes(conflict.destination.size)} · {date(conflict.destination.modified)} · {conflict.destination.type}</dd></dl>
  <label>Nowa nazwa (pusta = automatyczna)<input bind:value={name} spellcheck="false" /></label>
  <label class="check-label"><input type="checkbox" bind:checked={applyToAll} /> Zastosuj do wszystkich (kolejne nazwy automatyczne)</label>
  <div class="dialog-actions"><button disabled={submitting} onclick={() => resolve('skip')}>Pomiń</button><button disabled={submitting} onclick={() => resolve('rename')}>Zmień nazwę</button><button disabled={submitting} onclick={() => resolve('overwrite')}>Nadpisz</button><button disabled={submitting} onclick={() => resolve('cancel')}>Anuluj operację</button></div>
</dialog>
