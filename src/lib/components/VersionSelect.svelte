<script lang="ts">
  // Reusable version dropdown. Shows a list of versions fetched from
  // official sources, plus a "Custom..." option that reveals a text
  // input for typing any version.
  //
  // Usage:
  //   <VersionSelect
  //     value={input.versions.php}
  //     on:change={(e) => (input.versions.php = e.detail)}
  //     available={availableVersions?.php ?? ['8.3', '8.2', '8.1']}
  //     placeholder="8.3"
  //   />

  import { createEventDispatcher } from 'svelte';

  export let value: string;
  export let available: string[] = [];
  export let placeholder = '';
  export let label = '';

  const dispatch = createEventDispatcher<{ change: string }>();

  let isCustom = false;
  let customValue = '';

  // Initialize: if `value` is in the list, use select mode. Otherwise,
  // switch to custom mode and populate the text input.
  $: if (available.length > 0) {
    if (value && !available.includes(value) && value !== 'latest') {
      isCustom = true;
      customValue = value;
    }
  }

  function handleSelectChange(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (v === '__custom__') {
      isCustom = true;
      customValue = value;
      // Don't dispatch yet — wait for the user to type.
    } else {
      isCustom = false;
      value = v;
      dispatch('change', v);
    }
  }

  function handleCustomChange(e: Event) {
    const v = (e.currentTarget as HTMLInputElement).value;
    customValue = v;
    value = v;
    dispatch('change', v);
  }

  function backToList() {
    isCustom = false;
    // Revert to the first available version.
    if (available.length > 0) {
      value = available[0];
      dispatch('change', value);
    }
  }
</script>

{#if label}
  <label class="label">{label}</label>
{/if}

{#if isCustom}
  <div class="custom-row">
    <input
      class="input"
      type="text"
      bind:value={customValue}
      on:input={handleCustomChange}
      placeholder={placeholder}
      spellcheck="false"
    />
    <button class="btn btn-ghost btn-back" on:click={backToList} title="Back to list">
      ↩
    </button>
  </div>
{:else}
  <select class="input" value={value} on:change={handleSelectChange}>
    {#each available as v}
      <option value={v}>{v}</option>
    {/each}
    <option value="__custom__">Custom…</option>
  </select>
{/if}

<style>
  .custom-row {
    display: flex;
    gap: 0.375rem;
  }
  .custom-row .input {
    flex: 1;
  }
  .btn-back {
    padding: 0.5rem 0.625rem;
    font-size: 0.875rem;
  }
  .label {
    display: block;
    font-size: 0.75rem;
    font-weight: 600;
    color: rgb(var(--text-secondary));
    margin-bottom: 0.25rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
