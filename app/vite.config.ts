import { readFileSync } from 'node:fs'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// tauri.conf.json is the one place the app version is set; the UI shows it.
const tauriConf = JSON.parse(readFileSync(new URL('./src-tauri/tauri.conf.json', import.meta.url), 'utf8'))

// Tauri expects a fixed dev port and keeps its own console output visible.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    __APP_VERSION__: JSON.stringify(tauriConf.version),
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Skill icons live in <repo>/assets, outside the app folder.
    fs: { allow: ['..'] },
  },
})
