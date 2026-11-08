<script lang="ts">
  // Build progress modal. Shows live stdout/stderr from a source compile
  // (PHP or nginx) as it runs. The build runs in the background — the
  // user can dismiss the modal and the build keeps going. When it
  // finishes, we dispatch a `done` event with the install path.

  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import { onBuildProgress } from '$lib/api';
  import type { UnlistenFn } from '@tauri-apps/api/event';

  export let title = 'Compiling…';
  export let lines: { stage: string; line: string }[] = [];

  const dispatch = createEventDispatcher<{ done: string; error: string }>();

  let unlisten: UnlistenFn | null = null;
  let dismissed = false;
  let autoScroll = true;

  const MAX_LINES = 2000;

  onMount(async () => {
    unlisten = await onBuildProgress((p) => {
      lines = [...lines, p];
      if (lines.length > MAX_LINES) {
        lines = lines.slice(lines.length - MAX_LINES);
      }
      if (p.stage === 'done') {
        // The build succeeded — dispatch the path.
        dispatch('done', p.line);
      }
      if (autoScroll) scrollToBottom();
    });
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  function scrollToBottom() {
    requestAnimationFrame(() => {
      const el = document.getElementById('build-log-scroll');
      if (el) el.scrollTop = el.scrollHeight;
    });
  }

  function dismiss() {
    dismissed = true;
    dispatch('error', 'User dismissed the build dialog. The build continues in the background.');
  }

  $: lastLine = lines.length > 0 ? lines[lines.length - 1].line : '';
  $: currentStage = lines.length > 0 ? lines[lines.length - 1].stage : 'init';
</script>

<div class="modal-overlay" on:click|self={dismiss}>
  <div class="modal">
    <div class="modal-header">
      <h2>{title}</h2>
      <span class="stage-pill stage-{currentStage}">{currentStage}</span>
    </div>

    <div class="current-line" title={lastLine}>
      {lastLine || 'Starting…'}
    </div>

    <div class="log-window" id="build-log-scroll">
      {#each lines as l, i (i + ':' + l.line.slice(0, 20))}
        <div class="log-line">
          <span class="line-stage">{l.stage}</span>
          <span class="line-content">{l.line}</span>
        </div>
      {/each}
    </div>

    <div class="modal-actions">
      <label class="checkbox">
        <input type="checkbox" bind:checked={autoScroll} />
        <span>Auto-scroll</span>
      </label>
      <div class="spacer"></div>
      <button class="btn btn-ghost" on:click={dismiss}>
        Hide (build continues in background)
      </button>
    </div>
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
    width: 100%; max-width: 720px; height: 80vh;
    background: rgb(var(--bg-card));
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-popover);
    padding: 1.25rem;
    display: flex; flex-direction: column;
  }
  .modal-header {
    display: flex; align-items: center; justify-content: space-between;
    gap: 0.75rem; margin-bottom: 0.75rem;
  }
  .modal-header h2 {
    font-size: 1.0625rem; font-weight: 600; margin: 0;
  }
  .stage-pill {
    padding: 0.125rem 0.5rem;
    border-radius: 999px;
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: rgb(var(--text-tertiary) / 0.15);
    color: rgb(var(--text-secondary));
  }
  .stage-pill.stage-done {
    background: rgb(var(--success) / 0.15);
    color: rgb(var(--success));
  }
  .stage-pill.stage-make {
    background: rgb(var(--warning) / 0.15);
    color: rgb(var(--warning));
  }

  .current-line {
    font-family: theme('fontFamily.mono');
    font-size: 0.75rem;
    color: rgb(var(--text-secondary));
    background: rgb(var(--bg-hover) / 0.5);
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    margin-bottom: 0.75rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .log-window {
    flex: 1;
    background: rgb(0 0 0 / 0.92);
    color: rgb(220 220 220);
    border-radius: var(--radius);
    padding: 0.75rem;
    overflow-y: auto;
    font-family: theme('fontFamily.mono');
    font-size: 0.6875rem;
    line-height: 1.5;
  }
  .log-line {
    display: flex; gap: 0.5rem;
    padding: 0.0625rem 0;
  }
  .line-stage {
    color: rgba(255, 255, 255, 0.4);
    width: 5rem;
    flex-shrink: 0;
    text-transform: uppercase;
    font-size: 0.625rem;
    font-weight: 600;
    padding-top: 0.0625rem;
  }
  .line-content {
    white-space: pre-wrap;
    word-break: break-word;
    flex: 1;
  }

  .modal-actions {
    display: flex; align-items: center; gap: 0.75rem;
    margin-top: 0.75rem;
  }
  .spacer { flex: 1; }
  .checkbox {
    display: flex; align-items: center; gap: 0.375rem;
    font-size: 0.75rem; color: rgb(var(--text-secondary));
    cursor: pointer;
  }
</style>
