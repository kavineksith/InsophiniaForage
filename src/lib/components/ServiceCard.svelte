<script lang="ts">
  import type { ServiceStatus } from '$lib/types';
  import { services } from '$lib/stores';

  export let status: ServiceStatus;

  $: uptime = formatUptime(status.uptime_seconds);

  function formatUptime(s: number): string {
    if (s < 60) return `${s}s`;
    if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
    if (s < 86400) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
    return `${Math.floor(s / 86400)}d ${Math.floor((s % 86400) / 3600)}h`;
  }

  async function toggle() {
    if (status.running) {
      await services.stop(status.name);
    } else {
      await services.start(status.name);
    }
  }
</script>

<div class="card service-card">
  <div class="header">
    <div class="title-row">
      <span class="dot" class:on={status.running}></span>
      <h3 class="name">{status.name}</h3>
      {#if status.running}
        <span class="pill pill-success">Running</span>
      {:else}
        <span class="pill pill-neutral">Stopped</span>
      {/if}
    </div>
    <button
      class="btn"
      class:btn-primary={!status.running}
      class:btn-secondary={status.running}
      on:click={toggle}
      disabled={$services.loading}
    >
      {#if status.running}■ Stop{:else}▶ Start{/if}
    </button>
  </div>

  <div class="meta">
    {#if status.running}
      <div class="meta-row">
        <span class="meta-label">PID</span>
        <span class="mono">{status.pid}</span>
      </div>
      <div class="meta-row">
        <span class="meta-label">Uptime</span>
        <span>{uptime}</span>
      </div>
    {:else if status.exit_code !== null}
      <div class="meta-row">
        <span class="meta-label">Exit</span>
        <span class="mono">{status.exit_code}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .service-card {
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
  }
  .header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .name {
    font-size: 0.9375rem;
    font-weight: 600;
    margin: 0;
    text-transform: capitalize;
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding-top: 0.75rem;
    border-top: 1px solid rgb(var(--border));
  }
  .meta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.75rem;
  }
  .meta-label {
    color: rgb(var(--text-tertiary));
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: 600;
  }
  .mono {
    font-family: theme('fontFamily.mono');
    color: rgb(var(--text-primary));
  }
</style>
