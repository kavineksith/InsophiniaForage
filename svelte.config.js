import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Tauri uses a static front-end; SPA-style fallback so all routes resolve to the shell.
    adapter: adapter({
      fallback: 'index.html',
      strict: false
    }),
    alias: {
      $lib: 'src/lib'
    }
  }
};

export default config;
