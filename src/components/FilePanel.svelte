<script lang="ts">
  import type { PanelState, Root, Column } from '../filesystem/types';
  import { commander, load, rows, toggle, open, sort, hidden, type Side } from '../state/commander.svelte';
  import { bytes, date } from '../utils/format';
  let { panel, side, roots }: { panel: PanelState; side: Side; roots: Root[] } = $props();
  let draft = $state('');
  let scroller: HTMLDivElement;
  let scrollTop = $state(0);
  let height = $state(400);
  const rowHeight = 28;
  let items = $derived(rows(panel));
  let start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - 8));
  let end = $derived(Math.min(items.length, start + Math.ceil(height / rowHeight) + 16));
  let active = $derived(commander.activePanel === side);
  $effect(() => { draft = panel.path; });
  $effect(() => {
    const y = panel.cursor * rowHeight;
    if (scroller) { if (y < scroller.scrollTop) scroller.scrollTop = y; else if (y + rowHeight > scroller.scrollTop + height) scroller.scrollTop = y + rowHeight - height; }
  });
  function click(event: MouseEvent, index: number) {
    commander.activePanel = side;
    if (event.shiftKey) {
      const next = new Set(panel.selected);
      for (let i = Math.min(panel.cursor, index); i <= Math.max(panel.cursor, index); i++) if (!items[i].parentEntry) next.add(items[i].path);
      panel.selected = next;
    } else if (event.ctrlKey || event.metaKey) toggle(panel, items[index]);
    else panel.selected = new Set();
    panel.cursor = index;
    scroller.focus();
  }
  const columns: [Column, string][] = [['name', 'Nazwa'], ['extension', 'Typ'], ['size', 'Rozmiar'], ['modified', 'Zmodyfikowano']];
</script>

<section class:active class="panel" aria-label={side === 'left' ? 'Lewy panel' : 'Prawy panel'} onfocusin={() => commander.activePanel = side}>
  <div class="pathbar">
    <select aria-label="Dyski i punkty montowania" value="" onchange={(e) => { commander.activePanel = side; void load(panel, e.currentTarget.value); e.currentTarget.value = ''; }}>
      <option value="" disabled>Dyski ▾</option>
      {#each roots as root}<option value={root.path}>{root.name}</option>{/each}
    </select>
    <form onsubmit={(e) => { e.preventDefault(); commander.activePanel = side; void load(panel, draft); }}>
      <input aria-label="Ścieżka katalogu" bind:value={draft} spellcheck="false" onkeydown={(e) => { if (e.key === 'Escape') { draft = panel.path; scroller.focus(); } }} />
    </form>
    <button title="Katalog nadrzędny (Backspace)" disabled={!panel.parent || panel.loading} onclick={() => { commander.activePanel = side; void load(panel, panel.parent!, panel.path); }}>↑</button>
  </div>
  <div class="panel-tools"><span>{side === 'left' ? 'LEWY' : 'PRAWY'} PANEL</span><label><input type="checkbox" checked={panel.showHidden} onchange={() => hidden(panel)} /> Ukryte</label><button title="Odśwież (Ctrl+R)" onclick={() => void load(panel)}>↻</button></div>
  <div class="columns" role="row">
    {#each columns as [column, label]}<button role="columnheader" aria-sort={panel.sort.column === column ? (panel.sort.direction === 'asc' ? 'ascending' : 'descending') : 'none'} onclick={() => sort(panel, column)}>{label}{panel.sort.column === column ? (panel.sort.direction === 'asc' ? ' ▴' : ' ▾') : ''}</button>{/each}
  </div>
  <div class="file-list" id={`list-${side}`} bind:this={scroller} bind:clientHeight={height} onscroll={() => scrollTop = scroller.scrollTop} role="listbox" aria-label="Pliki i katalogi" aria-multiselectable="true" aria-busy={panel.loading} tabindex={active ? 0 : -1} onfocus={() => commander.activePanel = side} aria-activedescendant={items[panel.cursor] ? `${side}-${panel.cursor}` : undefined}>
    <div style:height={`${start * rowHeight}px`}></div>
    {#each items.slice(start, end) as entry, offset (entry.path)}
      {@const index = start + offset}
      <div id={`${side}-${index}`} role="option" aria-selected={panel.selected.has(entry.path)} tabindex="-1" class="file-row" class:cursor={panel.cursor === index} class:selected={panel.selected.has(entry.path)} class:hidden-entry={entry.hidden} onclick={(e) => click(e, index)} ondblclick={() => void open(panel, entry)} onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); void open(panel, entry); } }} title={entry.path}>
        <span class="filename"><span class="file-icon">{entry.parentEntry ? '↰' : entry.type === 'directory' ? '▸' : entry.type === 'symlink' ? '↗' : '·'}</span>{entry.name}</span>
        <span>{entry.type === 'directory' ? '' : entry.extension}</span>
        <span class="size">{entry.parentEntry ? '' : entry.type === 'directory' ? '<DIR>' : bytes(entry.size)}</span>
        <span>{entry.parentEntry ? '' : date(entry.modified)}</span>
      </div>
    {/each}
    <div style:height={`${Math.max(0, items.length - end) * rowHeight}px`}></div>
    {#if !items.length && !panel.loading}<div class="empty">Katalog jest pusty</div>{/if}
  </div>
  {#if panel.error}<div class="panel-error" role="alert">{panel.error}</div>{/if}
  <div class="panel-status"><span>{panel.loading ? 'Wczytywanie…' : `${items.filter(e => !e.parentEntry).length} elementów`}</span><span>{panel.selected.size} zaznaczonych · {bytes(panel.entries.filter(e => panel.selected.has(e.path)).reduce((n, e) => n + e.size, 0))}</span></div>
</section>
