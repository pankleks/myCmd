<script lang="ts">
  import { onMount, tick } from 'svelte';

  export interface MenuItem {
    label: string;
    shortcut?: string;
    disabled?: boolean;
    separator?: boolean;
    run: () => void;
  }
  let {
    x,
    y,
    items,
    onclose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: (restoreFocus?: boolean) => void;
  } = $props();
  let menu: HTMLDivElement;
  let left = $state(0);
  let top = $state(0);
  onMount(() => {
    left = Math.max(4, Math.min(x, window.innerWidth - menu.offsetWidth - 4));
    top = Math.max(4, Math.min(y, window.innerHeight - menu.offsetHeight - 4));
    void tick().then(() =>
      menu?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus(),
    );
  });
  function keydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Escape' || event.key === 'Tab') {
      event.preventDefault();
      onclose();
    } else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const buttons = [
        ...menu.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'),
      ];
      const index = buttons.indexOf(
        document.activeElement as HTMLButtonElement,
      );
      const next =
        event.key === 'Home'
          ? 0
          : event.key === 'End'
            ? buttons.length - 1
            : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) %
              buttons.length;
      buttons[next]?.focus();
    }
  }
</script>

<svelte:window
  onpointerdown={(event) => {
    if (menu && !menu.contains(event.target as Node)) onclose(false);
  }}
  onresize={() => onclose()}
  onblur={() => onclose(false)}
/>
<div
  bind:this={menu}
  class="context-menu"
  role="menu"
  aria-label="File actions"
  tabindex="-1"
  style:left={`${left}px`}
  style:top={`${top}px`}
  onkeydown={keydown}
  oncontextmenu={(event) => event.preventDefault()}
>
  {#each items as item}
    {#if item.separator}<div role="separator"></div>{/if}
    <button
      type="button"
      role="menuitem"
      tabindex="-1"
      disabled={item.disabled}
      onclick={() => {
        onclose();
        item.run();
      }}
    >
      <span>{item.label}</span>{#if item.shortcut}<kbd>{item.shortcut}</kbd
        >{/if}
    </button>
  {/each}
</div>

<style>
  .context-menu {
    position: fixed;
    z-index: 100;
    min-width: 240px;
    max-width: calc(100vw - 8px);
    max-height: calc(100vh - 8px);
    overflow: auto;
    background: #232b36;
    border: 1px solid #47566e;
    box-shadow: 0 10px 24px #0008;
    padding: 4px;
  }
  button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    width: 100%;
    padding: 7px 10px;
    text-align: left;
    background: transparent;
  }
  button:focus {
    background: #344056;
    outline: 1px solid #80aaff;
    outline-offset: -1px;
  }
  kbd {
    color: #9ba9bd;
    font: inherit;
    font-size: 12px;
  }
  [role='separator'] {
    border-top: 1px solid #47566e;
    margin: 4px 0;
  }
</style>
