import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Développement uniquement : en production, le serveur Rust sert lui-même le
// front compilé (dist/), l'API et le WebSocket sur une seule origine.
//
// En dev, le serveur Vite (HMR) est l'origine du navigateur : il relaie l'API et
// le WebSocket vers le serveur Rust par nom de service Docker. Rust servant tout
// sous /binome, les chemins sont transmis tels quels (aucune réécriture). Les
// appareils du LAN passent par le portail (../portail) : http://killiadmin.local:8095/binome/
const SERVER = process.env.VITE_PROXY_SERVER || 'http://server:8080'
// Préfixe public (doit être identique à BINOME_PUBLIC_PREFIX côté serveur) :
// VITE_BASE=/ pour servir l'application à la racine d'un domaine.
const BASE = (process.env.VITE_BASE ?? '/binome').replace(/\/+$/, '')

export default defineConfig({
  base: `${BASE}/`,
  plugins: [vue()],
  build: {
    // Polices toujours en fichiers : la CSP du serveur (font-src 'self') refuse data:.
    assetsInlineLimit: (file) => (file.endsWith('.woff2') ? false : undefined),
  },
  server: {
    host: '0.0.0.0',
    port: 5173,
    // Vite n'accepte par défaut que localhost et les IP ; le portail transmet le nom Bonjour.
    allowedHosts: ['.local'],
    proxy: {
      [`${BASE}/api`]: { target: SERVER, changeOrigin: true },
      [`${BASE}/ws`]: { target: SERVER, ws: true, changeOrigin: true },
    },
  },
})
