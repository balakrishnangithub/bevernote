import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '0.0.0.0',
    port: 7100,
    strictPort: false,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: 'es2022',
    sourcemap: true,
    chunkSizeWarningLimit: 900,
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    exclude: ['node_modules/**', 'dist/**', 'test-results/**'],
  },
});
