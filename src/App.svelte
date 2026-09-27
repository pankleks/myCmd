<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { isTauri } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import FilePanel from './components/FilePanel.svelte';
  import OperationDialog, { type Action } from './components/OperationDialog.svelte';
  import ConflictDialog from './components/ConflictDialog.svelte';
  import { commander, load, rows, sources, toggle, hidden, open, type Side } from './state/commander.svelte';
  import { api, errorMessage } from './filesystem/api';
  import type { FileEntry } from './filesystem/types';
  import type { Conflict, FileOperation, Progress, Resolution } from './operations/types';
  import { bytes } from './utils/format';
  let ready = $state(false);
  let error = $state('');
  let busy = $state(false);
  let progress = $state<Progress>();
  let conflict = $state<Conflict>();
  let dialog = $state<{ action: Action; entries: FileEntry[]; parent: string; destination: string; side: Side }>();
  let operationSide: Side = 'left';
  let operationId: string | undefined;
  let active = $derived(commander[commander.activePanel]);
  let percent = $derived(progress ? Math.min(100, Math.round(progress.totalBytes ? progress.processedBytes / progress.totalBytes * 100 : progress.totalItems ? progress.processedItems / progress.totalItems * 100 : 0)) : 0);
  const actions: [string, Action, string][] = [['F2', 'rename', 'Zmień nazwę'], ['F5', 'copy', 'Kopiuj'], ['F6', 'move', 'Przenieś'], ['F7', 'createDirectory', 'Nowy katalog'], ['F8', 'delete', 'Usuń']];
  function focusPanel() { document.getElementById(`list-${commander.activePanel}`)?.focus(); }
  function closeDialog() { dialog = undefined; void tick().then(focusPanel); }
  function request(action: Action) {
    if (!ready || busy || dialog || conflict || active.loading || !active.path) return;
    const entries = sources(active);
    if (action !== 'createDirectory' && !entries.length) return;
    if (action === 'rename' && entries.length !== 1) { error = 'Zmiana nazwy wymaga wybrania jednego elementu.'; return; }
    error = '';
    dialog = { action, entries, parent: active.path, destination: commander[commander.activePanel === 'left' ? 'right' : 'left'].path, side: commander.activePanel };
  }
  async function submit(operation: FileOperation) {
    operationSide = dialog!.side; busy = true; progress = undefined; operationId = undefined; closeDialog();
    try { const id = await api.start(operation); if (busy) operationId = id; } catch (e) { error = errorMessage(e); busy = false; }
  }
  async function resolve(resolution: Resolution) {
    if (!conflict) return;
    try { await api.resolve(conflict.operationId, resolution); conflict = undefined; await tick(); focusPanel(); } catch (e) { error = errorMessage(e); }
  }
  async function cancel() { const id = operationId ?? progress?.operationId; if (id) try { await api.cancel(id); } catch (e) { error = errorMessage(e); } }
  onMount(() => {
    let disposed = false;
    const unlisten: UnlistenFn[] = [];
    async function init() {
      if (!isTauri()) { error = 'Dostęp do plików wymaga aplikacji desktopowej. Uruchom: npm run tauri dev'; return; }
      try {
        const p = await listen<Progress>('operation-progress', ({ payload }) => {
          progress = payload; operationId = payload.operationId;
          if (['completed', 'failed', 'cancelled'].includes(payload.state)) {
            busy = false; conflict = undefined;
            if (payload.error && payload.state === 'failed') error = errorMessage(payload.error);
            void Promise.all([load(commander.left, undefined, operationSide === 'left' ? payload.resultPath : undefined), load(commander.right, undefined, operationSide === 'right' ? payload.resultPath : undefined)]).then(() => { commander[operationSide].selected = new Set(); });
          }
        });
        unlisten.push(p);
        const c = await listen<Conflict>('operation-conflict', ({ payload }) => conflict = payload); unlisten.push(c);
        if (disposed) { unlisten.forEach(fn => fn()); return; }
        commander.roots = await api.roots();
        const home = commander.roots.find(r => r.type === 'home')?.path ?? '~';
        await Promise.all([load(commander.left, home), load(commander.right, home)]);
        if (!disposed) { ready = true; focusPanel(); }
      } catch (e) { error = errorMessage(e); }
    }
    void init();
    return () => { disposed = true; unlisten.forEach(fn => fn()); };
  });
  function keydown(event: KeyboardEvent) {
    if (dialog || conflict || event.defaultPrevented || event.isComposing) return;
    const target = event.target as HTMLElement;
    if (target.closest('input, select, textarea, dialog')) return;
    const ctrl = event.ctrlKey || event.metaKey;
    if (event.key === 'Tab' && !ctrl && !event.altKey) { event.preventDefault(); commander.activePanel = commander.activePanel === 'left' ? 'right' : 'left'; focusPanel(); return; }
    const action = actions.find(([key]) => key === event.key)?.[1] ?? (event.key === 'Delete' ? 'delete' : undefined);
    if (action) { event.preventDefault(); request(action); return; }
    if (ctrl) {
      switch (event.key.toLowerCase()) {
        case 'a': event.preventDefault(); active.selected = new Set(rows(active).filter(e => !e.parentEntry).map(e => e.path)); break;
        case 'h': event.preventDefault(); hidden(active); break;
        case 'r': event.preventDefault(); void load(active); break;
        case 'l': event.preventDefault(); (document.querySelector('.panel.active .pathbar input') as HTMLInputElement)?.select(); break;
      }
      return;
    }
    if (target.closest('button') && ['Enter', ' '].includes(event.key)) return;
    const list = rows(active);
    const page = Math.max(1, Math.floor((document.getElementById(`list-${commander.activePanel}`)?.clientHeight ?? 280) / 28));
    const movement: Record<string, number> = { ArrowUp: -1, ArrowDown: 1, PageUp: -page, PageDown: page, Home: -list.length, End: list.length };
    if (event.key in movement) { event.preventDefault(); active.cursor = Math.max(0, Math.min(list.length - 1, active.cursor + movement[event.key])); focusPanel(); }
    else if (event.key === 'Enter') { event.preventDefault(); void open(active, list[active.cursor]); }
    else if (event.key === 'Backspace') { event.preventDefault(); if (active.parent) void load(active, active.parent, active.path); }
    else if (event.key === ' ' || event.key === 'Insert') { event.preventDefault(); toggle(active, list[active.cursor]); if (event.key === 'Insert') active.cursor = Math.min(Math.max(0, list.length - 1), active.cursor + 1); }
    else if (event.key === 'Escape') { active.selected = new Set(); error = ''; }
  }
