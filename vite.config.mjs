import { defineConfig } from 'vite';

export default defineConfig({
  server: { port: 1420, strictPort: true },
  preview: { port: 1420, strictPort: true },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  resolve: { preserveSymlinks: true },
  esbuild: { jsx: 'automatic' },
  optimizeDeps: { esbuildOptions: { preserveSymlinks: true } },
});
