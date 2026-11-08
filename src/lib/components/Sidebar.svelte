<script lang="ts">
  import { page } from '$app/stores';
  import { services } from '$lib/stores';

  const navItems = [
    { href: '/', label: 'Dashboard', icon: 'home' },
    { href: '/projects', label: 'Projects', icon: 'folder' },
    { href: '/databases', label: 'Databases', icon: 'database' },
    { href: '/wordpress', label: 'WordPress', icon: 'wp' },
    { href: '/logs', label: 'Logs', icon: 'log' },
    { href: '/settings', label: 'Settings', icon: 'settings' }
  ];

  $: runningCount = $services.list.filter((s) => s.running).length;
</script>

<aside class="sidebar">
  <div class="brand">
    <div class="logo">IF</div>
    <div class="brand-text">
      <div class="brand-name">InsophiniaForage</div>
      <div class="brand-sub">v0.1.0</div>
    </div>
  </div>

  <nav>
    {#each navItems as item}
      <a
        href={item.href}
        class="nav-item"
        class:active={$page.url.pathname === item.href}
      >
        <span class="nav-icon">
          {#if item.icon === 'home'}⌂{:else if item.icon === 'folder'}▸{:else if item.icon === 'database'}◴{:else if item.icon === 'wp'}W{:else if item.icon === 'log'}≡{:else if item.icon === 'settings'}⚙{/if}
        </span>
        <span class="nav-label">{item.label}</span>
        {#if item.href === '/' && runningCount > 0}
          <span class="nav-badge">{runningCount}</span>
        {/if}
      </a>
    {/each}
  </nav>

  <div class="footer">
    <div class="status-pill" class:on={runningCount > 0}>
      <span class="dot" class:on={runningCount > 0}></span>
      {runningCount} service{runningCount === 1 ? '' : 's'} running
    </div>
    <div class="experimental-badge" title="This is experimental software. Use at your own risk. Not for production.">
      ⚠ Experimental
    </div>
  </div>
</aside>

<style>
  .sidebar {
    background: rgb(var(--bg-sidebar));
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border-right: 1px solid rgb(var(--border));
    display: flex;
    flex-direction: column;
    padding: 1rem 0.75rem;
    user-select: none;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding: 0.5rem 0.5rem 1rem;
  }
  .logo {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: linear-gradient(135deg, rgb(var(--accent)), rgb(var(--accent-hover)));
    color: white;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 0.875rem;
    letter-spacing: -0.02em;
    box-shadow: 0 2px 8px rgba(0, 122, 255, 0.3);
  }
  .brand-text { line-height: 1.1; }
  .brand-name {
    font-size: 0.875rem;
    font-weight: 600;
    color: rgb(var(--text-primary));
  }
  .brand-sub {
    font-size: 0.6875rem;
    color: rgb(var(--text-tertiary));
    margin-top: 1px;
  }

  nav {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 0.5rem;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    font-size: 0.8125rem;
    font-weight: 500;
    color: rgb(var(--text-secondary));
    text-decoration: none;
    transition: background 0.1s, color 0.1s;
  }
  .nav-item:hover {
    background: rgb(var(--bg-hover));
    color: rgb(var(--text-primary));
    text-decoration: none;
  }
  .nav-item.active {
    background: rgb(var(--accent) / 0.12);
    color: rgb(var(--accent));
  }
  .nav-icon {
    width: 18px;
    text-align: center;
    font-size: 0.9375rem;
    opacity: 0.8;
  }
  .nav-badge {
    margin-left: auto;
    background: rgb(var(--success) / 0.15);
    color: rgb(var(--success));
    font-size: 0.6875rem;
    font-weight: 700;
    padding: 0 0.375rem;
    border-radius: 999px;
    min-width: 18px;
    text-align: center;
  }

  .footer {
    padding-top: 0.75rem;
    border-top: 1px solid rgb(var(--border));
  }
  .status-pill {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    font-size: 0.75rem;
    color: rgb(var(--text-secondary));
    background: rgb(var(--bg-hover) / 0.5);
  }
  .status-pill.on {
    color: rgb(var(--success));
  }
  .experimental-badge {
    margin-top: 0.5rem;
    padding: 0.25rem 0.5rem;
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: rgb(var(--warning));
    background: rgb(var(--warning) / 0.1);
    border-radius: 6px;
    text-align: center;
    cursor: help;
  }
</style>
