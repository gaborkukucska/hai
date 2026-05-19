// <!-- # START OF FILE hainet-portal/vite.config.ts -->
// Vite configuration for the HAI-Net Portal (headless web UI).
//
// In development: Vite runs on port 5173 and proxies /api/* to hainet-core on port 8080.
// In production: `npm run build` outputs static assets to dist/, which hainet-core
// embeds via rust-embed and serves directly on port 8080 (single port).

import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],

  // Prevent vite from obscuring rust errors
  clearScreen: false,

  // Development server settings
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      // Ignore Rust build artifacts during development
      ignored: ['**/target/**'],
    },
    // Proxy API calls to the hainet-core daemon during development
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
      '/health': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
    }
  },

  // Production build — outputs to dist/ for rust-embed
  build: {
    target: 'es2020',
    minify: 'esbuild',
    sourcemap: false,
    rollupOptions: {
      external: [],
    },
  },
})
