<script lang="ts">
  import Icon from '@iconify/svelte';
  import { icons } from '../utils/uiIcons';
  import { tick } from 'svelte';
  import type { Root } from '../filesystem/types';
  import { preferences, scheduleSave } from '../state/preferences.svelte';
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
    onconnections,
    remote = false,
  }: {
    path: string;
    label: string;
    roots: Root[];
    id: string;
    value?: string;
    ariaLabel?: string;
    buttonTitle?: string;
    required?: boolean;
    onchoose: (path: string, typed?: boolean) => void | Promise<void>;
    onescape?: () => void;
    onconnections?: () => void;
    remote?: boolean;
  } = $props();
  let query = $state('');
  let expanded = $state(false);
  let container: HTMLDivElement;
  let input: HTMLInputElement;
  let editingPin = $state<string | null>(null);
  let pinName = $state('');
  let nameInput = $state<HTMLInputElement>();
  let renameRow: HTMLElement;
  $effect(() => {
    if (!expanded) editingPin = null;
  });
  function favoriteName(pin: string) {
    return (
      preferences.pinnedDirectoryNames[pin] ??
      pin.split(/[\\/]/).filter(Boolean).pop() ??
      pin
    );
  }
  async function startRename(pin: string, button: HTMLButtonElement) {
    renameRow = button.parentElement!;
    pinName = favoriteName(pin);
    editingPin = pin;
    await tick();
    nameInput?.focus();
    nameInput?.select();
  }
  async function finishRename(save: boolean) {
    if (save && editingPin) {
      const names = { ...preferences.pinnedDirectoryNames };
      if (pinName.trim()) names[editingPin] = pinName.trim();
      else delete names[editingPin];
      preferences.pinnedDirectoryNames = names;
      scheduleSave();
    }
    editingPin = null;
    await tick();
    if (renameRow.isConnected)
      renameRow
        .querySelector<HTMLButtonElement>('[title="Rename favorite"]')
        ?.focus();
    else input.focus();
  }
  let menuId = $derived(`locations-${id}`);
  $effect(() => {
    value = displayPath(label);
    query = '';
    expanded = false;
  });
  let filteredRoots = $derived(roots.filter((r) => matches(r.name, r.path)));
  let filteredPins = $derived(
    preferences.pinnedDirectories
      .filter((p) => matches(favoriteName(p), p))
      .sort((a, b) => a.localeCompare(b)),
  );
  let firstMatch = $derived(
    expanded && query.trim() && !(remote && query.trim().startsWith('/'))
      ? (filteredRoots[0]?.path ?? filteredPins[0])
      : undefined,
  );
  function matches(name: string, value: string) {
    return `${name} ${value}`
      .toLowerCase()
      .includes(query.trim().toLowerCase());
  }
  async function choose(path: string, typed = false) {
    if (!path.trim()) return;
    expanded = false;
    value = displayPath(path);
    query = '';
    await onchoose(path, typed);
  }
  export function submit() {
    void choose(
      firstMatch ?? (value === displayPath(label) ? path : value),
      !firstMatch,
    );
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
  onfocusout={async (event) => {
    // WebKit can omit relatedTarget when focus moves to a dropdown button and
    // updates document.activeElement only later, so fall back to it after the
    // tick. <body> means the destination is unknown (e.g. a pressed button
    // that has not taken focus yet), not that focus left the picker.
    await tick();
    const next =
      (event.relatedTarget as Node | null) ??
      (document.activeElement === document.body
        ? null
        : document.activeElement);
    if (next && !container.contains(next)) expanded = false;
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
      {#if onconnections}<button
          type="button"
          class="location-option connections-button"
          onclick={() => {
            expanded = false;
            onconnections?.();
          }}>Connections…</button
        >{/if}
      {#each filteredPins as pin, index (pin)}
        <div class="pin-row">
          {#if editingPin === pin}
            <div class="pin-editor">
              <input
                bind:this={nameInput}
                bind:value={pinName}
                aria-label="Favorite name"
                onkeydown={(event) => {
                  event.stopPropagation();
                  if (event.key === 'Enter' || event.key === 'Escape') {
                    event.preventDefault();
                    void finishRename(event.key === 'Enter');
                  }
                }}
              />
              <button
                type="button"
                aria-label="Save favorite name"
                title="Save favorite name"
                onclick={() => void finishRename(true)}
                ><Icon icon={icons.check} width="16" height="16" /></button
              >
              <button
                type="button"
                aria-label="Cancel rename"
                title="Cancel rename"
                onclick={() => void finishRename(false)}
                ><Icon icon={icons.x} width="16" height="16" /></button
              >
              <small>{pin}</small>
            </div>
          {:else}
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
                ><Icon
                  icon={icons.star}
                  width="16"
                  height="16"
                  class="pin-star"
                />
                {favoriteName(pin)}</span
              ><small>{pin}</small>
            </button>
            <button
              type="button"
              aria-label={`Rename favorite ${favoriteName(pin)}`}
              title="Rename favorite"
              onclick={(event) => void startRename(pin, event.currentTarget)}
              ><Icon icon={icons.pencil} width="16" height="16" /></button
            >
          {/if}
        </div>
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
  .pin-row {
    display: flex;
    align-items: center;
  }
  .pin-row > .location-option {
    flex: 1;
    min-width: 0;
  }
  .pin-editor {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    flex: 1;
    min-width: 0;
    padding: 8px 12px;
  }
  .pin-editor input {
    box-sizing: border-box;
  }
  .pin-editor small {
    grid-column: 1 / -1;
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    color: #9ba9bd;
    white-space: nowrap;
  }
</style>
