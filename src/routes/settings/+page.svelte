<script lang="ts">
  import { onMount } from 'svelte';
  import { settings, binaries } from '$lib/stores';
  import {
    downloadBinary,
    listPhpVersions,
    installPhpVersion,
    getDetectionReport,
    checkCompatibility,
    setConflictPolicy,
    fetchAvailableVersions,
    isSourceCompileSupported,
    checkPhpBuildDeps,
    compilePhpFromSource,
    compileNginxFromSource
  } from '$lib/api';
  import type {
    PhpVersionInfo,
    DetectionReport,
    CompatibilityIssue,
    PlannedAction,
    AvailableVersions,
    BuildDepsReport
  } from '$lib/types';
  import VersionSelect from '$lib/components/VersionSelect.svelte';
  import BuildProgressModal from '$lib/components/BuildProgressModal.svelte';
  import InstallPlanModal from '$lib/components/InstallPlanModal.svelte';

  let phpVersions: PhpVersionInfo[] = [];
  let installingPhp: string | null = null;
  let detectionReport: DetectionReport | null = null;
  let compatIssues: CompatibilityIssue[] = [];
  let availableVersions: AvailableVersions | null = null;
  let sourceCompileSupported = false;

  // Source-compile UI state
  let showBuildModal = false;
  let buildModalTitle = '';
  let buildLines: { stage: string; line: string }[] = [];
  let buildDepsReport: BuildDepsReport | null = null;
  let buildDepsLoading = false;
  let compileVersion = '';
  let compileTarget: 'php' | 'nginx' = 'php';

  // Package-manager install UI state
  let showInstallPlanModal = false;
  let installPlanComponent: 'php' | 'nginx' = 'php';
  let installPlanVersion = '';

  onMount(async () => {
    settings.refresh();
    binaries.refresh();
    try {
      [phpVersions, detectionReport, compatIssues, availableVersions] = await Promise.all([
        listPhpVersions(),
        getDetectionReport(),
        checkCompatibility(),
        fetchAvailableVersions()
      ]);
      sourceCompileSupported = await isSourceCompileSupported();
    } catch (e) {
      console.warn('init failed', e);
    }
  });

  let activeTab: 'general' | 'php' | 'php-versions' | 'mariadb' | 'nginx' | 'binaries' | 'detection' = 'general';

  $: phpOptions = availableVersions?.php ?? ['8.4', '8.3', '8.2', '8.1'];
  $: mariadbOptions = availableVersions?.mariadb ?? ['11.6', '11.4', '10.11'];
  $: nginxOptions = availableVersions?.nginx ?? ['1.27', '1.26'];
  $: phpmyadminOptions = availableVersions?.phpmyadmin ?? ['5.2.2'];

  async function updateField(key: string, value: unknown) {
    await settings.updateField(key, value);
    if (key.startsWith('versions.')) {
      try {
        compatIssues = await checkCompatibility();
        detectionReport = await getDetectionReport();
      } catch {
        // ignore
      }
    }
  }

  async function handleDownload(name: string, version: string) {
    await binaries.download(name, version);
    detectionReport = await getDetectionReport();
  }

  async function handleInstallPhp(version: string) {
    installingPhp = version;
    try {
      await installPhpVersion(version);
      phpVersions = await listPhpVersions();
      detectionReport = await getDetectionReport();
    } catch (e) {
      alert(`Failed to install PHP ${version}: ${e}`);
    } finally {
      installingPhp = null;
    }
  }

  async function handleConflictPolicyChange(policy: string) {
    try {
      await setConflictPolicy(policy);
      detectionReport = await getDetectionReport();
      await settings.refresh();
    } catch (e) {
      alert(`Failed to set conflict policy: ${e}`);
    }
  }

  async function handleRefreshDetection() {
    detectionReport = await getDetectionReport();
    compatIssues = await checkCompatibility();
    try {
      availableVersions = await fetchAvailableVersions();
    } catch (e) {
      console.warn('refresh versions failed', e);
    }
  }

  async function handleCheckBuildDeps() {
    buildDepsLoading = true;
    try {
      buildDepsReport = await checkPhpBuildDeps();
    } catch (e) {
      alert(`Failed to check build deps: ${e}`);
    } finally {
      buildDepsLoading = false;
    }
  }

  async function handleCopyInstallCommand() {
    if (!buildDepsReport?.install_command) return;
    try {
      await navigator.clipboard.writeText(buildDepsReport.install_command);
      alert('Command copied to clipboard. Paste it into your terminal with sudo.');
    } catch (e) {
      prompt('Copy this command:', buildDepsReport.install_command);
    }
  }

  async function handleCompileFromSource(target: 'php' | 'nginx') {
    compileTarget = target;
    compileVersion = target === 'php'
      ? ($settings.current?.versions.php ?? '8.3')
      : ($settings.current?.versions.nginx ?? '1.27');

    // Don't ask for confirmation if user already typed a version in the modal.
    const version = prompt(
      `Enter ${target === 'php' ? 'PHP' : 'nginx'} version to compile from source:`,
      compileVersion
    );
    if (!version) return;
    compileVersion = version;

    // Pre-flight: check build deps.
    await handleCheckBuildDeps();
    if (buildDepsReport && !buildDepsReport.all_present) {
      // Show the deps report instead of starting the build.
      return;
    }

    // Start the build.
    buildLines = [];
    buildModalTitle = `Compiling ${target === 'php' ? 'PHP' : 'nginx'} ${compileVersion} from source`;
    showBuildModal = true;

    try {
      const result = target === 'php'
        ? await compilePhpFromSource(compileVersion)
        : await compileNginxFromSource(compileVersion);
      // Refresh the version lists so the new build shows up.
      phpVersions = await listPhpVersions();
      detectionReport = await getDetectionReport();
      await settings.refresh();
      // Modal will be dismissed by the done event handler.
    } catch (e) {
      buildLines = [...buildLines, { stage: 'error', line: String(e) }];
    }
  }

  function handleBuildDone(e: CustomEvent<string>) {
    showBuildModal = false;
    alert(`✓ Build complete. The new binary is now available.`);
  }

  function handleBuildError(e: CustomEvent<string>) {
    showBuildModal = false;
    console.warn('build dismissed:', e.detail);
  }

  function handleInstallViaPackageManager(target: 'php' | 'nginx') {
    const defaultVersion = target === 'php'
      ? ($settings.current?.versions.php ?? '8.3')
      : ($settings.current?.versions.nginx ?? '1.27');
    const version = prompt(
      `Enter ${target === 'php' ? 'PHP' : 'nginx'} version to install via package manager:`,
      defaultVersion
    );
    if (!version) return;
    installPlanComponent = target;
    installPlanVersion = version;
    showInstallPlanModal = true;
  }

  async function handleInstallPlanInstalled(e: CustomEvent<string>) {
    showInstallPlanModal = false;
    // Refresh version lists + detection report.
    try {
      phpVersions = await listPhpVersions();
      detectionReport = await getDetectionReport();
      await settings.refresh();
    } catch (err) {
      console.warn('refresh after install failed', err);
    }
    alert(`✓ ${installPlanComponent === 'php' ? 'PHP' : 'nginx'} detected on your system. The hybrid resolver will pick it up automatically.`);
  }

  function fmtBytes(b: number): string {
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
    if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
    return `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  function actionLabel(action: PlannedAction): string {
    return ({
      use_system: 'Use system install',
      use_bundled: 'Use bundled binary',
      use_downloaded: 'Use previously downloaded',
      download: 'Will download',
      nothing: 'Already satisfied',
      skip: 'Not available for this OS'
    } as const)[action];
  }

  function actionColor(action: PlannedAction): string {
    return ({
      use_system: 'pill-success',
      use_bundled: 'pill-success',
      use_downloaded: 'pill-success',
      download: 'pill-warning',
      nothing: 'pill-neutral',
      skip: 'pill-danger'
    } as const)[action];
  }

  async function handleReset() {
    if (!confirm('Reset all settings to defaults? Existing configs will be regenerated. Services will need to be restarted.')) return;
    await settings.reset();
    detectionReport = await getDetectionReport();
  }
</script>

<svelte:head><title>Settings — InsophiniaForage</title></svelte:head>

<div class="page">
  <div class="page-header">
    <div>
      <h1 class="page-title">Settings</h1>
      <p class="page-subtitle">Changes regenerate config files. Restart services to apply.</p>
    </div>
    <button class="btn btn-secondary" on:click={handleReset}>Reset to defaults</button>
  </div>

  <div class="tabs">
    {#each [['general','General'],['php','PHP'],['php-versions','PHP Versions'],['mariadb','MariaDB'],['nginx','nginx'],['binaries','Binaries'],['detection','Detection']] as [id, label]}
      <button class="tab" class:active={activeTab === id} on:click={() => (activeTab = id as any)}>{label}</button>
    {/each}
  </div>

  {#if $settings.current}
    {#if activeTab === 'general'}
      <div class="card">
        <h3 class="card-title">General</h3>
        <div class="grid">
          <div class="field">
            <label class="label" for="p-http">nginx HTTP port</label>
            <input id="p-http" class="input" type="number" min="1024" max="65535"
              value={$settings.current.ports.nginx_http}
              on:change={(e) => updateField('ports.nginx_http', +e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="p-https">nginx HTTPS port</label>
            <input id="p-https" class="input" type="number" min="1024" max="65535"
              value={$settings.current.ports.nginx_https}
              on:change={(e) => updateField('ports.nginx_https', +e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="p-maria">MariaDB port</label>
            <input id="p-maria" class="input" type="number" min="1024" max="65535"
              value={$settings.current.ports.mariadb}
              on:change={(e) => updateField('ports.mariadb', +e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="theme">Theme</label>
            <select id="theme" class="input"
              value={$settings.current.theme}
              on:change={(e) => updateField('theme', e.currentTarget.value)}>
              <option value="auto">Auto</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>
          <div class="field">
            <label class="label" for="bs">Binary strategy</label>
            <select id="bs" class="input"
              value={$settings.current.binary_strategy}
              on:change={(e) => updateField('binary_strategy', e.currentTarget.value)}>
              <option value="auto">Auto (system → bundled → download)</option>
              <option value="prefer_system">Prefer system</option>
              <option value="prefer_bundled">Prefer bundled</option>
              <option value="prefer_download">Prefer downloaded</option>
            </select>
            <div class="hint">Controls which binary is spawned at runtime when multiple sources are available.</div>
          </div>
          <div class="field">
            <label class="label" for="cp">Conflict policy</label>
            <select id="cp" class="input"
              value={$settings.current.conflict_policy}
              on:change={(e) => handleConflictPolicyChange(e.currentTarget.value)}>
              <option value="coexist">Coexist (recommended) — install app's own copy, leave system alone</option>
              <option value="use_system">Use system — don't download, use what's installed</option>
              <option value="replace">Replace — uninstall system package (not recommended)</option>
            </select>
            <div class="hint">
              Controls what happens when a component is already installed on your system.
              See the Detection tab for what's currently detected.
            </div>
          </div>
          <div class="field">
            <label class="checkbox">
              <input type="checkbox" checked={$settings.current.auto_start_services}
                on:change={(e) => updateField('auto_start_services', e.currentTarget.checked)} />
              <span>Auto-start services on app launch</span>
            </label>
          </div>
        </div>
      </div>

    {:else if activeTab === 'php'}
      <div class="card">
        <h3 class="card-title">PHP</h3>
        <div class="grid">
          <div class="field">
            <VersionSelect
              label="PHP version"
              value={$settings.current.versions.php}
              available={phpOptions}
              placeholder="8.3"
              on:change={(e) => updateField('versions.php', e.detail)}
            />
          </div>
          <div class="field">
            <label class="label" for="php-mem">memory_limit</label>
            <input id="php-mem" class="input" type="text"
              value={$settings.current.php.memory_limit}
              on:change={(e) => updateField('php.memory_limit', e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="php-up">upload_max_filesize</label>
            <input id="php-up" class="input" type="text"
              value={$settings.current.php.upload_max_filesize}
              on:change={(e) => updateField('php.upload_max_filesize', e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="php-post">post_max_size</label>
            <input id="php-post" class="input" type="text"
              value={$settings.current.php.post_max_size}
              on:change={(e) => updateField('php.post_max_size', e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="php-time">max_execution_time (s)</label>
            <input id="php-time" class="input" type="number" min="0"
              value={$settings.current.php.max_execution_time}
              on:change={(e) => updateField('php.max_execution_time', +e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="checkbox">
              <input type="checkbox" checked={$settings.current.php.display_errors}
                on:change={(e) => updateField('php.display_errors', e.currentTarget.checked)} />
              <span>display_errors</span>
            </label>
          </div>
        </div>
      </div>

    {:else if activeTab === 'php-versions'}
      <div class="card">
        <h3 class="card-title">PHP versions</h3>
        <p class="section-help">
          Install multiple PHP versions side-by-side. Each project can then
          pick its PHP version from the Projects tab. The default version
          (shown in the PHP tab) is used by projects without an override.
        </p>
        {#if phpVersions.length === 0}
          <div class="empty">Loading installed PHP versions…</div>
        {:else}
          <table class="bin-table">
            <thead>
              <tr>
                <th>Version</th>
                <th>Status</th>
                <th>Source</th>
                <th>Path</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {#each phpVersions as v}
                <tr>
                  <td><strong>PHP {v.version}</strong></td>
                  <td>
                    {#if v.installed}
                      <span class="pill pill-success">Installed</span>
                    {:else}
                      <span class="pill pill-neutral">Not installed</span>
                    {/if}
                  </td>
                  <td>{v.source || '—'}</td>
                  <td class="path">{v.path || '—'}</td>
                  <td>
                    {#if !v.installed}
                      <button
                        class="btn btn-primary btn-sm"
                        on:click={() => handleInstallPhp(v.version)}
                        disabled={installingPhp === v.version}
                      >
                        {installingPhp === v.version ? 'Installing…' : 'Install'}
                      </button>
                    {:else}
                      <span class="muted">—</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}

        <div class="pm-install-section">
          <h4 class="subsection">Install via package manager (recommended)</h4>
          <p class="section-help">
            Install PHP/nginx from <strong>official package manager sources</strong>:
            Homebrew on macOS, Ondřej Surý's PPA on Debian/Ubuntu (he's the
            official Debian PHP maintainer), Remi Collet's repo on Fedora
            (he's the official Fedora PHP maintainer), and nginx.org's apt/dnf
            repo for nginx. The app shows you the exact commands to run in
            your terminal — it never runs <code>sudo</code> itself.
          </p>

          <div class="compile-buttons">
            <button class="btn btn-primary" on:click={() => handleInstallViaPackageManager('php')}>
              📦 Install PHP via package manager
            </button>
            <button class="btn btn-primary" on:click={() => handleInstallViaPackageManager('nginx')}>
              📦 Install nginx via package manager
            </button>
          </div>

          <details class="pm-details">
            <summary>Which source will be used on my OS?</summary>
            <table class="pm-table">
              <thead>
                <tr><th>OS</th><th>PHP source</th><th>nginx source</th></tr>
              </thead>
              <tbody>
                <tr>
                  <td>macOS</td>
                  <td><code>brew install php@X.Y</code> — Homebrew official</td>
                  <td><code>brew install nginx</code> — Homebrew official</td>
                </tr>
                <tr>
                  <td>Debian/Ubuntu/Mint</td>
                  <td><code>packages.sury.org/php</code> — Ondřej Surý (official Debian PHP maintainer)</td>
                  <td><code>nginx.org/packages/ubuntu</code> — official nginx.org apt repo</td>
                </tr>
                <tr>
                  <td>Fedora/RHEL</td>
                  <td><code>rpms.remirepo.net</code> — Remi Collet (official Fedora PHP maintainer)</td>
                  <td><code>nginx.org/packages/centos</code> — official nginx.org yum repo</td>
                </tr>
              </tbody>
            </table>
          </details>
        </div>

        {#if sourceCompileSupported}
          <div class="source-compile-section">
            <h4 class="subsection">Compile from source (Linux/macOS only)</h4>
            <p class="section-help">
              Download the official PHP/nginx source tarball and compile it
              locally. The build installs to <code>~/.insophinia-forage/bin/</code>
              — no root needed for the build itself. You only need root once
              to install build dependencies (gcc, libxml2-dev, etc.).
            </p>

            <div class="compile-buttons">
              <button class="btn btn-secondary" on:click={() => handleCompileFromSource('php')}>
                ⚙ Compile PHP from source
              </button>
              <button class="btn btn-secondary" on:click={() => handleCompileFromSource('nginx')}>
                ⚙ Compile nginx from source
              </button>
              <button class="btn btn-ghost" on:click={handleCheckBuildDeps} disabled={buildDepsLoading}>
                {buildDepsLoading ? 'Checking…' : 'Check build deps'}
              </button>
            </div>

            {#if buildDepsReport}
              <div class="alert alert-{buildDepsReport.all_present ? 'success' : 'warning'}">
                {#if buildDepsReport.all_present}
                  <strong>✓ All build dependencies are installed.</strong>
                  You can compile from source right away.
                {:else}
                  <strong>Missing build dependencies.</strong>
                  Run this command in your terminal (requires <code>sudo</code>):
                  <pre class="install-cmd">{buildDepsReport.install_command}</pre>
                  <button class="btn btn-secondary btn-sm" on:click={handleCopyInstallCommand}>
                    Copy command
                  </button>
                  <div class="missing-list">
                    <strong>Missing:</strong>
                    {[...buildDepsReport.missing_tools, ...buildDepsReport.missing_libs].join(', ')}
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      </div>

    {:else if activeTab === 'mariadb'}
      <div class="card">
        <h3 class="card-title">MariaDB</h3>
        <div class="grid">
          <div class="field">
            <VersionSelect
              label="MariaDB version"
              value={$settings.current.versions.mariadb}
              available={mariadbOptions}
              placeholder="11.4"
              on:change={(e) => updateField('versions.mariadb', e.detail)}
            />
          </div>
          <div class="field">
            <label class="label" for="m-buf">innodb_buffer_pool_size</label>
            <input id="m-buf" class="input" type="text"
              value={$settings.current.mariadb.innodb_buffer_pool_size}
              on:change={(e) => updateField('mariadb.innodb_buffer_pool_size', e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="m-conn">max_connections</label>
            <input id="m-conn" class="input" type="number" min="1"
              value={$settings.current.mariadb.max_connections}
              on:change={(e) => updateField('mariadb.max_connections', +e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="m-user">App user</label>
            <input id="m-user" class="input" type="text"
              value={$settings.current.mariadb.app_user}
              on:change={(e) => updateField('mariadb.app_user', e.currentTarget.value)} />
          </div>
        </div>
        <div class="info-banner">
          The DB password is auto-generated and stored in <code>~/.insophinia-forage/settings.json</code>
          with 0600 permissions. Use the "Reset to defaults" button above to rotate it.
        </div>
      </div>

    {:else if activeTab === 'nginx'}
      <div class="card">
        <h3 class="card-title">nginx</h3>
        <div class="grid">
          <div class="field">
            <VersionSelect
              label="nginx version"
              value={$settings.current.versions.nginx}
              available={nginxOptions}
              placeholder="1.27"
              on:change={(e) => updateField('versions.nginx', e.detail)}
            />
          </div>
          <div class="field">
            <label class="label" for="n-cmb">client_max_body_size</label>
            <input id="n-cmb" class="input" type="text"
              value={$settings.current.nginx.client_max_body_size}
              on:change={(e) => updateField('nginx.client_max_body_size', e.currentTarget.value)} />
          </div>
          <div class="field">
            <label class="label" for="n-wp">worker_processes</label>
            <input id="n-wp" class="input" type="text"
              value={$settings.current.nginx.worker_processes}
              on:change={(e) => updateField('nginx.worker_processes', e.currentTarget.value)} />
          </div>
        </div>
      </div>

    {:else if activeTab === 'binaries'}
      <div class="card">
        <h3 class="card-title">Binaries</h3>
        {#if $binaries.list.length === 0}
          <div class="empty">No binaries resolved yet. Run the wizard or click refresh.</div>
        {:else}
          <table class="bin-table">
            <thead>
              <tr>
                <th>Component</th>
                <th>Status</th>
                <th>Source</th>
                <th>Version</th>
                <th>Path</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {#each $binaries.list as b}
                <tr>
                  <td><strong>{b.name}</strong></td>
                  <td>
                    {#if b.resolved}
                      <span class="pill pill-success">Resolved</span>
                    {:else}
                      <span class="pill pill-warning">Missing</span>
                    {/if}
                  </td>
                  <td>{b.source || '—'}</td>
                  <td>{b.version || '—'}</td>
                  <td class="path">{b.path || '—'}</td>
                  <td>
                    {#if !b.resolved && b.available_versions.length > 0}
                      <select class="input-sm" id="dl-{b.name}">
                        {#each b.available_versions as v}
                          <option value={v}>{v}</option>
                        {/each}
                      </select>
                      <button class="btn btn-secondary btn-sm"
                        on:click={() => {
                          const sel = document.getElementById('dl-' + b.name) as HTMLSelectElement;
                          handleDownload(b.name, sel.value);
                        }}>
                        Download
                      </button>
                    {:else}
                      <span class="muted">—</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
        <div class="actions">
          <button class="btn btn-secondary" on:click={() => binaries.refresh()}>Refresh</button>
        </div>
      </div>

    {:else if activeTab === 'detection'}
      <div class="card">
        <div class="card-head-row">
          <h3 class="card-title">Detection report</h3>
          <button class="btn btn-secondary" on:click={handleRefreshDetection}>Refresh</button>
        </div>
        <p class="section-help">
          What the app found on your system, what's bundled, and what it plans to do
          for each component. This drives the conflict resolution policy.
        </p>

        {#if detectionReport}
          {#if detectionReport.pending_downloads.length > 0}
            <div class="alert alert-warning">
              <strong>Pending downloads:</strong>
              {detectionReport.pending_downloads.join(', ')}
              (~{fmtBytes(detectionReport.estimated_download_bytes)})
            </div>
          {:else}
            <div class="alert alert-success">
              ✓ All components are already available — no downloads needed.
            </div>
          {/if}

          <table class="bin-table">
            <thead>
              <tr>
                <th>Component</th>
                <th>Planned action</th>
                <th>System</th>
                <th>Bundled</th>
                <th>Downloaded</th>
                <th>Recommended</th>
              </tr>
            </thead>
            <tbody>
              {#each detectionReport.rows as row}
                <tr>
                  <td><strong>{row.component}</strong></td>
                  <td>
                    <span class="pill {actionColor(row.action)}">
                      {actionLabel(row.action)}
                    </span>
                  </td>
                  <td class="path">
                    {#if row.system}
                      {row.system.path}<br/>
                      <span class="muted">v{row.system.version || '?'}</span>
                    {:else}
                      <span class="muted">—</span>
                    {/if}
                  </td>
                  <td class="path">
                    {#if row.bundled}
                      {row.bundled.path}<br/>
                      <span class="muted">v{row.bundled.version || '?'}</span>
                    {:else}
                      <span class="muted">—</span>
                    {/if}
                  </td>
                  <td class="path">
                    {#if row.downloaded}
                      {row.downloaded.path}<br/>
                      <span class="muted">v{row.downloaded.version || '?'}</span>
                    {:else}
                      <span class="muted">—</span>
                    {/if}
                  </td>
                  <td>{row.recommended_version}</td>
                </tr>
              {/each}
            </tbody>
          </table>

          <h4 class="subsection">Compatibility checks</h4>
          {#if compatIssues.length === 0}
            <div class="alert alert-success">✓ No compatibility issues.</div>
          {:else}
            {#each compatIssues as issue}
              <div class="alert alert-{issue.severity === 'error' ? 'error' : issue.severity === 'warning' ? 'warning' : 'info'}">
                <strong>{issue.component_a} {issue.version_a} ↔ {issue.component_b} {issue.version_b}</strong>
                {issue.message}
              </div>
            {/each}
          {/if}
        {:else}
          <div class="empty">Loading detection report…</div>
        {/if}
      </div>
    {/if}
  {:else}
    <div class="empty">Loading settings…</div>
  {/if}
</div>

{#if showBuildModal}
  <BuildProgressModal
    title={buildModalTitle}
    lines={buildLines}
    on:done={handleBuildDone}
    on:error={handleBuildError}
  />
{/if}

{#if showInstallPlanModal}
  <InstallPlanModal
    component={installPlanComponent}
    version={installPlanVersion}
    on:close={() => (showInstallPlanModal = false)}
    on:installed={handleInstallPlanInstalled}
  />
{/if}

<style>
  .page { display: flex; flex-direction: column; gap: 1rem; }
  .page-header { display: flex; align-items: flex-start; justify-content: space-between; }
  .page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
  .page-subtitle {
    font-size: 0.8125rem; color: rgb(var(--text-secondary)); margin: 0.25rem 0 0;
  }

  .tabs { display: flex; gap: 0.25rem; flex-wrap: wrap; }
  .tab {
    padding: 0.5rem 0.875rem;
    border-radius: 8px;
    font-size: 0.8125rem; font-weight: 500;
    color: rgb(var(--text-secondary));
  }
  .tab:hover { background: rgb(var(--bg-hover)); }
  .tab.active {
    background: rgb(var(--accent) / 0.12);
    color: rgb(var(--accent));
  }

  .card-title {
    font-size: 0.9375rem; font-weight: 600; margin: 0 0 1rem;
  }
  .section-help {
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    margin: 0 0 1rem; max-width: 60ch;
  }
  .card-head-row {
    display: flex; align-items: center; justify-content: space-between;
    gap: 0.75rem; margin-bottom: 0.5rem;
  }
  .card-head-row .card-title { margin: 0; }
  .subsection {
    font-size: 0.8125rem; font-weight: 600; margin: 1.5rem 0 0.5rem;
    color: rgb(var(--text-secondary));
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .alert {
    padding: 0.75rem 1rem; border-radius: var(--radius);
    font-size: 0.8125rem; margin-bottom: 0.5rem;
  }
  .alert strong { font-weight: 600; }
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
  .alert-info {
    background: rgb(var(--accent) / 0.06); color: rgb(var(--accent));
    border: 1px solid rgb(var(--accent) / 0.15);
  }
  .hint {
    font-size: 0.6875rem; color: rgb(var(--text-tertiary));
    margin-top: 0.375rem; line-height: 1.4;
  }
  .source-compile-section {
    margin-top: 1.5rem;
    padding-top: 1.5rem;
    border-top: 1px solid rgb(var(--border));
  }
  .pm-install-section {
    margin-top: 1.5rem;
    padding-top: 1.5rem;
    border-top: 1px solid rgb(var(--border));
  }
  .pm-details {
    margin-top: 0.875rem;
    background: rgb(var(--bg-hover) / 0.4);
    border-radius: var(--radius);
    padding: 0.625rem 0.875rem;
  }
  .pm-details summary {
    cursor: pointer;
    font-size: 0.8125rem;
    font-weight: 500;
    color: rgb(var(--text-secondary));
  }
  .pm-details[open] summary {
    margin-bottom: 0.625rem;
  }
  .pm-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.75rem;
  }
  .pm-table th {
    text-align: left;
    padding: 0.375rem 0.5rem;
    color: rgb(var(--text-tertiary));
    font-weight: 600;
    border-bottom: 1px solid rgb(var(--border));
  }
  .pm-table td {
    padding: 0.5rem 0.5rem;
    border-bottom: 1px solid rgb(var(--border));
    color: rgb(var(--text-secondary));
    vertical-align: top;
  }
  .pm-table td:first-child {
    font-weight: 600;
    color: rgb(var(--text-primary));
    white-space: nowrap;
  }
  .compile-buttons {
    display: flex; gap: 0.5rem; flex-wrap: wrap; margin: 0.75rem 0;
  }
  .install-cmd {
    margin: 0.5rem 0;
    padding: 0.625rem 0.75rem;
    background: rgb(0 0 0 / 0.85);
    color: rgb(220 220 220);
    border-radius: 6px;
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .missing-list {
    margin-top: 0.5rem;
    font-size: 0.75rem;
    color: rgb(var(--text-secondary));
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
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.875rem;
  }

  .checkbox {
    display: flex; align-items: center; gap: 0.5rem;
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    cursor: pointer;
    padding-top: 1.5rem;
  }

  .info-banner {
    margin-top: 1rem;
    padding: 0.75rem 1rem;
    background: rgb(var(--accent) / 0.06);
    border-radius: var(--radius);
    font-size: 0.75rem;
    color: rgb(var(--text-secondary));
  }
  code {
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    background: rgb(var(--bg-hover));
    padding: 0.0625rem 0.25rem;
    border-radius: 4px;
  }

  .bin-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8125rem;
  }
  .bin-table th {
    text-align: left;
    font-size: 0.6875rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: rgb(var(--text-tertiary));
    padding: 0.5rem 0.5rem;
    border-bottom: 1px solid rgb(var(--border));
  }
  .bin-table td {
    padding: 0.625rem 0.5rem;
    border-bottom: 1px solid rgb(var(--border));
    vertical-align: middle;
  }
  .bin-table td.path {
    font-family: theme('fontFamily.mono');
    font-size: 0.6875rem;
    color: rgb(var(--text-tertiary));
    max-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .input-sm {
    width: auto;
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
    display: inline-block;
    margin-right: 0.25rem;
  }
  .btn-sm { padding: 0.25rem 0.625rem; font-size: 0.75rem; }
  .muted { color: rgb(var(--text-tertiary)); }
  .actions { margin-top: 1rem; display: flex; justify-content: flex-end; }

  .empty {
    padding: 2rem 1rem; text-align: center;
    color: rgb(var(--text-tertiary)); font-size: 0.8125rem;
  }
</style>
