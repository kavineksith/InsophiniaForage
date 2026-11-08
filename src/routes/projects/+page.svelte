<script lang="ts">
  import { onMount } from 'svelte';
  import { projects } from '$lib/stores';
  import {
    openProjectInBrowser,
    openProjectInFileManager,
    listPhpVersions,
    getProjectPhpVersion,
    setProjectPhpVersion
  } from '$lib/api';
  import type { ProjectKind, PhpVersionInfo } from '$lib/types';

  let phpVersions: PhpVersionInfo[] = [];
  let projectPhpVersions: Record<string, string | null> = {};

  onMount(async () => {
    await projects.refresh();
    try {
      phpVersions = await listPhpVersions();
      // Load per-project PHP version overrides.
      for (const p of $projects.list) {
        projectPhpVersions[p.name] = await getProjectPhpVersion(p.name);
      }
    } catch (e) {
      console.warn('php version load failed', e);
    }
  });

  let showCreate = false;
  let newName = '';
  let newKind: ProjectKind = 'plain';
  let creating = false;
  let createError = '';

  async function handleCreate() {
    if (!newName) return;
    creating = true;
    createError = '';
    try {
      await projects.create(newName, newKind);
      newName = '';
      showCreate = false;
      // Refresh PHP version map.
      for (const p of $projects.list) {
        if (!(p.name in projectPhpVersions)) {
          projectPhpVersions[p.name] = await getProjectPhpVersion(p.name);
        }
      }
    } catch (e) {
      createError = String(e);
    } finally {
      creating = false;
    }
  }

  async function handleDelete(name: string) {
    if (!confirm(`Delete project "${name}"? This cannot be undone.`)) return;
    await projects.remove(name);
    delete projectPhpVersions[name];
  }

  async function handlePhpVersionChange(name: string, version: string) {
    const v = version === '' ? null : version;
    try {
      await setProjectPhpVersion(name, v);
      projectPhpVersions = { ...projectPhpVersions, [name]: v };
    } catch (e) {
      alert(`Failed to set PHP version: ${e}`);
    }
  }

  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  function kindLabel(kind: ProjectKind): string {
    return ({
      plain: 'Plain PHP',
      word_press: 'WordPress',
      laravel: 'Laravel',
      php_my_admin: 'phpMyAdmin',
      custom: 'Custom'
    } as const)[kind];
  }
</script>

<svelte:head><title>Projects — InsophiniaForage</title></svelte:head>

