<script lang="ts">
  import { slide } from 'svelte/transition';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    validateWizardInput,
    saveWizardInput,
    onDownloadProgress,
    checkCompatibilityFor,
    fetchAvailableVersions
  } from '$lib/api';
  import type {
    WizardInput,
    WizardResult,
    CompatibilityIssue,
    AvailableVersions
  } from '$lib/types';
  import VersionSelect from '$lib/components/VersionSelect.svelte';

  // Step 0: Welcome / binary strategy
  // Step 1: Version selection (with live compatibility check)
  // Step 2: Port selection
  // Step 3: Theme + finalize
  let step = 0;
  const totalSteps = 4;

  let input: WizardInput = {
    versions: {
      php: '8.3',
      mariadb: '11.4',
      nginx: '1.27',
      phpmyadmin: '5.2.2',
      wordpress: 'latest'
    },
    ports: {
      nginx_http: 8080,
      nginx_https: 8443,
      mariadb: 33060
    },
    binary_strategy: 'auto',
    theme: 'auto',
    auto_start_services: false,
    download_binaries: true
  };

  let validation: WizardResult | null = null;
  let saving = false;
  let downloadProgress: { name: string; done: number; total: number } | null = null;
  let compatIssues: CompatibilityIssue[] = [];
  let compatLoading = false;
  let availableVersions: AvailableVersions | null = null;
  let versionsLoading = true;
  let acceptedDisclaimer = false;

  // Hardcoded fallbacks — shown immediately while the fetch runs.
  const fallbackPhp = ['8.4', '8.3', '8.2', '8.1'];
  const fallbackMariadb = ['11.6', '11.4', '10.11'];
  const fallbackNginx = ['1.27', '1.26'];
  const fallbackPhpmyadmin = ['5.2.2'];
  const fallbackWordpress = ['latest'];

  $: phpVersions = availableVersions?.php ?? fallbackPhp;
  $: mariadbVersions = availableVersions?.mariadb ?? fallbackMariadb;
  $: nginxVersions = availableVersions?.nginx ?? fallbackNginx;
  $: phpmyadminVersions = availableVersions?.phpmyadmin ?? fallbackPhpmyadmin;
  $: wordpressVersions = availableVersions?.wordpress ?? fallbackWordpress;

  onMount(async () => {
    try {
      availableVersions = await fetchAvailableVersions();
    } catch (e) {
      console.warn('version fetch failed, using fallbacks', e);
    } finally {
      versionsLoading = false;
    }
  });

  function next() {
    if (step < totalSteps - 1) step++;
  }
  function back() {
    if (step > 0) step--;
  }

  // Live compatibility check whenever the user changes versions.
  async function refreshCompat() {
    if (step !== 1) return;
    compatLoading = true;
    try {
      compatIssues = await checkCompatibilityFor(
        input.versions.php,
        input.versions.mariadb,
        input.versions.nginx,
        input.versions.phpmyadmin,
        input.versions.wordpress
      );
    } catch (e) {
      console.warn('compat check failed', e);
      compatIssues = [];
    } finally {
      compatLoading = false;
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  $: trigger_compat = [
    input.versions.php,
    input.versions.mariadb,
    input.versions.nginx,
    input.versions.phpmyadmin,
    input.versions.wordpress
  ];
  $: if (step === 1 && trigger_compat) {
    refreshCompat();
  }

  $: blockingIssues = compatIssues.filter((i) => i.severity === 'error');
  $: warningIssues = compatIssues.filter((i) => i.severity === 'warning' || i.severity === 'info');

  async function finish() {
    saving = true;
    validation = null;
    try {
      validation = await validateWizardInput(input);
      if (!validation.success) return;

      const unlisten = await onDownloadProgress((p) => {
        downloadProgress = p;
      });

      const result = await saveWizardInput(input);
      unlisten();

      validation = result;
      if (result.success) {
        setTimeout(() => goto('/'), 1500);
      }
    } catch (e) {
      validation = {
        success: false,
        errors: [String(e)],
        warnings: []
      };
    } finally {
      saving = false;
      downloadProgress = null;
    }
  }
</script>

<div class="overlay">
  <div class="wizard">
    <div class="wizard-header">
      <div class="logo">IF</div>
      <div>
        <h1>Welcome to InsophiniaForage</h1>
        <p class="subtitle">A local PHP/WordPress stack for Linux, macOS, and Windows.</p>
      </div>
    </div>

    <div class="progress">
      {#each Array(totalSteps) as _, i}
        <div class="progress-dot" class:active={i === step} class:done={i < step}></div>
      {/each}
    </div>

    {#if step === 0}
      <section transition:slide>
        <div class="disclaimer-banner">
          <strong>⚠ Experimental software — use at your own risk</strong>
          <p>
            InsophiniaForage is an unfinished experimental tool for PHP development.
            It has not been audited or security-reviewed. It may destroy data,
            corrupt configs, or fail to start services. <strong>Back up your data
            before continuing.</strong> Not for production use.
          </p>
          <label class="disclaimer-checkbox">
            <input type="checkbox" bind:checked={acceptedDisclaimer} />
            <span>I understand this is experimental software and I use it at my own risk.</span>
          </label>
        </div>

        <h2>How should we find your binaries?</h2>
        <p class="section-help">
          We need nginx, PHP, and MariaDB. Choose how you want InsophiniaForage to find them.
          You can change this later in Settings.
        </p>
        <div class="options">
          <label class="option">
            <input type="radio" bind:group={input.binary_strategy} value="auto" />
            <div>
              <strong>Auto (recommended)</strong>
              <p>Detect system-installed binaries first, then bundled, then download.</p>
            </div>
          </label>
          <label class="option">
            <input type="radio" bind:group={input.binary_strategy} value="prefer_system" />
            <div>
              <strong>Prefer system</strong>
              <p>Use what's installed via your package manager (apt/dnf/brew). Lightest on RAM.</p>
            </div>
          </label>
          <label class="option">
            <input type="radio" bind:group={input.binary_strategy} value="prefer_bundled" />
            <div>
              <strong>Prefer bundled</strong>
              <p>Use binaries shipped with the installer. Most consistent across machines.</p>
            </div>
          </label>
          <label class="option">
            <input type="radio" bind:group={input.binary_strategy} value="prefer_download" />
            <div>
              <strong>Prefer downloaded</strong>
              <p>Download portable binaries into the app data dir. Easy to upgrade per-component.</p>
            </div>
          </label>
        </div>
        <label class="checkbox">
          <input type="checkbox" bind:checked={input.download_binaries} />
          <span>Download missing binaries now (~150 MB on a fresh install)</span>
        </label>
      </section>

    {:else if step === 1}
      <section transition:slide>
        <h2>Choose versions</h2>
        <p class="section-help">
          These versions will be used for the initial config. You can switch later
          (except for MariaDB — switching requires a data migration).
          {#if versionsLoading}
            <span class="muted">Fetching latest versions from official sources…</span>
          {:else if availableVersions?.fetched}
            <span class="muted">✓ Versions fetched live from php.net, nginx.org, mariadb.org, phpmyadmin.net, wordpress.org.</span>
          {:else if availableVersions?.error}
            <span class="muted warn">⚠ {availableVersions.error} — using fallback list.</span>
          {/if}
        </p>
        <div class="grid">
          <div>
            <VersionSelect
              label="PHP"
              value={input.versions.php}
              available={phpVersions}
              placeholder="8.3"
              on:change={(e) => (input.versions.php = e.detail)}
            />
          </div>
          <div>
            <VersionSelect
              label="MariaDB"
              value={input.versions.mariadb}
              available={mariadbVersions}
              placeholder="11.4"
              on:change={(e) => (input.versions.mariadb = e.detail)}
            />
          </div>
          <div>
            <VersionSelect
              label="nginx"
              value={input.versions.nginx}
              available={nginxVersions}
              placeholder="1.27"
              on:change={(e) => (input.versions.nginx = e.detail)}
            />
          </div>
          <div>
            <VersionSelect
              label="phpMyAdmin"
              value={input.versions.phpmyadmin}
              available={phpmyadminVersions}
              placeholder="5.2.2"
              on:change={(e) => (input.versions.phpmyadmin = e.detail)}
            />
          </div>
          <div>
            <VersionSelect
              label="WordPress"
              value={input.versions.wordpress}
              available={wordpressVersions}
              placeholder="latest"
              on:change={(e) => (input.versions.wordpress = e.detail)}
            />
          </div>
        </div>

        {#if compatLoading}
          <div class="compat-loading">Checking compatibility…</div>
        {:else if blockingIssues.length > 0}
          <div class="alert alert-error">
            <strong>Cannot proceed — fix these issues:</strong>
            <ul>{#each blockingIssues as i}<li>{i.message}</li>{/each}</ul>
          </div>
        {:else if warningIssues.length > 0}
          <div class="alert alert-warning">
            <strong>Heads up:</strong>
            <ul>{#each warningIssues as i}<li>{i.message}</li>{/each}</ul>
          </div>
        {:else if compatIssues.length === 0}
          <div class="alert alert-success">
            ✓ All versions are compatible.
          </div>
        {/if}
      </section>

    {:else if step === 2}
      <section transition:slide>
        <h2>Choose ports</h2>
        <p class="section-help">
          We use unprivileged ports (1024+) so no root is needed. If a port is
          already in use, we'll warn you before finishing.
        </p>
        <div class="grid">
          <div>
            <label class="label">nginx HTTP</label>
            <input class="input" type="number" min="1024" max="65535" bind:value={input.ports.nginx_http} />
          </div>
          <div>
            <label class="label">nginx HTTPS</label>
            <input class="input" type="number" min="1024" max="65535" bind:value={input.ports.nginx_https} />
          </div>
          <div>
            <label class="label">MariaDB</label>
            <input class="input" type="number" min="1024" max="65535" bind:value={input.ports.mariadb} />
          </div>
        </div>
      </section>

    {:else if step === 3}
      <section transition:slide>
        <h2>Final touches</h2>
        <p class="section-help">Choose how the app should look and behave.</p>
        <div class="grid">
          <div>
            <label class="label">Theme</label>
            <select class="input" bind:value={input.theme}>
              <option value="auto">Auto (follow system)</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>
        </div>
        <label class="checkbox">
          <input type="checkbox" bind:checked={input.auto_start_services} />
          <span>Auto-start all services when the app launches</span>
        </label>

        {#if validation}
          {#if validation.errors.length > 0}
            <div class="alert alert-error">
              <ul>{#each validation.errors as e}<li>{e}</li>{/each}</ul>
            </div>
          {/if}
          {#if validation.warnings.length > 0}
            <div class="alert alert-warning">
              <ul>{#each validation.warnings as w}<li>{w}</li>{/each}</ul>
            </div>
          {/if}
        {/if}

        {#if downloadProgress}
          <div class="download-progress">
            <div>Downloading {downloadProgress.name}…</div>
            <progress
              value={downloadProgress.total === 0 ? 0 : downloadProgress.done}
              max={downloadProgress.total || 1}
            ></progress>
            <div class="muted">
              {Math.round(downloadProgress.done / 1024 / 1024)} MB /
              {downloadProgress.total > 0 ? Math.round(downloadProgress.total / 1024 / 1024) + ' MB' : 'unknown'}
            </div>
          </div>
        {/if}
      </section>
    {/if}

    <div class="actions">
      <button class="btn btn-ghost" on:click={back} disabled={step === 0 || saving}>
        Back
      </button>
      <div class="spacer"></div>
      {#if step < totalSteps - 1}
        <button
          class="btn btn-primary"
          on:click={next}
          disabled={(step === 0 && !acceptedDisclaimer) || (step === 1 && blockingIssues.length > 0)}
        >
          Continue
        </button>
      {:else}
        <button class="btn btn-primary" on:click={finish} disabled={saving}>
          {saving ? 'Setting up…' : 'Finish setup'}
        </button>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgb(var(--bg-base));
    backdrop-filter: blur(40px);
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
  }
  .wizard {
    width: 100%;
    max-width: 640px;
    background: rgb(var(--bg-card));
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-popover);
    padding: 2rem;
  }
  .wizard-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  .logo {
    width: 48px; height: 48px;
    border-radius: 12px;
    background: linear-gradient(135deg, rgb(var(--accent)), rgb(var(--accent-hover)));
    color: white;
    display: flex; align-items: center; justify-content: center;
    font-weight: 700; font-size: 1.25rem;
    box-shadow: 0 4px 12px rgba(0, 122, 255, 0.4);
  }
  h1 { font-size: 1.25rem; font-weight: 700; margin: 0; }
  .subtitle { font-size: 0.8125rem; color: rgb(var(--text-secondary)); margin: 0.25rem 0 0; }

  .progress {
    display: flex; gap: 0.5rem; margin-bottom: 1.5rem;
  }
  .progress-dot {
    flex: 1; height: 4px; border-radius: 2px;
    background: rgb(var(--text-tertiary) / 0.2);
    transition: background 0.2s;
  }
  .progress-dot.active { background: rgb(var(--accent)); }
  .progress-dot.done { background: rgb(var(--accent) / 0.5); }

  section h2 { font-size: 1.0625rem; margin: 0 0 0.5rem; }
  .section-help {
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    margin: 0 0 1.25rem;
  }

  .options { display: flex; flex-direction: column; gap: 0.5rem; margin-bottom: 1rem; }
  .option {
    display: flex; gap: 0.625rem; padding: 0.75rem;
    border: 1px solid rgb(var(--border));
    border-radius: var(--radius);
    cursor: pointer;
  }
  .option:hover { background: rgb(var(--bg-hover) / 0.5); }
  .option strong { font-size: 0.875rem; display: block; }
  .option p { font-size: 0.75rem; color: rgb(var(--text-secondary)); margin: 0.25rem 0 0; }

  .checkbox {
    display: flex; align-items: center; gap: 0.5rem;
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    margin-top: 1rem;
    cursor: pointer;
  }
  .checkbox input { width: 16px; height: 16px; }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.875rem;
  }

  .actions {
    display: flex; align-items: center; gap: 0.5rem;
    margin-top: 1.5rem;
    padding-top: 1.25rem;
    border-top: 1px solid rgb(var(--border));
  }
  .spacer { flex: 1; }

  .alert {
    padding: 0.75rem 1rem; border-radius: var(--radius);
    font-size: 0.8125rem; margin-top: 1rem;
  }
  .alert ul { margin: 0; padding-left: 1.25rem; }
  .alert-error {
    background: rgb(var(--danger) / 0.1); color: rgb(var(--danger));
    border: 1px solid rgb(var(--danger) / 0.2);
  }
  .alert-warning {
    background: rgb(var(--warning) / 0.1); color: rgb(var(--warning));
    border: 1px solid rgb(var(--warning) / 0.2);
  }
  .alert-success {
    background: rgb(var(--success) / 0.1); color: rgb(var(--success));
    border: 1px solid rgb(var(--success) / 0.2);
  }
  .compat-loading {
    font-size: 0.8125rem; color: rgb(var(--text-tertiary));
    margin-top: 1rem; font-style: italic;
  }
  .muted { color: rgb(var(--text-tertiary)); font-size: 0.75rem; }
  .muted.warn { color: rgb(var(--warning)); }

  .disclaimer-banner {
    background: rgb(var(--warning) / 0.08);
    border: 1px solid rgb(var(--warning) / 0.25);
    border-radius: var(--radius);
    padding: 0.875rem 1rem;
    margin-bottom: 1.25rem;
  }
  .disclaimer-banner strong {
    display: block;
    font-size: 0.875rem;
    color: rgb(var(--warning));
    margin-bottom: 0.375rem;
  }
  .disclaimer-banner p {
    font-size: 0.75rem;
    color: rgb(var(--text-secondary));
    margin: 0 0 0.625rem;
    line-height: 1.5;
  }
  .disclaimer-checkbox {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: rgb(var(--text-primary));
    cursor: pointer;
    padding-top: 0.375rem;
    border-top: 1px solid rgb(var(--warning) / 0.15);
  }
  .disclaimer-checkbox input {
    margin-top: 0.125rem;
    flex-shrink: 0;
  }
  .alert ul { margin: 0.5rem 0 0; padding-left: 1.25rem; }
  .alert strong { display: block; }
  .download-progress {
    margin-top: 1rem; padding: 0.875rem;
    background: rgb(var(--bg-hover) / 0.5);
    border-radius: var(--radius);
    font-size: 0.8125rem;
  }
  .download-progress progress {
    width: 100%; height: 6px; margin: 0.5rem 0;
    border-radius: 3px;
  }
  .muted { color: rgb(var(--text-tertiary)); font-size: 0.75rem; }
</style>
