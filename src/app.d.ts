// See https://kit.svelte.dev/docs/types#app
import type { invoke } from '@tauri-apps/api/core';

declare global {
  // Type alias for the Tauri invoke function so we can use it in stores.
  type InvokeFn = typeof invoke;
}

export {};
