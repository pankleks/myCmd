<script lang="ts">
  import Icon from '@iconify/svelte';
  import { icons } from '../utils/uiIcons';
  import { tick } from 'svelte';
  import type { Root } from '../filesystem/types';
  import { preferences } from '../state/preferences.svelte';
  import { displayPath } from '../utils/paths';

  let {
    path,
    label,
    roots,
    id,
    value = $bindable(''),
    ariaLabel = 'Directory path',
    buttonTitle = 'Choose folder',
    required = false,
    onchoose,
    onescape,
  }: {
    path: string;
    label: string;
    roots: Root[];
    id: string;
    value?: string;
    ariaLabel?: string;
    buttonTitle?: string;
    required?: boolean;
    onchoose: (path: string) => void | Promise<void>;
    onescape?: () => void;
  } = $props();
  let query = $state('');
  let expanded = $state(false);
  let container: HTMLDivElement;
  let input: HTMLInputElement;
  let menuId = $derived(`locations-${id}`);
  $effect(() => {
    value = displayPath(label);
    query = '';
    expanded = false;
  });
  let filteredRoots = $derived(roots.filter((r) => matches(r.name, r.path)));
  let filteredPins = $derived(
    preferences.pinnedDirectories.filter((p) => matches(p, p)),
  );
  let firstMatch = $derived(
    expanded && query.trim()
      ? (filteredRoots[0]?.path ?? filteredPins[0])
      : undefined,
  );
  function matches(name: string, value: string) {
    return `${name} ${value}`
      .toLowerCase()
      .includes(query.trim().toLowerCase());
  }
  async function choose(path: string) {
    if (!path.trim()) return;
    expanded = false;
    value = displayPath(path);
    query = '';
    await onchoose(path);
  }
  export function submit() {
    void choose(firstMatch ?? (value === displayPath(label) ? path : value));
  }
  function escape() {
    query = '';
    expanded = false;
    onescape?.();
  }
  async function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && (expanded || onescape)) {
      event.preventDefault();
      event.stopPropagation();
      escape();
    }
    if (event.key === 'Enter' && expanded && event.target === input) {
      event.preventDefault();
      event.stopPropagation();
      submit();
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      event.stopPropagation();
      expanded = true;
      await tick();
      const buttons = Array.from(
        container.querySelectorAll<HTMLButtonElement>('.location-option'),
      );
      const index = buttons.indexOf(
        document.activeElement as HTMLButtonElement,
      );
      const next =
        index < 0
          ? event.key === 'ArrowDown'
            ? 0
            : buttons.length - 1
          : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) %
            buttons.length;
      buttons[next]?.focus();
    }
  }
</script>

<svelte:window
  onpointerdown={(event) => {
    if (expanded && !container.contains(event.target as Node)) expanded = false;
  }}
/>
<!-- svelte-ignore a11y_no_static_element_interactions (delegated shortcuts for the input, toggle, and dropdown options) -->
<div
  class="folder-picker"
  bind:this={container}
  onkeydown={keydown}
  onfocusout={async () => {
    // WebKit can omit relatedTarget when focus moves to a dropdown button.
    await tick();
    if (!container.contains(document.activeElement)) expanded = false;
  }}
>
  <input
    bind:this={input}
    bind:value
    aria-label={ariaLabel}
    {required}
    aria-expanded={expanded}
    aria-controls={menuId}
    aria-autocomplete="list"
    aria-activedescendant={firstMatch ? `${menuId}-match` : undefined}
    role="combobox"
    spellcheck="false"
    autocomplete="off"
    onfocus={() => {
      input.select();
      query = '';
      expanded = true;
    }}
    oninput={() => {
      query = value;
      expanded = true;
    }}
  />
  <button
    type="button"
    {id}
    aria-label="Choose location"
    aria-expanded={expanded}
    aria-controls={menuId}
    title={buttonTitle}
    onclick={() => {
      const open = !expanded;
      query = '';
      input.focus();
      expanded = open;
    }}><Icon icon={icons['chevron-down']} width="16" height="16" /></button
  >
  {#if expanded}
    <div id={menuId} class="location-menu" role="group" aria-label="Locations">
      <div class="location-heading">Drives</div>
      {#each filteredRoots as root, index}
        <button
          type="button"
          class="location-option"
          class:location-match={firstMatch === root.path && index === 0}
          id={firstMatch === root.path && index === 0
            ? `${menuId}-match`
            : undefined}
          onclick={() => void choose(root.path)}
        >
          <span
            ><Icon icon={icons['hard-drive']} width="16" height="16" />
            {root.name}</span
          ><small>{root.path}</small>
        </button>
      {/each}
      <div class="location-heading">Pinned directories</div>
      {#each filteredPins as pin, index}
        <button
          type="button"
          class="location-option"
          class:location-match={!filteredRoots.length &&
            index === 0 &&
            firstMatch === pin}
          id={!filteredRoots.length && index === 0 && firstMatch === pin
            ? `${menuId}-match`
            : undefined}
          onclick={() => void choose(pin)}
        >
          <span
            ><Icon icon={icons.star} width="16" height="16" class="pin-star" />
            {pin.split(/[\\/]/).filter(Boolean).pop() ?? pin}</span
          ><small>{pin}</small>
        </button>
      {/each}
      {#if !filteredRoots.length && !filteredPins.length}
        <p class="location-empty">
          No matching locations. Enter uses the typed path.
        </p>
      {/if}
      {#if !query && path}
        <div class="location-heading">Current directory</div>
        <button
          type="button"
          class="location-option"
          onclick={() => void choose(path)}
        >
          <span
            ><Icon icon={icons.check} width="16" height="16" />
            {displayPath(label)}</span
          >
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .folder-picker {
    position: relative;
    display: flex;
    min-width: 0;
    flex: 1;
  }
  input {
    flex: 1;
    width: 100%;
  }
  button {
    flex: none;
  }
  .location-menu {
    top: calc(100% + 7px);
    left: 0;
    right: 0;
  }
</style>
