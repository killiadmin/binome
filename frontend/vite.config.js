import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// The dev server is the single origin the browser talks to. Every device on the
// LAN loads the app from http://<host-ip>:5173 and all API / WebSocket traffic
// is proxied from here to the backend & Reverb containers by Docker service
// name — so the host's LAN IP never has to appear in any frontend config or
// .env, and switching networks needs no regeneration.
const BACKEND = process.env.VITE_PROXY_BACKEND || 'http://backend:8000'
const REVERB = process.env.VITE_PROXY_REVERB || 'http://reverb:8080'

export default defineConfig({
  plugins: [vue()],
  server: {
    host: '0.0.0.0',
    port: 5173,
    proxy: {
      '/api': { target: BACKEND, changeOrigin: true },
      '/broadcasting': { target: BACKEND, changeOrigin: true },
      // Reverb's client WebSocket endpoint (pusher protocol): /app/{key}
      '/app': { target: REVERB, ws: true, changeOrigin: true },
    },
  },
})
