<script setup>
import {computed, ref, watch} from 'vue'

// Contenu d'un badge joueur : sa photo si elle existe, sinon ses initiales
// (le comportement d'origine). Le conteneur (taille, forme, couleur) reste
// celui de la page appelante : ce composant ne fait que le remplir.

defineOptions({inheritAttrs: false})

const props = defineProps({
  pseudo:  {type: String, default: ''},
  url:     {type: String, default: null},
  // Nombre de lettres du badge sans photo (le lobby en affiche 1, la partie 2)
  letters: {type: Number, default: 2},
})

// Photo introuvable ou supprimée entre-temps : on retombe sur les initiales.
const failed = ref(false)
watch(() => props.url, () => { failed.value = false })

const initials = computed(() => (props.pseudo ?? '').slice(0, props.letters).toUpperCase())
</script>

<template>
  <img
      v-if="url && !failed"
      :src="url"
      :alt="`Photo de ${pseudo}`"
      class="player-avatar-img"
      loading="lazy"
      decoding="async"
      @error="failed = true"
  />
  <template v-else>{{ initials }}</template>
</template>

<style scoped>
.player-avatar-img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
}
</style>
