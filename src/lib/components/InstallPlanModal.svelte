<script lang="ts">
  // Modal that shows a package-manager install plan: ordered list of
  // shell commands, with copy buttons, sudo warnings, and a "I've run
  // these — verify" button that pings the backend to check if the
  // binary is now on PATH.

  import { createEventDispatcher } from 'svelte';
  import { getInstallPlan, verifySystemInstall } from '$lib/api';
  import type { InstallPlan } from '$lib/types';

  export let component: 'php' | 'nginx';
  export let version: string;

  const dispatch = createEventDispatcher<{ close: void; installed: string }>();

  let plan: InstallPlan | null = null;
  let loading = true;
  let error = '';
  let verifying = false;
  let verifyResult: 'pending' | 'success' | 'not-yet' = 'pending';

  async function load() {
    loading = true;
    error = '';
    try {
      plan = await getInstallPlan(component, version);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function copyAll() {
    if (!plan) return;
    const text = plan.steps.map((s) => s.command).join('\n');
    try {
      await navigator.clipboard.writeText(text);
      alert('All commands copied to clipboard. Paste into your terminal and run them in order.');
    } catch {
      prompt('Copy these commands:', text);
    }
  }

  async function copyOne(cmd: string) {
    try {
      await navigator.clipboard.writeText(cmd);
    } catch {
      prompt('Copy this command:', cmd);
    }
  }

  async function handleVerify() {
    if (!plan) return;
    verifying = true;
    verifyResult = 'pending';
    try {
      const path = await verifySystemInstall(component, version);
      if (path) {
        verifyResult = 'success';
        setTimeout(() => dispatch('installed', path), 1500);
      } else {
        verifyResult = 'not-yet';
      }
    } catch (e) {
      verifyResult = 'not-yet';
    } finally {
      verifying = false;
    }
  }

  load();
</script>

<div class="modal-overlay" on:click|self={() => dispatch('close')}>
  <div class="modal">
    <div class="modal-header">
      <h2>Install {component === 'php' ? 'PHP' : 'nginx'} {version} via package manager</h2>
      <button class="btn btn-ghost" on:click={() => dispatch('close')}>✕</button>
    </div>

    {#if loading}
      <div class="empty">Generating install plan…</div>
    {:else if error}
      <div class="alert alert-error">{error}</div>
    {:else if plan}
      <p class="intro">
        Using <strong>{plan.package_manager}</strong> on your system. Run these commands
        in your terminal, in order. The app never runs <code>sudo</code> — you do.
      </p>

      {#if plan.notes.length > 0}
        <div class="notes">
          {#each plan.notes as note}
            <div class="note">ℹ {note}</div>
          {/each}
        </div>
      {/if}

      <div class="steps">
        {#each plan.steps as step, i (i)}
          <div class="step">
            <div class="step-header">
              <span class="step-num">{i + 1}</span>
              <span class="step-desc">{step.description}</span>
              {#if step.requires_sudo}
                <span class="sudo-pill">sudo</span>
              {/if}
            </div>
            <div class="cmd-row">
              <pre class="cmd">{step.command}</pre>
              <button class="btn btn-ghost btn-sm" on:click={() => copyOne(step.command)} title="Copy">
                ⧉
              </button>
            </div>
          </div>
        {/each}
      </div>

      <div class="actions">
        <button class="btn btn-secondary" on:click={copyAll}>
          ⧉ Copy all commands
        </button>
        <div class="spacer"></div>
        {#if verifyResult === 'success'}
          <span class="pill pill-success">✓ Verified</span>
        {:else if verifyResult === 'not-yet'}
          <span class="pill pill-warning">Not yet — try again</span>
        {/if}
        <button class="btn btn-primary" on:click={handleVerify} disabled={verifying}>
          {verifying ? 'Checking…' : "I've run these — verify"}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed; inset: 0; z-index: 60;
    background: rgba(0, 0, 0, 0.5);
    display: flex; align-items: center; justify-content: center;
    padding: 1.5rem;
  }
  .modal {
    width: 100%; max-width: 760px; max-height: 85vh;
    background: rgb(var(--bg-card));
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-popover);
    padding: 1.25rem;
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .modal-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 0.75rem;
  }
  .modal-header h2 {
    font-size: 1.0625rem; font-weight: 600; margin: 0;
  }

  .intro {
    font-size: 0.8125rem; color: rgb(var(--text-secondary));
    margin: 0 0 0.75rem;
  }
  code {
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    background: rgb(var(--bg-hover));
    padding: 0.0625rem 0.25rem;
    border-radius: 4px;
  }

  .notes {
    background: rgb(var(--accent) / 0.06);
    border-radius: var(--radius);
    padding: 0.625rem 0.75rem;
    margin-bottom: 0.75rem;
    max-height: 120px;
    overflow-y: auto;
  }
  .note {
    font-size: 0.75rem; color: rgb(var(--text-secondary));
    margin-bottom: 0.25rem;
  }

  .steps {
    flex: 1;
    overflow-y: auto;
    padding-right: 0.25rem;
  }
  .step {
    margin-bottom: 0.75rem;
    padding: 0.625rem 0.75rem;
    background: rgb(var(--bg-hover) / 0.4);
    border-radius: var(--radius);
  }
  .step-header {
    display: flex; align-items: center; gap: 0.5rem;
    margin-bottom: 0.375rem;
  }
  .step-num {
    width: 20px; height: 20px;
    border-radius: 50%;
    background: rgb(var(--accent));
    color: white;
    display: flex; align-items: center; justify-content: center;
    font-size: 0.6875rem; font-weight: 700;
    flex-shrink: 0;
  }
  .step-desc {
    flex: 1;
    font-size: 0.8125rem;
    color: rgb(var(--text-primary));
  }
  .sudo-pill {
    background: rgb(var(--warning) / 0.15);
    color: rgb(var(--warning));
    font-size: 0.625rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.0625rem 0.375rem;
    border-radius: 4px;
  }
  .cmd-row {
    display: flex; gap: 0.25rem;
    align-items: stretch;
  }
  .cmd {
    flex: 1;
    margin: 0;
    padding: 0.5rem 0.625rem;
    background: rgb(0 0 0 / 0.88);
    color: rgb(220 220 220);
    border-radius: 6px;
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .btn-sm {
    padding: 0.25rem 0.5rem;
    font-size: 0.875rem;
  }

  .actions {
    display: flex; align-items: center; gap: 0.5rem;
    margin-top: 0.875rem;
    padding-top: 0.875rem;
    border-top: 1px solid rgb(var(--border));
  }
  .spacer { flex: 1; }

  .alert-error {
    padding: 0.75rem 1rem; border-radius: var(--radius);
    background: rgb(var(--danger) / 0.1); color: rgb(var(--danger));
    border: 1px solid rgb(var(--danger) / 0.2);
    font-size: 0.8125rem;
  }
  .empty {
    text-align: center; padding: 2rem; color: rgb(var(--text-tertiary));
  }
</style>
