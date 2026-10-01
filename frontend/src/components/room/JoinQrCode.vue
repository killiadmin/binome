<script setup>
import {ref, watch} from 'vue'
import QRCode from 'qrcode'
import {resolveJoinUrl} from '../../services/joinUrl'

// QR code du salon : pointe sur /rooms?code=XXXXXX à l'adresse LAN de l'hôte,
// RoomPage ouvre alors directement la modale « Rejoindre » avec le code rempli.
// L'adresse est résolue comme le bouton « Partager le lien » de la navbar
// (voir services/joinUrl.js) : relue à chaque affichage, donc à jour après un
// changement de Wi-Fi tant que update-lan-ip.sh --watch tourne.

const props = defineProps({
  code: {type: String, required: true},
})

const svg = ref('')
const joinUrl = ref(null)
const verified = ref(true)
const loading = ref(false)

async function render() {
  loading.value = true
  try {
    const {url, verified: ok} = await resolveJoinUrl()
    verified.value = ok
    if (!url) {
      joinUrl.value = null
      svg.value = ''
      return
    }
    joinUrl.value = new URL(`rooms?code=${encodeURIComponent(props.code)}`, url).href
    svg.value = await QRCode.toString(joinUrl.value, {
      type: 'svg',
      margin: 1,
      errorCorrectionLevel: 'M',
      color: {dark: '#242424', light: '#FFFFFF'},
    })
  } catch {
    svg.value = ''
  } finally {
    loading.value = false
  }
}

watch(() => props.code, render, {immediate: true})
</script>

<template>
  <div class="join-qr">
    <div v-if="loading" class="join-qr__placeholder">…</div>

    <template v-else-if="svg">
      <!-- SVG généré localement par la lib qrcode : aucune donnée externe -->
      <div class="join-qr__code" role="img" :aria-label="`QR code pour rejoindre le salon ${code}`" v-html="svg"></div>
      <p class="join-qr__url">{{ joinUrl }}</p>
      <p v-if="!verified" class="join-qr__warn">
        <i class="fa-solid fa-triangle-exclamation"></i>
        Cette adresse ne répond pas : relance <code>./start.sh</code> si tu as changé de réseau.
      </p>
    </template>

    <p v-else class="join-qr__warn">
      <i class="fa-solid fa-circle-info"></i>
      Adresse réseau inconnue : lance le jeu avec <code>./start.sh</code>, ou ouvre cette page via l'IP de l'ordinateur.
    </p>
  </div>
</template>

<style scoped>
.join-qr {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.4rem;
  margin: 0.75rem 0 1rem;
}

.join-qr__code {
  width: min(220px, 70vw);
  padding: 6px;
  background: #fff;
  border: 3px solid var(--arcade-blue-grey-dark);
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.35);
}

.join-qr__code :deep(svg) {
  display: block;
  width: 100%;
  height: auto;
}

.join-qr__placeholder {
  width: min(220px, 70vw);
  aspect-ratio: 1;
  display: grid;
  place-items: center;
  color: var(--arcade-blue-grey-dark);
  border: 3px dashed var(--arcade-blue-grey);
}

.join-qr__url {
  margin: 0;
  max-width: 100%;
  font-size: 0.75rem;
  color: var(--arcade-blue-grey-dark);
  overflow-wrap: anywhere;
  text-align: center;
}

.join-qr__warn {
  margin: 0;
  font-size: 0.8rem;
  color: #8a5a00;
  text-align: center;
}
</style>
