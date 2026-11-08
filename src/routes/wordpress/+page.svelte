<script lang="ts">
  import { installWordPress } from '$lib/api';
  import type { WordPressInstallResult } from '$lib/types';

  let projectName = '';
  let siteTitle = '';
  let adminUser = 'admin';
  let adminPassword = '';
  let adminEmail = '';
  let installing = false;
  let error = '';
  let result: WordPressInstallResult | null = null;

  async function handleInstall() {
    if (!projectName) return;
    installing = true;
    error = '';
    result = null;
    try {
      result = await installWordPress(
        projectName,
        siteTitle || undefined,
        adminUser || undefined,
        adminPassword || undefined,
        adminEmail || undefined
      );
    } catch (e) {
      error = String(e);
    } finally {
      installing = false;
    }
  }
</script>

<svelte:head><title>WordPress — InsophiniaForage</title></svelte:head>

<div class="page">
  <div class="page-header">
    <div>
      <h1 class="page-title">One-click WordPress</h1>
      <p class="page-subtitle">
        Downloads the latest WordPress, extracts it into a new project, creates a database,
        and writes <code>wp-config.php</code> with secure random keys.
      </p>
    </div>
  </div>

  {#if error}<div class="alert alert-error">{error}</div>{/if}

  {#if result}
    <div class="alert alert-success">
      <strong>WordPress installed!</strong> Open the setup wizard to finish:
      <a href={result.admin_url} target="_blank" rel="noopener">{result.admin_url}</a>
      <div class="result-meta">
        <div>Project: <code>{result.project_name}</code></div>
        <div>Database: <code>{result.database_name}</code></div>
        <div>User: <code>{result.database_user}</code></div>
      </div>
    </div>
  {/if}

  <div class="card">
    <h2 class="card-title">Install settings</h2>
    <div class="grid">
      <div class="field">
        <label class="label" for="pj">Project name *</label>
        <input id="pj" class="input" type="text" bind:value={projectName} placeholder="my-blog" />
        <div class="hint">URL will be http://{projectName || 'name'}:8080</div>
      </div>
      <div class="field">
        <label class="label" for="st">Site title</label>
        <input id="st" class="input" type="text" bind:value={siteTitle} placeholder="My Blog" />
      </div>
      <div class="field">
        <label class="label" for="au">Admin username</label>
        <input id="au" class="input" type="text" bind:value={adminUser} />
      </div>
      <div class="field">
        <label class="label" for="ap">Admin password</label>
        <input id="ap" class="input" type="text" bind:value={adminPassword} placeholder="leave blank to set in browser" />
      </div>
      <div class="field full">
        <label class="label" for="ae">Admin email</label>
        <input id="ae" class="input" type="email" bind:value={adminEmail} placeholder="admin@example.com" />
      </div>
    </div>

    <div class="actions">
      <button class="btn btn-primary" on:click={handleInstall} disabled={!projectName || installing}>
        {installing ? 'Installing…' : 'Install WordPress'}
      </button>
    </div>

    {#if installing}
      <div class="progress-hint">
        This downloads ~30 MB and creates the database. It may take a minute or two.
      </div>
    {/if}
  </div>

  <div class="card info-card">
    <h3>What happens behind the scenes</h3>
    <ol>
      <li>Download <code>wordpress.org/latest.zip</code> (~30 MB) to <code>~/.insophinia-forage/cache/</code>.</li>
      <li>Extract it into <code>~/.insophinia-forage/www/&lt;name&gt;/</code>.</li>
      <li>Create a MariaDB database named <code>wp_&lt;name&gt;</code> using your dev user.</li>
      <li>Generate 64-char random salts and write <code>wp-config.php</code>.</li>
      <li>Open the WordPress 5-minute setup wizard at <code>/wp-admin/install.php</code>.</li>
    </ol>
    <p class="muted">
      We never set the WP admin password from the installer — that's done in the browser step.
      Pass an admin email so WP can send password resets.
    </p>
  </div>
</div>

<style>
  .page { display: flex; flex-direction: column; gap: 1.5rem; }
  .page-header { display: flex; align-items: flex-start; justify-content: space-between; }
  .page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
  .page-subtitle {
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    margin: 0.25rem 0 0; max-width: 600px;
  }
  code {
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    background: rgb(var(--bg-hover));
    padding: 0.0625rem 0.25rem;
    border-radius: 4px;
  }

  .card-title {
    font-size: 0.9375rem; font-weight: 600; margin: 0 0 1rem;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.875rem;
  }
  .field.full { grid-column: 1 / -1; }
  .hint { font-size: 0.6875rem; color: rgb(var(--text-tertiary)); margin-top: 0.25rem; }

  .actions { display: flex; justify-content: flex-end; margin-top: 1rem; }
  .progress-hint {
    font-size: 0.75rem; color: rgb(var(--text-tertiary));
    margin-top: 0.5rem; text-align: right;
  }

  .alert {
    padding: 0.875rem 1rem; border-radius: var(--radius);
    font-size: 0.8125rem;
  }
  .alert-success {
    background: rgb(var(--success) / 0.1); color: rgb(var(--success));
    border: 1px solid rgb(var(--success) / 0.2);
  }
  .alert-success a { color: rgb(var(--accent)); font-weight: 600; }
  .alert-success .result-meta {
    margin-top: 0.5rem; font-size: 0.75rem;
    color: rgb(var(--text-secondary));
    display: flex; flex-direction: column; gap: 0.25rem;
  }
  .alert-error {
    background: rgb(var(--danger) / 0.1); color: rgb(var(--danger));
    border: 1px solid rgb(var(--danger) / 0.2);
  }

  .info-card h3 {
    font-size: 0.875rem; margin: 0 0 0.5rem;
  }
  .info-card ol {
    margin: 0 0 0.75rem; padding-left: 1.25rem;
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    line-height: 1.7;
  }
  .muted { color: rgb(var(--text-tertiary)); font-size: 0.75rem; margin: 0; }
</style>
