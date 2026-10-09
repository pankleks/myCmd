<script lang="ts">
  import Icon from '@iconify/svelte';
  import { icons } from '../utils/uiIcons';
  import FolderPicker from './FolderPicker.svelte';
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
  let value = $state('');
  let picker: { submit: () => void };
  let pinned = $derived(preferences.pinnedDirectories.includes(path));

  function togglePin() {
    preferences.pinnedDirectories = pinned
      ? preferences.pinnedDirectories.filter((pin) => pin !== path)
      : [...preferences.pinnedDirectories, path];
    scheduleSave();
  }
</script>

<div class="pathbar location-bar">
  <form
    onsubmit={(event) => {
      event.preventDefault();
      picker.submit();
    }}
  >
    <FolderPicker
      bind:this={picker}
      bind:value
      {path}
      {label}
      {roots}
      id={`drive-${side}`}
      buttonTitle={side === 'left'
        ? 'Locations (Alt+F1)'
        : 'Locations (Alt+F2)'}
      onchoose={async (path) => {
        await onnavigate(path);
        value = displayPath(label);
      }}
      onescape={() => {
        value = displayPath(label);
        onescape();
      }}
    />
  </form>
  <button
    type="button"
    class="location-pin"
    class:pinned
    aria-pressed={pinned}
    disabled={!path || !isLocalPath(path)}
    title={pinned ? 'Unpin current directory' : 'Pin current directory'}
    aria-label={pinned ? 'Unpin current directory' : 'Pin current directory'}
    onclick={togglePin}
    ><Icon icon={icons.star} width="18" height="18" /></button
  >
</div>