<div class="page">
  <div class="page-header">
    <div>
      <h1 class="page-title">Projects</h1>
      <p class="page-subtitle">
        Each project is a folder under <code>~/.insophinia-forage/www/</code>, accessible at
        <code>http://&lt;name&gt;:&lt;port&gt;</code>.
      </p>
    </div>
    <button class="btn btn-primary" on:click={() => (showCreate = true)}>+ New project</button>
  </div>

  {#if $projects.error}
    <div class="alert alert-error">{$projects.error}</div>
  {/if}

  {#if $projects.list.length === 0 && !$projects.loading}
    <div class="empty">
      <div class="empty-icon">▸</div>
      <h3>No projects yet</h3>
      <p>Create your first PHP project, install WordPress, or add Laravel.</p>
    </div>
  {:else}
    <div class="grid">
      {#each $projects.list as p (p.name)}
        <div class="card project-card">
          <div class="card-head">
            <div>
              <h3 class="name">{p.name}</h3>
              <span class="pill pill-neutral">{kindLabel(p.kind)}</span>
            </div>
            <div class="size">{fmtSize(p.size_bytes)}</div>
          </div>
          <div class="card-url">{p.url}</div>

          <div class="php-version-row">
            <label class="php-label">PHP</label>
            <select
              class="input-sm"
              value={projectPhpVersions[p.name] ?? ''}
              on:change={(e) => handlePhpVersionChange(p.name, e.currentTarget.value)}
            >
              <option value="">Default</option>
              {#each phpVersions as v}
                <option value={v.version} disabled={!v.installed}>
                  {v.version}{#if !v.installed} (not installed){/if}
                </option>
              {/each}
            </select>
            {#if projectPhpVersions[p.name]}
              <span class="pill pill-warning">override</span>
            {/if}
          </div>

          <div class="card-actions">
            <button class="btn btn-secondary" on:click={() => openProjectInBrowser(p.name)}>
              Open ↗
            </button>
            <button class="btn btn-ghost" on:click={() => openProjectInFileManager(p.name)}>
              Files
            </button>
            <button class="btn btn-ghost btn-danger-text" on:click={() => handleDelete(p.name)}>
              Delete
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if showCreate}
  <div class="modal-overlay" on:click|self={() => (showCreate = false)}>
    <div class="modal">
      <h2>Create new project</h2>
      <div class="field">
        <label class="label" for="proj-name">Name</label>
        <input id="proj-name" class="input" type="text" bind:value={newName} placeholder="my-app" />
        <div class="hint">Will be accessible at <code>http://{newName || 'name'}:8080</code></div>
      </div>
      <div class="field">
        <label class="label">Type</label>
        <select class="input" bind:value={newKind}>
          <option value="plain">Plain PHP (just phpinfo stub)</option>
          <option value="word_press">WordPress (use the WordPress tab for full install)</option>
          <option value="laravel">Laravel (run <code>composer install</code> after)</option>
          <option value="custom">Custom (empty folder)</option>
        </select>
      </div>
      {#if createError}<div class="alert alert-error">{createError}</div>{/if}
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
  code {
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    background: rgb(var(--bg-hover));
    padding: 0.0625rem 0.25rem;
    border-radius: 4px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 1rem;
  }

  .project-card { display: flex; flex-direction: column; gap: 0.75rem; }
  .card-head {
    display: flex; align-items: flex-start; justify-content: space-between; gap: 0.5rem;
  }
  .name { font-size: 1rem; font-weight: 600; margin: 0 0 0.25rem; }
  .size { font-size: 0.75rem; color: rgb(var(--text-tertiary)); }
  .card-url {
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    color: rgb(var(--accent));
    padding: 0.375rem 0.5rem;
    background: rgb(var(--accent) / 0.08);
    border-radius: 6px;
    word-break: break-all;
  }

  .php-version-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0;
    border-top: 1px solid rgb(var(--border));
    border-bottom: 1px solid rgb(var(--border));
    font-size: 0.75rem;
  }
  .php-label {
    font-size: 0.6875rem;
    font-weight: 600;
    color: rgb(var(--text-tertiary));
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .input-sm {
    flex: 1;
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
    border-radius: 6px;
    background: rgb(var(--bg-base));
    border: 1px solid rgb(var(--border));
    color: rgb(var(--text-primary));
    outline: none;
  }
  .input-sm:focus {
    border-color: rgb(var(--accent));
  }
  .card-actions { display: flex; gap: 0.375rem; margin-top: auto; }
  .btn-danger-text { color: rgb(var(--danger)); margin-left: auto; }
  .btn-danger-text:hover { background: rgb(var(--danger) / 0.1); }

  .empty {
    padding: 4rem 1rem; text-align: center;
    color: rgb(var(--text-tertiary));
  }
  .empty-icon { font-size: 2rem; margin-bottom: 0.5rem; }
  .empty h3 { color: rgb(var(--text-secondary)); margin: 0 0 0.25rem; }
  .empty p { margin: 0; font-size: 0.8125rem; }

  .modal-overlay {
    position: fixed; inset: 0; z-index: 50;
    background: rgba(0, 0, 0, 0.4);
    display: flex; align-items: center; justify-content: center;
    padding: 2rem;
  }
  .modal {
    width: 100%; max-width: 440px;
    background: rgb(var(--bg-card));
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-popover);
    padding: 1.5rem;
  }
  .modal h2 { font-size: 1.125rem; margin: 0 0 1rem; }
  .field { margin-bottom: 1rem; }
  .hint {
    font-size: 0.6875rem; color: rgb(var(--text-tertiary));
    margin-top: 0.25rem;
  }
  .modal-actions {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    margin-top: 1.5rem;
  }

  .alert {
    padding: 0.75rem 1rem; border-radius: var(--radius);
    font-size: 0.8125rem;
  }
  .alert-error {
    background: rgb(var(--danger) / 0.1); color: rgb(var(--danger));
    border: 1px solid rgb(var(--danger) / 0.2);
  }
</style>
