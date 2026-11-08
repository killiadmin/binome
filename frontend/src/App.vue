<script setup>
import Navbar from './components/navigation/Navbar.vue';
import Footer from './components/footer/Footer.vue';
import {isImmersive} from './composables/useImmersiveMode';
</script>

<template>
  <!-- La navbar se masque elle-même en mode plein écran. -->
  <Navbar />
  <main class="app-main" :class="{ 'is-immersive': isImmersive }">
    <router-view/>
  </main>
  <Footer v-if="!isImmersive" />
</template>

<style>
/* Hauteur de la navbar fixed-top, utilisée pour décaler le contenu placé
   en dessous. Navbar.vue la remplace par la hauteur mesurée (ResizeObserver) ;
   cette valeur — la navbar mobile sur une ligne — ne sert qu'avant le montage.
   --footer-height : place réservée en bas de page pour le footer fixed.
   Sur téléphone (portrait ou paysage), le footer n'est PAS fixe : 72px de
   copyright en permanence mangeaient jusqu'à un quart de l'écran de jeu. Il
   suit alors le contenu ; il ne devient fixe qu'à partir d'une tablette
   (voir Footer.vue, même media query). */
:root {
  --navbar-height: 63px;
  --footer-height: 0px;
  /* Respiration entre la navbar et le haut des pages. */
  --navbar-gap: 10px;
}

@media (min-width: 768px) and (min-height: 501px) {
  :root {
    /* 71.4px mesurés, arrondi au-dessus */
    --footer-height: 72px;
  }
}

/* Footer non fixe : il reste collé en bas sur une page courte */
#app {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  min-height: 100dvh;
}

/* Mode plein écran (composables/useImmersiveMode.js) : plus de footer. */
:root:has(.app-main.is-immersive) {
  --footer-height: 0px;
}

.app-main {
  flex: 1 0 auto;
  padding-top: calc(var(--navbar-height) + var(--navbar-gap));
  padding-bottom: var(--footer-height);
}

.app-main.is-immersive {
  padding-top: 0;
}
</style>
