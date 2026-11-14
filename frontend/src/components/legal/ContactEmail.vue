<script>
import { ref } from 'vue'

// Adresse de contact affichée seulement au clic, pour les robots qui collectent
// les e-mails : elle n'apparaît en entier ni dans le HTML, ni dans le JS compilé
// (pas de « @ » littéral), et n'est assemblée qu'au premier clic.
// `revealed` est partagé : un clic dévoile toutes les occurrences de la page.
const revealed = ref(false)
const PARTS = ['killian.dev4014', 'gmail.com']

function assemble() {
  return PARTS.join(String.fromCharCode(64))
}
</script>

<script setup>
import { computed } from 'vue'

const email = computed(() => (revealed.value ? assemble() : ''))
</script>

<template>
  <a v-if="revealed" :href="`mailto:${email}`" class="contact-email">{{ email }}</a>
  <button v-else type="button" class="contact-email contact-email--hidden" @click="revealed = true">
    afficher l'adresse
  </button>
</template>

<style scoped>
.contact-email {
  color: var(--arcade-blue-grey-dark);
  font-weight: 700;
  text-decoration: underline;
}

/* Bouton présenté comme un lien, dans le fil du texte. */
.contact-email--hidden {
  display: inline;
  padding: 0;
  border: 0;
  background: none;
  font: inherit;
  font-weight: 700;
  text-align: left;
  cursor: pointer;
}
</style>
