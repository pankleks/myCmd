<script lang="ts">
  import { tick } from 'svelte';
  import Icon from '@iconify/svelte';
  import fileIconData from '../file-icons.generated.json';
  import type { PanelState, Root, Column } from '../filesystem/types';
  import {
    commander,
    load,
    rows,
    toggle,
    open,
    sort,
    dirSizing,
    type Side,
  } from '../state/commander.svelte';
  import { bytes, date } from '../utils/format';
  import { displayPath } from '../utils/paths';
  import { resizeColumn } from '../utils/resizeColumns';
  import { preferences } from '../state/preferences.svelte';
  let {
    panel,
    side,
    roots,
    hidden = false,
  }: {
    panel: PanelState;
    side: Side;
    roots: Root[];
    hidden?: boolean;
  } = $props();
  let draft = $state('');
  let scroller: HTMLDivElement;
  let header: HTMLDivElement;
  let scrollTop = $state(0);
  let scrollbarWidth = $state(0);
  let height = $state(400);
  let resizing = $state<{
    index: number;
    pointerId: number;
    startX: number;
    startWidths: number[];
  } | null>(null);
  const minimumColumnWidths = [80, 46, 72, 170];
  let columnTemplate = $derived(
    preferences.columnWidths
      ?.map((width, i) => `minmax(${minimumColumnWidths[i]}px, ${width}fr)`)
      .join(' '),
  );
  const rowHeight = 28;
  const fileIcons = fileIconData.icons as Record<
    string,
    { body: string; width: number; height: number }
  >;
  const extensionIcons = fileIconData.extensions as Record<string, string>;
  const filenameIcons = fileIconData.filenames as Record<string, string>;
  let items = $derived(rows(panel));
  let start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - 8));
  let end = $derived(
    Math.min(items.length, start + Math.ceil(height / rowHeight) + 16),
  );
  let active = $derived(commander.activePanel === side);
  function iconFor(name: string, extension: string) {
    const key =
      filenameIcons[name.toLowerCase()] ??
      extensionIcons[extension.toLowerCase()] ??
      'default-file';
    return fileIcons[key] ?? fileIcons['default-file'];
  }
  $effect(() => {
    draft = displayPath(panel.path);
  });
  $effect(() => {
    items.length;
    height;
    preferences.columnWidths;
    void tick().then(() => {
      if (scroller)
        scrollbarWidth = scroller.offsetWidth - scroller.clientWidth;
    });
  });
  $effect(() => {
    const y = panel.cursor * rowHeight;
    if (scroller) {
      if (y < scroller.scrollTop) scroller.scrollTop = y;
      else if (y + rowHeight > scroller.scrollTop + height)
        scroller.scrollTop = y + rowHeight - height;
    }
  });
  function click(event: MouseEvent, index: number) {
    commander.activePanel = side;
    if (event.shiftKey) {
      const next = new Set(panel.selected);
      for (
        let i = Math.min(panel.cursor, index);
        i <= Math.max(panel.cursor, index);
        i++
      )
        if (!items[i].parentEntry) next.add(items[i].path);
      panel.selected = next;
    } else if (event.ctrlKey || event.metaKey) toggle(panel, items[index]);
    else panel.selected = new Set();
    panel.cursor = index;
    scroller.focus();
  }
  const columns: { column: Column; label: string }[] = [
    { column: 'name', label: 'Name' },
    { column: 'extension', label: 'Ext' },
    { column: 'size', label: 'Size' },
    { column: 'modified', label: 'Date' },
  ];

  function measureColumns() {
    const header = document.querySelector(`#panel-${side} .columns`);
    return header
      ? Array.from(
          header.children,
          (cell) => cell.getBoundingClientRect().width,
        )
      : [];
  }

  function startResize(event: PointerEvent, index: number) {
    if (event.button !== 0) return;
    const widths = measureColumns();
    if (widths.length !== columns.length) return;
    event.preventDefault();
    event.stopPropagation();
    preferences.columnWidths = widths;
    resizing = {
      index,
      pointerId: event.pointerId,
      startX: event.clientX,
      startWidths: widths,
    };
    event.currentTarget instanceof HTMLElement &&
      event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveResize(event: PointerEvent, index: number) {
    if (
      !resizing ||
      resizing.index !== index ||
      resizing.pointerId !== event.pointerId
    )
      return;
    event.preventDefault();
    preferences.columnWidths = resizeColumn(
      resizing.startWidths,
      minimumColumnWidths,
      index,
      event.clientX - resizing.startX,
    );
  }

  function keyboardResize(event: KeyboardEvent, index: number) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    const widths = preferences.columnWidths ?? measureColumns();
    if (widths.length !== columns.length) return;
    event.preventDefault();
    event.stopPropagation();
    preferences.columnWidths = resizeColumn(
      widths,
      minimumColumnWidths,
      index,
      event.key === 'ArrowRight' ? 16 : -16,
    );
  }

  function syncHeaderScroll() {
    scrollTop = scroller.scrollTop;
    if (header) header.scrollLeft = scroller.scrollLeft;
  }
