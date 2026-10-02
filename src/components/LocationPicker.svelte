<script lang="ts">
  import { tick } from 'svelte';
  import type { Root } from '../filesystem/types';
  import { preferences, scheduleSave } from '../state/preferences.svelte';
  import { displayPath } from '../utils/paths';
  import { isLocalPath } from '../filesystem/providers';

  let {
    path,
    label,
    roots,
    side,
    onnavigate,
    onescape,
  }: {
    path: string;
    label: string;
    roots: Root[];
    side: string;
    onnavigate: (path: string) => Promise<void>;
    onescape: () => void;
  } = $props();
  let draft = $state('');
  let query = $state('');
  let expanded = $state(false);
  let container: HTMLDivElement;
  let input: HTMLInputElement;
  let menuId = $derived(`locations-${side}`);
  $effect(() => {
    draft = displayPath(label);
    query = '';
    expanded = false;
  });
  let filteredRoots = $derived(roots.filter((r) => matches(r.name, r.path)));
  let filteredPins = $derived(
    preferences.pinnedDirectories.filter((p) => matches(p, p)),
  );
  let pinned = $derived(preferences.pinnedDirectories.includes(path));
  function matches(name: string, value: string) {
    return `${name} ${value}`
      .toLowerCase()
      .includes(query.trim().toLowerCase());
  }
  async function navigate(value: string) {
    if (!value.trim()) return;
    expanded = false;
    await onnavigate(value);
    draft = displayPath(label);
    query = '';
  }
  function togglePin() {
    preferences.pinnedDirectories = pinned
      ? preferences.pinnedDirectories.filter((p) => p !== path)
      : [...preferences.pinnedDirectories, path];
    scheduleSave();
  }
  function escape() {
    draft = displayPath(label);
    query = '';
    expanded = false;
    onescape();
  }
  async function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      escape();
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      expanded = true;
      await tick();
      container.querySelector<HTMLButtonElement>('.location-option')?.focus();
    }
  }
</script>

<svelte:window
  onpointerdown={(event) => {
    if (expanded && !container.contains(event.target as Node)) expanded = false;
  }}
/>
<div
  class="pathbar location-bar"
  bind:this={container}
  onfocusout={(event) => {
    if (!container.contains(event.relatedTarget as Node)) expanded = false;
  }}
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      void navigate(draft === displayPath(label) ? path : draft);
    }}
  >
    <input
      bind:this={input}
      bind:value={draft}
      aria-label="Directory path"
      aria-expanded={expanded}
      aria-controls={menuId}
      aria-autocomplete="list"
      role="combobox"
      spellcheck="false"
      autocomplete="off"
      oninput={() => {
        query = draft;
        expanded = true;
      }}
      onkeydown={keydown}
    />
    <button
      type="button"
      id={`drive-${side}`}
      aria-label="Choose location"
      aria-expanded={expanded}
      aria-controls={menuId}
      title={side === 'left' ? 'Locations (Alt+F1)' : 'Locations (Alt+F2)'}
      onclick={() => {
        expanded = !expanded;
        query = '';
        input.focus();
      }}>▾</button
    >
  </form>
  <button
    type="button"
    class="location-pin"
    class:pinned
    aria-pressed={pinned}
    disabled={!path || !isLocalPath(path)}
    title={pinned ? 'Unpin current directory' : 'Pin current directory'}
    aria-label={pinned ? 'Unpin current directory' : 'Pin current directory'}
    onclick={togglePin}>{pinned ? '★' : '☆'}</button
  >
  {#if expanded}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions (delegated keyboard navigation for child buttons) -->
    <div
      id={menuId}
      class="location-menu"
      role="group"
      aria-label="Locations"
      onkeydown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          event.stopPropagation();
          escape();
        }
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          event.preventDefault();
          const buttons = Array.from(
            container.querySelectorAll<HTMLButtonElement>('.location-option'),
          );
          const i = buttons.indexOf(
            document.activeElement as HTMLButtonElement,
          );
          buttons[
            (i + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) %
              buttons.length
          ]?.focus();
        }
      }}
    >
      <div class="location-heading">Drives</div>
      {#each filteredRoots as root}
        <button
          type="button"
          class="location-option"
          onclick={() => void navigate(root.path)}
        >
          <span>▣ {root.name}</span><small>{root.path}</small>
        </button>
      {/each}
      <div class="location-heading">Pinned directories</div>
      {#each filteredPins as pin}
        <button
          type="button"
          class="location-option"
          onclick={() => void navigate(pin)}
        >
          <span
            ><span class="pin-star" aria-hidden="true">★</span>
            {pin.split(/[\\/]/).filter(Boolean).pop() ?? pin}</span
          ><small>{pin}</small>
        </button>
      {/each}
      {#if !filteredRoots.length && !filteredPins.length}
        <p class="location-empty">
          No matching locations. Enter navigates to the typed path.
        </p>
      {/if}
      {#if !query && path}
        <div class="location-heading">Current directory</div>
        <button
          type="button"
          class="location-option"
          onclick={() => void navigate(path)}
        >
          <span>✓ {displayPath(label)}</span>
        </button>
      {/if}
    </div>
  {/if}
</div>
