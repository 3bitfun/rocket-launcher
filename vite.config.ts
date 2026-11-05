import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  cacheDir: process.env.VITEST ? 'node_modules/.vite-tests' : 'node_modules/.vite-desktop',
  optimizeDeps: { include: ['react', 'react-dom/client', 'react/jsx-dev-runtime', 'motion/react', 'lucide-react', '@tauri-apps/api/core', '@tauri-apps/plugin-dialog'] },
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: { target: 'es2022' },
  test: { environment: 'jsdom', setupFiles: ['./src/test/setup.ts'], css: false },
});
