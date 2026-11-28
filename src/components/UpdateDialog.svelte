<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage, type AvailableUpdate } from '../filesystem/api';
  import { noteBlocks } from '../utils/releaseNotes';

  let {
    update,
    current,
    onclose,
  }: { update: AvailableUpdate; current: string; onclose: () => void } =
    $props();
  let dialog: HTMLDialogElement;
  let message = $state('');
  let blocks = $derived(noteBlocks(update.notes));
  let published = $derived.by(() => {
    const date = new Date(update.publishedAt);
    return Number.isNaN(date.getTime())
      ? ''
      : date.toLocaleDateString(undefined, {
          year: 'numeric',
          month: 'long',
          day: 'numeric',
        });
  });

  onMount(() => dialog.showModal());

  async function copy(command: string) {
    try {
      await api.copyText(command);
      message = 'Command copied.';
    } catch (cause) {
      message = `Copy failed: ${errorMessage(cause)}`;
    }
  }

  async function openReleasePage() {
    try {
      await api.openReleasePage();
    } catch (cause) {
      message = errorMessage(cause);
    }
  }
</script>

{#snippet spans(block: { spans: import('../utils/releaseNotes').NoteSpan[] })}
  {#each block.spans as span, i (i)}{#if span.code}<code>{span.text}</code
      >{:else if span.bold}<strong>{span.text}</strong
      >{:else}{span.text}{/if}{/each}
{/snippet}

<dialog
  class="update-dialog sectioned dialog-lg"
  aria-labelledby="update-title"
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <header>
    <h2 id="update-title">myCmd {update.version} is available</h2>
    <p>
      You have {current}{#if published}&nbsp;· released {published}{/if}
    </p>
  </header>
  <section aria-labelledby="update-install-title">
    <h3 id="update-install-title">Update this installation</h3>
    {#each update.instructions as item (item.target)}
      <div class="update-step">
        <strong>{item.label}</strong>
        <small>{item.note}</small>
        <div class="update-command">
          <code>{item.command}</code>
          <button
            type="button"
            title="Copy the command"
            onclick={() => copy(item.command)}>Copy</button
          >
        </div>
      </div>
    {:else}
      <p class="update-empty">
        No ready-made command for this system. Download the new version from the
        release page.
      </p>
    {/each}
  </section>
  <section aria-labelledby="update-notes-title">
    <h3 id="update-notes-title">Release notes</h3>
    <div class="release-notes">
      {#each blocks as block, index (index)}
        {#if block.kind === 'heading'}
          <p class={`note-heading level-${block.level}`}>
            {@render spans(block)}
          </p>
        {:else if block.kind === 'item'}
          <p
            class="note-item"
            style:padding-left={`${14 + block.depth * 16}px`}
          >
            {@render spans(block)}
          </p>
        {:else if block.kind === 'quote'}
          <p class="note-quote">{@render spans(block)}</p>
        {:else}
          <p>{@render spans(block)}</p>
        {/if}
      {:else}
        <p class="update-empty">No release notes.</p>
      {/each}
      {#if update.notesTruncated}<p class="update-empty">
          The notes are shortened; the release page has them in full.
        </p>{/if}
    </div>
  </section>
  <footer>
    <span class="message" role="status">{message}</span>
    <div class="dialog-actions">
      <button type="button" onclick={openReleasePage}>Release page</button>
      <button type="button" class="primary" onclick={onclose}>Close</button>
    </div>
  </footer>
</dialog>

<style>
  header p {
    margin: 6px 0 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  .update-step {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 0;
  }
  .update-step small {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }
  .update-command {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .update-command code {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid var(--divider);
    background: #181d26;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: all;
  }
  .release-notes {
    font-size: 13px;
  }
  .release-notes p {
    margin: 0 0 6px;
    line-height: 1.5;
  }
  .release-notes code {
    padding: 0 4px;
    background: var(--control);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .release-notes .note-heading {
    font-weight: 600;
    margin-top: 10px;
  }
  .note-heading.level-1,
  .note-heading.level-2 {
    font-size: 14px;
  }
  .note-item {
    position: relative;
  }
  .note-item::before {
    content: '•';
    position: absolute;
    margin-left: -12px;
    color: var(--text-muted);
  }
  .note-quote {
    border-left: 3px solid var(--accent-hover);
    padding-left: 10px;
    color: var(--text-muted);
  }
  .update-empty {
    color: var(--text-muted);
    font-size: 13px;
  }
  .message {
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
