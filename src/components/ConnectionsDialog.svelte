<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { api, errorMessage } from '../filesystem/api';
  import { preferences, scheduleSave } from '../state/preferences.svelte';
  import type {
    SftpConnectRequest,
    SftpConnectResult,
    SftpProfile,
  } from '../filesystem/types';

  let {
    initialId,
    onconnected,
    ondisconnected,
    onclose,
  }: {
    initialId?: string;
    onconnected: (
      connection: Extract<SftpConnectResult, { status: 'connected' }>,
    ) => Promise<void>;
    ondisconnected: (id: string) => Promise<void>;
    onclose: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  let selected = $state('');
  let id = $state<string>(crypto.randomUUID());
  let name = $state('');
  let host = $state('');
  let port = $state(22);
  let username = $state('');
  let directory = $state('');
  let keyPath = $state('');
  let auth = $state<SftpConnectRequest['auth']>('agent');
  let password = $state('');
  let passphrase = $state('');
  let remember = $state(true);
  let busy = $state(false);
  let fingerprint = $state('');
  let error = $state('');
  let disposed = false;

  function select(value: string) {
    selected = value;
    const profile = preferences.sftpProfiles.find(
      (profile) => profile.id === value,
    );
    id = profile?.id ?? crypto.randomUUID();
    name = profile?.name ?? '';
    host = profile?.host ?? '';
    port = profile?.port ?? 22;
    username = profile?.username ?? '';
    directory = profile?.directory ?? '';
    keyPath = profile?.keyPath ?? '';
    auth = keyPath ? 'key' : 'agent';
    password = '';
    passphrase = '';
    fingerprint = '';
    error = '';
  }
  onMount(() => {
    select(initialId ?? '');
    dialog.showModal();
    dialog.querySelector<HTMLElement>('select')?.focus();
  });
  onDestroy(() => {
    disposed = true;
    password = '';
    passphrase = '';
  });

  async function connect(trust = false) {
    if (busy) return;
    busy = true;
    error = '';
    const profile: SftpProfile = {
      id,
      name: name.trim(),
      host: host.trim(),
      port,
      username: username.trim(),
      directory: directory.trim(),
      keyPath: keyPath.trim() || null,
    };
    try {
      const result = await api.connectSftp({
        profile,
        auth,
        password: auth === 'password' ? password : undefined,
        passphrase: auth === 'key' ? passphrase : undefined,
        trustFingerprint: trust ? fingerprint : undefined,
      });
      if (disposed) {
        if (result.status === 'connected') await api.disconnectSftp(result.id);
        return;
      }
      if (result.status === 'hostKey') {
        fingerprint = result.fingerprint;
        return;
      }
      password = '';
      passphrase = '';
      fingerprint = '';
      if (remember) {
        preferences.sftpProfiles = [
          ...preferences.sftpProfiles.filter((saved) => saved.id !== id),
          profile,
        ];
        scheduleSave();
      }
      await onconnected(result);
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      if (!disposed) busy = false;
    }
  }
  async function disconnect() {
    if (!selected || busy) return;
    busy = true;
    error = '';
    try {
      await api.disconnectSftp(selected);
      if (!disposed) await ondisconnected(selected);
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      if (!disposed) busy = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  class="connections-dialog"
  aria-labelledby="connections-title"
  oncancel={(event) => {
    event.preventDefault();
    if (!busy) onclose();
  }}
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      void connect(!!fingerprint);
    }}
  >
    <h2 id="connections-title">SFTP connections</h2>
    <fieldset disabled={busy || !!fingerprint}>
      <label
        >Saved connection<select
          value={selected}
          onchange={(event) => select(event.currentTarget.value)}
        >
          <option value="">New connection</option>
          {#each preferences.sftpProfiles as profile}<option value={profile.id}
              >{profile.name}</option
            >{/each}
        </select></label
      >
      <label
        >Name<input
          bind:value={name}
          required
          maxlength="200"
          autocomplete="off"
        /></label
      >
      <div class="host-row">
        <label
          >Host<input
            bind:value={host}
            required
            spellcheck="false"
            autocomplete="off"
          /></label
        >
        <label
          >Port<input
            type="number"
            bind:value={port}
            required
            min="1"
            max="65535"
          /></label
        >
      </div>
      <label
        >Username<input
          bind:value={username}
          required
          autocomplete="off"
          spellcheck="false"
        /></label
      >
      <label
        >Starting directory<input
          bind:value={directory}
          placeholder="Server home directory"
          autocomplete="off"
          spellcheck="false"
        /></label
      >
      <label
        >Authentication<select bind:value={auth}>
          <option value="agent">SSH agent</option><option value="key"
            >Private key file</option
          ><option value="password">Password</option>
        </select></label
      >
      {#if auth === 'key'}
        <label
          >Private key path<input
            bind:value={keyPath}
            required
            autocomplete="off"
            spellcheck="false"
          /></label
        >
        <label
          >Key passphrase<input
            type="password"
            bind:value={passphrase}
            autocomplete="off"
          /></label
        >
      {:else if auth === 'password'}
        <label
          >Password<input
            type="password"
            bind:value={password}
            required
            autocomplete="off"
          /></label
        >
      {/if}
      <label class="check-label"
        ><input type="checkbox" bind:checked={remember} /> Remember connection details
        (never passwords or passphrases)</label
      >
    </fieldset>
    <p class="hint">
      Uses SSH host keys from ~/.ssh/known_hosts and keys trusted in myCmd. SSH
      config aliases, ProxyJump, and automatic key-file discovery are not
      applied; enter the resolved host, user, and key path.
    </p>
    {#if fingerprint}
      <section aria-label="Verify server host key">
        <h3>Unknown server host key</h3>
        <p>
          Verify this fingerprint with the server administrator before trusting
          this server:
        </p>
        <code>{fingerprint}</code>
        <button
          type="button"
          disabled={busy}
          onclick={() => {
            fingerprint = '';
          }}>Back</button
        >
      </section>
    {/if}
    {#if error}<p class="danger-text" role="alert">{error}</p>{/if}
    {#if busy}<p role="status">
        Connecting… Individual network requests time out after 15 seconds.
      </p>{/if}
    <div class="dialog-actions">
      {#if selected}<button
          type="button"
          disabled={busy || !!fingerprint}
          onclick={disconnect}>Disconnect</button
        >{/if}
      <button type="button" disabled={busy} onclick={onclose}>Cancel</button>
      <button type="submit" class="primary" disabled={busy}
        >{fingerprint ? 'Trust and connect' : 'Connect'}</button
      >
    </div>
  </form>
</dialog>

<style>
  .connections-dialog {
    width: 580px;
    max-height: 90vh;
    overflow: auto;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  .host-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 90px;
    gap: 12px;
    margin-top: 12px;
  }
  .host-row label {
    margin-top: 0;
  }
  select {
    width: 100%;
    margin-top: 4px;
  }
  .hint {
    font-size: 12px;
    color: #9ba9bd;
  }
  code {
    display: block;
    overflow-wrap: anywhere;
    margin-bottom: 12px;
  }
</style>
