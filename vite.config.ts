import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Tauri expects a fixed dev server port (tauri.conf.json's `devUrl`).
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // `target/` is written and locked by cargo during rebuilds — watching
      // it causes EBUSY crashes in Vite's fs watcher on Windows. This is a
      // Cargo workspace, so the real build output is the repo-root
      // `target/`, not `src-tauri/target/` — both are ignored here.
      ignored: ['**/src-tauri/**', '**/target/**'],
    },
  },
})