</script>

<section
  id={`panel-${side}`}
  class:active
  class:resizing={resizing !== null}
  class:custom-columns={preferences.columnWidths !== null}
  class:panel-hidden={hidden}
  class="panel"
  style:--panel-column-template={columnTemplate}
  aria-label={side === 'left' ? 'Left panel' : 'Right panel'}
  onfocusin={() => (commander.activePanel = side)}
>
  <div class="pathbar">
    <select
      id={`drive-${side}`}
      aria-label="Drives and mount points"
      title={side === 'left' ? 'Drives (Alt+F1)' : 'Drives (Alt+F2)'}
      value=""
      onchange={async (e) => {
        const path = e.currentTarget.value;
        commander.activePanel = side;
        e.currentTarget.value = '';
        if (!path) return;
        await load(panel, path);
        scroller.focus();
      }}
    >
      <option value="" disabled>Drives ▾</option>
      {#each roots as root}<option value={root.path}>{root.name}</option>{/each}
    </select>
    <form
      onsubmit={async (e) => {
        e.preventDefault();
        commander.activePanel = side;
        await load(panel, draft);
        if (!panel.error) scroller.focus();
      }}
    >
      <input
        aria-label="Directory path"
        bind:value={draft}
        spellcheck="false"
        onkeydown={(e) => {
          if (e.key === 'Escape') {
            draft = displayPath(panel.path);
            scroller.focus();
          }
        }}
      />
    </form>
    <button
      title="Parent directory (Backspace)"
      disabled={!panel.parent || panel.loading}
      onclick={() => {
        commander.activePanel = side;
        void load(panel, panel.parent!, panel.path);
      }}>↑</button
    >
  </div>
  <div
    class="columns"
    role="row"
    bind:this={header}
    style:padding-right={`${scrollbarWidth}px`}
  >
    {#each columns as item, index}
      <div class="column-heading">
        <button
          role="columnheader"
          aria-sort={panel.sort.column === item.column
            ? panel.sort.direction === 'asc'
              ? 'ascending'
              : 'descending'
            : 'none'}
          onclick={() => sort(panel, item.column)}
          ondblclick={() => (preferences.columnWidths = null)}
          title="Double-click to reset column widths"
          >{item.label}{panel.sort.column === item.column
            ? panel.sort.direction === 'asc'
              ? ' ▴'
              : ' ▾'
            : ''}</button
        >
        {#if index < columns.length - 1}
          <button
            type="button"
            class="column-resizer"
            aria-label={`Resize ${item.label} column`}
            aria-keyshortcuts="ArrowLeft ArrowRight"
            title={`Resize ${item.label} column with the mouse or arrow keys. Double-click a column header to reset widths.`}
            tabindex="0"
            onpointerdown={(event) => startResize(event, index)}
            onpointermove={(event) => moveResize(event, index)}
            onpointerup={() => (resizing = null)}
            onpointercancel={() => (resizing = null)}
            onkeydown={(event) => keyboardResize(event, index)}
            ondblclick={() => (preferences.columnWidths = null)}
          ></button>
        {/if}
      </div>
    {/each}
  </div>
  <div
    class="file-list"
    id={`list-${side}`}
    bind:this={scroller}
    bind:clientHeight={height}
    onscroll={syncHeaderScroll}
    role="listbox"
    aria-label="Files and folders"
    aria-multiselectable="true"
    aria-busy={panel.loading}
    tabindex={active ? 0 : -1}
    onfocus={() => (commander.activePanel = side)}
    aria-activedescendant={items[panel.cursor]
      ? `${side}-${panel.cursor}`
      : undefined}
  >
    <div style:height={`${start * rowHeight}px`}></div>
    {#each items.slice(start, end) as entry, offset (entry.path)}
      {@const index = start + offset}
      <div
        id={`${side}-${index}`}
        role="option"
        aria-selected={panel.selected.has(entry.path)}
        tabindex="-1"
        class="file-row"
        class:cursor={panel.cursor === index}
        class:selected={panel.selected.has(entry.path)}
        class:directory={entry.type === 'directory'}
        class:hidden-entry={entry.hidden}
        onclick={(e) => click(e, index)}
        onmousedown={(e) => {
          // Keep focus (and panel activation) deferred until click, so the
          // stale cursor doesn't flash before jumping to the clicked row.
          if (e.button === 0) e.preventDefault();
        }}
        ondblclick={() => void open(panel, entry)}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.stopPropagation();
            void open(panel, entry);
          }
        }}
        title={displayPath(entry.path)}
      >
        <span class="filename"
          ><span class="file-icon" aria-hidden="true"
            >{#if entry.parentEntry}
              <Icon
                icon={fileIcons['default-folder-opened']}
                width="16"
                height="16"
              />
            {:else if entry.type === 'directory'}
              <Icon icon={fileIcons['default-folder']} width="16" height="16" />
            {:else if entry.type === 'symlink'}
              ↗
            {:else}
              <Icon
                icon={iconFor(entry.name, entry.extension)}
                width="16"
                height="16"
              />
            {/if}</span
          >{entry.type === 'directory' ? `[${entry.name}]` : entry.name}</span
        >
        <span>{entry.type === 'directory' ? '' : entry.extension}</span>
        <span class="size"
          >{entry.parentEntry
            ? ''
            : entry.type === 'directory'
              ? dirSizing.paths.includes(entry.path)
                ? '?'
                : entry.size > 0
                  ? bytes(entry.size)
                  : '<DIR>'
              : bytes(entry.size)}</span
        >
        <span>{entry.parentEntry ? '' : date(entry.modified)}</span>
      </div>
    {/each}
    <div
      style:height={`${Math.max(0, items.length - end) * rowHeight}px`}
    ></div>
    {#if !items.length && !panel.loading}<div class="empty">
        This folder is empty
      </div>{/if}
  </div>
  {#if panel.error}<div class="panel-error" role="alert">
      {panel.error}
    </div>{/if}
  {#if commander.quickFind?.side === side}
    <div class="quick-find" role="status">
      <span>Search:</span><span class="quick-find-query"
        >{commander.quickFind.query}</span
      >{#if !commander.quickFind.matched}<span class="quick-find-miss"
          >no match</span
        >{/if}
    </div>
  {/if}
  <div class="panel-status">
    <span
      >{panel.loading
        ? 'Loading…'
        : `${items.filter((e) => !e.parentEntry).length} items`}</span
    ><span
      >{panel.selected.size} selected · {bytes(
        panel.entries
          .filter((e) => panel.selected.has(e.path))
          .reduce((n, e) => n + e.size, 0),
      )}</span
    >
  </div>
</section>
