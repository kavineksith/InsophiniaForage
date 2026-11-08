import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

// Tauri dev server runs on a fixed port so the Rust side can pick it up.
const host = process.env.TAURI_DEV_HOST || 'localhost';
const port = Number(process.env.TAURI_DEV_PORT) || 5173;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [sveltekit()],

  // Tauri requires a deterministic output dir.
  clearScreen: false,
  server: {
    host,
    port,
    strictPort: true,
    fs: {
      // Allow serving files from the workspace root.
      allow: ['..']
    }
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    // Produce smaller bundles — important for desktop app size.
    target: 'es2021',
    minify: 'esbuild',
    sourcemap: false
  }
});