</script>

<svelte:window onkeydown={keydown} />
<main>
  <header><strong>my<span>Cmd</span></strong><span class="subtitle">Menedżer plików</span><span class="keyboard-hint">Tab · zmień panel &nbsp; Ctrl+L · ścieżka &nbsp; Ctrl+R · odśwież</span></header>
  <div class="panels"><FilePanel panel={commander.left} side="left" roots={commander.roots} /><FilePanel panel={commander.right} side="right" roots={commander.roots} /></div>
  {#if error}<div role="alert" class="app-error"><span>{error}</span><button onclick={() => error = ''} aria-label="Zamknij komunikat">×</button></div>{/if}
  <div class="operation-status" role="status" aria-live="polite">
    {#if progress}<span>{({ queued: 'Oczekuje', running: 'Operacja w toku', completed: 'Zakończono', failed: 'Błąd operacji', cancelled: 'Anulowano' })[progress.state]} · {progress.processedItems}/{progress.totalItems} · {bytes(progress.processedBytes)}</span>{#if busy}<progress max="100" value={percent}></progress><span>{percent}%</span><button onclick={cancel}>Anuluj</button>{/if}<span class="current-item" title={progress.currentItem}>{progress.currentItem ?? ''}</span>
    {:else}<span>{busy ? 'Przygotowywanie operacji…' : ready ? 'Gotowy' : 'Oczekiwanie na backend Tauri'}</span>{/if}
  </div>
  <footer>{#each actions as [key, action, label]}<button disabled={!ready || busy} onclick={() => request(action)}><kbd>{key}</kbd>{label}</button>{/each}</footer>
</main>
{#if dialog}<OperationDialog {...dialog} onsubmit={submit} onclose={closeDialog} />{/if}
{#if conflict}<ConflictDialog {conflict} onresolve={resolve} />{/if}
