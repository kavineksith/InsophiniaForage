<script lang="ts">
  import { onMount } from 'svelte';
  import ServiceCard from '$lib/components/ServiceCard.svelte';
  import { services, settings } from '$lib/stores';

  onMount(() => {
    services.refresh();
  });

  $: settings_ = $settings.current;
  $: anyRunning = $services.list.some((s) => s.running);
</script>

<svelte:head><title>Dashboard — InsophiniaForage</title></svelte:head>

<div class="page">
  <div class="page-header">
    <div>
      <h1 class="page-title">Dashboard</h1>
      <p class="page-subtitle">
        {#if anyRunning}
          {$services.list.filter((s) => s.running).length} of {$services.list.length} services running.
        {:else}
          All services are stopped. Start them below or use the tray menu.
        {/if}
      </p>
    </div>
    <div class="header-actions">
      <button class="btn btn-primary" on:click={() => services.startAll()} disabled={$services.loading || anyRunning}>
        ▶ Start all
      </button>
      <button class="btn btn-secondary" on:click={() => services.stopAll()} disabled={$services.loading || !anyRunning}>
        ■ Stop all
      </button>
    </div>
  </div>

  {#if $services.error}
    <div class="alert alert-error">{$services.error}</div>
  {/if}

  <div class="services-grid">
    {#each $services.list as svc (svc.name)}
      <ServiceCard status={svc} />
    {:else}
      <div class="empty">
        <div class="empty-icon">▸</div>
        <div class="empty-text">No services yet. Click <strong>Start all</strong> to begin.</div>
      </div>
    {/each}
  </div>

  {#if settings_}
    <div class="quick-info">
      <div class="info-card">
        <div class="info-label">Web server</div>
        <div class="info-value">http://&lt;project&gt;:{settings_.ports.nginx_http}</div>
      </div>
      <div class="info-card">
        <div class="info-label">Database</div>
        <div class="info-value">127.0.0.1:{settings_.ports.mariadb}</div>
      </div>
      <div class="info-card">
        <div class="info-label">PHP version</div>
        <div class="info-value">{settings_.versions.php}</div>
      </div>
      <div class="info-card">
        <div class="info-label">MariaDB version</div>
        <div class="info-value">{settings_.versions.mariadb}</div>
      </div>
    </div>
  {/if}
</div>

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

  .alert {
    padding: 0.75rem 1rem; border-radius: var(--radius);
    font-size: 0.8125rem; font-weight: 500;
  }
  .alert-error {
    background: rgb(var(--danger) / 0.1); color: rgb(var(--danger));
    border: 1px solid rgb(var(--danger) / 0.2);
  }

  .services-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 1rem;
  }

  .empty {
    grid-column: 1 / -1;
    padding: 3rem 1rem;
    text-align: center;
    color: rgb(var(--text-tertiary));
  }
  .empty-icon { font-size: 2rem; margin-bottom: 0.5rem; }

  .quick-info {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.75rem;
  }
  .info-card {
    padding: 0.875rem 1rem;
    background: rgb(var(--bg-card));
    border-radius: var(--radius);
    border: 1px solid rgb(var(--border));
  }
  .info-label {
    font-size: 0.6875rem; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.04em;
    color: rgb(var(--text-tertiary)); margin-bottom: 0.25rem;
  }
  .info-value {
    font-family: theme('fontFamily.mono');
    font-size: 0.8125rem; color: rgb(var(--text-primary));
  }
</style>
