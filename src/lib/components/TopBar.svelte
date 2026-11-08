<script lang="ts">
  import { services } from '$lib/stores';

  let now = new Date();
  $: now = new Date();
  setInterval(() => (now = new Date()), 60000);

  function timeStr(d: Date) {
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }
</script>

<header class="topbar">
  <div class="left">
    <h1 class="title"><slot name="title" /></h1>
  </div>
  <div class="right">
    <slot name="actions" />
    <div class="clock">{timeStr(now)}</div>
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1.5rem;
    background: rgb(var(--bg-sidebar));
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border-bottom: 1px solid rgb(var(--border));
    min-height: 56px;
    user-select: none;
  }
  .left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .title {
    font-size: 1rem;
    font-weight: 600;
    margin: 0;
    color: rgb(var(--text-primary));
  }
  .right {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .clock {
    font-size: 0.75rem;
    color: rgb(var(--text-tertiary));
    font-variant-numeric: tabular-nums;
  }
</style>
