<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listDatabases, createDatabase, dropDatabase, queryDatabase, openPhpmyadmin } from '$lib/api';
  import type { Database } from '$lib/types';

  let dbs: Database[] = [];
  let loading = false;
  let error = '';
  let showCreate = false;
  let newName = '';
  let creating = false;
  let selectedDb = '';
  let sqlInput = 'SHOW TABLES;';
  let sqlOutput = '';
  let sqlRunning = false;
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  async function refresh() {
    loading = true;
    error = '';
    try {
      dbs = await listDatabases();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
    // Poll every 5s — DB list doesn't change often, but we want fresh stats.
    pollTimer = setInterval(refresh, 5000);
  });
  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });

  async function handleCreate() {
    if (!newName) return;
    creating = true;
    try {
      await createDatabase(newName);
      newName = '';
      showCreate = false;
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      creating = false;
    }
  }

  async function handleDelete(name: string) {
    if (!confirm(`Drop database "${name}"? All data will be lost.`)) return;
    try {
      await dropDatabase(name);
      if (selectedDb === name) {
        selectedDb = '';
        sqlOutput = '';
      }
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function runQuery() {
    if (!selectedDb || !sqlInput) return;
    sqlRunning = true;
    sqlOutput = '';
    try {
      sqlOutput = await queryDatabase(selectedDb, sqlInput);
    } catch (e) {
      sqlOutput = `Error: ${e}`;
    } finally {
      sqlRunning = false;
    }
  }

  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  }
</script>

<svelte:head><title>Databases — InsophiniaForage</title></svelte:head>

<div class="page">
  <div class="page-header">
    <div>
      <h1 class="page-title">Databases</h1>
      <p class="page-subtitle">
        Manage MariaDB databases. Run SQL queries directly, or open phpMyAdmin for a full GUI.
      </p>
    </div>
    <div class="header-actions">
      <button class="btn btn-secondary" on:click={() => openPhpmyadmin()}>Open phpMyAdmin ↗</button>
      <button class="btn btn-primary" on:click={() => (showCreate = true)}>+ New database</button>
    </div>
  </div>

  {#if error}<div class="alert alert-error">{error}</div>{/if}

  <div class="two-col">
    <div class="card db-list">
      <h3 class="card-title">Databases {#if loading}<span class="muted">(loading…)</span>{/if}</h3>
      {#if dbs.length === 0 && !loading}
        <div class="empty">No databases yet. Create one above.</div>
      {:else}
        <ul>
          {#each dbs as db (db.name)}
            <li>
              <button class="db-row" class:selected={selectedDb === db.name} on:click={() => (selectedDb = db.name)}>
                <div class="db-name">{db.name}</div>
                <div class="db-meta">
                  <span>{db.tables_count} tables</span>
                  <span>·</span>
                  <span>{fmtSize(db.size_bytes)}</span>
                </div>
              </button>
              <button class="btn btn-ghost btn-danger-text" on:click={() => handleDelete(db.name)}>Drop</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="card sql-panel">
      <h3 class="card-title">SQL query</h3>
      {#if !selectedDb}
        <div class="empty">Select a database on the left to run queries.</div>
      {:else}
        <div class="sql-current">Database: <code>{selectedDb}</code></div>
        <textarea
          class="input sql-input"
          rows="6"
          bind:value={sqlInput}
          spellcheck="false"
          placeholder="SHOW TABLES;"
        ></textarea>
        <div class="sql-actions">
          <button class="btn btn-primary" on:click={runQuery} disabled={sqlRunning}>
            {sqlRunning ? 'Running…' : 'Run ▸'}
          </button>
        </div>
        {#if sqlOutput}
          <pre class="sql-output">{sqlOutput}</pre>
        {/if}
      {/if}
    </div>
  </div>
</div>

{#if showCreate}
  <div class="modal-overlay" on:click|self={() => (showCreate = false)}>
    <div class="modal">
      <h2>Create new database</h2>
      <label class="label" for="db-name">Name</label>
      <input id="db-name" class="input" type="text" bind:value={newName} placeholder="my_app" />
      <div class="hint">Allowed: a-z, 0-9, underscore, hyphen. Max 64 chars.</div>
      <div class="modal-actions">
        <button class="btn btn-ghost" on:click={() => (showCreate = false)}>Cancel</button>
        <button class="btn btn-primary" on:click={handleCreate} disabled={!newName || creating}>
          {creating ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { display: flex; flex-direction: column; gap: 1.5rem; }
  .page-header {
    display: flex; align-items: flex-start; justify-content: space-between;
    gap: 1rem; flex-wrap: wrap;
  }
  .page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
  .page-subtitle {
    font-size: 0.8125rem; color: rgb(var(--text-secondary)); margin: 0.25rem 0 0;
  }
  .header-actions { display: flex; gap: 0.5rem; }

  .two-col {
    display: grid;
    grid-template-columns: 1fr 1.5fr;
    gap: 1rem;
  }
  @media (max-width: 900px) {
    .two-col { grid-template-columns: 1fr; }
  }

  .card-title {
    font-size: 0.875rem; font-weight: 600; margin: 0 0 0.75rem;
    color: rgb(var(--text-secondary));
  }
  .muted { color: rgb(var(--text-tertiary)); font-weight: 400; }

  .db-list ul { list-style: none; padding: 0; margin: 0; }
  .db-list li {
    display: flex; align-items: center; gap: 0.25rem;
    padding: 0.25rem 0;
  }
  .db-row {
    flex: 1; text-align: left;
    padding: 0.5rem 0.625rem;
    border-radius: var(--radius);
    transition: background 0.1s;
  }
  .db-row:hover { background: rgb(var(--bg-hover)); }
  .db-row.selected { background: rgb(var(--accent) / 0.12); }
  .db-name { font-weight: 500; font-size: 0.875rem; color: rgb(var(--text-primary)); }
  .db-meta {
    font-size: 0.75rem; color: rgb(var(--text-tertiary));
    margin-top: 2px; display: flex; gap: 0.375rem;
  }
  .btn-danger-text { color: rgb(var(--danger)); padding: 0.5rem; }
  .btn-danger-text:hover { background: rgb(var(--danger) / 0.1); }

  .sql-current {
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    margin-bottom: 0.5rem;
  }
  code {
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    background: rgb(var(--bg-hover));
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
  }
  .sql-input {
    font-family: theme('fontFamily.mono');
    font-size: 0.8125rem;
    resize: vertical;
    min-height: 120px;
  }
  .sql-actions { margin-top: 0.5rem; display: flex; justify-content: flex-end; }
  .sql-output {
    margin-top: 1rem;
    padding: 0.875rem;
    background: rgb(var(--bg-base));
    border: 1px solid rgb(var(--border));
    border-radius: var(--radius);
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    color: rgb(var(--text-primary));
    overflow-x: auto;
    max-height: 320px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .empty {
    padding: 2rem 1rem; text-align: center;
    color: rgb(var(--text-tertiary)); font-size: 0.8125rem;
  }

  .alert {
    padding: 0.75rem 1rem; border-radius: var(--radius);
    font-size: 0.8125rem;
  }
  .alert-error {
    background: rgb(var(--danger) / 0.1); color: rgb(var(--danger));
    border: 1px solid rgb(var(--danger) / 0.2);
  }

  .modal-overlay {
    position: fixed; inset: 0; z-index: 50;
    background: rgba(0, 0, 0, 0.4);
    display: flex; align-items: center; justify-content: center;
    padding: 2rem;
  }
  .modal {
    width: 100%; max-width: 420px;
    background: rgb(var(--bg-card));
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-popover);
    padding: 1.5rem;
  }
  .modal h2 { font-size: 1.125rem; margin: 0 0 1rem; }
  .hint {
    font-size: 0.6875rem; color: rgb(var(--text-tertiary));
    margin-top: 0.375rem;
  }
  .modal-actions {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    margin-top: 1.5rem;
  }
</style>
