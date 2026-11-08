<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import TopBar from '$lib/components/TopBar.svelte';
  import { isInitialized } from '$lib/api';
  import { services, settings, binaries } from '$lib/stores';
  import { projects as projectsStore } from '$lib/stores';

  let initialized: boolean | null = null;

  onMount(async () => {
    if (!browser) return;
    try {
      initialized = await isInitialized();
      if (!initialized) {
        goto('/wizard');
        return;
      }
      await Promise.all([
        settings.refresh(),
        binaries.refresh(),
        projectsStore.refresh()
      ]);
      services.startPolling();
    } catch (e) {
      console.error('init failed', e);
      initialized = true; // Show the shell anyway during dev.
    }
  });

  // Wizard route renders without the shell.
  $: isWizard = $page.url.pathname === '/wizard';
</script>

{#if isWizard}
  <slot />
{:else}
  <div class="app-shell">
    <Sidebar />
    <main class="main">
      <TopBar />
      <div class="content">
        <slot />
      </div>
    </main>
  </div>
{/if}

<style>
  .app-shell {
    display: grid;
    grid-template-columns: 240px 1fr;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
  .main {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem 2rem 3rem;
  }
  @media (max-width: 768px) {
    .app-shell {
      grid-template-columns: 64px 1fr;
    }
    .content {
      padding: 1rem;
    }
  }
</style>
