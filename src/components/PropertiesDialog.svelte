<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { api, errorMessage } from '../filesystem/api';
  import type { FileProperties } from '../filesystem/types';
  import { bytes, date } from '../utils/format';
  import { displayPath } from '../utils/paths';

  let {
    path,
    onclose,
    onapplied,
  }: {
    path: string;
    onclose: () => void;
    onapplied: () => Promise<void>;
  } = $props();
  let dialog: HTMLDialogElement;
  let info = $state<FileProperties>();
  let loading = $state(true);
  let saving = $state(false);
  let sizing = $state(false);
  let size = $state<number>();
  let mode = $state(0);
  let readOnly = $state(false);
  let error = $state('');
  let notice = $state('');
  let disposed = false;
  let sizingId: string | undefined;
  const subjects = [
    ['Owner', 6],
    ['Group', 3],
    ['Others', 0],
  ] as const;
  const permissions = [
    ['Read', 4],
    ['Write', 2],
    ['Execute', 1],
  ] as const;
  let dirty = $derived(
    info &&
      info.type !== 'symlink' &&
      (info.mode != null ? mode !== info.mode : readOnly !== info.readOnly),
  );

  function update(properties: FileProperties) {
    info = properties;
    mode = properties.mode ?? 0;
    readOnly = properties.readOnly;
  }
  onMount(() => {
    dialog.showModal();
    dialog.querySelector<HTMLButtonElement>('button')?.focus();
    void api
      .properties(path)
      .then((properties) => {
        if (!disposed) update(properties);
      })
      .catch((cause) => {
        if (!disposed) error = errorMessage(cause);
      })
      .finally(() => {
        if (!disposed) loading = false;
      });
  });
  onDestroy(() => {
    disposed = true;
    if (sizingId) void api.cancelDirectorySizing([sizingId]).catch(() => {});
  });
  async function calculateSize() {
    if (!info || info.type !== 'directory' || sizing) return;
    sizing = true;
    error = '';
    sizingId = crypto.randomUUID();
    try {
      const result = await api.measureDirectory(info.path, sizingId);
      if (!disposed) size = result;
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      sizingId = undefined;
      if (!disposed) sizing = false;
    }
  }
  async function copyPath() {
    error = '';
    try {
      await api.copyText(displayPath(info?.path ?? path));
      if (!disposed) notice = 'Path copied.';
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    }
  }
  async function apply() {
    if (!info || !dirty || saving) return;
    saving = true;
    error = '';
    notice = '';
    try {
      const properties = await api.setPermissions(
        info.path,
        info.mode != null ? { mode } : { readOnly },
      );
      if (disposed) return;
      update(properties);
      notice = 'Permissions updated.';
      await onapplied();
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      if (!disposed) saving = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  class="properties-dialog"
  aria-labelledby="properties-title"
  oncancel={(event) => {
    event.preventDefault();
    if (!saving) onclose();
  }}
>
  <h2 id="properties-title">Properties</h2>
  {#if loading}<p role="status">Loading properties…</p>{/if}
  {#if info}
    <dl>
      <dt>Name</dt>
      <dd>{info.name}</dd>
      <dt>Type</dt>
      <dd>
        {info.type === 'directory'
          ? 'Folder'
          : info.type === 'symlink'
            ? 'Symbolic link'
            : 'File'}
      </dd>
      <dt>Full path</dt>
      <dd class="path">
        {displayPath(info.path)}
        <button type="button" onclick={copyPath}>Copy path</button>
      </dd>
      <dt>Size</dt>
      <dd>
        {#if info.type === 'directory'}
          {size == null
            ? 'Not calculated'
            : `${bytes(size)} (${size.toLocaleString()} bytes)`}
          <button
            type="button"
            disabled={sizing || saving}
            onclick={calculateSize}
            >{sizing ? 'Calculating…' : 'Calculate size'}</button
          >
        {:else}{bytes(info.size)} ({info.size.toLocaleString()} bytes){/if}
      </dd>
      <dt>Modified</dt>
      <dd>{date(info.modified)}</dd>
      {#if info.linkTarget != null}
        <dt>Link target</dt>
        <dd class="path">{displayPath(info.linkTarget)}</dd>
        <dt>Target status</dt>
        <dd>
          {info.linkExists === true
            ? 'Resolves'
            : info.linkExists === false
              ? 'Missing target'
              : 'Unable to resolve target'}
        </dd>
      {/if}
      {#if info.owner != null}<dt>Owner</dt>
        <dd>{info.owner}</dd>{/if}
      {#if info.group != null}<dt>Group</dt>
        <dd>{info.group}</dd>{/if}
    </dl>
    <fieldset disabled={saving || info.type === 'symlink'}>
      <legend>Permissions</legend>
      {#if info.mode != null}
        <table>
          <thead
            ><tr
              ><th scope="col">Applies to</th>{#each permissions as [label]}<th
                  scope="col">{label}</th
                >{/each}</tr
            ></thead
          >
          <tbody
            >{#each subjects as [subject, shift]}<tr>
                <th scope="row">{subject}</th>
                {#each permissions as [label, bit]}
                  <td
                    ><input
                      type="checkbox"
                      aria-label={`${subject} ${label.toLowerCase()}`}
                      checked={(mode & (bit << shift)) !== 0}
                      onchange={(event) => {
                        const mask = bit << shift;
                        mode = event.currentTarget.checked
                          ? mode | mask
                          : mode & ~mask;
                      }}
                    /></td
                  >
                {/each}
              </tr>{/each}</tbody
          >
        </table>
      {:else}
        <label class="check-label"
          ><input type="checkbox" bind:checked={readOnly} /> Read-only</label
        >
      {/if}
    </fieldset>
    <p class="hint">
      {info.type === 'symlink'
        ? 'Symbolic link permissions cannot be edited. Targets are not modified.'
        : 'Changes apply only to this item, not its contents.'}
    </p>
  {/if}
  {#if error}<p class="danger-text" role="alert">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  <div class="dialog-actions">
    <button type="button" disabled={saving} onclick={onclose}>Close</button>
    <button
      type="button"
      class="primary"
      disabled={!dirty || saving}
      onclick={apply}>{saving ? 'Applying…' : 'Apply'}</button
    >
  </div>
</dialog>

<style>
  .properties-dialog {
    width: 620px;
    max-width: calc(100vw - 40px);
    max-height: 90vh;
    overflow: auto;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    align-items: baseline;
    gap: 10px 16px;
  }
  dt {
    margin: 0;
  }
  dt,
  .hint {
    color: #9ba9bd;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  dd button {
    margin-left: 8px;
  }
  .path {
    white-space: pre-wrap;
  }
  fieldset {
    margin-top: 20px;
    border: 1px solid #47566e;
  }
  table {
    width: 100%;
    table-layout: fixed;
    text-align: center;
  }
  th {
    font-weight: normal;
  }
  th,
  td {
    padding: 6px;
    vertical-align: middle;
  }
  th:first-child {
    text-align: left;
  }
  td input {
    vertical-align: middle;
  }
  .hint {
    font-size: 12px;
  }
</style>
