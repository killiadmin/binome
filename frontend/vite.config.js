import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// The dev server is the single origin the browser talks to. Every device on the
// LAN loads the app from http://killiadmin.local:8095/binome/ (the shared nginx
// portal in ../portail forwards /binome/ here, prefix kept) and all API /
// WebSocket traffic is proxied from here to the backend & Reverb containers by
// Docker service name — so the host's LAN IP never has to appear in any
// frontend config or .env, and switching networks needs no regeneration.
const BACKEND = process.env.VITE_PROXY_BACKEND || 'http://backend:8000'
const REVERB = process.env.VITE_PROXY_REVERB || 'http://reverb:8080'

// The whole app lives under this path, on the portal and on :5173 alike. The
// proxies strip it so the backend and Reverb keep their usual /api, /broadcasting, /app.
const BASE = '/binome'
const stripBase = (path) => path.slice(BASE.length)

export default defineConfig({
  base: `${BASE}/`,
  plugins: [vue()],
  server: {
    host: '0.0.0.0',
    port: 5173,
    // Vite only trusts localhost and IPs by default; the portal forwards the
    // Bonjour name (killiadmin.local) as Host.
    allowedHosts: ['.local'],
    proxy: {
      [`${BASE}/api`]: { target: BACKEND, changeOrigin: true, rewrite: stripBase },
      [`${BASE}/broadcasting`]: { target: BACKEND, changeOrigin: true, rewrite: stripBase },
      // Reverb's client WebSocket endpoint (pusher protocol): /app/{key}
      [`${BASE}/app`]: { target: REVERB, ws: true, changeOrigin: true, rewrite: stripBase },
    },
  },
})
